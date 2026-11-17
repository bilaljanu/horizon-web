//! Version ranges used by Fabric (`>=1.20 <1.21`, `^0.16`) and NeoForge
//! (`[1.20.1,1.21)`) mod metadata.
//!
//! Minecraft versions are not semantic versions: `26.3` is newer than
//! `1.21.11`, while `24w14a` is a snapshot. Versions are compared by their
//! numeric components first, then by their text, with the well-known
//! pre-release qualifiers ordered before a release.

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Version {
    /// The version exactly as the mod or the game wrote it.
    pub text: String,
    parts: Vec<Part>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Part {
    Number(u64),
    Text(String),
}

impl Version {
    pub fn parse(text: &str) -> Self {
        let text = text.trim();
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut numeric = None;
        for ch in text.chars() {
            let digit = ch.is_ascii_digit();
            if numeric.is_some_and(|n| n != digit) {
                push_part(&mut parts, &current, numeric == Some(true));
                current.clear();
            }
            numeric = Some(digit);
            if ch == '-' || ch == '+' || ch == '.' || ch == '_' {
                if !current.is_empty() {
                    push_part(&mut parts, &current, numeric == Some(true) && digit);
                }
                current.clear();
                numeric = None;
                if ch == '-' || ch == '+' {
                    // A qualifier: keep it in the text stream, the comparison
                    // below knows the qualifiers that sort before a release.
                    parts.push(Part::Text(ch.to_string()));
                }
            } else {
                current.push(ch.to_ascii_lowercase());
            }
        }
        if !current.is_empty() {
            push_part(&mut parts, &current, numeric == Some(true));
        }
        Self {
            text: text.to_owned(),
            parts,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    fn numbers(&self) -> impl Iterator<Item = u64> + '_ {
        self.parts.iter().filter_map(|p| match p {
            Part::Number(n) => Some(*n),
            Part::Text(_) => None,
        })
    }

    /// The first `n` numeric components, missing ones as zero.
    fn prefix(&self, n: usize) -> Vec<u64> {
        let mut out: Vec<u64> = self.numbers().take(n).collect();
        out.resize(n, 0);
        out
    }
}

fn push_part(parts: &mut Vec<Part>, text: &str, number: bool) {
    if number && let Ok(n) = text.parse() {
        parts.push(Part::Number(n));
    } else if !text.is_empty() {
        parts.push(Part::Text(text.to_owned()));
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        // Numeric components decide first, one for one.
        let n = self.parts.len().max(other.parts.len()).min(12);
        for i in 0..n {
            let a = self.parts.get(i);
            let b = other.parts.get(i);
            let ord = match (a, b) {
                (Some(Part::Number(a)), Some(Part::Number(b))) => a.cmp(b),
                (Some(Part::Number(a)), None) => a.cmp(&0),
                (None, Some(Part::Number(b))) => 0.cmp(b),
                // A qualifier before the end of the release sorts first:
                // 1.0-alpha < 1.0, and 1.0 < 1.0.1.
                (Some(Part::Text(a)), Some(Part::Text(b))) => qualifier(a).cmp(&qualifier(b)),
                // A number outranks a qualifier at the same position:
                // 1.0.1 > 1.0.0 and 1.0.0 > 1.0.0-alpha.
                (Some(Part::Number(_)), Some(Part::Text(_))) => Ordering::Greater,
                (Some(Part::Text(_)), Some(Part::Number(_))) => Ordering::Less,
                (Some(Part::Text(_)), None) => Ordering::Less,
                (None, Some(Part::Text(_))) => Ordering::Greater,
                (None, None) => break,
            };
            if ord != Ordering::Equal {
                return ord;
            }
        }
        self.parts.len().cmp(&other.parts.len())
    }
}

/// Qualifiers that sort before a release, as Maven orders them.
fn qualifier(text: &str) -> (u8, &str) {
    match text {
        "-" => (0, ""),
        "alpha" | "a" => (1, ""),
        "beta" | "b" => (2, ""),
        "milestone" | "m" => (3, ""),
        "rc" | "cr" => (4, ""),
        "snapshot" => (5, ""),
        "pre" | "preview" => (6, ""),
        "release" | "final" | "ga" => (9, ""),
        "sp" => (10, ""),
        other => (7, other),
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// A version predicate: Fabric's predicate syntax, or a Maven range
/// (NeoForge's `versionRange`). `*` accepts everything.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionRange {
    pub text: String,
    terms: Vec<Term>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Term {
    op: Op,
    version: Version,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    /// `~1.2.3`: at least the version and below the next minor.
    Tilde,
    /// `^1.2.3`: at least the version and below the next major.
    Caret,
}

impl VersionRange {
    /// Parse Fabric's or Maven's syntax. An empty range accepts everything
    /// (Fabric treats a missing dependency version that way).
    pub fn parse(text: &str) -> Self {
        let text = text.trim();
        let mut terms = Vec::new();
        let mut rest = text;
        // Maven ranges are bracketed; a bare version means "at least" in
        // NeoForge and "exactly" in Fabric. The caller's loader decides by
        // calling `parse_maven` explicitly for NeoForge.
        while let Some(start) = rest.find(['[', '(']) {
            let after = &rest[start..];
            let Some(end) = after.find([']', ')']) else {
                break;
            };
            let body = &after[1..end];
            let (low, high) = body.split_once(',').unwrap_or((body, ""));
            let include_low = after.starts_with('[');
            let include_high = after.as_bytes().get(end).is_some_and(|b| *b == b']');
            if !low.trim().is_empty() {
                terms.push(Term {
                    op: if include_low { Op::Ge } else { Op::Gt },
                    version: Version::parse(low),
                });
            }
            if !high.trim().is_empty() {
                terms.push(Term {
                    op: if include_high { Op::Le } else { Op::Lt },
                    version: Version::parse(high),
                });
            }
            rest = &after[end + 1..];
        }
        let rest = rest.trim();
        if !rest.is_empty() {
            terms.extend(parse_fabric_terms(rest));
        }
        Self {
            text: text.to_owned(),
            terms,
        }
    }

    /// Whether a version matches. An empty predicate matches everything.
    pub fn matches(&self, version: &str) -> bool {
        let v = Version::parse(version);
        if self.text.trim().is_empty() || self.text.trim() == "*" {
            return true;
        }
        // A lone bare version in a Fabric dependency means "this or newer".
        self.terms.iter().all(|term| match term.op {
            Op::Eq => compare_eq(&v, &term.version),
            Op::Ne => !compare_eq(&v, &term.version),
            Op::Gt => v > term.version,
            Op::Ge => v >= term.version,
            Op::Lt => v < term.version,
            Op::Le => v <= term.version,
            Op::Tilde => {
                let prefix = term.version.prefix(2);
                v >= term.version && v.prefix(2) == prefix && {
                    // The next minor: bump the second component.
                    v < next(&prefix, 1)
                }
            }
            Op::Caret => {
                let mut prefix = term.version.prefix(3);
                // ^0.x.y stays below 0.(x+1); ^x.y.z below (x+1).
                let bump = if prefix[0] == 0 { 1 } else { 0 };
                if bump == 1 {
                    prefix.truncate(2);
                } else {
                    prefix.truncate(1);
                }
                v >= term.version && v < next(&prefix, bump)
            }
        })
    }

    /// Whether this is `*` or empty (every game version is acceptable).
    pub fn is_any(&self) -> bool {
        self.text.trim().is_empty() || self.text.trim() == "*"
    }
}

fn compare_eq(a: &Version, b: &Version) -> bool {
    a == b || a.prefix(12) == b.prefix(12)
}

fn next(prefix: &[u64], bump: usize) -> Version {
    let mut parts = prefix.to_vec();
    while parts.len() <= bump {
        parts.push(0);
    }
    parts[bump] += 1;
    parts.truncate(bump + 1);
    for p in &mut parts[bump + 1..] {
        *p = 0;
    }
    Version::parse(
        &parts
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join("."),
    )
}

fn parse_fabric_terms(text: &str) -> Vec<Term> {
    let mut terms = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        let (op, after) = if let Some(r) = rest.strip_prefix(">=") {
            (Op::Ge, r)
        } else if let Some(r) = rest.strip_prefix("<=") {
            (Op::Le, r)
        } else if let Some(r) = rest.strip_prefix('>') {
            (Op::Gt, r)
        } else if let Some(r) = rest.strip_prefix('<') {
            (Op::Lt, r)
        } else if let Some(r) = rest.strip_prefix('=') {
            (Op::Eq, r)
        } else if let Some(r) = rest.strip_prefix('~') {
            (Op::Tilde, r)
        } else if let Some(r) = rest.strip_prefix('^') {
            (Op::Caret, r)
        } else {
            (Op::Ge, rest)
        };
        let end = after
            .find(|c: char| c == ' ' || c == ',' || c == '\t' || c == '\n')
            .unwrap_or(after.len());
        let v = after[..end].trim();
        if v.is_empty() {
            break;
        }
        terms.push(Term {
            op,
            version: Version::parse(v),
        });
        rest = &after[end..];
        while rest.starts_with([',', ' ']) {
            rest = &rest[1..];
        }
    }
    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_minecraft_and_loader_versions() {
        assert!(Version::parse("26.3") > Version::parse("26.2"));
        assert!(Version::parse("26.3") > Version::parse("1.21.11"));
        assert!(Version::parse("1.21.4") > Version::parse("1.21.1"));
        assert!(Version::parse("0.16.9") > Version::parse("0.15.11"));
        assert!(Version::parse("1.0-alpha") < Version::parse("1.0"));
        assert!(Version::parse("1.0") < Version::parse("1.0.1"));
    }

    #[test]
    fn fabric_predicates() {
        let r = VersionRange::parse(">=1.20.1 <1.21");
        assert!(r.matches("1.20.1"));
        assert!(r.matches("1.20.4"));
        assert!(!r.matches("1.21"));
        assert!(!r.matches("1.19.4"));
        assert!(VersionRange::parse("26.3").matches("26.3"));
        assert!(VersionRange::parse(">=0.19.5").matches("0.19.5"));
        assert!(VersionRange::parse("*").matches("26.3"));
        assert!(VersionRange::parse("").matches("26.3"));
        assert!(VersionRange::parse("^1.20").matches("1.20.9"));
        assert!(VersionRange::parse("^1.20").matches("1.21"));
        assert!(!VersionRange::parse("^1.20").matches("2.0"));
        assert!(VersionRange::parse("~1.20.1").matches("1.20.7"));
        assert!(!VersionRange::parse("~1.20.1").matches("1.21"));
    }

    #[test]
    fn maven_ranges() {
        let r = VersionRange::parse("[1.20.1,1.21)");
        assert!(r.matches("1.20.1"));
        assert!(r.matches("1.20.9"));
        assert!(!r.matches("1.21"));
        assert!(!r.matches("1.20"));
        assert!(VersionRange::parse("[1.20.1,)").matches("26.3"));
        assert!(VersionRange::parse("(,1.21]").matches("1.20"));
        assert!(!VersionRange::parse("(,1.21]").matches("1.21.1"));
        assert!(VersionRange::parse("[26.3]").matches("26.3"));
    }
}
