use crate::cache::{CacheError, CacheKey, SchemaCache};
use crate::parser::{GenericHelpParser, ParseWarning};
use crate::probe::{ProbeConfig, ProbeError, ProbeRequest, ProbeRunner};
use crate::schema::SchemaDocument;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

const PARSER_VERSION: &str = "generic-gnu-help-v1";

#[derive(Debug)]
pub enum DiscoveryError {
    InvalidCommand,
    UnavailableCommand(String),
    Cache(CacheError),
    Probe(ProbeError),
    Parse(String),
    Validation(String),
    ProbeSkipped { command: String },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCommand => formatter.write_str("an executable command is required"),
            Self::UnavailableCommand(command) => {
                write!(formatter, "command is unavailable: {command}")
            }
            Self::Cache(error) => write!(formatter, "cache failure: {error}"),
            Self::Probe(error) => write!(formatter, "probe failed: {error}"),
            Self::Parse(error) => write!(formatter, "help output could not be parsed: {error}"),
            Self::Validation(error) => write!(formatter, "discovered schema is invalid: {error}"),
            Self::ProbeSkipped { command } => write!(
                formatter,
                "no cached schema for {command}; remove --no-exec-probe to discover it"
            ),
        }
    }
}

impl std::error::Error for DiscoveryError {}

impl From<CacheError> for DiscoveryError {
    fn from(error: CacheError) -> Self {
        Self::Cache(error)
    }
}

impl From<ProbeError> for DiscoveryError {
    fn from(error: ProbeError) -> Self {
        Self::Probe(error)
    }
}

#[derive(Debug)]
pub struct DiscoveryResult {
    pub document: SchemaDocument,
    pub warnings: Vec<ParseWarning>,
    pub executable: PathBuf,
    pub argv: Vec<String>,
    pub cache: CacheState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheState {
    Hit,
    Miss,
    Skipped,
    Unavailable,
}

impl CacheState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Miss => "miss",
            Self::Skipped => "skipped",
            Self::Unavailable => "unavailable",
        }
    }
}

pub fn discover(
    command: &[String],
    execute_probe: bool,
) -> Result<DiscoveryResult, DiscoveryError> {
    let command_name = command.first().ok_or(DiscoveryError::InvalidCommand)?;
    let executable = resolve_executable(command_name)
        .ok_or_else(|| DiscoveryError::UnavailableCommand(command_name.clone()))?;
    let argv = command[1..].to_vec();
    let key = CacheKey::from_executable(executable.clone(), argv.clone(), PARSER_VERSION)?;
    let cache = SchemaCache::default_location().map(SchemaCache::new);
    let cache_available = cache.is_some();

    if let Some(cache) = &cache {
        if let Some(document) = cache.get(&key)? {
            return Ok(DiscoveryResult {
                document,
                warnings: Vec::new(),
                executable,
                argv,
                cache: CacheState::Hit,
            });
        }
    }
    if !execute_probe {
        return Err(DiscoveryError::ProbeSkipped {
            command: command.join(" "),
        });
    }

    let mut probe_argv = command[1..].iter().map(OsString::from).collect::<Vec<_>>();
    if !probe_argv.iter().any(|argument| argument == "--help") {
        probe_argv.push(OsString::from("--help"));
    }
    let request = ProbeRequest::new(executable.clone(), probe_argv.clone())?;
    let runner = ProbeRunner::new(ProbeConfig {
        execution_enabled: true,
        ..ProbeConfig::default()
    })?;
    let output = runner.run(&request)?;
    let bytes = if output.stdout.bytes.is_empty() {
        &output.stderr.bytes
    } else {
        &output.stdout.bytes
    };
    let help = String::from_utf8_lossy(bytes);
    let mut report = GenericHelpParser::parse(command_name, &help)
        .map_err(|error| DiscoveryError::Parse(format!("{error:?}")))?;
    report.document.root.executable.path = Some(executable.to_string_lossy().into_owned());
    report
        .document
        .validate()
        .map_err(|error| DiscoveryError::Validation(error.to_string()))?;
    if let Some(cache) = cache {
        cache.put(&key, &report.document)?;
    }
    Ok(DiscoveryResult {
        document: report.document,
        warnings: report.warnings,
        executable,
        argv,
        cache: if cache_available {
            CacheState::Miss
        } else {
            CacheState::Unavailable
        },
    })
}

pub fn resolve_executable(command: &str) -> Option<PathBuf> {
    let candidate = Path::new(command);
    if candidate.is_absolute() {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }
    if command.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }
    std::env::var_os("PATH")?
        .split_paths()
        .find_map(|directory| {
            let path = directory.join(command);
            path.is_file().then_some(path)
        })
}
