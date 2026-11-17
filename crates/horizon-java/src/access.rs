//! Access wideners (Fabric) and access transformers (NeoForge/Forge).
//!
//! Horizon's world, items and blocks are Rust, so these files cannot change
//! JVM bytecode access in Horizon itself. Their entries are still read and
//! validated: the loader uses them to decide which Minecraft members a mod
//! expects widened, and a diagnostic names each one Horizon's compatibility
//! layer does not currently provide. Silently accepting and ignoring them
//! would be a lie about compatibility.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessAction {
    Accessible,
    Extendable,
    Mutable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind {
    Class,
    Method,
    Field,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessEntry {
    pub action: AccessAction,
    pub kind: AccessKind,
    /// JVM internal name (`net/minecraft/world/item/Item`) for a class, or
    /// the owner of a member.
    pub class: String,
    /// The member name, empty for a class.
    pub name: String,
    /// The member descriptor, empty when the file gives none.
    pub descriptor: String,
    /// `transitive-` entries also apply to subclasses.
    pub transitive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessWidener {
    pub namespace: String,
    pub version: u32,
    pub entries: Vec<AccessEntry>,
}

/// Parse Fabric's `.accesswidener` (v1 and v2).
pub fn access_widener(text: &str) -> Result<AccessWidener, String> {
    let mut lines = text.lines();
    let header = lines
        .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .ok_or("empty access widener")?
        .split_whitespace()
        .collect::<Vec<_>>();
    if header.len() != 3 || header[0] != "accessWidener" {
        return Err("the header must be `accessWidener v1|v2 <namespace>`".into());
    }
    let version = header[1]
        .strip_prefix('v')
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v == 1 || *v == 2)
        .ok_or_else(|| format!("unsupported access widener version {}", header[1]))?;
    let namespace = header[2].to_owned();
    if namespace.is_empty() {
        return Err("the access widener has no namespace".into());
    }
    let mut entries = Vec::new();
    for (number, raw) in lines.enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let (transitive, action) = match parts.first().copied() {
            Some("accessible") => (false, AccessAction::Accessible),
            Some("extendable") => (false, AccessAction::Extendable),
            Some("mutable") => (false, AccessAction::Mutable),
            Some("transitive-accessible") => (true, AccessAction::Accessible),
            Some("transitive-extendable") => (true, AccessAction::Extendable),
            Some("transitive-mutable") => (true, AccessAction::Mutable),
            Some(other) => return Err(format!("line {}: unknown action {other:?}", number + 2)),
            None => continue,
        };
        let kind = match parts.get(1).copied() {
            Some("class") => AccessKind::Class,
            Some("method") => AccessKind::Method,
            Some("field") => AccessKind::Field,
            Some(other) => return Err(format!("line {}: unknown kind {other:?}", number + 2)),
            None => return Err(format!("line {}: missing kind", number + 2)),
        };
        let class = parts
            .get(2)
            .ok_or_else(|| format!("line {}: missing class", number + 2))?
            .to_string();
        check_class_name(&class).map_err(|e| format!("line {}: {e}", number + 2))?;
        let (name, descriptor) = match kind {
            AccessKind::Class => (String::new(), String::new()),
            AccessKind::Method => (
                parts
                    .get(3)
                    .ok_or_else(|| format!("line {}: missing method name", number + 2))?
                    .to_string(),
                parts
                    .get(4)
                    .ok_or_else(|| format!("line {}: missing method descriptor", number + 2))?
                    .to_string(),
            ),
            AccessKind::Field => (
                parts
                    .get(3)
                    .ok_or_else(|| format!("line {}: missing field name", number + 2))?
                    .to_string(),
                parts
                    .get(4)
                    .filter(|d| !d.is_empty())
                    .map(|d| (*d).to_string())
                    .unwrap_or_default(),
            ),
        };
        entries.push(AccessEntry {
            action,
            kind,
            class,
            name,
            descriptor,
            transitive,
        });
    }
    Ok(AccessWidener {
        namespace,
        version,
        entries,
    })
}

