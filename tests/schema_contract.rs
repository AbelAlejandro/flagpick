use flagpick::schema::{
    CommandSpec, Confidence, ExecutableIdentity, HelpSource, InsertionPolicy, OptionId, OptionName,
    OptionSpec, PositionalSpec, Repeatability, SchemaDocument, Scope, SourceSpan, SpecMetadata,
    SubcommandSpec, ValueArity, ValueSpec, ValueType,
};
use serde_json::json;
use std::collections::BTreeMap;

fn metadata(confidence: Confidence) -> SpecMetadata {
    SpecMetadata {
        source: HelpSource::BuiltIn,
        parser: None,
        confidence,
        executable_version: None,
        fingerprint: None,
        extensions: BTreeMap::new(),
    }
}

fn flag_value() -> ValueSpec {
    ValueSpec {
        arity: ValueArity::None,
        name: None,
        value_type: ValueType::Bool,
        possible_values: vec![],
    }
}

fn option(id: &str, names: Vec<OptionName>, value: ValueSpec) -> OptionSpec {
    OptionSpec {
        id: OptionId::new(id),
        names,
        description: None,
        value,
        repeatability: Repeatability::Single,
        required: false,
        scope: Scope::Local,
        hidden: false,
        negates: None,
        conflicts_with: vec![],
        requires: vec![],
        insertion_policy: InsertionPolicy::Append,
        source_span: None,
    }
}

fn empty_command(name: &str) -> CommandSpec {
    CommandSpec {
        executable: ExecutableIdentity {
            name: "git".to_owned(),
            path: None,
        },
        name: name.to_owned(),
        description: None,
        usage: vec![],
        subcommands: vec![],
        options: vec![],
        positionals: vec![],
        metadata: metadata(Confidence::Unknown),
    }
}

fn git_fixture() -> SchemaDocument {
    let mut verbose = option(
        "verbose",
        vec![OptionName::Long("verbose".into()), OptionName::Short('v')],
        flag_value(),
    );
    verbose.description = Some("Increase verbosity".into());
    verbose.repeatability = Repeatability::Repeatable;
    verbose.scope = Scope::Global;
    verbose.insertion_policy = InsertionPolicy::BeforePositionals;
    verbose.source_span = Some(SourceSpan { start: 10, end: 22 });

    let mut color = option(
        "color",
        vec![OptionName::Long("color".into())],
        ValueSpec {
            arity: ValueArity::Required,
            name: Some("WHEN".into()),
            value_type: ValueType::Enum,
            possible_values: vec!["always".into(), "auto".into(), "never".into()],
        },
    );
    color.required = true;

    let mut no_color = option(
        "no-color",
        vec![OptionName::Long("no-color".into())],
        flag_value(),
    );
    no_color.negates = Some(OptionId::new("color"));

    let mut version = option(
        "version",
        vec![OptionName::SingleDashLong("version".into())],
        ValueSpec {
            arity: ValueArity::Unknown,
            name: None,
            value_type: ValueType::Unknown,
            possible_values: vec![],
        },
    );
    version.repeatability = Repeatability::Unknown;
    version.scope = Scope::Unknown;
    version.insertion_policy = InsertionPolicy::Unknown;

    let mut message = option(
        "message",
        vec![OptionName::Short('m'), OptionName::Long("message".into())],
        ValueSpec {
            arity: ValueArity::Optional,
            name: Some("MESSAGE".into()),
            value_type: ValueType::String,
            possible_values: vec![],
        },
    );
    message.description = Some("Use the given commit message".into());

    let mut commit = empty_command("commit");
    commit.description = Some("Record changes".into());
    commit.options.push(message);
    commit.positionals.push(PositionalSpec {
        name: "path".into(),
        description: Some("Paths to record".into()),
        arity: ValueArity::Multiple,
        value_type: ValueType::Path,
        required: false,
        possible_values: vec![],
        source_span: None,
    });

    let mut extensions = BTreeMap::new();
    extensions.insert("z_last".into(), json!("preserved"));
    extensions.insert("a_first".into(), json!({ "future": true }));
    let root = CommandSpec {
        executable: ExecutableIdentity {
            name: "git".into(),
            path: Some("/usr/bin/git".into()),
        },
        name: "git".into(),
        description: Some("Distributed version control".into()),
        usage: vec!["git [OPTIONS] <COMMAND>".into()],
        subcommands: vec![SubcommandSpec {
            name: "commit".into(),
            aliases: vec!["ci".into()],
            command: Box::new(commit),
        }],
        options: vec![verbose, color, no_color, version],
        positionals: vec![],
        metadata: SpecMetadata {
            source: HelpSource::BuiltIn,
            parser: Some("fixture".into()),
            confidence: Confidence::High,
            executable_version: Some("2.0".into()),
            fingerprint: Some("fixture-v1".into()),
            extensions,
        },
    };
    SchemaDocument::new(root)
}

