use crate::adapters::{self, ProbePlan};
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
    InvalidInvocation(String),
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
            Self::InvalidInvocation(message) => formatter.write_str(message),
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
    pub probe: Option<ProbeDiagnostics>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ProbeDiagnostics {
    pub source: &'static str,
    pub duration_ms: u128,
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub stdout_bytes: u64,
    pub stdout_truncated: bool,
    pub stderr_bytes: u64,
    pub stderr_truncated: bool,
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
    if command.iter().any(|token| {
        token.is_empty()
            || token.trim() != token
            || token.chars().any(char::is_control)
            || token.chars().any(|character| {
                matches!(character, '\'' | '"' | '`' | ';' | '|' | '&' | '<' | '>')
            })
    }) {
        return Err(DiscoveryError::InvalidInvocation(
            "command and subcommand names must be nonempty safe tokens".into(),
        ));
    }
    let plan = adapters::plan(command).or_else(|| {
        (command.len() == 1).then(|| ProbePlan {
            argv: vec!["--help".into()],
            parser: PARSER_VERSION,
            source: crate::schema::HelpSource::GenericHelp,
            confidence: crate::schema::Confidence::Medium,
            command_path: command.to_vec(),
        })
    });
    let Some(plan) = plan else {
        return Err(DiscoveryError::InvalidInvocation(
            "unsupported command path; only registered adapters accept subcommands".into(),
        ));
    };
    let executable = resolve_executable(command_name)
        .ok_or_else(|| DiscoveryError::UnavailableCommand(command_name.clone()))?;
    let key = CacheKey::from_executable(executable.clone(), plan.argv.clone(), plan.parser)?;
    let cache = SchemaCache::default_location().map(SchemaCache::new);
    let cache_available = cache.is_some();

    if let Some(cache) = &cache {
        if let Some(document) = cache.get(&key)? {
            return Ok(DiscoveryResult {
                document,
                warnings: Vec::new(),
                executable,
                argv: plan.argv.clone(),
                cache: CacheState::Hit,
                probe: None,
            });
        }
    }
    if !execute_probe {
        return Err(DiscoveryError::ProbeSkipped {
            command: command.join(" "),
        });
    }

    let probe_argv: Vec<OsString> = plan.argv.iter().cloned().map(OsString::from).collect();
    let request = ProbeRequest::new(executable.clone(), probe_argv.clone())?;
    let runner = ProbeRunner::new(ProbeConfig {
        execution_enabled: true,
        ..ProbeConfig::default()
    })?;
    let output = runner.run(&request)?;
    if output.timed_out {
        return Err(DiscoveryError::InvalidInvocation(
            "probe timed out; no schema was cached".into(),
        ));
    }
    if output.stdout.truncated || output.stderr.truncated {
        return Err(DiscoveryError::InvalidInvocation(
            "probe output exceeded its safety bound; no schema was cached".into(),
        ));
    }
    if plan.parser != PARSER_VERSION && output.status.code != Some(0) {
        return Err(DiscoveryError::InvalidInvocation(format!(
            "{} adapter probe exited with status {:?}; no schema was cached",
            plan.parser, output.status.code
        )));
    }
    let bytes = if output.stdout.bytes.is_empty() {
        &output.stderr.bytes
    } else {
        &output.stdout.bytes
    };
    let help = sanitize_help(bytes);
    let mut report = GenericHelpParser::parse(
        plan.command_path
            .last()
            .map(String::as_str)
            .unwrap_or(command_name),
        &help,
    )
    .map_err(|error| DiscoveryError::Parse(format!("{error:?}")))?;
    report.document.root.executable.path = Some(executable.to_string_lossy().into_owned());
    report.document.root.metadata.parser = Some(plan.parser.into());
    report.document.root.metadata.source = plan.source;
    report.document.root.metadata.confidence = plan.confidence;
    if plan.command_path.len() > 1 {
        let child = report.document.root.clone();
        report.document.root.name = plan.command_path[0].clone();
        report.document.root.executable.name = plan.command_path[0].clone();
        report.document.root.options.clear();
        report.document.root.positionals.clear();
        report.document.root.usage.clear();
        report.document.root.subcommands = vec![crate::schema::SubcommandSpec {
            name: plan.command_path[1].clone(),
            aliases: Vec::new(),
            command: Box::new(child),
        }];
    }
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
        argv: probe_argv
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect(),
        cache: if cache_available {
            CacheState::Miss
        } else {
            CacheState::Unavailable
        },
        probe: Some(ProbeDiagnostics {
            source: "direct",
            duration_ms: output.duration.as_millis(),
            code: output.status.code,
            signal: output.status.signal,
            stdout_bytes: output.stdout.total_bytes,
            stdout_truncated: output.stdout.truncated,
            stderr_bytes: output.stderr.total_bytes,
            stderr_truncated: output.stderr.truncated,
        }),
    })
}

fn sanitize_help(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\r' | '\t'))
        .collect()
}

pub fn resolve_executable(command: &str) -> Option<PathBuf> {
    let candidate = Path::new(command);
    if candidate.is_absolute() || command.contains(std::path::MAIN_SEPARATOR) {
        return std::fs::canonicalize(candidate)
            .ok()
            .filter(|path| path.is_file());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|directory| {
        let path = directory.join(command);
        path.is_file().then_some(path)
    })
}

pub fn executable_from_buffer(buffer: &str) -> Result<String, DiscoveryError> {
    let token = buffer
        .split_whitespace()
        .next()
        .ok_or(DiscoveryError::InvalidCommand)?;
    if token.contains(|character: char| {
        character.is_control()
            || matches!(character, '\'' | '"' | '`' | ';' | '|' | '&' | '<' | '>')
    }) {
        return Err(DiscoveryError::InvalidInvocation(
            "shell quoting and operators are not accepted for discovery".into(),
        ));
    }
    Ok(token.to_owned())
}
