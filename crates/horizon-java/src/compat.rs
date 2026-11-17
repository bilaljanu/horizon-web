//! What a Fabric or NeoForge jar declares, checked against what this Horizon
//! build can actually run.
//!
//! A report never says a mod is compatible because its metadata parses. The
//! status separates:
//!
//! * [`Support::Unsupported`] — a required dependency, game/loader version,
//!   Java class version or malformed file makes it impossible to start;
//! * [`Support::Partial`] — metadata was inspected, but execution and gameplay
//!   requirements remain unverified;
//! * [`Support::Runnable`] — reserved for execution validation, never assigned
//!   solely by metadata inspection. `verified` requires observed mod behavior.

use crate::access::{self, AccessEntry, AccessWidener};
use crate::jar::Jar;
use crate::metadata::{
    self, Dependency, DependencyKind, Detected, Loader, MixinConfig, ModMetadata,
};
use crate::version::{Version, VersionRange};
use std::collections::{BTreeMap, BTreeSet};

/// The Minecraft version this build speaks.
pub const MINECRAFT: &str = "26.3";
/// The Fabric loader surface the compatibility loader implements.
pub const FABRIC_LOADER: &str = "0.19.5";
/// The NeoForge loader surface the compatibility loader implements.
pub const NEOFORGE_LOADER: &str = "26.3.0";
/// The Fabric API surface Horizon implements natively.
pub const FABRIC_API: &str = "0.140.0+26.3";
/// The newest Java class-file major version the interpreter runs (Java 25).
pub const MAX_CLASS_MAJOR: u16 = 69;
/// The Java release that major version means.
pub const JAVA_RELEASE: u32 = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Support {
    /// Required functionality is missing.
    Unsupported,
    /// Metadata inspected, execution requirements not fully validated.
    Partial,
    /// Within the implemented subset.
    Runnable,
}

impl Support {
    pub fn label(self) -> &'static str {
        match self {
            Support::Unsupported => "Unsupported",
            Support::Partial => "Partial",
            Support::Runnable => "Runnable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// A stable code tools and tests can assert on (`game.version`).
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    /// The class, file or dependency the diagnostic is about.
    pub context: Option<String>,
}

impl Diagnostic {
    fn new(code: &'static str, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            context: None,
        }
    }
    fn at(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}

/// This build's versions, for tests and diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameInfo {
    pub minecraft: &'static str,
    pub fabric_loader: &'static str,
    pub neoforge_loader: &'static str,
    pub fabric_api: &'static str,
    pub java_release: u32,
}

impl Default for GameInfo {
    fn default() -> Self {
        Self {
            minecraft: MINECRAFT,
            fabric_loader: FABRIC_LOADER,
            neoforge_loader: NEOFORGE_LOADER,
            fabric_api: FABRIC_API,
            java_release: JAVA_RELEASE,
        }
    }
}

