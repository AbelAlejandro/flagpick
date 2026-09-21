# Canonical schema contract

## Scope

The canonical schema is provided by the `flagpick::schema` library module and the `flagpick schema <command...> --format json` command. The command resolves only the explicitly selected executable, probes it directly with bounded settings, validates the result, and emits canonical JSON.

A `SchemaDocument` contains `schema_version` and `root`, the root `CommandSpec`. The current `SCHEMA_VERSION` is `1`. Root depth is zero, a command at depth 32 is valid, and a child at depth 33 is rejected.

## Model

The command tree represents:

- executable identity;
- descriptions and usage forms;
- subcommands and aliases;
- typed options and positionals;
- source confidence and metadata.

Option names use tagged short, long, or single-dash-long forms. Option references use an `OptionId` local to their command node.

Value arity and type retain producer-declared uncertainty through explicit `unknown` variants. Unrecognized future enum strings are rejected so read-modify-write operations cannot silently replace an unknown producer token with `unknown`. Validation never promotes uncertainty or invents certainty.

Unrecognized metadata belongs in `metadata.extensions`. Extensions use a `BTreeMap<String, serde_json::Value>` so key order is deterministic.

## APIs

- `SchemaDocument::new`
- `SchemaDocument::validate`
- `SchemaDocument::to_canonical_json`
- `SchemaDocument::from_json`
- `CommandSpec::validate`

`from_json` parses but does not implicitly validate. Call `validate` before relying on a parsed document.

## Validation

Validation rejects:

- unsupported schema versions;
- empty or untrimmed command and executable identity values;
- invalid or duplicate node-local option IDs and spellings;
- options without names;
- invalid or duplicate subcommand tokens;
- unresolved, duplicate, or self-referencing option links;
- contradictory value definitions;
- invalid positionals and source spans;
- command depth beyond 32.

Errors accumulate in traversal order. Every error carries a JSON-like path, stable code, and actionable message.

## Canonical JSON

`to_canonical_json` emits pretty JSON with schema-ordered struct fields, ordered vectors, sorted extension keys, and exactly one trailing newline.

A minimal valid document is:

```json
{
  "schema_version": 1,
  "root": {
    "executable": {
      "name": "tool"
    },
    "name": "tool",
    "metadata": {
      "source": "unknown",
      "confidence": "unknown"
    }
  }
}
```

The checked-in Git-like golden fixture covers richer option, value, subcommand, source, and uncertainty forms.

## Versioning

Version 1 readers accept omitted optional, vector, and map fields through Serde defaults. They accept the explicit `unknown` enum value but reject unrecognized future enum strings.

Producers emit the current exact schema version. Breaking field meaning or shape requires a schema version bump. Additive optional fields and metadata extensions do not.

## Current limits

The public command supports `--no-exec-probe` for cache-only operation. A cache miss in that mode is an actionable error; it never starts the target. The command does not invoke a shell or execute a generated command. Specialized adapters, TUI integration, networking, and package distribution remain unavailable.
