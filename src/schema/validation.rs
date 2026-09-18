use super::{
    CommandSpec, MAX_COMMAND_DEPTH, OptionId, OptionName, PositionalSpec, SCHEMA_VERSION,
    SchemaDocument, SourceSpan, SpecMetadata, ValueArity, ValueSpec, ValueType,
};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    pub path: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationErrors {
    pub errors: Vec<ValidationError>,
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index != 0 {
                writeln!(formatter)?;
            }
            write!(
                formatter,
                "{}: {} ({})",
                error.path, error.message, error.code
            )?;
        }
        Ok(())
    }
}

impl Error for ValidationErrors {}

pub(super) fn validate_document(document: &SchemaDocument) -> Result<(), ValidationErrors> {
    let mut validator = Validator::default();
    if document.schema_version != SCHEMA_VERSION {
        validator.error(
            "$.schema_version",
            "unsupported_schema_version",
            format!(
                "schema version {} is unsupported; expected {SCHEMA_VERSION}",
                document.schema_version
            ),
        );
    }
    validator.command(&document.root, "$.root", 0);
    validator.finish()
}

pub(super) fn validate_command(command: &CommandSpec) -> Result<(), ValidationErrors> {
    let mut validator = Validator::default();
    validator.command(command, "$.root", 0);
    validator.finish()
}

#[derive(Default)]
struct Validator {
    errors: Vec<ValidationError>,
}