/// The outcome of inspecting one jar.
#[derive(Clone, Debug)]
pub struct Inspected {
    pub metadata: ModMetadata,
    pub all: Vec<ModMetadata>,
    pub support: Support,
    pub verified: bool,
    pub diagnostics: Vec<Diagnostic>,
    /// Mixin configurations and whether their classes are present.
    pub mixins: Vec<MixinFile>,
    pub access_widener: Option<AccessWidener>,
    pub access_transformer: Vec<AccessEntry>,
    /// Every mandatory dependency and this build's answer.
    pub dependencies: Vec<DependencyCheck>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixinFile {
    pub config: MixinConfig,
    /// The mixin classes (binary names) found in the jar.
    pub present: Vec<String>,
    pub missing: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyCheck {
    pub dependency: Dependency,
    pub satisfied: bool,
    /// The version that satisfies it, when one does.
    pub available: Option<String>,
    /// Why it is not satisfied.
    pub problem: Option<String>,
}

impl Inspected {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn missing_dependencies(&self) -> Vec<&DependencyCheck> {
        self.dependencies
            .iter()
            .filter(|d| !d.satisfied && d.dependency.kind == DependencyKind::Depends)
            .collect()
    }

    /// A one-line status for the mod manager.
    pub fn summary(&self) -> String {
        let errors = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        let warnings = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count();
        match (errors, warnings) {
            (0, 0) => format!("{}: execution unverified", self.support.label()),
            (0, n) => format!(
                "{}: execution unverified; {n} limitation{}",
                self.support.label(),
                if n == 1 { "" } else { "s" }
            ),
            (n, _) => format!(
                "{}: {n} problem{}",
                self.support.label(),
                if n == 1 { "" } else { "s" }
            ),
        }
    }
}

/// Built-in mods the compatibility layer provides and their versions.
pub fn builtin_version(id: &str, game: &GameInfo) -> Option<&'static str> {
    match id {
        "minecraft" => Some(game.minecraft),
        "fabricloader" | "fabric-loader" => Some(game.fabric_loader),
        "fabric-api" => Some(game.fabric_api),
        "java" => Some("25"),
        "neoforge" => Some(game.neoforge_loader),
        "horizon" => Some(env!("CARGO_PKG_VERSION")),
        _ => None,
    }
}

/// Inspect a jar already opened. `known` maps ids of other installed mods to
/// their versions, for cross-mod dependency checks.
pub fn inspect(
    jar: &mut Jar,
    game: &GameInfo,
    known: &BTreeMap<String, String>,
) -> Result<Inspected, String> {
    let detected = metadata::detect(jar).map_err(|e| format!("mod metadata: {e}"))?;
    Ok(inspect_detected(jar, game, known, detected))
}

fn inspect_detected(
    jar: &mut Jar,
    game: &GameInfo,
    known: &BTreeMap<String, String>,
    detected: Detected,
) -> Inspected {
    let Detected {
        metadata,
        all,
        warnings,
    } = detected;
    let mut diagnostics = Vec::new();
    let mut support = Support::Partial;
    for warning in warnings {
        diagnostics.push(Diagnostic::new(
            "metadata.unreadable",
            Severity::Error,
            warning,
        ));
        support = Support::Unsupported;
    }

    // Loader.
    match metadata.loader {
        Loader::Fabric | Loader::NeoForge => {}
        Loader::Quilt => {
            diagnostics.push(Diagnostic::new(
                "loader.quilt",
                Severity::Warning,
                "Quilt metadata: Horizon starts Fabric-compatible entrypoints only; Quilt-specific APIs are not provided",
            ));
            support = support.min(Support::Partial);
        }
        Loader::Forge => {
            diagnostics.push(Diagnostic::new(
                "loader.forge",
                Severity::Error,
                "legacy Forge is not implemented; only Fabric and NeoForge mods are supported",
            ));
            support = Support::Unsupported;
        }
    }

    // Java class version.
    if let Some(required) = metadata.required_java() {
        let major = metadata.class_major.unwrap_or(0);
        if major > MAX_CLASS_MAJOR {
            diagnostics.push(
                Diagnostic::new(
                    "java.version",
                    Severity::Error,
                    format!(
                        "class files need Java {required} (major {major}); this runtime interprets up to Java {JAVA_RELEASE} (major {MAX_CLASS_MAJOR})"
                    ),
                )
                .at(metadata.classes.first().cloned().unwrap_or_default()),
            );
            support = Support::Unsupported;
        } else if required > game.java_release {
            diagnostics.push(Diagnostic::new(
                "java.version",
                Severity::Warning,
                format!(
                    "class files need Java {required}; Horizon reports Java {} to mods",
                    game.java_release
                ),
            ));
            support = support.min(Support::Partial);
        }
    } else if metadata.classes.is_empty() {
        diagnostics.push(Diagnostic::new(
            "classes.empty",
            Severity::Warning,
            "the jar has no class files; it can contribute resources only",
        ));
        support = support.min(Support::Partial);
    }

    // Entrypoints.
    let loader_entrypoints = metadata
        .entrypoints
        .iter()
        .filter(|e| matches!(e.kind.as_str(), "main" | "client" | "server" | "mod"))
        .count();
    if metadata.loader == Loader::Fabric && metadata.entrypoints.is_empty() {
        diagnostics.push(Diagnostic::new(
            "entrypoint.missing",
            Severity::Warning,
            "no entrypoints: this is a resource-only Fabric mod",
        ));
        support = support.min(Support::Partial);
    }
    if metadata.loader == Loader::NeoForge && !metadata.classes.is_empty() {
        // The @Mod class is discovered from the class file's annotations by
        // the loader; absence is diagnosed there with the class name.
        diagnostics.push(Diagnostic::new(
            "neoforge.scan",
            Severity::Info,
            "NeoForge @Mod classes are discovered from class annotations",
        ));
    }
    for entry in &metadata.entrypoints {
        if !matches!(
            entry.kind.as_str(),
            "main" | "client" | "server" | "mod" | "modmenu" | "fabric-datagen"
        ) {
            diagnostics.push(
                Diagnostic::new(
                    "entrypoint.unknown",
                    Severity::Info,
                    format!(
                        "entrypoint kind {:?} is not invoked by Horizon; only main/client/server entrypoints run",
                        entry.kind
                    ),
                )
                .at(entry.class.clone()),
            );
        }
        if entry.kind == "fabric-datagen" {
            continue;
        }
        if !entry.class.is_empty() {
            let internal = entry.class.replace('.', "/");
            if !jar.contains(&format!("{internal}.class")) {
                diagnostics.push(
                    Diagnostic::new(
                        "entrypoint.missing_class",
                        Severity::Error,
                        format!("entrypoint class {} is not in the jar", entry.class),
                    )
                    .at(entry.class.clone()),
                );
                support = Support::Unsupported;
            }
        }
    }

    // Dependencies.
    let mut dependency_checks = Vec::new();
    for dependency in &metadata.dependencies {
        if dependency.kind != DependencyKind::Depends {
            continue;
        }
        let (available, problem) = dependency_answer(dependency, game, known);
        let satisfied = available
            .as_deref()
            .is_some_and(|v| dependency.range.matches(v));
        if !satisfied {
            let severity = match dependency.kind {
                DependencyKind::Depends if dependency.mandatory => Severity::Error,
                _ => Severity::Warning,
            };
            let reason = match (&available, &problem) {
                (Some(v), _) => format!(
                    "needs {} {}, found {v}",
                    dependency.id, dependency.range.text
                ),
                (None, Some(p)) => {
                    format!("needs {} {}: {p}", dependency.id, dependency.range.text)
                }
                (None, None) => format!("needs {} {}", dependency.id, dependency.range.text),
            };
            diagnostics.push(
                Diagnostic::new("dependency.missing", severity, reason).at(dependency.id.clone()),
            );
            if severity == Severity::Error {
                support = Support::Unsupported;
            } else {
                support = support.min(Support::Partial);
            }
        }
        dependency_checks.push(DependencyCheck {
            dependency: dependency.clone(),
            satisfied,
            available,
            problem,
        });
    }

    // Mixins.
    let mut mixins = Vec::new();
    let mut mixin_seen = BTreeSet::new();
    for config in &metadata.mixins {
        if !mixin_seen.insert(config.path.clone()) {
            continue;
        }
        let mut config = config.clone();
        let mut present = Vec::new();
        let mut missing = Vec::new();
        match metadata::read_mixin_config(jar, &mut config) {
            Ok(()) => {
                let package = config.package.clone().unwrap_or_default();
                for class in config
                    .mixins
                    .iter()
                    .chain(&config.client)
                    .chain(&config.server)
                {
                    let binary = if package.is_empty() {
                        class.clone()
                    } else {
                        format!("{package}.{class}")
                    };
                    let internal = binary.replace('.', "/");
                    if jar.contains(&format!("{internal}.class")) {
                        present.push(binary);
                    } else {
                        missing.push(binary);
                    }
                }
                if !missing.is_empty() {
                    diagnostics.push(Diagnostic::new(
                        "mixin.missing_class",
                        Severity::Warning,
                        format!(
                            "{} names {} mixin class{} not in the jar",
                            config.path,
                            missing.len(),
                            if missing.len() == 1 { "" } else { "es" }
                        ),
                    ));
                    support = support.min(Support::Partial);
                }
                diagnostics.push(Diagnostic::new(
                    "mixin.not_applied",
                    Severity::Warning,
                    format!(
                        "{}: Sponge Mixin transformations are not applied to Horizon's Rust engine; the mod's mixin behaviour is absent",
                        config.path
                    ),
                ));
                support = support.min(Support::Partial);
            }
            Err(e) => {
                diagnostics.push(
                    Diagnostic::new("mixin.invalid", Severity::Error, e).at(config.path.clone()),
                );
                support = Support::Unsupported;
            }
        }
        mixins.push(MixinFile {
            config,
            present,
            missing,
        });
    }

    // Access wideners and transformers.
    let mut access_widener = None;
    let mut access_transformer = Vec::new();
    if let Some(path) = &metadata.access_widener {
        match jar.read_text(path) {
            Ok(text) => match access::access_widener(&text) {
                Ok(aw) => {
                    if !aw.entries.is_empty() {
                        let classes = aw
                            .entries
                            .iter()
                            .map(|e| e.class.as_str())
                            .collect::<BTreeSet<_>>()
                            .len();
                        diagnostics.push(Diagnostic::new(
                            "access_widener.unmapped",
                            Severity::Warning,
                            format!(
                                "{path}: {} access entries over {classes} classes are not applied; Horizon's Rust Minecraft classes are not JVM classes",
                                aw.entries.len()
                            ),
                        ));
                        support = support.min(Support::Partial);
                    }
                    access_widener = Some(aw);
                }
                Err(e) => {
                    diagnostics.push(
                        Diagnostic::new(
                            "access_widener.invalid",
                            Severity::Error,
                            format!("{path}: {e}"),
                        )
                        .at(path.clone()),
                    );
                    support = Support::Unsupported;
                }
            },
            Err(e) => {
                diagnostics.push(
                    Diagnostic::new(
                        "access_widener.missing",
                        Severity::Error,
                        format!("declared access widener cannot be read: {e}"),
                    )
                    .at(path.clone()),
                );
                support = Support::Unsupported;
            }
        }
    }
    if let Some(path) = &metadata.access_transformer {
        match jar.read_text(path) {
            Ok(text) => match access::access_transformer(&text) {
                Ok(entries) => {
                    if !entries.is_empty() {
                        diagnostics.push(Diagnostic::new(
                            "access_transformer.unmapped",
                            Severity::Warning,
                            format!(
                                "{path}: {} access transformer entries are not applied; Horizon's Rust Minecraft classes are not JVM classes",
                                entries.len()
                            ),
                        ));
                        support = support.min(Support::Partial);
                    }
                    access_transformer = entries;
                }
                Err(e) => {
                    diagnostics.push(
                        Diagnostic::new(
                            "access_transformer.invalid",
                            Severity::Error,
                            format!("{path}: {e}"),
                        )
                        .at(path.clone()),
                    );
                    support = Support::Unsupported;
                }
            },
            Err(e) => {
                diagnostics.push(
                    Diagnostic::new(
                        "access_transformer.missing",
                        Severity::Error,
                        format!("declared access transformer cannot be read: {e}"),
                    )
                    .at(path.clone()),
                );
                support = Support::Unsupported;
            }
        }
    }

    // Nested jars are not loaded by the interpreter: they are separate
    // classpaths in Fabric's loader.
    if !metadata.nested_jars.is_empty() {
        diagnostics.push(Diagnostic::new(
            "nested_jars.not_loaded",
            Severity::Warning,
            format!(
                "{} nested jar{} are not put on the interpreter's classpath",
                metadata.nested_jars.len(),
                if metadata.nested_jars.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ),
        ));
        support = support.min(Support::Partial);
    }

    // A server-only mod in this client build: Horizon has an integrated
    // server, so the entrypoint can start; it is a limitation only when the
    // mod expects a dedicated server's APIs.
    if metadata.environment == metadata::Environment::Server {
        diagnostics.push(Diagnostic::new(
            "environment.server",
            Severity::Info,
            "server-only metadata: the integrated server runs it, a remote server does not",
        ));
    }

    diagnostics.sort_by_key(|d| (d.severity, d.code));
    let verified = false;
    let all_reports = all.iter().map(|m| m.id.clone()).collect::<Vec<String>>();
    let extra = all_reports.len().saturating_sub(1);
    if extra > 0 {
        diagnostics.push(Diagnostic::new(
            "metadata.multiple_mods",
            Severity::Info,
            format!(
                "the jar declares {extra} further NeoForge mod id(s): {}",
                all_reports[1..].join(", ")
            ),
        ));
    }
    let _ = loader_entrypoints;
    Inspected {
        metadata,
        all,
        support,
        verified,
        diagnostics,
        mixins,
        access_widener,
        access_transformer,
        dependencies: dependency_checks,
    }
}

fn dependency_answer(
    dependency: &Dependency,
    game: &GameInfo,
    known: &BTreeMap<String, String>,
) -> (Option<String>, Option<String>) {
    if let Some(version) = builtin_version(&dependency.id, game) {
        return (Some(version.to_owned()), None);
    }
    if let Some(version) = known.get(&dependency.id) {
        return (Some(version.clone()), None);
    }
    if dependency.id == "fabric-api" || dependency.id.starts_with("fabric-api-") {
        return (
            Some(game.fabric_api.to_owned()),
            Some("Horizon provides a subset of Fabric API".into()),
        );
    }
    (None, Some("not installed".into()))
}

/// One installed mod's metadata, for resolving several jars together.
#[derive(Clone, Debug)]
pub struct InstalledMod {
    pub id: String,
    pub version: String,
    pub loader: Loader,
}

/// Cross-mod dependency answer: which installed mods satisfy each report's
/// mandatory dependencies, excluding itself.
pub fn resolve<'a>(
    report: &Inspected,
    installed: &'a [InstalledMod],
) -> Vec<(String, Option<&'a InstalledMod>)> {
    report
        .dependencies
        .iter()
        .filter(|d| d.dependency.kind == DependencyKind::Depends)
        .map(|check| {
            let found = installed
                .iter()
                .find(|m| m.id == check.dependency.id && m.id != report.metadata.id);
            (
                check.dependency.id.clone(),
                found.filter(|m| check.dependency.range.matches(&m.version)),
            )
        })
        .collect()
}

