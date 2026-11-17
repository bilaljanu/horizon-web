//! Fabric's `fabric.mod.json` and NeoForge's `META-INF/neoforge.mods.toml`
//! (and the older `mods.toml`) read into one metadata model.
//!
//! Nothing here executes mod code. The loader metadata is only what the mod
//! declares; [`crate::compat`] checks it against this build and
//! [`crate::loader`] runs the entrypoints that survive the checks.

use crate::jar::Jar;
use crate::version::VersionRange;
use serde_json::Value;
use std::collections::BTreeMap;

pub const FABRIC_METADATA: &str = "fabric.mod.json";
pub const NEOFORGE_METADATA: &str = "META-INF/neoforge.mods.toml";
pub const FORGE_METADATA: &str = "META-INF/mods.toml";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loader {
    Fabric,
    Quilt,
    NeoForge,
    Forge,
}

impl Loader {
    pub fn name(self) -> &'static str {
        match self {
            Loader::Fabric => "Fabric",
            Loader::Quilt => "Quilt",
            Loader::NeoForge => "NeoForge",
            Loader::Forge => "Forge",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    Client,
    Server,
    Both,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entrypoint {
    /// Fabric's key (`main`, `client`, `server`, `modmenu`, …), or
    /// NeoForge's `mod` for the `@Mod` class.
    pub kind: String,
    pub class: String,
    /// Fabric's language adapter (`default` when the mod names none).
    pub adapter: String,
}

/// How strongly a dependency is needed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyKind {
    Depends,
    Recommends,
    Suggests,
    Breaks,
    Conflicts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub id: String,
    pub kind: DependencyKind,
    /// The versions that satisfy it; `*` for any.
    pub range: VersionRange,
    /// NeoForge's `ordering` (`NONE`, `BEFORE`, `AFTER`).
    pub ordering: String,
    /// NeoForge's `side` (`BOTH`, `CLIENT`, `SERVER`), or Fabric's
    /// environment when the dependency applies to one side only.
    pub side: String,
    /// NeoForge's `mandatory`: the dependency must be present.
    pub mandatory: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixinConfig {
    pub path: String,
    /// The `package` from the config, when it could be read.
    pub package: Option<String>,
    /// Mixin class names (binary `a.b.C` names) the config declares.
    pub mixins: Vec<String>,
    pub client: Vec<String>,
    pub server: Vec<String>,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModMetadata {
    pub loader: Loader,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub license: String,
    pub environment: Environment,
    pub entrypoints: Vec<Entrypoint>,
    pub dependencies: Vec<Dependency>,
    pub mixins: Vec<MixinConfig>,
    pub access_widener: Option<String>,
    pub access_transformer: Option<String>,
    /// The metadata file this came from.
    pub source: String,
    /// `schemaVersion` (Fabric) or `modLoader` (NeoForge).
    pub format: String,
    /// For a jar carrying several NeoForge mods: every `modId`.
    pub extra_mod_ids: Vec<String>,
    /// Fabric's nested jars (`jars`).
    pub nested_jars: Vec<String>,
    /// The lowest Java class-file major version the jar carries, when read.
    pub class_major: Option<u16>,
    /// Classes in the jar, sorted.
    pub classes: Vec<String>,
    /// Raw JSON/TOML values keep loader-specific facts available to the
    /// diagnostics without inventing a field for each one.
    pub raw: String,
}

impl ModMetadata {
    /// A mod id safe for a folder and a namespace: Fabric's and NeoForge's
    /// lower-case id rules, without path separators.
    pub fn valid_id(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 64
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
            && !id.starts_with('-')
    }

    /// The Java release a class-file major version needs (52 is Java 8).
    pub fn java_release(major: u16) -> Option<u32> {
        (major >= 45).then(|| (major as u32).saturating_sub(44))
    }

    /// The Java release this mod's classes need.
    pub fn required_java(&self) -> Option<u32> {
        self.class_major.and_then(Self::java_release)
    }

    pub fn depends_on(&self, id: &str) -> Option<&Dependency> {
        self.dependencies
            .iter()
            .find(|d| d.id == id && d.kind == DependencyKind::Depends)
    }
}

/// What a jar declares across all of the loader metadata files it carries.
#[derive(Clone, Debug)]
pub struct Detected {
    /// The primary metadata (the first mod).
    pub metadata: ModMetadata,
    /// Every metadata file that was found: `(path, parsed)`.
    pub all: Vec<ModMetadata>,
    /// Files that look like loader metadata but couldn't be parsed.
    pub warnings: Vec<String>,
}

/// Reads whichever loader metadata the jar has. A jar with both is unusual
/// but not an error; Fabric is preferred, then NeoForge, then Forge.
pub fn detect(jar: &mut Jar) -> Result<Detected, String> {
    let mut warnings = Vec::new();
    let mut all = Vec::new();
    if jar.contains(FABRIC_METADATA) {
        let text = jar.read_text(FABRIC_METADATA)?;
        match fabric(&text) {
            Ok(mods) => all.extend(mods),
            Err(e) => warnings.push(format!("{FABRIC_METADATA}: {e}")),
        }
    }
    for path in [NEOFORGE_METADATA, FORGE_METADATA] {
        if !jar.contains(path) {
            continue;
        }
        let text = jar.read_text(path)?;
        match neoforge(&text, path) {
            Ok(mods) => all.extend(mods),
            Err(e) => warnings.push(format!("{path}: {e}")),
        }
        // neoforge.mods.toml wins over mods.toml in the same jar.
        if all.iter().any(|m| m.loader == Loader::NeoForge) {
            break;
        }
    }
    if all.is_empty() {
        let found = warnings.join("; ");
        return Err(match found.is_empty() {
            true => format!("no {FABRIC_METADATA}, {NEOFORGE_METADATA} or {FORGE_METADATA}"),
            false => format!("no readable mod metadata ({found})"),
        });
    }
    let mut metadata = all.remove(0);
    // Every metadata record sees the jar's classes and nested jars.
    metadata.classes = jar.class_names();
    metadata.nested_jars = jar.nested_jars();
    metadata.class_major = lowest_class_major(jar, &metadata.classes);
    for other in &mut all {
        other.classes.clone_from(&metadata.classes);
        other.nested_jars.clone_from(&metadata.nested_jars);
        other.class_major = metadata.class_major;
    }
    all.insert(0, metadata.clone());
    Ok(Detected {
        metadata,
        all,
        warnings,
    })
}

/// The lowest class-file major version, and each class's, so a mod mixing
/// Java 8 and Java 21 classes needs only the highest (the runtime can run
/// all of them; the *requirement* is the highest). Returned as the highest,
/// because that is the runtime a mod needs.
fn lowest_class_major(jar: &mut Jar, classes: &[String]) -> Option<u16> {
    let mut highest = None;
    for class in classes {
        let Ok(bytes) = jar.read(&format!("{class}.class")) else {
            continue;
        };
        if bytes.len() < 8 || &bytes[..4] != b"\xca\xfe\xba\xbe" {
            continue;
        }
        let major = u16::from_be_bytes([bytes[6], bytes[7]]);
        highest = Some(highest.map_or(major, |h: u16| h.max(major)));
    }
    highest
}

fn fabric(text: &str) -> Result<Vec<ModMetadata>, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let object = value.as_object().ok_or("the metadata is not an object")?;
    let schema = object
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    if schema != 1 {
        return Err(format!("unsupported schemaVersion {schema}"));
    }
    let id = string(object.get("id")).ok_or("missing id")?;
    if !ModMetadata::valid_id(&id) {
        return Err(format!("invalid mod id {id:?}"));
    }
    let version = string(object.get("version")).ok_or("missing version")?;
    let name = string(object.get("name")).unwrap_or_else(|| id.clone());
    let description = string(object.get("description")).unwrap_or_default();
    let license = string(object.get("license")).unwrap_or_default();
    let authors = match object.get("authors") {
        Some(Value::Array(items)) => items.iter().filter_map(text_of).collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    };
    let environment = match string(object.get("environment")).as_deref() {
        Some("client") => Environment::Client,
        Some("server") => Environment::Server,
        _ => Environment::Both,
    };
    let mut entrypoints = Vec::new();
    if let Some(Value::Object(map)) = object.get("entrypoints") {
        for (kind, value) in map {
            parse_fabric_entrypoints(kind, value, &mut entrypoints)?;
        }
    }
    let mut dependencies = Vec::new();
    for (key, kind) in [
        ("depends", DependencyKind::Depends),
        ("recommends", DependencyKind::Recommends),
        ("suggests", DependencyKind::Suggests),
        ("breaks", DependencyKind::Breaks),
        ("conflicts", DependencyKind::Conflicts),
    ] {
        if let Some(Value::Object(map)) = object.get(key) {
            for (id, range) in map {
                let range = match range {
                    Value::Array(items) => items
                        .iter()
                        .filter_map(text_of)
                        .collect::<Vec<_>>()
                        .join(" "),
                    other => text_of(other).unwrap_or_else(|| "*".into()),
                };
                dependencies.push(Dependency {
                    id: id.clone(),
                    kind,
                    range: VersionRange::parse(&range),
                    ordering: "NONE".into(),
                    side: "BOTH".into(),
                    mandatory: kind == DependencyKind::Depends,
                });
            }
        }
    }
    let mut mixins = Vec::new();
    if let Some(Value::Array(items)) = object.get("mixins") {
        for item in items {
            match item {
                Value::String(path) => mixins.push(MixinConfig {
                    path: path.clone(),
                    package: None,
                    mixins: Vec::new(),
                    client: Vec::new(),
                    server: Vec::new(),
                    required: true,
                }),
                Value::Object(o) => {
                    if let Some(path) = string(o.get("config")) {
                        mixins.push(MixinConfig {
                            path,
                            package: None,
                            mixins: Vec::new(),
                            client: Vec::new(),
                            server: Vec::new(),
                            required: o.get("required").and_then(Value::as_bool).unwrap_or(true),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    let nested_jars = match object.get("jars") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|i| string(i.get("file")).or_else(|| text_of(i)))
            .collect(),
        _ => Vec::new(),
    };
    Ok(vec![ModMetadata {
        loader: Loader::Fabric,
        id,
        name,
        version,
        description,
        authors,
        license,
        environment,
        entrypoints,
        dependencies,
        mixins,
        access_widener: string(object.get("accessWidener")),
        access_transformer: None,
        source: FABRIC_METADATA.into(),
        format: format!("schemaVersion {schema}"),
        extra_mod_ids: Vec::new(),
        nested_jars,
        class_major: None,
        classes: Vec::new(),
        raw: text.to_owned(),
    }])
}

fn parse_fabric_entrypoints(
    kind: &str,
    value: &Value,
    out: &mut Vec<Entrypoint>,
) -> Result<(), String> {
    match value {
        Value::String(class) => out.push(Entrypoint {
            kind: kind.into(),
            class: class.clone(),
            adapter: "default".into(),
        }),
        Value::Array(items) => {
            for item in items {
                parse_fabric_entrypoints(kind, item, out)?;
            }
        }
        Value::Object(object) => {
            let class = string(object.get("value"))
                .ok_or_else(|| format!("entrypoint {kind}: missing value"))?;
            out.push(Entrypoint {
                kind: kind.into(),
                class,
                adapter: string(object.get("adapter")).unwrap_or_else(|| "default".into()),
            });
        }
        _ => return Err(format!("entrypoint {kind}: not a class name")),
    }
    Ok(())
}

fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
    })
}

fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn neoforge(text: &str, path: &str) -> Result<Vec<ModMetadata>, String> {
    let root: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let table = root.as_table().ok_or("the metadata is not a table")?;
    let loader_name = table
        .get("modLoader")
        .and_then(toml::Value::as_str)
        .unwrap_or("javafml");
    let loader = match loader_name {
        "lowcodefml" | "javafml" => Loader::NeoForge,
        "forge" | "fml" => Loader::Forge,
        other => return Err(format!("unsupported modLoader {other:?}")),
    };
    let mods = table
        .get("mods")
        .and_then(toml::Value::as_array)
        .ok_or("no [[mods]]")?;
    if mods.is_empty() {
        return Err("[[mods]] is empty".to_owned());
    }
    let dependency_table = table.get("dependencies").and_then(toml::Value::as_table);
    let mixin_configs: Vec<String> = table
        .get("mixins")
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|m| m.get("config").and_then(toml::Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let access_transformer = table
        .get("accessTransformers")
        .and_then(toml::Value::as_array)
        .and_then(|items| items.first())
        .and_then(|m| m.get("file").and_then(toml::Value::as_str))
        .map(str::to_owned)
        .or_else(|| {
            table
                .get("accessTransformer")
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
        });
    let license = table
        .get("license")
        .and_then(toml::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut out = Vec::new();
    for (index, m) in mods.iter().enumerate() {
        let id = m
            .get("modId")
            .and_then(toml::Value::as_str)
            .ok_or("a [[mods]] entry has no modId")?
            .to_owned();
        if !ModMetadata::valid_id(&id) {
            return Err(format!("invalid mod id {id:?}"));
        }
        let version = m
            .get("version")
            .and_then(toml::Value::as_str)
            .unwrap_or("0")
            .to_owned();
        let name = m
            .get("displayName")
            .and_then(toml::Value::as_str)
            .unwrap_or(&id)
            .to_owned();
        let description = m
            .get("description")
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let authors = m
            .get("authors")
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let authors = authors
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let side = m
            .get("side")
            .and_then(toml::Value::as_str)
            .unwrap_or("BOTH");
        let environment = match side.to_ascii_uppercase().as_str() {
            "CLIENT" => Environment::Client,
            "SERVER" => Environment::Server,
            _ => Environment::Both,
        };
        let mut dependencies = Vec::new();
        if let Some(deps) = dependency_table
            .and_then(|t| t.get(&id))
            .and_then(toml::Value::as_array)
        {
            for dep in deps {
                let dep_id = dep
                    .get("modId")
                    .and_then(toml::Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                if dep_id.is_empty() {
                    continue;
                }
                let range = dep
                    .get("versionRange")
                    .and_then(toml::Value::as_str)
                    .unwrap_or("*");
                dependencies.push(Dependency {
                    id: dep_id,
                    kind: DependencyKind::Depends,
                    range: VersionRange::parse(range),
                    ordering: dep
                        .get("ordering")
                        .and_then(toml::Value::as_str)
                        .unwrap_or("NONE")
                        .to_owned(),
                    side: dep
                        .get("side")
                        .and_then(toml::Value::as_str)
                        .unwrap_or("BOTH")
                        .to_owned(),
                    mandatory: dep
                        .get("mandatory")
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(true),
                });
            }
        }
        let mut entrypoints = Vec::new();
        if index == 0 {
            // The @Mod class is found by scanning RuntimeVisibleAnnotations;
            // this placeholder tells the loader to do that.
            entrypoints.push(Entrypoint {
                kind: "mod".into(),
                class: String::new(),
                adapter: "neoforge".into(),
            });
            // Keep the metadata's mixin list once, on the primary mod.
            out.push(ModMetadata {
                loader,
                id,
                name,
                version,
                description,
                authors,
                license: license.clone(),
                environment,
                entrypoints: std::mem::take(&mut entrypoints),
                dependencies,
                mixins: mixin_configs
                    .iter()
                    .map(|path| MixinConfig {
                        path: path.clone(),
                        package: None,
                        mixins: Vec::new(),
                        client: Vec::new(),
                        server: Vec::new(),
                        required: true,
                    })
                    .collect(),
                access_widener: None,
                access_transformer: access_transformer.clone(),
                source: path.into(),
                format: format!("{loader_name} / mods.toml"),
                extra_mod_ids: Vec::new(),
                nested_jars: Vec::new(),
                class_major: None,
                classes: Vec::new(),
                raw: text.to_owned(),
            });
        } else {
            out.push(ModMetadata {
                loader,
                id,
                name,
                version,
                description,
                authors,
                license: license.clone(),
                environment,
                entrypoints: Vec::new(),
                dependencies,
                mixins: Vec::new(),
                access_widener: None,
                access_transformer: None,
                source: path.into(),
                format: format!("{loader_name} / mods.toml"),
                extra_mod_ids: Vec::new(),
                nested_jars: Vec::new(),
                class_major: None,
                classes: Vec::new(),
                raw: text.to_owned(),
            });
        }
    }
    // Record the sibling ids on the primary entry.
    let ids: Vec<String> = out.iter().map(|m| m.id.clone()).collect();
    if let Some(first) = out.first_mut() {
        first.extra_mod_ids = ids.into_iter().skip(1).collect();
    }
    Ok(out)
}

/// Merge a mixin config file's contents into the metadata's list.
pub fn read_mixin_config(jar: &mut Jar, config: &mut MixinConfig) -> Result<(), String> {
    let text = jar.read_text(&config.path)?;
    let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("{}: not an object", config.path))?;
    config.package = string(object.get("package"));
    let names = |key: &str| -> Vec<String> {
        object
            .get(key)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(text_of).collect())
            .unwrap_or_default()
    };
    config.mixins = names("mixins");
    config.client = names("client");
    config.server = names("server");
    if config.package.is_none() {
        return Err(format!("{}: no package", config.path));
    }
    Ok(())
}

/// Group Fabric's entrypoints by kind, keeping declaration order.
pub fn entrypoints_by_kind(entries: &[Entrypoint]) -> BTreeMap<&str, Vec<&Entrypoint>> {
    let mut out: BTreeMap<&str, Vec<&Entrypoint>> = BTreeMap::new();
    for e in entries {
        out.entry(e.kind.as_str()).or_default().push(e);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fabric_metadata() {
        let json = r#"{
            "schemaVersion": 1,
            "id": "ruby",
            "version": "1.2.0",
            "name": "Ruby",
            "description": "Adds ruby",
            "authors": ["A"],
            "license": "MIT",
            "environment": "client",
            "entrypoints": {
                "main": ["ruby.RubyMod"],
                "client": [{"adapter": "default", "value": "ruby.RubyClient"}]
            },
            "mixins": ["ruby.mixins.json", {"config": "ruby.client.mixins.json", "required": false}],
            "accessWidener": "ruby.accesswidener",
            "depends": {"minecraft": "26.3", "fabricloader": ">=0.19.5", "java": ">=21"},
            "recommends": {"fabric-api": "*"}
        }"#;
        let mods = fabric(json).unwrap();
        let m = &mods[0];
        assert_eq!(m.loader, Loader::Fabric);
        assert_eq!(m.id, "ruby");
        assert_eq!(m.environment, Environment::Client);
        assert_eq!(m.entrypoints.len(), 2);
        assert_eq!(m.mixins.len(), 2);
        assert!(!m.mixins[1].required);
        assert!(m.depends_on("minecraft").unwrap().range.matches("26.3"));
        assert_eq!(m.access_widener.as_deref(), Some("ruby.accesswidener"));
    }

    #[test]
    fn reads_neoforge_metadata() {
        let toml = r#"
modLoader = "javafml"
loaderVersion = "[4,)"
license = "MIT"
[[mixins]]
config = "ruby.mixins.json"
[[accessTransformers]]
file = "META-INF/accesstransformer.cfg"
[[mods]]
modId = "ruby"
version = "1.2.0"
displayName = "Ruby"
description = "Adds ruby"
authors = "A, B"
side = "CLIENT"
[[dependencies.ruby]]
modId = "neoforge"
type = "required"
versionRange = "[26.3,)"
ordering = "NONE"
side = "BOTH"
mandatory = true
[[dependencies.ruby]]
modId = "minecraft"
type = "required"
versionRange = "[26.3,26.4)"
ordering = "NONE"
side = "BOTH"
mandatory = true
"#;
        let mods = neoforge(toml, NEOFORGE_METADATA).unwrap();
        let m = &mods[0];
        assert_eq!(m.loader, Loader::NeoForge);
        assert_eq!(m.id, "ruby");
        assert_eq!(m.authors, ["A", "B"]);
        assert_eq!(m.environment, Environment::Client);
        assert_eq!(m.mixins[0].path, "ruby.mixins.json");
        assert_eq!(
            m.access_transformer.as_deref(),
            Some("META-INF/accesstransformer.cfg")
        );
        assert!(m.depends_on("neoforge").unwrap().range.matches("26.3.1"));
        assert!(!m.depends_on("minecraft").unwrap().range.matches("26.4"));
    }

    #[test]
    fn refuses_bad_ids() {
        let json = r#"{"schemaVersion":1,"id":"../evil","version":"1"}"#;
        assert!(fabric(json).is_err());
        let json = r#"{"schemaVersion":1,"id":"ok","version":"1"}"#;
        assert!(fabric(json).is_ok());
    }
}