#[test]
fn golden_schema_is_valid_stable_and_lossless() {
    let document = git_fixture();
    document.validate().expect("fixture validates");
    let fixture = include_str!("fixtures/git-schema-v1.json");
    let canonical = document.to_canonical_json().expect("serialize fixture");
    assert_eq!(canonical, fixture);
    assert_eq!(
        SchemaDocument::from_json(fixture).expect("parse fixture"),
        document
    );
    assert!(canonical.find("\"a_first\"").unwrap() < canonical.find("\"z_last\"").unwrap());
}

#[test]
fn future_enum_values_are_rejected_instead_of_silently_lost() {
    let json = git_fixture().to_canonical_json().unwrap().replacen(
        "\"confidence\": \"high\"",
        "\"confidence\": \"future\"",
        1,
    );
    assert!(
        SchemaDocument::from_json(&json).is_err(),
        "unrecognized enum values must not be rewritten as explicit unknown"
    );
}

#[test]
fn validation_errors_are_path_aware_and_deterministic() {
    let mut document = git_fixture();
    document.schema_version = 2;
    document.root.name = " bad ".into();
    document.root.options[0].names.clear();
    document.root.options[1].names = vec![OptionName::Long("no-color".into())];
    document.root.options[2].value.name = Some("VALUE".into());
    document.root.options[3].requires = vec![OptionId::new("missing")];
    document.root.options[3].source_span = Some(SourceSpan { start: 4, end: 4 });

    let first = document.validate().expect_err("invalid document rejected");
    let second = document.validate().expect_err("validation is repeatable");
    assert_eq!(first, second);
    let pairs = first
        .errors
        .iter()
        .map(|error| (error.path.as_str(), error.code))
        .collect::<Vec<_>>();
    for expected in [
        ("$.schema_version", "unsupported_schema_version"),
        ("$.root.name", "invalid_command_name"),
        ("$.root.options[0].names", "missing_option_name"),
        ("$.root.options[2].names[0]", "duplicate_option_spelling"),
        (
            "$.root.options[2].value.name",
            "contradictory_value_metadata",
        ),
        ("$.root.options[3].requires[0]", "unknown_option_reference"),
        ("$.root.options[3].source_span", "invalid_source_span"),
    ] {
        assert!(
            pairs.contains(&expected),
            "missing {expected:?} in {pairs:?}"
        );
    }
}

fn nested_command(depth: usize) -> CommandSpec {
    let mut command = empty_command("leaf");
    for index in 0..depth {
        command = CommandSpec {
            subcommands: vec![SubcommandSpec {
                name: format!("level-{index}"),
                aliases: vec![],
                command: Box::new(command),
            }],
            ..empty_command("parent")
        };
    }
    command
}

#[test]
fn recursive_validation_accepts_depth_32_and_rejects_33() {
    SchemaDocument::new(nested_command(32))
        .validate()
        .expect("depth 32 is valid");
    let errors = SchemaDocument::new(nested_command(33))
        .validate()
        .expect_err("depth 33 is rejected");
    assert_eq!(
        errors.errors.last().map(|error| error.code),
        Some("max_depth_exceeded")
    );
}

#[test]
fn validation_rejects_invalid_aliases_references_and_value_shapes() {
    let mut document = git_fixture();
    document.root.subcommands[0]
        .aliases
        .push("bad alias".into());
    document.root.options[0].id = OptionId::new("bad id");
    document.root.options[0].conflicts_with = vec![
        OptionId::new("bad id"),
        OptionId::new("bad id"),
        OptionId::new("missing"),
    ];
    document.root.options[1].value.possible_values.clear();
    document.root.options[3].value.value_type = ValueType::String;
    document.root.options[3].value.possible_values = vec!["unexpected".into()];
    document.root.options[3]
        .names
        .push(OptionName::SingleDashLong("v".into()));

    let errors = document
        .validate()
        .expect_err("invalid shapes are rejected");
    let codes = errors
        .errors
        .iter()
        .map(|error| error.code)
        .collect::<Vec<_>>();
    for code in [
        "invalid_option_id",
        "self_option_reference",
        "duplicate_option_reference",
        "unknown_option_reference",
        "missing_enum_values",
        "contradictory_value_metadata",
        "invalid_option_name",
        "invalid_subcommand_alias",
    ] {
        assert!(codes.contains(&code), "missing {code} in {codes:?}");
    }
}

#[test]
fn rendered_option_spelling_controls_collisions() {
    let mut distinct = git_fixture();
    distinct.root.options[3].names = vec![OptionName::SingleDashLong("verbose".into())];
    distinct
        .validate()
        .expect("--verbose and -verbose are distinct rendered tokens");

    distinct.root.options[3].names = vec![OptionName::Short('v')];
    let errors = distinct
        .validate()
        .expect_err("two rendered -v spellings collide");
    assert!(
        errors
            .errors
            .iter()
            .any(|error| error.code == "duplicate_option_spelling")
    );
}