/// Parse a NeoForge/Forge `accesstransformer.cfg`.
pub fn access_transformer(text: &str) -> Result<Vec<AccessEntry>, String> {
    let mut entries = Vec::new();
    for (number, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let action = match parts.first().copied() {
            Some("public") | Some("protected") | Some("default") | Some("private") => {
                AccessAction::Accessible
            }
            Some("public-f") | Some("protected-f") | Some("default-f") | Some("private-f") => {
                AccessAction::Accessible
            }
            Some("public+") | Some("protected+") | Some("default+") => AccessAction::Mutable,
            Some(other) => {
                return Err(format!(
                    "line {}: unknown access transformer {other:?}",
                    number + 1
                ));
            }
            None => continue,
        };
        // The dialect after the access level is `net.minecraft.Class`, then
        // optionally `method(args)ret` or a field name.
        let target = parts
            .get(1)
            .ok_or_else(|| format!("line {}: missing target", number + 1))?;
        let (class, member) = match parts.get(2).copied() {
            // `Class member` (the form Forge's config uses) is split by
            // `split_whitespace`, so the member is its own token.
            Some(member) => (*target, Some(member)),
            None => split_target(target),
        };
        let class = class.trim();
        if class.is_empty() {
            return Err(format!("line {}: missing class", number + 1));
        }
        let (kind, name, descriptor) = match member {
            Some(member) if member.contains('(') => {
                let (name, descriptor) = member
                    .split_once('(')
                    .map(|(name, rest)| (name, format!("({rest}")))
                    .unwrap_or((member, String::new()));
                (AccessKind::Method, name.to_owned(), descriptor)
            }
            Some(member) => (AccessKind::Field, member.to_owned(), String::new()),
            None => (AccessKind::Class, String::new(), String::new()),
        };
        entries.push(AccessEntry {
            action,
            kind,
            class: class.replace('.', "/"),
            name,
            descriptor,
            transitive: false,
        });
    }
    Ok(entries)
}

/// `a.b.C.method` → (`a.b.C`, Some(`method`)); `a.b.C` → (`a.b.C`, None).
/// A member may be attached with a space in some generated transformers, so
/// both forms are accepted.
fn split_target(target: &str) -> (&str, Option<&str>) {
    // A class ends with an upper-case segment; a member follows the last dot
    // whose following segment starts lower-case. This is ambiguous for
    // lower-case class names, which are not used by Minecraft but can be by
    // mods; those take the "whole target is a class" reading unless the
    // caller separated the member with a space.
    match target.rsplit_once('.') {
        Some((class, member))
            if member
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_lowercase() || c == '_') =>
        {
            (class, Some(member))
        }
        _ => (target, None),
    }
}

fn check_class_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.starts_with('/')
        || name.contains("..")
        || name.contains(':')
        || name.contains('\\')
        || name
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(format!("invalid class name {name:?}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_access_widener_v2() {
        let text = "\
accessWidener v2 named
# a comment
accessible class net/minecraft/world/item/Item
extendable method net/minecraft/world/level/block/Block <init> (Lnet/minecraft/world/level/block/Block$Properties;)V
transitive-mutable field net/minecraft/world/entity/Entity x D
";
        let aw = access_widener(text).unwrap();
        assert_eq!(aw.version, 2);
        assert_eq!(aw.namespace, "named");
        assert_eq!(aw.entries.len(), 3);
        assert_eq!(aw.entries[0].kind, AccessKind::Class);
        assert_eq!(aw.entries[1].kind, AccessKind::Method);
        assert_eq!(aw.entries[1].name, "<init>");
        assert!(aw.entries[2].transitive);
    }

    #[test]
    fn refuses_unknown_actions() {
        assert!(access_widener("accessWidener v2 named\nopen class a/B\n").is_err());
        assert!(access_widener("accessWidener v3 named\n").is_err());
        assert!(access_widener("accessible class a/B\n").is_err());
    }

    #[test]
    fn reads_access_transformer() {
        let text = "\
public net.minecraft.world.item.Item
public-f net.minecraft.world.level.Level method(Ljava/lang/String;)V
public net.minecraft.world.entity.Entity field
";
        let at = access_transformer(text).unwrap();
        assert_eq!(at.len(), 3);
        assert_eq!(at[0].class, "net/minecraft/world/item/Item");
        assert_eq!(at[1].kind, AccessKind::Method);
        assert_eq!(at[1].descriptor, "(Ljava/lang/String;)V");
        assert_eq!(at[2].kind, AccessKind::Field);
    }
}
