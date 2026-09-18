use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

mod validation;

pub use validation::{ValidationError, ValidationErrors};

pub const SCHEMA_VERSION: u16 = 1;
pub const MAX_COMMAND_DEPTH: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaDocument {
    pub schema_version: u16,
    pub root: CommandSpec,
}

impl SchemaDocument {
    pub fn new(root: CommandSpec) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            root,
        }
    }

    pub fn to_canonical_json(&self) -> Result<String, JsonError> {
        let mut json = serde_json::to_string_pretty(self).map_err(JsonError)?;
        json.push('\n');
        Ok(json)
    }

    pub fn from_json(json: &str) -> Result<Self, JsonError> {
        serde_json::from_str(json).map_err(JsonError)
    }

    pub fn validate(&self) -> Result<(), ValidationErrors> {
        validation::validate_document(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    pub executable: ExecutableIdentity,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub usage: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subcommands: Vec<SubcommandSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<OptionSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub positionals: Vec<PositionalSpec>,
    pub metadata: SpecMetadata,
}

impl CommandSpec {
    pub fn validate(&self) -> Result<(), ValidationErrors> {
        validation::validate_command(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableIdentity {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubcommandSpec {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    pub command: Box<CommandSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionSpec {
    pub id: OptionId,
    pub names: Vec<OptionName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub value: ValueSpec,
    pub repeatability: Repeatability,
    pub required: bool,
    pub scope: Scope,
    pub hidden: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negates: Option<OptionId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts_with: Vec<OptionId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<OptionId>,
    pub insertion_policy: InsertionPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OptionId(String);

impl OptionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum OptionName {
    Short(char),
    Long(String),
    SingleDashLong(String),
}

impl OptionName {
    pub fn spelling(&self) -> String {
        match self {
            Self::Short(value) => format!("-{value}"),
            Self::Long(value) => format!("--{value}"),
            Self::SingleDashLong(value) => format!("-{value}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueSpec {
    pub arity: ValueArity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub value_type: ValueType,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub possible_values: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionalSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub arity: ValueArity,
    pub value_type: ValueType,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub possible_values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueArity {
    None,
    Required,
    Optional,
    Multiple,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    Bool,
    String,
    Integer,
    Float,
    Path,
    Enum,
    KeyValue,
    Url,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeatability {
    Single,
    Repeatable,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Local,
    Global,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertionPolicy {
    Append,
    BeforePositionals,
    BeforeTerminator,
    AtCursor,
    CommandSpecific,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelpSource {
    BuiltIn,
    MachineReadable,
    Completion,
    FrameworkHelp,
    GenericHelp,
    ManPage,
    Fallback,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecMetadata {
    pub source: HelpSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser: Option<String>,
    pub confidence: Confidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug)]
pub struct JsonError(serde_json::Error);

impl fmt::Display for JsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for JsonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}