/// The order to initialize mods in: dependencies first, then NeoForge's
/// `BEFORE`/`AFTER` ordering. Cycles keep their declaration order and are
/// reported.
pub fn load_order(reports: &[Inspected]) -> (Vec<usize>, Vec<String>) {
    let ids: BTreeMap<&str, usize> = reports
        .iter()
        .enumerate()
        .map(|(i, r)| (r.metadata.id.as_str(), i))
        .collect();
    let mut incoming: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); reports.len()];
    let mut outgoing: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); reports.len()];
    let mut problems = Vec::new();
    for (index, report) in reports.iter().enumerate() {
        for dep in &report.metadata.dependencies {
            let Some(&other) = ids.get(dep.id.as_str()) else {
                continue;
            };
            if other == index {
                continue;
            }
            match (dep.ordering.to_ascii_uppercase().as_str(), dep.kind) {
                ("BEFORE", _) => {
                    outgoing[index].insert(other);
                    incoming[other].insert(index);
                }
                ("AFTER", _) | (_, DependencyKind::Depends) => {
                    outgoing[other].insert(index);
                    incoming[index].insert(other);
                }
                _ => {}
            }
        }
    }
    let mut order = Vec::new();
    let mut ready: BTreeSet<usize> = (0..reports.len())
        .filter(|i| incoming[*i].is_empty())
        .collect();
    while let Some(index) = ready.pop_first() {
        order.push(index);
        for next in outgoing[index].clone() {
            incoming[next].remove(&index);
            if incoming[next].is_empty() {
                ready.insert(next);
            }
        }
    }
    if order.len() != reports.len() {
        let cycle: Vec<String> = (0..reports.len())
            .filter(|i| !order.contains(i))
            .map(|i| reports[i].metadata.id.clone())
            .collect();
        problems.push(format!(
            "dependency cycle among {}: keeping declaration order",
            cycle.join(", ")
        ));
        order.extend(0..reports.len());
        order.sort_unstable();
        order.dedup();
    }
    (order, problems)
}

