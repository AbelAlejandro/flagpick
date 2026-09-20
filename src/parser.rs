use std::collections::{BTreeMap, HashMap};

use crate::schema::{
    CommandSpec, Confidence, ExecutableIdentity, HelpSource, InsertionPolicy, OptionId,
    OptionName, OptionSpec, PositionalSpec, Repeatability, SchemaDocument, Scope, SpecMetadata,
    ValueArity, ValueSpec, ValueType,
};

pub const MAX_HELP_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenericHelpError {
    EmptyExecutable,
    InputTooLarge { bytes: usize, maximum: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseWarning {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ParseReport {
    pub document: SchemaDocument,
    pub warnings: Vec<ParseWarning>,
}

pub struct GenericHelpParser;

impl GenericHelpParser {
    pub fn parse(
        executable: impl Into<String>,
        help: &str,
    ) -> Result<ParseReport, GenericHelpError> {
        let executable = executable.into();
        if executable.trim().is_empty() {
            return Err(GenericHelpError::EmptyExecutable);
        }
        if help.len() > MAX_HELP_BYTES {
            return Err(GenericHelpError::InputTooLarge {
                bytes: help.len(),
                maximum: MAX_HELP_BYTES,
            });
        }

        let mut warnings = Vec::new();
        let mut options = Vec::new();
        let mut names = HashMap::<String, OptionId>::new();
        let mut in_section = false;
        let mut last_option = None;
        let mut usage = Vec::new();
        let mut positionals = Vec::new();

        for (index, line) in help.lines().enumerate() {
            let line_number = index + 1;
            let trimmed = line.trim();
            let lower = trimmed.to_ascii_lowercase();

            if is_section_heading(trimmed) {
                in_section = true;
                continue;
            }
            if in_section && is_heading(trimmed) {
                in_section = false;
            }
            if is_usage_line(trimmed) {
                usage.push(trimmed.to_owned());
                positionals.extend(parse_usage_positionals(trimmed));
            }

            if let Some(marker) = lower.find("possible values:") {
                if let Some(index) = last_option {
                    if let Some(spec) = options.get_mut(index) {
                        spec.value.value_type = ValueType::Enum;
                        let values = trimmed[marker + "possible values:".len()..].trim();
                        spec.value.possible_values = parse_possible_values(values);
                    }
                } else {
                    warnings.push(ParseWarning {
                        line: line_number,
                        message: "possible-values marker has no preceding option".into(),
                    });
                }
                continue;
            }

            let eligible = in_section || (options.is_empty() && trimmed.starts_with('-'));
            if !eligible || !trimmed.starts_with('-') {
                if in_section && !trimmed.is_empty() {
                    warnings.push(ParseWarning {
                        line: line_number,
                        message: "non-option line in options section skipped".into(),
                    });
                }
                continue;
            }

            let Some((tokens, description)) = split_option_line(trimmed) else {
                warnings.push(ParseWarning {
                    line: line_number,
                    message: "ambiguous option line skipped".into(),
                });
                continue;
            };
            let parsed: Vec<_> = tokens.iter().filter_map(|token| parse_name(token)).collect();
            if parsed.is_empty() {
                warnings.push(ParseWarning {
                    line: line_number,
                    message: "unsupported option spelling skipped".into(),
                });
                continue;
            }
            if parsed
                .iter()
                .any(|(_, spelling)| names.contains_key(spelling))
            {
                warnings.push(ParseWarning {
                    line: line_number,
                    message: "duplicate option spelling skipped".into(),
                });
                continue;
            }

            let arity = tokens
                .iter()
                .find_map(|token| value_arity(token))
                .unwrap_or(ValueArity::None);
            let repeatability = if description
                .as_deref()
                .map(|text| {
                    let text = text.to_ascii_lowercase();
                    !["not", "no", "never"]
                        .iter()
                        .any(|word| text.split_whitespace().any(|part| part == *word))
                        && (text.contains("repeatable") || text.contains("multiple times"))
                })
                .unwrap_or(false)
            {
                Repeatability::Repeatable
            } else {
                Repeatability::Unknown
            };
            let canonical = parsed
                .first()
                .map(|(_, spelling)| spelling.clone())
                .unwrap_or_default();
            let id = OptionId::new(canonical);
            let option = OptionSpec {
                id: id.clone(),
                names: parsed.into_iter().map(|(name, _)| name).collect(),
                description,
                value: ValueSpec {
                    arity,
                    name: value_name(&tokens),
                    value_type: ValueType::Unknown,
                    possible_values: Vec::new(),
                },
                repeatability,
                required: false,
                scope: Scope::Unknown,
                hidden: false,
                negates: None,
                conflicts_with: Vec::new(),
                requires: Vec::new(),
                insertion_policy: InsertionPolicy::Append,
                source_span: None,
            };
            for name in &option.names {
                names.insert(name.spelling(), id.clone());
            }
            last_option = Some(options.len());
            options.push(option);
        }

        let mut negations = HashMap::new();
        for option in &options {
            for name in &option.names {
                if let OptionName::Long(value) = name {
                    if let Some(base) = value.strip_prefix("no-") {
                        negations.insert(option.id.clone(), format!("--{base}"));
                    }
                }
            }
        }
        for option in &mut options {
            if let Some(base) = negations.get(&option.id) {
                option.negates = names.get(base).cloned();
            }
        }

        let document = SchemaDocument {
            schema_version: crate::schema::SCHEMA_VERSION,
            root: CommandSpec {
                executable: ExecutableIdentity {
                    name: executable.clone(),
                    path: None,
                },
                name: executable,
                description: None,
                usage,
                subcommands: Vec::new(),
                options,
                positionals,
                metadata: SpecMetadata {
                    source: HelpSource::GenericHelp,
                    parser: Some("generic-gnu-help".into()),
                    confidence: Confidence::Medium,
                    executable_version: None,
                    fingerprint: None,
                    extensions: BTreeMap::new(),
                },
            },
        };
        Ok(ParseReport { document, warnings })
    }
}

fn is_section_heading(line: &str) -> bool {
    matches!(
        line.trim_end_matches(':').to_ascii_lowercase().as_str(),
        "options" | "flags"
    )
}

fn is_heading(line: &str) -> bool {
    !line.is_empty()
        && !line.starts_with('-')
        && line.ends_with(':')
        && line.len() < 80
}

fn is_usage_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("usage:") || lower.starts_with("synopsis:")
}

fn split_option_line(line: &str) -> Option<(Vec<String>, Option<String>)> {
    let (specification, description) = line.split_once("  ").unwrap_or((line, ""));
    if !specification.trim_start().starts_with('-') {
        return None;
    }
    Some((
        specification
            .split_whitespace()
            .flat_map(|token| token.split(','))
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
            .collect(),
        nonempty(description),
    ))
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn parse_name(token: &str) -> Option<(OptionName, String)> {
    let token = token.trim_matches(',');
    if let Some(value) = token.strip_prefix("--") {
        let (name, _) = value.split_once(['=', '[', '<']).unwrap_or((value, ""));
        return (!name.is_empty()).then(|| {
            (OptionName::Long(name.to_owned()), format!("--{name}"))
        });
    }
    let value = token.strip_prefix('-')?;
    if value.is_empty() {
        return None;
    }
    if value.chars().count() == 1 {
        let character = value.chars().next()?;
        return Some((OptionName::Short(character), format!("-{character}")));
    }
    let name = value.split_once(['=', '[', '<']).map_or(value, |(name, _)| name);
    Some((
        OptionName::SingleDashLong(name.to_owned()),
        format!("-{name}"),
    ))
}

fn value_arity(token: &str) -> Option<ValueArity> {
    if token.contains("[=") || token.contains("[<") {
        Some(ValueArity::Optional)
    } else if token.contains('=') || token.contains('<') {
        Some(ValueArity::Required)
    } else if token.ends_with("...") {
        Some(ValueArity::Multiple)
    } else {
        None
    }
}

fn value_name(tokens: &[String]) -> Option<String> {
    tokens.iter().find_map(|token| {
        let value = if token.starts_with(['<', '[']) {
            token
        } else {
            token.split_once(['=', '[', '<'])?.1
        };
        let value = value.trim_end_matches(']').trim_end_matches('>');
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn parse_usage_positionals(line: &str) -> Vec<PositionalSpec> {
    let Some((_, usage)) = line.split_once(':') else {
        return Vec::new();
    };
    usage
        .split_whitespace()
        .filter_map(|token| {
            let optional = token.starts_with('[');
            let token = token.trim_matches(['[', ']']);
            if token.is_empty() || token.starts_with('-') || token.eq_ignore_ascii_case("options") {
                return None;
            }
            let name = token.trim_matches(['<', '>']).to_owned();
            if name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
            {
                Some(PositionalSpec {
                    name,
                    description: None,
                    arity: ValueArity::Unknown,
                    value_type: ValueType::Unknown,
                    required: !optional,
                    possible_values: Vec::new(),
                    source_span: None,
                })
            } else {
                None
            }
        })
        .collect()
}

fn parse_possible_values(values: &str) -> Vec<String> {
    values
        .split([',', '|'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_gnu_options_without_prose_options() {
        let input = include_str!("../tests/fixtures/generic-help.txt");
        let report = GenericHelpParser::parse("demo", input).unwrap();
        report.document.validate().unwrap();
        assert_eq!(report.document.root.options.len(), 7);
        assert_eq!(report.document.root.options[1].value.arity, ValueArity::Required);
        assert_eq!(report.document.root.options[2].value.arity, ValueArity::Optional);
        assert_eq!(report.document.root.options[2].value.value_type, ValueType::Enum);
        assert_eq!(
            report.document.root.options[2].value.possible_values,
            vec!["auto", "always", "never"]
        );
        assert_eq!(report.document.root.positionals.len(), 1);
        assert!(report.document.root.options.iter().any(|option| {
            option.names.iter().any(|name| {
                matches!(name, OptionName::SingleDashLong(value) if value == "version")
            })
        }));
        assert!(!report.document.root.options.iter().any(|option| {
            option.names.iter().any(|name| name.spelling() == "--fabricated")
        }));
        assert!(!report.warnings.is_empty());
    }

    #[test]
    fn rejects_empty_and_unbounded_input() {
        assert_eq!(
            GenericHelpParser::parse("", "").unwrap_err(),
            GenericHelpError::EmptyExecutable
        );
        assert!(matches!(
            GenericHelpParser::parse("demo", &"x".repeat(MAX_HELP_BYTES + 1)),
            Err(GenericHelpError::InputTooLarge { .. })
        ));
    }
}