impl Validator {
    fn finish(self) -> Result<(), ValidationErrors> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationErrors {
                errors: self.errors,
            })
        }
    }

    fn error(&mut self, path: impl Into<String>, code: &'static str, message: impl Into<String>) {
        self.errors.push(ValidationError {
            path: path.into(),
            code,
            message: message.into(),
        });
    }

    fn command(&mut self, command: &CommandSpec, path: &str, depth: usize) {
        self.token(
            &command.name,
            &format!("{path}.name"),
            "invalid_command_name",
        );
        self.token(
            &command.executable.name,
            &format!("{path}.executable.name"),
            "invalid_executable_name",
        );
        if let Some(executable_path) = &command.executable.path {
            self.trimmed(
                executable_path,
                &format!("{path}.executable.path"),
                "invalid_executable_path",
            );
        }
        for (index, usage) in command.usage.iter().enumerate() {
            self.trimmed(usage, &format!("{path}.usage[{index}]"), "invalid_usage");
        }
        self.metadata(&command.metadata, &format!("{path}.metadata"));

        let mut option_ids = BTreeSet::new();
        let mut spellings = BTreeSet::new();
        for (index, option) in command.options.iter().enumerate() {
            let option_path = format!("{path}.options[{index}]");
            let id = option.id.as_str();
            if !valid_option_id(id) {
                self.error(
                    format!("{option_path}.id"),
                    "invalid_option_id",
                    "option ID must be nonempty ASCII [A-Za-z0-9._-]+",
                );
            }
            if !option_ids.insert(id.to_owned()) {
                self.error(
                    format!("{option_path}.id"),
                    "duplicate_option_id",
                    format!("option ID `{id}` is already used in this command"),
                );
            }
            if option.names.is_empty() {
                self.error(
                    format!("{option_path}.names"),
                    "missing_option_name",
                    "option records must define at least one name",
                );
            }
            for (name_index, name) in option.names.iter().enumerate() {
                let name_path = format!("{option_path}.names[{name_index}]");
                if !valid_option_name(name) {
                    self.error(
                        &name_path,
                        "invalid_option_name",
                        "option name has an invalid spelling",
                    );
                }
                let spelling = name.spelling();
                if !spellings.insert(spelling.clone()) {
                    self.error(
                        name_path,
                        "duplicate_option_spelling",
                        format!("option spelling `{spelling}` is already used in this command"),
                    );
                }
            }
            self.value(&option.value, &format!("{option_path}.value"), true);
            if let Some(span) = &option.source_span {
                self.span(span, &format!("{option_path}.source_span"));
            }
        }

        for (index, option) in command.options.iter().enumerate() {
            let option_path = format!("{path}.options[{index}]");
            if let Some(reference) = &option.negates {
                self.reference(
                    option.id.as_str(),
                    reference,
                    &option_ids,
                    &format!("{option_path}.negates"),
                    &mut BTreeSet::new(),
                );
            }
            self.references(
                option.id.as_str(),
                &option.conflicts_with,
                &option_ids,
                &format!("{option_path}.conflicts_with"),
            );
            self.references(
                option.id.as_str(),
                &option.requires,
                &option_ids,
                &format!("{option_path}.requires"),
            );
        }

        for (index, positional) in command.positionals.iter().enumerate() {
            self.positional(positional, &format!("{path}.positionals[{index}]"));
        }

        let mut subcommand_tokens = BTreeSet::new();
        for (index, subcommand) in command.subcommands.iter().enumerate() {
            let subcommand_path = format!("{path}.subcommands[{index}]");
            self.token(
                &subcommand.name,
                &format!("{subcommand_path}.name"),
                "invalid_subcommand_name",
            );
            self.unique_subcommand_token(
                &subcommand.name,
                &mut subcommand_tokens,
                &format!("{subcommand_path}.name"),
            );
            for (alias_index, alias) in subcommand.aliases.iter().enumerate() {
                let alias_path = format!("{subcommand_path}.aliases[{alias_index}]");
                self.token(alias, &alias_path, "invalid_subcommand_alias");
                self.unique_subcommand_token(alias, &mut subcommand_tokens, &alias_path);
            }
            let child_depth = depth + 1;
            let child_path = format!("{subcommand_path}.command");
            if child_depth > MAX_COMMAND_DEPTH {
                self.error(
                    child_path,
                    "max_depth_exceeded",
                    format!("command depth must not exceed {MAX_COMMAND_DEPTH}"),
                );
            } else {
                self.command(&subcommand.command, &child_path, child_depth);
            }
        }
    }

    fn unique_subcommand_token(&mut self, token: &str, tokens: &mut BTreeSet<String>, path: &str) {
        if !tokens.insert(token.to_owned()) {
            self.error(
                path,
                "duplicate_subcommand_token",
                format!("subcommand token `{token}` is already used in this command"),
            );
        }
    }

    fn references(
        &mut self,
        owner: &str,
        references: &[OptionId],
        known: &BTreeSet<String>,
        path: &str,
    ) {
        let mut seen = BTreeSet::new();
        for (index, reference) in references.iter().enumerate() {
            self.reference(
                owner,
                reference,
                known,
                &format!("{path}[{index}]"),
                &mut seen,
            );
        }
    }

    fn reference(
        &mut self,
        owner: &str,
        reference: &OptionId,
        known: &BTreeSet<String>,
        path: &str,
        seen: &mut BTreeSet<String>,
    ) {
        let value = reference.as_str();
        if value == owner {
            self.error(
                path,
                "self_option_reference",
                "option references must not refer to themselves",
            );
        }
        if !seen.insert(value.to_owned()) {
            self.error(
                path,
                "duplicate_option_reference",
                format!("option ID `{value}` is listed more than once"),
            );
        }
        if !known.contains(value) {
            self.error(
                path,
                "unknown_option_reference",
                format!("option ID `{value}` does not exist in this command"),
            );
        }
    }

    fn positional(&mut self, positional: &PositionalSpec, path: &str) {
        self.trimmed(
            &positional.name,
            &format!("{path}.name"),
            "invalid_positional_name",
        );
        if positional.arity == ValueArity::None {
            self.error(
                format!("{path}.arity"),
                "invalid_positional_arity",
                "positionals may not use none arity",
            );
        }
        let value = ValueSpec {
            arity: positional.arity,
            name: None,
            value_type: positional.value_type,
            possible_values: positional.possible_values.clone(),
        };
        self.value(&value, path, false);
        if let Some(span) = &positional.source_span {
            self.span(span, &format!("{path}.source_span"));
        }
    }

    fn value(&mut self, value: &ValueSpec, path: &str, allow_none: bool) {
        if value.arity == ValueArity::None {
            if !allow_none {
                self.error(
                    format!("{path}.arity"),
                    "invalid_value_arity",
                    "this value may not use none arity",
                );
            }
            if !matches!(value.value_type, ValueType::Bool | ValueType::Unknown) {
                self.error(
                    format!("{path}.value_type"),
                    "contradictory_value_metadata",
                    "none arity permits only bool or unknown value types",
                );
            }
            if value.name.is_some() {
                self.error(
                    format!("{path}.name"),
                    "contradictory_value_metadata",
                    "none arity may not define a value name",
                );
            }
            if !value.possible_values.is_empty() {
                self.error(
                    format!("{path}.possible_values"),
                    "contradictory_value_metadata",
                    "none arity may not define possible values",
                );
            }
        }
        if value.value_type == ValueType::Bool
            && !matches!(value.arity, ValueArity::None | ValueArity::Unknown)
        {
            self.error(
                format!("{path}.arity"),
                "contradictory_value_metadata",
                "bool values permit only none or unknown arity",
            );
        }
        if let Some(name) = &value.name {
            self.trimmed(name, &format!("{path}.name"), "invalid_value_name");
        }
        if value.value_type == ValueType::Enum {
            if value.possible_values.is_empty() {
                self.error(
                    format!("{path}.possible_values"),
                    "missing_enum_values",
                    "enum values require at least one possible value",
                );
            }
            let mut seen = BTreeSet::new();
            for (index, possible) in value.possible_values.iter().enumerate() {
                self.trimmed(
                    possible,
                    &format!("{path}.possible_values[{index}]"),
                    "invalid_possible_value",
                );
                if !seen.insert(possible) {
                    self.error(
                        format!("{path}.possible_values[{index}]"),
                        "duplicate_enum_value",
                        format!("possible value `{possible}` is repeated"),
                    );
                }
            }
        } else if value.value_type != ValueType::Unknown && !value.possible_values.is_empty() {
            self.error(
                format!("{path}.possible_values"),
                "contradictory_value_metadata",
                "only enum or unknown value types may define possible values",
            );
        }
    }

    fn metadata(&mut self, metadata: &SpecMetadata, path: &str) {
        for (field, value) in [
            ("parser", metadata.parser.as_deref()),
            ("executable_version", metadata.executable_version.as_deref()),
            ("fingerprint", metadata.fingerprint.as_deref()),
        ] {
            if let Some(value) = value {
                self.trimmed(value, &format!("{path}.{field}"), "invalid_metadata");
            }
        }
    }

    fn span(&mut self, span: &SourceSpan, path: &str) {
        if span.start >= span.end {
            self.error(
                path,
                "invalid_source_span",
                "source span start must be less than end",
            );
        }
    }

    fn trimmed(&mut self, value: &str, path: &str, code: &'static str) {
        if value.is_empty() || value.trim() != value {
            self.error(path, code, "value must be nonempty and trimmed");
        }
    }

    fn token(&mut self, value: &str, path: &str, code: &'static str) {
        if value.is_empty()
            || value.trim() != value
            || value
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            self.error(
                path,
                code,
                "token must be nonempty, trimmed, and contain no whitespace or controls",
            );
        }
    }
}

fn valid_option_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_option_name(name: &OptionName) -> bool {
    match name {
        OptionName::Short(value) => value.is_ascii_graphic() && *value != '-',
        OptionName::Long(value) => valid_long_name(value),
        OptionName::SingleDashLong(value) => value.len() >= 2 && valid_long_name(value),
    }
}

fn valid_long_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value.chars().all(|character| character.is_ascii_graphic())
}