/// Check a game metadata range against this build using the loader's
/// syntax. Exposed for the mod manager's manual version filter.
pub fn game_matches(range: &str, game: &GameInfo) -> bool {
    VersionRange::parse(range).matches(game.minecraft)
}

/// Whether `version` is greater than this Horizon build's Minecraft
/// version, for the "newer than Horizon" warning.
pub fn newer_than_game(version: &str, game: &GameInfo) -> bool {
    Version::parse(version) > Version::parse(game.minecraft)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn zip(files: &[(&str, &[u8])]) -> Jar {
        let mut out = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(Cursor::new(&mut out));
            let options: zip::write::SimpleFileOptions = Default::default();
            for (name, body) in files {
                writer.start_file(*name, options).unwrap();
                std::io::Write::write_all(&mut writer, body).unwrap();
            }
            writer.finish().unwrap();
        }
        Jar::open(out).unwrap()
    }

    fn class(major: u16) -> Vec<u8> {
        let mut bytes = b"\xca\xfe\xba\xbe\0\0".to_vec();
        bytes.extend_from_slice(&major.to_be_bytes());
        bytes.push(0);
        bytes
    }

    #[test]
    fn minimal_fabric_metadata_does_not_prove_execution() {
        let json = br#"{"schemaVersion":1,"id":"ruby","version":"1.0.0","entrypoints":{"main":["ruby.RubyMod"]},"depends":{"minecraft":"26.3","fabricloader":">=0.19.5","java":">=21"}}"#;
        let mut jar = zip(&[
            ("fabric.mod.json", json),
            ("ruby/RubyMod.class", &class(65)),
        ]);
        let report = inspect(&mut jar, &GameInfo::default(), &BTreeMap::new()).unwrap();
        assert_eq!(report.support, Support::Partial);
        assert!(!report.verified);
        assert!(
            report
                .diagnostics
                .iter()
                .all(|d| d.severity != Severity::Error)
        );
    }

    #[test]
    fn a_newer_minecraft_is_unsupported() {
        let json = br#"{"schemaVersion":1,"id":"ruby","version":"1","entrypoints":{"main":["ruby.M"]},"depends":{"minecraft":"[26.4,)"}}"#;
        let mut jar = zip(&[("fabric.mod.json", json), ("ruby/M.class", &class(65))]);
        let report = inspect(&mut jar, &GameInfo::default(), &BTreeMap::new()).unwrap();
        assert_eq!(report.support, Support::Unsupported);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "dependency.missing" && d.severity == Severity::Error)
        );
    }

    #[test]
    fn mixins_are_partial_and_never_verified() {
        let json = br#"{"schemaVersion":1,"id":"ruby","version":"1","entrypoints":{"main":["ruby.M"]},"mixins":["ruby.mixins.json"]}"#;
        let mixins = br#"{"required":true,"package":"ruby.mixin","mixins":["RubyMixin"]}"#;
        let mut jar = zip(&[
            ("fabric.mod.json", json),
            ("ruby.mixins.json", mixins),
            ("ruby/M.class", &class(65)),
            ("ruby/mixin/RubyMixin.class", &class(65)),
        ]);
        let report = inspect(&mut jar, &GameInfo::default(), &BTreeMap::new()).unwrap();
        assert_eq!(report.support, Support::Partial);
        assert_eq!(report.mixins.len(), 1);
        assert_eq!(report.mixins[0].present, ["ruby.mixin.RubyMixin"]);
        assert!(!report.verified);
    }

    #[test]
    fn access_widener_entries_are_named() {
        let json = br#"{"schemaVersion":1,"id":"ruby","version":"1","entrypoints":{"main":["ruby.M"]},"accessWidener":"ruby.accesswidener"}"#;
        let aw = b"accessWidener v2 named\naccessible class net/minecraft/world/item/Item\n";
        let mut jar = zip(&[
            ("fabric.mod.json", json),
            ("ruby.accesswidener", aw),
            ("ruby/M.class", &class(65)),
        ]);
        let report = inspect(&mut jar, &GameInfo::default(), &BTreeMap::new()).unwrap();
        assert_eq!(report.support, Support::Partial);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "access_widener.unmapped")
        );
    }

    #[test]
    fn newer_class_files_are_unsupported() {
        let json =
            br#"{"schemaVersion":1,"id":"ruby","version":"1","entrypoints":{"main":["ruby.M"]}}"#;
        let mut jar = zip(&[("fabric.mod.json", json), ("ruby/M.class", &class(70))]);
        let report = inspect(&mut jar, &GameInfo::default(), &BTreeMap::new()).unwrap();
        assert_eq!(report.support, Support::Unsupported);
        assert!(report.diagnostics.iter().any(|d| d.code == "java.version"));
    }

    #[test]
    fn load_order_puts_dependencies_first() {
        fn report(id: &str, dep: Option<&str>) -> Inspected {
            let dep = dep.map(|d| format!(r#""depends":{{"{d}":"*"}}"#));
            let json = format!(
                r#"{{"schemaVersion":1,"id":"{id}","version":"1","entrypoints":{{"main":["{id}.M"]}}{}}}"#,
                dep.map(|d| format!(",{d}")).unwrap_or_default()
            );
            let mut jar = zip(&[
                ("fabric.mod.json", json.as_bytes()),
                (&format!("{id}/M.class"), &class(65)),
            ]);
            inspect(&mut jar, &GameInfo::default(), &BTreeMap::new()).unwrap()
        }
        let reports = vec![report("b", Some("a")), report("a", None)];
        let (order, problems) = load_order(&reports);
        assert_eq!(order, vec![1, 0]);
        assert!(problems.is_empty());
    }
}
