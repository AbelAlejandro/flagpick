use crate::schema::{SCHEMA_VERSION, SchemaDocument};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use sha2::{Digest, Sha256};

pub const CACHE_VERSION: u16 = 1;
pub const MAX_CACHE_BYTES: usize = 16 * 1024 * 1024;
const CACHE_FILE_PREFIX: &str = "v1-";
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

    pub fn from_executable(
        executable_path: impl Into<PathBuf>,
        invoked: Vec<String>,
        parser_version: impl Into<String>,
    ) -> Result<Self, CacheError> {
        let path = executable_path.into();
        if !path.is_absolute() {
            return Err(CacheError::InvalidExecutablePath);
        }
        let mut file = File::open(&path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        let fingerprint = hex_digest(hasher.finalize());
        Ok(Self::new(
            path.to_string_lossy().into_owned(),
            fingerprint,
            invoked,
            parser_version,
        ))
    }
}

#[derive(Debug)]
pub enum CacheError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidDocument,
    RecordTooLarge,
    InvalidExecutablePath,
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "cache I/O error: {error}"),
            Self::Json(error) => write!(formatter, "cache JSON error: {error}"),
            Self::InvalidDocument => formatter.write_str("cache document failed schema validation"),
            Self::RecordTooLarge => formatter.write_str("cache record exceeds the safety limit"),
            Self::InvalidExecutablePath => formatter.write_str("cache executable path must be absolute"),
        }
    }
}

impl Error for CacheError {}

impl From<std::io::Error> for CacheError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for CacheError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
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
        Self {
            root: root.into(),
            max_bytes: MAX_CACHE_BYTES,
        }
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
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return Ok(None),
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
        if record.cache_version != CACHE_VERSION
            || record.document.validate().is_err()
        {
            let _ = fs::remove_file(path);
            return Ok(None);
        }
        Ok(Some(record.document))
    }

    pub fn put(&self, key: &CacheKey, document: &SchemaDocument) -> Result<(), CacheError> {
        document
            .validate()
            .map_err(|_| CacheError::InvalidDocument)?;
        let record = CacheRecord {
            cache_version: CACHE_VERSION,
            key: key.clone(),
            document: document.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&record)?;
        if bytes.len() > self.max_bytes {
            return Err(CacheError::RecordTooLarge);
        }
        fs::create_dir_all(&self.root)?;
        let path = self.path_for(key);
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary = self.root.join(format!(".{CACHE_FILE_PREFIX}{counter}.tmp"));
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(temporary, path)?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), CacheError> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let path = entry?.path();
            if is_cache_file(&path) || is_temp_file(&path) {
                let _ = fs::remove_file(path);
            }
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<CacheKey>, CacheError> {
        let mut entries = Vec::new();
        let directory = match fs::read_dir(&self.root) {
            Ok(directory) => directory,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return Ok(entries),
            Err(error) => return Err(error.into()),
        };
        for entry in directory {
            let path = entry?.path();
            if !is_cache_file(&path) {
                continue;
            }
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => continue,
                Err(error) => return Err(error.into()),
            };
            match serde_json::from_slice::<CacheRecord>(&bytes) {
                Ok(record) if record.cache_version == CACHE_VERSION && record.document.validate().is_ok() => {
                    entries.push(record.key)
                }
                _ => {
                    let _ = fs::remove_file(path);
                }
            }
        }
        entries.sort_by(|left, right| left.executable_path.cmp(&right.executable_path));
        Ok(entries)
    }

    fn path_for(&self, key: &CacheKey) -> PathBuf {
        let bytes = serde_json::to_vec(key).expect("CacheKey serialization cannot fail");
        let digest = Sha256::digest(bytes);
        self.root.join(format!("{CACHE_FILE_PREFIX}{}.json", hex_digest(digest)))
    }
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_cache_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else { return false };
    let digest = name.strip_prefix(CACHE_FILE_PREFIX).and_then(|name| name.strip_suffix(".json"));
    digest.is_some_and(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn is_temp_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(counter) = name
        .strip_prefix(&format!(".{CACHE_FILE_PREFIX}"))
        .and_then(|name| name.strip_suffix(".tmp"))
    else {
        return false;
    };
    !counter.is_empty() && counter.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{CommandSpec, Confidence, ExecutableIdentity, HelpSource, SpecMetadata};
    use std::collections::BTreeMap;

    fn document() -> SchemaDocument {
        SchemaDocument::from_json(include_str!("../tests/fixtures/git-schema-v1.json")).unwrap_or_else(|_| SchemaDocument::new(CommandSpec {
            executable: ExecutableIdentity {
                name: "demo".into(),
                path: Some("/tmp/demo".into()),
            },
            name: "demo".into(),
            description: None,
            usage: vec![],
            subcommands: vec![],
            options: vec![],
            positionals: vec![],
            metadata: SpecMetadata {
                source: HelpSource::GenericHelp,
                parser: Some("test".into()),
                confidence: Confidence::Medium,
                executable_version: None,
                fingerprint: Some("fingerprint".into()),
                extensions: BTreeMap::new(),
            },
        }))
    }

    fn cache() -> (SchemaCache, PathBuf) {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("flagpick-cache-{}-{unique}", std::process::id()));
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
    fn executable_bytes_change_the_fingerprint() {
        let (cache, root) = cache();
        let executable = root.join("demo");
        fs::create_dir_all(&root).unwrap();
        fs::write(&executable, b"one").unwrap();
        let first = CacheKey::from_executable(&executable, vec!["demo".into()], "generic-v1")
            .unwrap();
        cache.put(&first, &document()).unwrap();
        fs::write(&executable, b"two").unwrap();
        let changed =
            CacheKey::from_executable(&executable, vec!["demo".into()], "generic-v1").unwrap();
        assert_ne!(first.executable_fingerprint, changed.executable_fingerprint);
        assert_eq!(cache.get(&changed).unwrap(), None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_record_versions_and_oversized_files_are_removed() {
        let (cache, root) = cache();
        let key = CacheKey::new("/tmp/demo", "one", vec!["demo".into()], "generic-v1");
        cache.put(&key, &document()).unwrap();
        let path = cache.path_for(&key);
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["cache_version"] = serde_json::json!(CACHE_VERSION + 1);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(cache.get(&key).unwrap(), None);
        assert!(!path.exists());
        fs::write(&path, vec![b'x'; MAX_CACHE_BYTES + 1]).unwrap();
        assert_eq!(cache.get(&key).unwrap(), None);
        assert!(!path.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_records_are_misses_and_do_not_persist_shell_text() {
        let (cache, root) = cache();
        let key = CacheKey::new("/tmp/demo", "one", vec!["demo".into()], "generic-v1");
        cache.put(&key, &document()).unwrap();
        let path = cache.path_for(&key);
        let bytes = fs::read(&path).unwrap();
        let shell_line = "--token SHELL_LINE_CANARY";
        assert!(!String::from_utf8_lossy(&bytes).contains(shell_line));
        fs::write(&path, b"not-json").unwrap();
        assert_eq!(cache.get(&key).unwrap(), None);
        assert!(cache.list().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
