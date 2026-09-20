use crate::schema::{SCHEMA_VERSION, SchemaDocument};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub const CACHE_VERSION: u16 = 1;
pub const MAX_CACHE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    pub executable_path: String,
    pub executable_fingerprint: String,
    pub invoked: Vec<String>,
    pub parser_version: String,
    pub schema_version: u16,
}

impl CacheKey {
    pub fn new(
        executable_path: impl Into<String>,
        executable_fingerprint: impl Into<String>,
        invoked: Vec<String>,
        parser_version: impl Into<String>,
    ) -> Self {
        Self {
            executable_path: executable_path.into(),
            executable_fingerprint: executable_fingerprint.into(),
            invoked,
            parser_version: parser_version.into(),
            schema_version: SCHEMA_VERSION,
        }
    }
}

#[derive(Debug)]
pub enum CacheError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidDocument,
    RecordTooLarge,
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "cache I/O error: {error}"),
            Self::Json(error) => write!(formatter, "cache JSON error: {error}"),
            Self::InvalidDocument => formatter.write_str("cache document failed schema validation"),
            Self::RecordTooLarge => formatter.write_str("cache record exceeds the safety limit"),
        }
    }
}

impl Error for CacheError {}

impl From<std::io::Error> for CacheError {
    fn from(error: std::io::Error) -> Self { Self::Io(error) }
}

impl From<serde_json::Error> for CacheError {
    fn from(error: serde_json::Error) -> Self { Self::Json(error) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CacheRecord {
    cache_version: u16,
    key: CacheKey,
    document: SchemaDocument,
}

#[derive(Clone, Debug)]
pub struct SchemaCache {
    root: PathBuf,
    max_bytes: usize,
}

impl SchemaCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), max_bytes: MAX_CACHE_BYTES }
    }

    pub fn default_location() -> Option<PathBuf> {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
            .map(|root| root.join("flagpick"))
    }

    pub fn get(&self, key: &CacheKey) -> Result<Option<SchemaDocument>, CacheError> {
        let path = self.path_for(key);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if bytes.len() > self.max_bytes {
            let _ = fs::remove_file(path);
            return Ok(None);
        }
        let record: CacheRecord = match serde_json::from_slice(&bytes) {
            Ok(record) => record,
            Err(_) => {
                let _ = fs::remove_file(path);
                return Ok(None);
            }
        };
        if record.cache_version != CACHE_VERSION || record.key != *key || record.document.validate().is_err() {
            let _ = fs::remove_file(path);
            return Ok(None);
        }
        Ok(Some(record.document))
    }

    pub fn put(&self, key: &CacheKey, document: &SchemaDocument) -> Result<(), CacheError> {
        document.validate().map_err(|_| CacheError::InvalidDocument)?;
        let record = CacheRecord { cache_version: CACHE_VERSION, key: key.clone(), document: document.clone() };
        let bytes = serde_json::to_vec_pretty(&record)?;
        if bytes.len() > self.max_bytes {
            return Err(CacheError::RecordTooLarge);
        }
        fs::create_dir_all(&self.root)?;
        let path = self.path_for(key);
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, bytes)?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), CacheError> {
        match fs::remove_dir_all(&self.root) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn list(&self) -> Result<Vec<PathBuf>, CacheError> {
        let mut entries = Vec::new();
        let directory = match fs::read_dir(&self.root) {
            Ok(directory) => directory,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(error) => return Err(error.into()),
        };
        for entry in directory {
            let path = entry?.path();
            if path.extension().is_some_and(|extension| extension == "json") {
                entries.push(path);
            }
        }
        entries.sort();
        Ok(entries)
    }

    fn path_for(&self, key: &CacheKey) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        self.root.join(format!("{:016x}.json", hasher.finish()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Confidence, ExecutableIdentity, HelpSource, SpecMetadata, CommandSpec};
    use std::collections::BTreeMap;

    fn document() -> SchemaDocument {
        SchemaDocument::new(CommandSpec {
            executable: ExecutableIdentity { name: "demo".into(), path: Some("/tmp/demo".into()) },
            name: "demo".into(), description: None, usage: vec![], subcommands: vec![],
            options: vec![], positionals: vec![], metadata: SpecMetadata {
                source: HelpSource::GenericHelp, parser: Some("test".into()), confidence: Confidence::Medium,
                executable_version: None, fingerprint: Some("fingerprint".into()), extensions: BTreeMap::new(),
            },
        })
    }

    fn cache() -> (SchemaCache, PathBuf) {
        let root = std::env::temp_dir().join(format!("flagpick-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        (SchemaCache::new(&root), root)
    }

    #[test]
    fn round_trips_and_invalidates_by_key() {
        let (cache, root) = cache();
        let key = CacheKey::new("/tmp/demo", "one", vec!["demo".into()], "generic-v1");
        cache.put(&key, &document()).unwrap();
        assert_eq!(cache.get(&key).unwrap(), Some(document()));
        let changed = CacheKey::new("/tmp/demo", "two", vec!["demo".into()], "generic-v1");
        assert_eq!(cache.get(&changed).unwrap(), None);
        cache.clear().unwrap();
        assert!(cache.list().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_records_are_misses_and_do_not_persist_shell_text() {
        let (cache, root) = cache();
        let key = CacheKey::new("/tmp/demo", "one", vec!["demo".into()], "generic-v1");
        cache.put(&key, &document()).unwrap();
        let path = cache.list().unwrap().pop().unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("shell buffer"));
        fs::write(&path, b"not-json").unwrap();
        assert_eq!(cache.get(&key).unwrap(), None);
        assert!(cache.list().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
