//! Safe reading of Java mod archives.
//!
//! A mod `.jar` is a ZIP. Archived paths are validated before they are used
//! as paths in Horizon's virtual file system: no absolute paths, no `..`,
//! no backslashes, no drive letters, and no symbolic links. Every read is
//! bounded, so a malformed or hostile archive cannot exhaust the browser's
//! heap.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};

/// The largest mod archive Horizon opens.
pub const MAX_JAR: u64 = 128 << 20;
/// The largest single entry a mod may carry.
pub const MAX_ENTRY: u64 = 64 << 20;
/// The most entries an archive may hold.
pub const MAX_ENTRIES: usize = 100_000;

#[derive(Clone, Debug)]
struct Entry {
    index: usize,
    size: u64,
    directory: bool,
}

/// A mod archive held in memory. Both native and browser builds use this:
/// no file handle is needed after [`Jar::open`].
pub struct Jar {
    archive: zip::ZipArchive<Cursor<Vec<u8>>>,
    entries: BTreeMap<String, Entry>,
    files: Vec<String>,
}

impl Jar {
    pub fn open(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_JAR {
            return Err(format!("the archive is over {} MiB", MAX_JAR >> 20));
        }
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| format!("not a readable .jar: {e}"))?;
        if archive.len() > MAX_ENTRIES {
            return Err(format!("the archive has more than {MAX_ENTRIES} entries"));
        }
        let mut entries = BTreeMap::new();
        let mut files = Vec::new();
        for i in 0..archive.len() {
            let entry = archive
                .by_index_raw(i)
                .map_err(|e| format!("entry {i}: {e}"))?;
            let name = entry
                .name()
                .map_err(|e| format!("entry {i}: {e}"))?
                .into_owned();
            let mode = entry.unix_mode();
            // Directories end in `/`; anything else must be a regular file.
            if mode.is_some_and(|m| !matches!(m & 0o170000, 0 | 0o100000 | 0o040000)) {
                return Err(format!(
                    "{name}: symbolic links and devices are not allowed"
                ));
            }
            if name.contains('\\')
                || name.starts_with('/')
                || name.contains(':')
                || name
                    .strip_suffix('/')
                    .unwrap_or(&name)
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
            {
                return Err(format!("invalid archive path: {name:?}"));
            }
            let directory = name.ends_with('/') || entry.is_dir();
            let size = entry.size();
            if size > MAX_ENTRY {
                return Err(format!("{name} is over {} MiB", MAX_ENTRY >> 20));
            }
            let name = name.trim_end_matches('/').to_owned();
            if name.is_empty() {
                continue;
            }
            if entries
                .insert(
                    name.clone(),
                    Entry {
                        index: i,
                        size,
                        directory,
                    },
                )
                .is_some()
            {
                return Err(format!("{name} appears twice in the archive"));
            }
            if !directory {
                files.push(name);
            }
        }
        files.sort();
        Ok(Self {
            archive,
            entries,
            files,
        })
    }

    /// Every file (not directory) path, sorted.
    pub fn files(&self) -> &[String] {
        &self.files
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, String> {
        let entry = self
            .entries
            .get(name)
            .ok_or_else(|| format!("{name}: not in the .jar"))?;
        if entry.directory {
            return Err(format!("{name}: is a directory"));
        }
        let mut file = self
            .archive
            .by_index(entry.index)
            .map_err(|e| format!("{name}: {e}"))?;
        let limit = entry.size.min(MAX_ENTRY);
        let mut out = Vec::with_capacity(limit as usize);
        file.take(limit + 1)
            .read_to_end(&mut out)
            .map_err(|e| format!("{name}: {e}"))?;
        if out.len() as u64 > limit {
            return Err(format!("{name}: larger than the archive's index says"));
        }
        Ok(out)
    }

    pub fn read_text(&mut self, name: &str) -> Result<String, String> {
        let bytes = self.read(name)?;
        String::from_utf8(bytes).map_err(|_| format!("{name}: not UTF-8"))
    }

    pub fn read_optional(&mut self, name: &str) -> Result<Option<Vec<u8>>, String> {
        if self.contains(name) {
            self.read(name).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn read_optional_text(&mut self, name: &str) -> Result<Option<String>, String> {
        self.read_optional(name)?
            .map(|bytes| String::from_utf8(bytes).map_err(|_| format!("{name}: not UTF-8")))
            .transpose()
    }

    /// JVM class names (`a/b/C`) of every `.class` file.
    pub fn class_names(&self) -> Vec<String> {
        self.files
            .iter()
            .filter_map(|f| f.strip_suffix(".class"))
            .filter(|n| !n.is_empty() && !n.starts_with("META-INF/versions/"))
            .map(str::to_owned)
            .collect()
    }

    /// Nested jars a mod may contain (Fabric's `jars` and libraries): their
    /// paths in the archive.
    pub fn nested_jars(&self) -> Vec<String> {
        self.files
            .iter()
            .filter(|f| {
                f.ends_with(".jar")
                    && (f.starts_with("META-INF/jars/")
                        || f.starts_with("META-INF/libraries/")
                        || f.contains("/libs/"))
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
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
        out
    }

    #[test]
    fn reads_files_and_classes() {
        let bytes = zip(&[
            ("fabric.mod.json", b"{\"id\":\"x\"}"),
            ("a/B.class", b"\xca\xfe\xba\xbe"),
            ("README.txt", b"hi"),
        ]);
        let mut jar = Jar::open(bytes).unwrap();
        assert_eq!(jar.read_text("fabric.mod.json").unwrap(), "{\"id\":\"x\"}");
        assert_eq!(jar.class_names(), ["a/B"]);
        assert!(jar.read("missing").is_err());
    }

    #[test]
    fn refuses_traversal() {
        let bytes = zip(&[("../evil", b"x")]);
        let error = Jar::open(bytes).err().unwrap();
        assert!(error.contains("invalid archive path"), "{error}");
    }

    #[test]
    fn refuses_nested_absolute_paths() {
        let bytes = zip(&[("a/../../evil", b"x")]);
        assert!(Jar::open(bytes).is_err());
        let bytes = zip(&[("/etc/passwd", b"x")]);
        assert!(Jar::open(bytes).is_err());
    }
}
