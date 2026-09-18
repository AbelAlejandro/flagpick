# Flagpick — Product & Engineering Specification

**Status:** Draft v0.2  
**Product type:** Local-first open-source terminal utility  
**Primary platforms:** macOS and Linux  
**Primary shells:** Zsh, Bash, Fish  
**Secondary shell:** Nushell  
**Implementation:** Rust, single native binary  
**Distribution:** GitHub Releases, Homebrew, Nix/Nixpkgs or flake, `cargo install`  
**Runtime dependencies:** None for the core binary  
**Network requirement:** None  
**Account requirement:** None  
**Working tagline:** **Press a key. Pick the flags. Get your command back.**

---

# 1. Executive summary

Flagpick is a small local terminal utility that turns an installed command's help surface into an interactive command builder.

A user starts typing a normal command in their existing shell:

```sh
ffmpeg -i input.mov
```

They press a configurable shell binding such as `Ctrl-G`.

Flagpick determines the executable and current subcommand context, discovers the relevant options, and opens a small terminal UI:

```text
 ffmpeg
──────────────────────────────────────────────────────────────
 Search: quality

 [ ] -q:v              fixed quality scale
 [x] -crf <value>      constant quality mode             20
 [ ] -preset <name>    encoding preset
 [ ] -c:v <codec>      video codec                    libx264

 Current command
 ffmpeg -i input.mov -c:v libx264 -crf 20

 Enter insert   Tab edit value   Space toggle   Esc cancel
```

When the user confirms, Flagpick returns a modified command line to the existing shell prompt:

```sh
ffmpeg -i input.mov -c:v libx264 -crf 20
```

**Flagpick does not execute the completed command.**

The product deliberately owns only the gap between:

> “I know which CLI I want”

and

> “I remember the exact flag/subcommand syntax.”

It should feel like **Magit Transient for arbitrary CLIs**, without requiring the CLI author to integrate anything.

---

# 2. Product goal

## 2.1 Goal

Make unfamiliar, flag-heavy, or infrequently used command-line applications discoverable and usable without leaving the shell, memorizing syntax, opening a browser, or delegating a deterministic syntax lookup to an AI agent.

A successful interaction is:

1. User types a normal partial command.
2. User invokes Flagpick.
3. Flagpick identifies the command and subcommand context.
4. User searches option names **and descriptions**.
5. User toggles flags and supplies values.
6. Flagpick writes the edited command back into the shell buffer.
7. User reviews the result.
8. **The user presses Enter themselves.**

The shell remains the primary interface. Flagpick is transient.

---

# 3. Why this product should exist

The pain is not theoretical. Repeated public discussions show several consistent problems.

## 3.1 People forget syntax for commands they do not use every day

A Reddit user described repeatedly forgetting flags and syntax for `awk`, `sed`, and `find`, relearning them, and then forgetting them again. The workarounds suggested were personal notes, `--help`, or ChatGPT.

**UX implication:** Flagpick should optimize for *recognition over recall*. Search must match descriptions such as “modified”, “recursive”, “codec”, or “namespace”, not merely exact flag names.

Source:  
https://www.reddit.com/r/devops/comments/1cetuur/remembering_shell_commands_that_you_are_not/

---

## 3.2 Large help surfaces become walls of text

A 2026 GitHub issue for `cmux` describes roughly 150 top-level commands and a `--help` output that had become a “200-line wall”; the user explicitly says they cannot hold the command/flag namespace in their head.

**UX implication:** Flagpick should never reproduce `--help` as a scrolling wall. It must provide:

- fuzzy search,
- semantic grouping when the source exposes groups,
- subcommand scoping,
- short descriptions,
- selected-options view,
- progressive disclosure.

Source:  
https://github.com/manaflow-ai/cmux/issues/6616

---

## 3.3 Completion alone does not solve discoverability

A Claude Code feature request notes that users must repeatedly use `--help` to discover deeply nested subcommands and flags. Another issue describes similar-prefix slash commands becoming painful as the namespace grows.

**UX implication:** Flagpick is not just autocomplete. It should expose **name + description + argument type + aliases + command hierarchy** and let users search across those fields.

Sources:  
https://github.com/anthropics/claude-code/issues/40503  
https://github.com/anthropics/claude-code/issues/40538

---

## 3.4 Users want contextual explanation, not just a manual page

In a Reddit discussion about a CLI explanation tool, one commenter specifically valued seeing flags “in context with each other” and argued that `man`/`--help` are not necessarily quick or contextual.

**UX implication:** The UI should always show the **resulting command** as it is being assembled, so flags are understandable in combination rather than in isolation.

Source:  
https://www.reddit.com/r/commandline/comments/ixzr2i

---

## 3.5 Long, variable command lines are a real workflow problem

An Ask HN thread describes keeping a huge QEMU/build/SSH invocation in shell history and manually changing pieces depending on the scenario. The author explicitly cites Magit as inspiration: a discoverable text UI for building complex command lines from varying flags.

**UX implication:** Flagpick must work well on an **existing partially built command**, rather than assuming an empty prompt or making the user start over.

Source:  
https://news.ycombinator.com/item?id=40142791

---

## 3.6 Quoting is a persistent shell footgun

A GitHub CLI issue documents the common failure where a value containing spaces is passed as multiple arguments because the user did not quote or escape it.

**UX implication:** Flagpick should know which selections represent one argument and produce correct shell-specific quoting when it adds a new value. It should preserve the user's existing text instead of reparsing and reserializing the entire command.

Source:  
https://github.com/cli/cli/issues/756

---

## 3.7 Account/network friction is inappropriate for this problem

In a discussion of a CLI-reference tool, a user objected to having to sign in from the terminal.

**UX implication:** Flagpick must remain fully local, anonymous, and usable offline. No signup, telemetry requirement, cloud account, model API, or package registry call should be necessary after installation.

Source:  
https://www.reddit.com/r/commandline/comments/ixzr2i

---

# 4. Jobs to be done

## Primary job

> When I know which command I need but do not remember its exact options, help me construct the command without leaving my shell.

## Secondary jobs

> When I encounter a new CLI, let me explore what it can do without reading an unstructured help wall.

> When I have already typed half a complex command, let me safely add or modify flags without destroying what I wrote.

> When a coding agent suggests or needs a local command, expose deterministic structured CLI metadata so the agent does not have to guess syntax.

> When I return to a machine months later, work immediately without requiring a personal snippet database.

---

# 5. Non-goals

Flagpick is **not**:

- a terminal emulator,
- a replacement shell,
- a command executor,
- a shell-history manager,
- a snippet manager,
- an AI chat application,
- a workflow/task runner,
- a package manager,
- a remote documentation service,
- a telemetry product,
- a generalized shell AST rewriter,
- a replacement for native shell completion,
- a replacement for man pages.

The MVP must not require:

- an account,
- internet connectivity,
- an API key,
- Node.js,
- Python,
- Java,
- a daemon,
- Docker,
- root privileges,
- a local database server.

---

# 6. Product principles

## 6.1 Insertion, never surprise execution

Normal interactive operation only returns text.

Flagpick must not automatically execute the completed command.

There should be **no “Enter = run” mode in v1**.

This is both a usability and a security principle: the user gets a final review boundary at their own shell prompt.

---

## 6.2 Preserve user-authored text

Do not normalize, reorder, quote, or rewrite text the user did not explicitly ask Flagpick to change.

If the incoming buffer is:

```sh
curl -H 'X-Thing: a b' "$URL" \
  --retry 3
```

and the user adds `--fail-with-body`, Flagpick should preserve the existing spelling, quoting, whitespace, and line continuation wherever practical.

**Implementation rule:** operate on source spans and append/replace selected spans. Do not round-trip the entire shell line through a generic tokenizer.

---

## 6.3 Recognition over recall

Search descriptions, aliases, groups, and subcommand names.

A user should be able to type:

```text
recursive
```

and find `-r`, `-R`, or `--recursive` depending on the current CLI.

---

## 6.4 Progressive disclosure

Default to the relevant command/subcommand.

Allow explicit expansion to inherited/global options and advanced help.

Do not dump everything immediately.

---

## 6.5 Local-first and deterministic

Help data should come primarily from the installed command and local docs.

No LLM is required for core operation.

AI integrations are optional consumers of Flagpick's structured metadata, not a dependency of Flagpick.

---

## 6.6 Transparent discovery

A user can always see how Flagpick learned a command's schema.

Example:

```sh
flagpick inspect curl
```

```text
executable: /usr/bin/curl
version: curl 8.7.1
source: argv help probe
probe: ["--help", "all"]
parser: curl-help
cache: hit
confidence: high
```

---

# 7. Core UX

## 7.1 Invocation model

Preferred shell binding:

```text
Ctrl-G
```

The default should be configurable because shells, tmux, editors, and terminal applications may already use it.

Alternative invocation:

```sh
flagpick
flagpick --line 'docker run --rm'
flagpick docker run
```

The shell integration is the premium UX; explicit invocation is the universal fallback.

---

## 7.2 Main TUI layout

```text
 docker run
──────────────────────────────────────────────────────────────────
 Search: volume

 Flags
 > -v, --volume <list>       Bind mount a volume
   --volume-driver <name>    Optional volume driver
   --volumes-from <list>     Mount volumes from another container

 Selected
   --rm
   --name demo
   -p 8080:80

 Result
 docker run --rm --name demo -p 8080:80

──────────────────────────────────────────────────────────────────
 Space toggle   Enter insert   Tab value   ? details   Esc cancel
```

### Required properties

- search input is focused immediately;
- search matches names and descriptions;
- selected options remain visible;
- result command is visible;
- value-required options are visually distinct;
- inherited/global options can be shown separately;
- mutually exclusive options can be marked when known;
- duplicate/repeatable options can be represented;
- user can inspect longer help without leaving the TUI.

---

## 7.3 Search behavior

Rank roughly by:

1. exact flag name,
2. flag prefix,
3. alias match,
4. subcommand name,
5. description token match,
6. fuzzy description match.

Do **not** require embeddings or network access.

Recommended implementation: `nucleo-matcher` or a similarly lightweight fuzzy matcher.

---

## 7.4 Value editing

Options may be:

- boolean,
- required scalar,
- optional scalar,
- repeatable scalar,
- enum,
- path,
- integer,
- key/value,
- list,
- unknown/raw.

When type information is not available, treat the value as raw text and quote safely for the active shell.

For enum-like usage text:

```text
--color <WHEN>  [possible values: auto, always, never]
```

show selectable values.

For paths, optionally invoke shell-native or Flagpick-native filesystem completion.

---

## 7.5 Editing an existing option

If the command already contains:

```sh
--crf 24
```

and the user selects `--crf`, Flagpick should default to editing `24`, not adding a second `--crf 20`, unless metadata marks the option repeatable.

If repeatability is unknown, display:

```text
Already present: --crf 24
[E] edit   [A] add another
```

---

## 7.6 Cancel behavior

`Esc` must return to the original shell line with no modifications.

Cancellation must be lossless.

---

# 8. Shell support

# 8.1 Zsh — Tier 1

macOS defaults to Zsh, so Zsh is a launch blocker.

Use a ZLE widget.

Conceptually:

```zsh
function flagpick-widget() {
  local new_buffer
  new_buffer="$(flagpick shell-edit zsh --buffer "$BUFFER" --cursor "$CURSOR")" || return
  BUFFER="$new_buffer"
  CURSOR=${#BUFFER}
  zle redisplay
}
zle -N flagpick-widget
bindkey '^G' flagpick-widget
```

Production integration must avoid unsafe interpolation and should use stdin/stdout or a temporary transport rather than embedding arbitrary buffer content in a shell-evaluated string.

### Requirements

- preserve `$BUFFER`;
- preserve cursor position when possible;
- support multiline buffers;
- return untouched buffer on cancel;
- no `eval`;
- no generated shell code from untrusted help text.

---

# 8.2 Bash — Tier 1

Use Readline variables exposed by `bind -x`:

- `READLINE_LINE`
- `READLINE_POINT`

Conceptual adapter:

```bash
_flagpick_widget() {
  local result
  result="$(
    printf '%s' "$READLINE_LINE" |
      flagpick shell-edit bash --cursor "$READLINE_POINT"
  )" || return
  READLINE_LINE="$result"
  READLINE_POINT=${#READLINE_LINE}
}

bind -x '"\C-g":_flagpick_widget'
```

### Pitfalls

- `READLINE_POINT` semantics can differ from Rust byte indexing for non-ASCII input;
- multiline input must be tested;
- Bash versions bundled on macOS can be old;
- interactive Readline behavior differs from non-interactive Bash.

Treat cursor positions as character/grapheme aware at the adapter boundary.

---

# 8.3 Fish — Tier 1

Fish exposes its command buffer through `commandline`.

Conceptual integration:

```fish
function __flagpick_widget
    set -l current (commandline)
    set -l cursor (commandline -C)
    set -l result (printf '%s' "$current" | flagpick shell-edit fish --cursor $cursor)
    or return
    commandline -r -- $result
end

bind \cg __flagpick_widget
```

### Fish-specific advantages

Fish already has rich generated completions and descriptions. In a later phase Flagpick may ingest Fish completion metadata as a higher-confidence source than parsing human-readable help.

### Requirement

Do not assume POSIX shell quoting is identical to Fish quoting.

---

# 8.4 Nushell — Tier 2

Nushell has a structured command model and its own line editor behavior.

Goals:

- support explicit `flagpick <command...>` first;
- support a keybinding once stable buffer-get/buffer-replace behavior is validated across supported Nushell versions;
- use Nushell signatures/completion metadata when safely obtainable;
- preserve Nushell-specific syntax rather than treating it as POSIX shell.

Nushell should not delay v0.1, but should be architecturally supported through the shell-adapter interface.

---

# 8.5 Unsupported initially

- PowerShell: useful, but outside initial macOS/Linux scope.
- tcsh/csh: no launch requirement.
- Elvish: later community adapter.
- Xonsh: later community adapter.

The core binary must not hardcode Zsh/Bash semantics; shell adapters own quoting, cursor mapping, and buffer replacement.

---

# 9. Architecture

Recommended workspace:

```text
flagpick/
├── crates/
│   ├── flagpick-cli/
│   ├── flagpick-core/
│   ├── flagpick-model/
│   ├── flagpick-discovery/
│   ├── flagpick-parsers/
│   ├── flagpick-shell/
│   ├── flagpick-tui/
│   └── flagpick-mcp/          # optional feature/binary later
├── integrations/
│   ├── zsh/
│   ├── bash/
│   ├── fish/
│   ├── nushell/
│   ├── opencode/
│   ├── codex/
│   └── claude-code/
├── fixtures/
├── tests/
└── Formula/
```

---

# 10. Why Rust

Rust is the preferred implementation language.

## Advantages

### Single self-contained binaries

Produce native binaries for:

- macOS arm64 (`aarch64-apple-darwin`)
- macOS x86_64 (`x86_64-apple-darwin`)
- Linux x86_64 GNU
- Linux arm64 GNU
- optionally Linux x86_64 musl
- optionally Linux arm64 musl

No interpreter/runtime bootstrap.

### Homebrew-friendly

Rust CLI projects are straightforward to distribute as bottles or prebuilt release artifacts.

### Nix-friendly

Reproducible builds are well supported.

### Strong safety properties

Rust removes broad classes of memory-safety bugs relevant to a utility that processes arbitrary executable output and shell text.

### Excellent TUI ecosystem

Recommended:

- `ratatui`
- `crossterm`

### Good process control

`std::process::Command` permits direct argv execution without invoking `/bin/sh`.

### Extensibility

Use trait-based parsers and discovery strategies instead of plugins loading arbitrary dynamic code.

---

# 11. Why not Go

Go is a credible second choice and would also satisfy the deployment goals.

Rust wins because:

- the application is parser-heavy;
- precise byte/string ownership matters;
- shell-buffer mutation benefits from strict types;
- Ratatui provides a strong ecosystem for polished TUIs;
- Rust makes it easy to ship a library and CLI from one workspace;
- optional compile-time feature gates are useful for integrations.

If contributor velocity is valued above all else, Go would be reasonable. For a long-lived open-source terminal utility with parsers and safety boundaries, use Rust.

---

# 12. Why not Python/Node

They are excellent for prototypes but conflict with the deployment goal:

- runtime dependency,
- environment/version problems,
- slower cold-start potential,
- packaging complexity on clean machines,
- greater friction for Homebrew/Nix “one binary” expectations.

The shell integration should not depend on them.

---

# 13. Internal data model

Canonical internal schema:

```rust
struct CommandSpec {
    executable: ExecutableIdentity,
    name: String,
    description: Option<String>,
    usage: Vec<String>,
    subcommands: Vec<SubcommandSpec>,
    options: Vec<OptionSpec>,
    positionals: Vec<PositionalSpec>,
    metadata: SpecMetadata,
}

struct OptionSpec {
    long: Option<String>,
    shorts: Vec<char>,
    aliases: Vec<String>,
    description: Option<String>,
    value: ValueArity,
    value_name: Option<String>,
    value_type: ValueType,
    repeatability: Repeatability,
    required: bool,
    global: bool,
    hidden: bool,
    conflicts_with: Vec<OptionId>,
    requires: Vec<OptionId>,
    possible_values: Vec<String>,
    source_span: Option<SourceSpan>,
}

enum ValueArity {
    None,
    Required,
    Optional,
    Multiple,
    Unknown,
}

enum ValueType {
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

struct SpecMetadata {
    source: HelpSource,
    parser: String,
    confidence: Confidence,
    executable_version: Option<String>,
    fingerprint: String,
}
```

Unknown values are valid. The parser must degrade gracefully instead of inventing certainty.

---

# 14. Help discovery pipeline

There is no universal help convention. Discovery must be **strategy-based**, not “always run `--help`”.

Each discovery strategy returns:

```rust
DiscoveryResult {
    stdout,
    stderr,
    exit_status,
    argv,
    confidence,
    observed_format,
}
```

---

# 15. Source priority

Recommended source priority:

1. **Explicit built-in adapter/schema**
2. **Machine-readable CLI schema**, when the program exposes one
3. **Native shell completion metadata**, if trustworthy and available
4. **Known framework-specific help format**
5. **Generic help probe**
6. **Local man page**
7. **Fallback minimal mode**

Do not fetch internet docs during normal use.

---

# 16. Generic help probing

For an unknown executable, use conservative probes.

Suggested order:

```text
<cmd> --help
<cmd> -h
<cmd> help
```

But this is not blindly correct for every executable.

Probing should be driven by heuristics and an adapter registry.

## Probe constraints

Every probe:

- invokes the executable directly using an argv array;
- never passes through a shell;
- has a timeout;
- captures stdout and stderr;
- caps total captured output;
- never provides stdin interaction;
- uses a controlled environment;
- does not follow arbitrary pager invocations;
- records exit status but does not require zero.

Suggested defaults:

```text
soft timeout: 800 ms
hard timeout: 2 s
stdout cap: 4 MiB
stderr cap: 1 MiB
stdin: null
PAGER=cat
GIT_PAGER=cat
MANPAGER=cat
TERM=dumb for discovery probes where appropriate
NO_COLOR=1 where appropriate
```

Timeouts should be configurable.

---

# 17. Critical security caveat: probing executes the target

Running:

```sh
some-command --help
```

still runs `some-command`.

A malicious or badly designed binary can ignore `--help` and perform side effects.

**Flagpick cannot provide a universal portable sandbox for arbitrary executables while remaining dependency-free.**

Therefore:

- only probe an executable the user explicitly typed or selected;
- resolve the executable deterministically;
- show the resolved path in `inspect`;
- never scan and execute every binary on `$PATH`;
- never probe commands speculatively in the background;
- cache successful schemas;
- provide `--no-exec-probe`;
- provide allow/deny configuration;
- prefer local static/man/completion sources when available;
- never probe a shell function automatically;
- never execute shell aliases as commands merely to inspect them.

Optional future Linux sandboxing may use an external capability such as bubblewrap if present, but it must not be required for normal installation.

---

# 18. Help invocation edge cases

The test suite must explicitly cover non-universal conventions.

## 18.1 Standard GNU-ish help

Examples:

```sh
rg --help
fd --help
tar --help
```

Typical parser shape:

```text
Usage:
Options:
  -x, --example <VALUE>   description
```

---

## 18.2 `-h` differs from `--help`

Git is a major example of help behavior with multiple levels.

Git documents:

```sh
git -h
git help <verb>
git <verb> --help
```

`git <verb> --help` may invoke a manual viewer. Flagpick must prevent pager/UI takeover by controlling pager-related environment variables and should prefer concise non-pager paths where possible.

For a Git subcommand:

```sh
git commit -h
```

may be a better machine-parsing source than opening the full manual.

Source:  
https://git-scm.com/docs/git  
https://git-scm.com/book/en/v2/Getting-Started-Getting-Help.html

---

## 18.3 Help is a subcommand

Examples:

```sh
openssl help
git help
```

OpenSSL also supports:

```sh
openssl -help
```

Source:  
https://docs.openssl.org/3.4/man1/openssl/

---

## 18.4 Help accepts a subject/category

Current curl supports:

```sh
curl --help
curl --help all
curl --help category
curl --help --insecure
```

The basic form intentionally shows only important options.

Flagpick should use `curl --help all` in the curl adapter when the user wants the complete option index.

Source:  
https://curl.se/docs/manpage.html

---

## 18.5 Tool-specific extended help

FFmpeg supports multiple help levels:

```sh
ffmpeg -h
ffmpeg -h long
ffmpeg -h full
ffmpeg -h encoder=libx264
```

The generic `-h` is intentionally incomplete.

Flagpick should use an FFmpeg adapter and avoid treating the basic help output as a complete schema.

Source:  
https://ffmpeg.org/ffmpeg.html

---

## 18.6 Subcommand-scoped `--help`

Docker documents:

```sh
docker run --help
```

Kubectl similarly exposes help on its command hierarchy.

The discovery engine should construct a probe using the already-recognized subcommand prefix:

```text
docker run --help
kubectl get --help
git remote -h
```

rather than always probing only the root executable.

Sources:  
https://docs.docker.com/reference/cli/docker/  
https://kubernetes.io/docs/reference/kubectl/generated/kubectl/

---

## 18.7 Help printed to stderr

Many programs print usage/help to stderr, especially when invoked with a short help flag that is also treated as an error path.

Parser input is:

```text
stdout + stderr
```

with streams retained separately for diagnostics.

A non-zero exit code does not automatically mean discovery failed.

---

## 18.8 No help switch

Some commands may have only:

- man pages,
- `info`,
- shell builtin help,
- external documentation.

Fallback sequence may attempt local man-page extraction **without launching an interactive pager**.

Example strategy:

```sh
man -P cat <cmd>
```

But man-page probing should be an explicit discovery strategy with timeout/output limits.

---

## 18.9 Shell builtins

Examples:

```text
cd
alias
export
read
set
```

There may be no external executable to spawn.

Adapters:

- Bash builtin: `help <builtin>` is possible but should be called only from the Bash adapter.
- Zsh builtin docs have different mechanisms.
- Fish builtins have native help.

Builtins should be treated as shell-specific command providers, not generic executables.

---

## 18.10 Multi-call binaries

BusyBox-style systems expose many commands through a single binary.

Fingerprinting should include the invoked command name/argv0 context, not only the underlying inode.

---

## 18.11 Aliases

Given:

```sh
alias k=kubectl
```

Flagpick should not execute arbitrary alias text.

Possible behavior:

1. shell adapter detects simple alias,
2. resolves it structurally when safe,
3. displays:

```text
k → kubectl
```

4. uses `kubectl` metadata,
5. preserves the user's spelling `k`.

Complex aliases containing operators, substitutions, or functions should fall back to raw mode.

---

## 18.12 Shell functions

Do not automatically run shell functions to discover help.

Functions may mutate the environment, filesystem, network, or shell state.

Future opt-in adapters may allow manually registered functions.

---

## 18.13 Wrappers and launchers

Examples:

```text
nix run ...
uv run ...
poetry run ...
cargo run ...
sudo ...
env VAR=value ...
command ...
```

Flagpick needs a **command-prefix recognizer**.

Examples:

```sh
sudo kubectl get
env FOO=1 rg
command git commit
```

The effective CLI is not necessarily token 0.

Initial safe wrappers:

- `sudo` — parse flags, then identify child executable; do not invoke sudo merely for help.
- `env` — parse assignments/options, then identify child.
- `command` — shell adapter can identify child.
- `nice` — optional.
- `time` — shell dependent; handle conservatively.

Complex launchers such as `nix run` are commands in their own right unless explicit child-command semantics are available.

---

## 18.14 Options before subcommands

Some CLIs allow:

```sh
kubectl --namespace foo get pods
git -C repo commit
```

The subcommand resolver must tolerate recognized global options before the subcommand.

---

## 18.15 Combined short flags

Examples:

```sh
ls -lah
tar -xvf
```

Do not assume every `-abc` means `-a -b -c`; some programs treat it as a value or custom syntax.

Only expand combined flags if the command schema or parser convention makes it safe.

---

## 18.16 `--flag=value`

Support both:

```sh
--output file
--output=file
```

Preserve whichever style the user already uses.

Per-option metadata may record preferred rendering.

---

## 18.17 Negative flags

Examples:

```text
--color / --no-color
--foo / --no-foo
```

Represent these as related alternatives when detectable.

---

## 18.18 Repeated flags

Examples:

```sh
-H header1 -H header2
-vvv
--include a --include b
```

Repeatability is an explicit schema property when known.

Unknown repeatability must never lead Flagpick to silently delete existing occurrences.

---

## 18.19 Variadic positionals

Examples:

```text
cp SOURCE... DEST
git add [PATHSPEC...]
```

Positionals should be shown separately from options.

The MVP can focus on flags, but the schema must support them from day one.

---

## 18.20 `--` option terminator

Anything after:

```sh
--
```

must normally be treated as positional/raw input for the child program and not parsed as flags.

---

## 18.21 Commands that prompt

Help probes get stdin closed/null.

If a supposed help probe prompts, it should time out instead of blocking the terminal.

---

## 18.22 Commands that initialize state even on help

Some CLIs create config/cache directories during startup.

This is another reason to:

- cache aggressively,
- avoid speculative probing,
- document that executing the target for help may have program-defined side effects,
- support static adapters and `--no-exec-probe`.

---

# 19. Parsing architecture

Define a trait:

```rust
trait HelpParser {
    fn id(&self) -> &'static str;
    fn detect(&self, probe: &ProbeOutput) -> DetectionScore;
    fn parse(&self, probe: &ProbeOutput) -> Result<CommandSpec>;
}
```

Initial parsers:

```text
ClapParser
CobraParser
ClickParser
ArgparseParser
GoFlagParser
DocoptLikeParser
GNUGenericParser
DockerParser
GitParser
CurlParser
FfmpegParser
OpenSslParser
```

Specialized parsers can reuse generic components.

---

# 20. Confidence model

Each parsed field may carry confidence:

```text
Exact       machine-readable/native schema
High        known help framework
Medium      generic structured parse
Low         heuristic extraction
Unknown     inferred only
```

UI behavior:

- exact/high: enable richer editing;
- medium: allow normal use;
- low: display subtle `?` marker;
- unknown: never claim conflicts/types that were not observed.

---

# 21. Caching

Cache location:

Linux:

```text
$XDG_CACHE_HOME/flagpick/
~/.cache/flagpick/
```

macOS:

prefer XDG if configured; otherwise a documented user-cache path.

Cache key should include:

- absolute executable path,
- file metadata/fingerprint,
- executable version if cheaply obtainable,
- invoked alias/multicall identity,
- subcommand path,
- parser version,
- Flagpick schema version.

Do not use only the command name.

Example:

```text
/usr/bin/git
git version 2.48.1
subcommand=commit
parser=git-v2
```

---

# 22. Executable version detection

Version probing is also execution and must not be speculative.

Prefer:

1. file modification fingerprint;
2. help output's version if present;
3. adapter-specific `--version` only if needed.

A changed binary invalidates its cache.

---

# 23. Security model

## 23.1 Threat model

Flagpick processes:

- untrusted shell text,
- untrusted executable names,
- arbitrary local executable output,
- arbitrary descriptions/control characters,
- arbitrary filenames,
- possibly malicious ANSI sequences.

Primary risks:

- shell injection,
- terminal escape injection,
- unexpected command execution,
- parser denial of service,
- memory exhaustion,
- path confusion,
- malicious help output,
- accidental secret exposure through cache/logs,
- unsafe AI-tool exposure.

---

## 23.2 Hard security invariants

### Never use `sh -c`

Target probes must use:

```rust
Command::new(executable).args(argv)
```

not:

```text
sh -c "<user content>"
```

### Never `eval` generated shell text

Shell adapters assign returned data to the editor buffer.

They do not evaluate it.

### Sanitize terminal output

Descriptions displayed in the TUI must strip or neutralize:

- ANSI escape sequences,
- OSC sequences,
- terminal title commands,
- hyperlinks,
- C0/C1 control characters except explicitly supported whitespace.

### Bound all input

Set:

- maximum help bytes,
- maximum lines,
- maximum option count,
- maximum description length,
- maximum nesting depth.

### No automatic final execution

Always return to the user's shell.

### No secrets in telemetry

There is no telemetry by default.

### Cache schema, not user command lines

Avoid persisting complete shell buffers.

---

# 24. Practical security limitations

Flagpick is not a sandbox.

If the user types:

```sh
./malicious-binary
```

and Flagpick probes:

```sh
./malicious-binary --help
```

the binary can execute arbitrary behavior with the user's privileges.

The product must state this plainly.

A “strict discovery” mode should support:

```sh
flagpick --no-exec-probe
```

which uses only:

- existing cache,
- registered adapters,
- local completion metadata,
- man pages where configured.

---

# 25. Terminal escape defense

Malicious help text can contain escape sequences.

Before rendering any executable-generated text:

1. parse bytes lossily/strictly;
2. remove ANSI CSI;
3. remove OSC;
4. remove DCS/APC/PM;
5. replace unsafe control bytes;
6. enforce display-width bounds.

Add fuzz tests specifically for escape-sequence injection.

---

# 26. Shell parsing strategy

Do **not** attempt to implement a fully correct Zsh/Bash/Fish parser in v1.

Use a layered approach:

1. shell adapter provides the raw buffer + cursor;
2. identify the active command segment conservatively;
3. recognize executable/subcommand tokens only where syntax is unambiguous;
4. preserve all untouched text as source spans;
5. refuse destructive rewriting on ambiguous constructs.

Ambiguity examples:

```sh
$(...)
`...`
<(...)
{ ...; }
foo && bar
foo | bar
VAR="$(cmd)"
```

Flagpick can still operate on the last/simple command segment if confidently isolated.

If confidence is low:

```text
Complex shell syntax detected.
Flagpick will append only; existing text will not be rewritten.
```

---

# 27. Rendering / quoting

Each shell adapter implements:

```rust
trait ShellQuoter {
    fn quote_argument(&self, value: &str) -> String;
}
```

Requirements:

- no word splitting;
- preserve a literal newline only if supported;
- correctly escape quotes;
- account for empty string;
- avoid accidental variable/command expansion when adding literal values.

Where possible, provide rendering modes:

```text
preserve
space
equals
```

Example:

```text
--name "hello world"
--name=hello
```

---

# 28. Command editing representation

Do not reduce the input to `Vec<String>` and then rebuild it.

Use:

```rust
struct ShellBuffer {
    original: String,
    cursor: Cursor,
    regions: Vec<Region>,
}

struct Region {
    range: Range<usize>,
    kind: RegionKind,
    confidence: Confidence,
}
```

Edits become patches:

```rust
enum Edit {
    Insert { at: usize, text: String },
    Replace { range: Range<usize>, text: String },
    Delete { range: Range<usize> },
}
```

Apply patches to the original text.

This is central to preserving quoting and formatting.

---

# 29. Installation UX

## macOS Homebrew

Target:

```sh
brew install flagpick
```

Then:

```sh
flagpick init zsh
```

Recommended `init` behavior:

```text
Detected shell: zsh
Add this line to ~/.zshrc:

  eval "$(flagpick shell-init zsh)"

Or run:
  flagpick init zsh --write
```

Do **not** silently modify shell startup files without explicit `--write` or confirmation.

For development/beta:

```sh
brew install owner/tap/flagpick
```

Long-term goal: acceptance into Homebrew Core once stable and sufficiently used.

---

# 30. Homebrew packaging

Preferred release model:

- GitHub tag,
- signed checksums,
- prebuilt macOS arm64/x86_64 archives,
- source build remains possible.

Formula should avoid runtime dependencies.

For universal macOS distribution, either:

- ship architecture-specific bottles, preferred; or
- ship a universal binary if size/CI complexity is acceptable.

Apple Silicon must be first-class.

---

# 31. Nix

Support both:

```nix
nix run github:owner/flagpick
```

and a flake:

```nix
{
  inputs.flagpick.url = "github:owner/flagpick";
}
```

Expose:

```text
packages.x86_64-linux.default
packages.aarch64-linux.default
packages.x86_64-darwin.default
packages.aarch64-darwin.default
```

Long-term: submit to `nixpkgs`.

The binary must not assume `/usr/bin`, `/bin`, or Homebrew filesystem layouts.

---

# 32. Linux distribution

Minimum:

- tarballs on GitHub Releases,
- static or mostly-static musl builds where practical,
- x86_64 and arm64.

Nice-to-have later:

- `.deb`
- `.rpm`
- Arch AUR
- Alpine package.

Avoid making package-manager coverage a v0.1 blocker.

---

# 33. Architecture matrix

CI should build/test:

| OS | Arch | Tier |
|---|---|---|
| macOS | arm64 | Required |
| macOS | x86_64 | Required |
| Linux glibc | x86_64 | Required |
| Linux glibc | arm64 | Required |
| Linux musl | x86_64 | Recommended |
| Linux musl | arm64 | Recommended |

---

# 34. CLI surface

Proposed:

```text
flagpick
flagpick <command...>
flagpick inspect <command...>
flagpick refresh <command...>
flagpick cache list
flagpick cache clear [command]
flagpick shell-init <shell>
flagpick shell-edit <shell>
flagpick completion <shell>
flagpick schema <command...> --format json
flagpick doctor
flagpick config path
flagpick version
```

Potential later:

```text
flagpick mcp
flagpick serve-stdio
flagpick adapters list
```

---

# 35. `flagpick schema`

This is strategically important.

Example:

```sh
flagpick schema git commit --format json
```

returns structured local metadata without opening the TUI.

This turns Flagpick from a UI-only product into a reusable **CLI introspection engine**.

Example output:

```json
{
  "command": ["git", "commit"],
  "description": "Record changes to the repository",
  "options": [
    {
      "names": ["-m", "--message"],
      "value": {
        "required": true,
        "name": "msg",
        "type": "string"
      },
      "description": "Use the given message as the commit message"
    }
  ]
}
```

This is the foundation for AI-agent integration.

---

# 36. AI coding-agent integration philosophy

Do **not** make Flagpick an AI app.

Instead expose deterministic local capabilities that AI agents can consume.

The useful primitive is:

> Given a local executable/subcommand, return a bounded structured schema and optionally construct a command string, without executing the final command.

This directly complements coding agents, which frequently need to invoke tools installed in a repository or workstation.

---

# 37. OpenCode integration

OpenCode supports local/global plugins and custom tools. Its plugin system can expose a tool with a typed schema.

A small OpenCode plugin can expose:

```text
flagpick_schema
flagpick_build
```

Example conceptual tool:

```ts
tool: {
  flagpick_schema: tool({
    description: "Inspect locally installed CLI flags without executing the final command",
    args: {
      command: tool.schema.string(),
    },
    async execute({ command }) {
      return await $`flagpick schema ${command} --format json`.text()
    }
  })
}
```

**Production implementation must not interpolate one raw command string through a shell.** The plugin should pass an argv array or invoke a library/process API safely.

OpenCode can also have a TUI-only plugin that opens Flagpick for a command under discussion.

Relevant current OpenCode capabilities include local project/global plugins, CLI plugins, custom tools, and custom commands.

Sources:  
https://opencode.ai/v2/docs/plugins  
https://opencode.ai/v2/docs/cli/plugins  
https://dev.opencode.ai/docs/commands/

---

# 38. Codex integration

Current Codex supports:

- local tools,
- skills,
- plugins,
- MCP connections,
- shell completion.

The lightest integration is a **Codex skill/plugin instruction** telling Codex:

> Before guessing flags for an unfamiliar installed CLI, call `flagpick schema ... --format json`.

A richer option is a local MCP server backed by the same Rust core.

Potential MCP tools:

```text
inspect_cli
build_cli_arguments
explain_cli_option
```

Do **not** expose:

```text
run_command
```

from Flagpick's MCP server.

The agent can already use its own command-execution permission model. Flagpick should remain introspection-only.

Current Codex documentation explicitly presents MCP as the mechanism for external tools and skills/plugins as reusable customization layers.

Sources:  
https://developers.openai.com/docs/codex/cli  
https://developers.openai.com/docs/customization/overview  
https://developers.openai.com/docs/build-skills

---

# 39. Claude Code integration

Claude Code supports MCP and has a large CLI surface itself.

A Flagpick MCP server can be registered locally:

```sh
claude mcp add flagpick -- flagpick mcp
```

Conceptual tools:

```text
inspect_cli(command_argv)
build_cli(command_argv, selected_options)
```

Again: return data/text only. Never execute the final generated command.

A lightweight non-MCP integration can simply add project instructions:

```text
When you are uncertain about flags for an installed command,
use `flagpick schema ... --format json` instead of guessing.
```

This is particularly relevant because public Claude Code issues have requested richer shell completion for its own growing hierarchy of flags and subcommands.

Sources:  
https://docs.anthropic.com/en/docs/claude-code/cli-usage  
https://docs.anthropic.com/en/docs/claude-code/mcp  
https://github.com/anthropics/claude-code/issues/40503

---

# 40. MCP server design

Optional feature/build:

```sh
flagpick mcp
```

Transport:

- stdio only for v1;
- no listening TCP socket;
- no authentication problem;
- no daemon.

Tools:

## `inspect_cli`

Input:

```json
{
  "argv": ["ffmpeg"],
  "subcommand": []
}
```

Output: `CommandSpec`.

## `inspect_option`

Input:

```json
{
  "argv": ["curl"],
  "option": "--retry"
}
```

## `render_command`

Input:

```json
{
  "shell": "zsh",
  "base": "curl https://example.com",
  "options": [
    {"name": "--retry", "value": "3"}
  ]
}
```

Output:

```json
{
  "command": "curl https://example.com --retry 3",
  "executed": false
}
```

No MCP tool should execute generated commands.

---

# 41. Test philosophy

Tests are a product feature.

CLI ecosystems are inconsistent, and regressions will otherwise be subtle.

Testing layers:

1. pure parser unit tests;
2. golden fixture tests;
3. shell-buffer edit tests;
4. shell integration PTY tests;
5. real executable compatibility tests;
6. fuzz tests;
7. packaging tests;
8. security tests.

---

# 42. Fixture strategy

Store sanitized help fixtures:

```text
fixtures/
├── git/
│   ├── root-h.txt
│   ├── commit-h.txt
│   └── version.txt
├── docker/
│   └── run-help.txt
├── kubectl/
├── ffmpeg/
├── curl/
├── openssl/
├── rg/
├── fd/
├── cargo/
├── rustc/
├── npm/
├── pnpm/
├── python/
├── pip/
├── uv/
├── gh/
├── jq/
└── tar/
```

Each fixture has expected normalized schema JSON.

Golden tests allow parser changes to be reviewed as diffs.

---

# 43. Required real-application compatibility matrix

The release test suite should validate at least:

| CLI | Why |
|---|---|
| `git` | subcommands, `-h` vs manual help, aliases |
| `docker` | deep subcommands, large option sets |
| `kubectl` | inherited/global flags, deep commands |
| `ffmpeg` | nonstandard single-dash long flags, help levels |
| `curl` | huge option set, help categories |
| `openssl` | help subcommand / unusual hierarchy |
| `rg` | Clap-style rich help |
| `fd` | modern Rust CLI |
| `bat` | modern Rust CLI, enums |
| `cargo` | nested Rust toolchain commands |
| `rustc` | compiler-style options |
| `gh` | nested commands and value quoting |
| `jq` | compact Unix CLI |
| `tar` | legacy option syntax |
| `find` | expression language, intentionally difficult |
| `ssh` | unusual help/error behavior |
| `rsync` | very large legacy option surface |
| `python` | interpreter flags |
| `pip` | Python subcommands |
| `uv` | modern Python tooling |
| `npm` | Node CLI/help conventions |
| `pnpm` | Node CLI variant |
| `nix` | large experimental/stable command hierarchy |
| `brew` | macOS package manager commands |
| `systemctl` | Linux-only large CLI |
| `journalctl` | Linux option-heavy CLI |

Not every CLI needs 100% schema fidelity in v1.

---

# 44. Compatibility scoring

For each fixture/app, record:

```text
command discovery
subcommand discovery
short flags
long flags
descriptions
required values
optional values
enums
repeatability
global flags
conflicts
positionals
help strategy
```

Score each field:

```text
PASS
PARTIAL
UNKNOWN
FAIL
N/A
```

Do not reduce everything to one vanity percentage internally; regressions should remain diagnosable.

---

# 45. Parser acceptance bar

For the initial supported set:

- ≥ 95% of visible options discovered for high-priority CLIs;
- ≥ 98% correct flag-name extraction;
- zero invented flag names;
- no crashes on malformed help;
- unknown arity preferred over wrong arity;
- descriptions may be partial but must not attach to the wrong flag.

Correct uncertainty is better than confident fabrication.

---

# 46. Shell integration tests

Use pseudo-terminal integration tests where possible.

## Zsh

Test:

- empty line;
- partial command;
- multiline command;
- quoted values;
- Unicode;
- cursor in middle;
- aliases;
- pipelines;
- cancel;
- append;
- replace existing option.

## Bash

Same matrix, including older Bash behavior relevant to macOS.

## Fish

Same matrix with Fish quoting rules.

---

# 47. Security tests

Required:

## Shell injection

Input:

```sh
echo "$(touch /tmp/flagpick-owned)"
```

Flagpick must not execute substitution while parsing/editing.

## Semicolon

```sh
echo hi; touch /tmp/flagpick-owned
```

No side effect from inspecting buffer text.

## Backticks

```sh
echo `touch /tmp/flagpick-owned`
```

No execution.

## Malicious help ANSI

Fixture contains OSC title changes, clipboard sequences, hyperlinks, cursor movement.

Rendered output must be inert.

## Gigantic help output

Process emits > configured cap.

Flagpick must terminate/cap it.

## Hanging help

Process sleeps forever.

Timeout must kill it cleanly.

## Forking child

Probe spawns children.

Process-group termination behavior should be tested per platform.

---

# 48. Fuzzing

Use `cargo-fuzz` or equivalent on:

- generic help parser;
- shell command-segment recognizer;
- ANSI sanitizer;
- quoting renderer;
- schema deserializer.

Properties:

- never panic;
- never allocate unbounded memory from small input;
- never emit invalid UTF-8 in JSON output;
- sanitized display never contains terminal-control sequences.

---

# 49. Performance targets

Warm-cache shell invocation:

```text
P50 < 40 ms until TUI initialization
P95 < 100 ms
```

Cold parse of ordinary CLI:

```text
P50 < 250 ms, excluding slow target executable startup
```

TUI interaction:

```text
search update < 16 ms for 5,000 options
```

Binary startup matters because the product is used in the typing loop.

---

# 50. Binary size

Target:

```text
< 15 MiB compressed release artifact
```

Not a hard blocker, but dependencies should be audited.

Avoid pulling in:

- async runtime unless justified,
- TLS/network stack in core,
- browser stack,
- embedded JS runtime,
- database engine.

---

# 51. Configuration

Config location:

```text
$XDG_CONFIG_HOME/flagpick/config.toml
~/.config/flagpick/config.toml
```

Example:

```toml
key = "ctrl-g"
default_shell = "zsh"

[discovery]
exec_probe = true
timeout_ms = 1200
max_stdout_bytes = 4194304

[ui]
show_result = true
show_global_options = false

[commands.ffmpeg]
strategy = "ffmpeg"

[commands.internal-prod-tool]
exec_probe = false
```

Project-local config should be optional and opt-in to avoid executing configuration from untrusted repositories.

---

# 52. Per-command adapters

Adapters should be data-first where possible.

Example:

```toml
[adapter]
match = "mycli"

[[probe]]
args = ["help", "--format=json"]
format = "json"
```

But loading arbitrary executable parser plugins is not recommended initially.

Extension mechanisms:

1. declarative probe profiles;
2. Rust traits compiled into the binary;
3. optional external adapters only through explicit user configuration.

Avoid dynamic `.so`/`.dylib` plugin loading for v1.

---

# 53. Extensibility without vulnerability

The core extensibility model should be **data + traits**, not arbitrary scripts.

Safe:

- TOML adapter declaring help argv;
- JSON schema importer;
- compiled parser crate;
- MCP read-only interface.

Riskier:

- `adapter.command = "some shell string"`;
- automatically sourced shell scripts;
- dynamic native plugins;
- arbitrary repo-local hooks.

Do not support the risky forms by default.

---

# 54. Native completion ingestion

Longer-term, Flagpick can ingest:

- Zsh completion definitions,
- Fish completion metadata,
- Bash completion scripts,
- generated completion specs.

However, Bash/Zsh completion scripts can execute arbitrary shell code.

Therefore:

- do not `source` unknown completion scripts in the Flagpick process;
- prefer static parsing of known formats;
- use shell-native completion data only through controlled shell integration;
- treat execution-based completion as a separate opt-in trust boundary.

---

# 55. Man-page ingestion

Fallback parser should recognize common man-page sections:

```text
SYNOPSIS
OPTIONS
COMMANDS
DESCRIPTION
```

Use local man output only.

Set pager to `cat`.

Man parsing is less reliable than native structured help and should have lower confidence.

---

# 56. Error UX

Errors should be actionable.

Bad:

```text
parse failed
```

Good:

```text
Flagpick could not confidently parse `weirdcli`.

Tried:
  weirdcli --help       exit 2, 0 B stdout, 418 B stderr
  weirdcli -h           timed out after 1.2 s

Try:
  flagpick inspect weirdcli
  flagpick weirdcli --raw
  flagpick config adapter weirdcli
```

---

# 57. Raw/fallback mode

If metadata discovery fails, Flagpick can still offer:

- current tokens;
- shell-safe argument insertion;
- recent values within current session only;
- optional raw help viewer.

It must not hallucinate options.

---

# 58. Accessibility and terminal ergonomics

Requirements:

- usable without color;
- `NO_COLOR` honored;
- keyboard-only;
- no mouse required;
- screen-reader-friendly plain fallback output where possible;
- narrow terminal mode;
- minimum terminal width handled gracefully;
- Unicode width handled correctly;
- visible focus state without relying only on color.

---

# 59. First-run experience

After installation:

```sh
$ flagpick
Flagpick is installed.

Enable shell integration:
  eval "$(flagpick shell-init zsh)"

Try it now:
  flagpick curl
```

If interactive shell is detected:

```text
Detected zsh.
Recommended binding: Ctrl-G
```

Do not ask for an account, analytics consent, or internet access.

---

# 60. `flagpick doctor`

Example:

```text
Flagpick 0.1.0
OS              macOS 15 arm64
Shell           zsh 5.9
Binding         Ctrl-G installed
Cache           /Users/me/Library/Caches/flagpick
Terminal        xterm-256color
Probe execution enabled
Homebrew        detected

Checks
✓ TUI available
✓ shell buffer round-trip
✓ git parsed
✓ curl parsed
! ffmpeg not installed
```

This reduces support burden.

---

# 61. Telemetry

Default: **none**.

If anonymous telemetry is ever considered, it must be:

- opt-in,
- documented,
- disabled by default,
- never include command lines,
- never include argument values,
- never include paths.

There is no strong product reason to ship telemetry in early versions.

---

# 62. Logging

Debug logging:

```sh
FLAGPICK_LOG=debug flagpick inspect git commit
```

Logs should redact or avoid:

- full user shell line,
- environment variables,
- argument values when unnecessary.

Debug mode may explicitly reveal more and should warn accordingly.

---

# 63. Success definition

Flagpick is successful when experienced terminal users install it because it is faster than remembering/searching flags, while newer terminal users use it without feeling that they need to learn a second shell.

More concretely:

## Product success

A user can install and get a working shell binding in under two minutes on a clean macOS or Linux machine.

A first-time user can:

```text
type partial command → invoke Flagpick → discover option → insert command
```

without reading documentation.

For the supported compatibility set, users should rarely need to know whether the source came from `--help`, `-h`, a help subcommand, a framework parser, or a special adapter.

---

# 64. v1 measurable success criteria

## Installation

- Homebrew install succeeds on macOS arm64 and x86_64.
- Nix install/run succeeds on Linux/macOS x86_64/arm64.
- no runtime interpreter dependency.
- no root required.

## Reliability

- ≥ 99.9% no-crash rate in parser fixture/fuzz corpus.
- cancel is lossless in all Tier-1 shell tests.
- zero known cases where final command is executed by Flagpick itself.
- zero use of `eval`/`sh -c` on target/user content.

## Compatibility

- high-quality support for at least 20 common CLIs;
- generic useful support for common Clap/Cobra/Click/argparse-style applications;
- special adapters for Git, curl, FFmpeg, Docker/Kubectl family behaviors where needed.

## Speed

- cached UI opens subjectively instantly;
- P95 cached initialization <100 ms on representative hardware.

## UX

In a small usability test, users should complete representative tasks such as:

```text
Find curl retry flag
Add Docker bind mount
Find kubectl namespace option
Set ffmpeg CRF
Create Git commit message option
```

without browser/search documentation in ≥80% of attempts.

---

# 65. North-star metric

Because the application is local and telemetry-free, do not design around a server-side engagement metric.

Use community/release indicators:

- repeat usage reported in user studies;
- Homebrew/Nix package adoption;
- GitHub stars are secondary, not success itself;
- issue reports increasingly concern edge cases rather than “it doesn't understand my CLI”;
- external tools begin consuming `flagpick schema`.

A meaningful qualitative north star is:

> Users instinctively invoke Flagpick before opening a browser for CLI syntax.

---

# 66. MVP scope — v0.1

Must ship:

- Rust binary;
- Ratatui TUI;
- Zsh adapter;
- Bash adapter;
- Fish adapter;
- generic `--help` / `-h` / `help` discovery pipeline;
- stdout + stderr parsing;
- timeout/output limits;
- ANSI/control sanitization;
- source-span-preserving command edits;
- cache;
- `inspect`;
- `schema --format json`;
- generic parsers for major help styles;
- special handling for Git, curl, FFmpeg;
- release binaries;
- Homebrew tap/formula;
- Nix flake;
- fixture + integration + fuzz tests.

Do not ship MCP in the first milestone unless the core is already stable.

---

# 67. v0.2

- Nushell integration;
- Docker/Kubectl specialized hierarchy improvements;
- man-page fallback;
- aliases/simple wrappers;
- better enum/value extraction;
- native completion metadata experiments;
- `flagpick doctor`.

---

# 68. v0.3 / v1 candidate

- stdio MCP server;
- OpenCode plugin;
- Codex skill/plugin packaging;
- Claude Code MCP install docs;
- declarative third-party adapters;
- more Linux packaging;
- accessibility hardening;
- compatibility dashboard generated from CI.

---

# 69. Repository policy

Recommended license:

```text
MIT OR Apache-2.0
```

Rationale:

- developer-tool friendly;
- easy embedding of core library;
- permissive ecosystem adoption;
- Apache-2.0 provides explicit patent language.

Contributions:

- parser fixtures are first-class contributions;
- each parser bug should add a regression fixture;
- adapter PRs require source/help examples;
- no dependencies solely for convenience if a small implementation suffices.

---

# 70. Release engineering

Use GitHub Actions or equivalent to:

1. format/lint/test;
2. run fixture suite;
3. run shell integration matrix;
4. run security tests;
5. build release targets;
6. generate SHA-256 checksums;
7. optionally sign with Sigstore/cosign;
8. create GitHub Release;
9. update Homebrew tap formula;
10. verify Nix flake build.

Reproducibility is desirable.

---

# 71. Dependency policy

Every dependency should answer:

- Does it run in the hot path?
- Does it pull networking/TLS?
- Does it execute code?
- Does it increase binary size substantially?
- Is it maintained?
- Does it support all target architectures?

Likely dependencies:

```text
ratatui
crossterm
clap            # Flagpick's own CLI
serde
serde_json
toml
thiserror
regex or regex-automata
nucleo-matcher
unicode-width
directories     # or small internal path handling
```

Avoid unnecessary async/runtime/network crates in core.

---

# 72. Potential pitfalls

## 72.1 “Universal help parser” overconfidence

Human-readable help is not a standard.

Mitigation:

- parser confidence;
- adapters;
- machine-readable schema when available;
- no invented fields.

---

## 72.2 Help command side effects

Mitigation:

- explicit trust model;
- no background scanning;
- timeout/caps;
- cache;
- strict mode.

---

## 72.3 Shell syntax complexity

Mitigation:

- patch original buffer;
- conservative segmentation;
- append-only fallback;
- per-shell renderer.

---

## 72.4 TUI conflicts with terminal multiplexers

Test under:

- tmux,
- screen where feasible,
- iTerm2,
- Terminal.app,
- Kitty,
- Alacritty,
- WezTerm,
- GNOME Terminal.

Do not depend on proprietary escape extensions.

---

## 72.5 Keybinding conflicts

`Ctrl-G` is a default recommendation, not an invariant.

Offer:

```sh
flagpick shell-init zsh --bind '^F'
```

and document terminal/shell conflicts.

---

## 72.6 Massive command trees

Do not eagerly recurse through every subcommand by executing help repeatedly.

Lazy-load subcommands when entered.

Cache each node.

---

## 72.7 Commands requiring environment/context

Some help output changes by:

- plugins,
- current repository,
- config,
- environment,
- server connection.

Cache should be invalidatable and possibly scoped by relevant context for known commands.

Never silently assume help is globally immutable.

---

## 72.8 Plugin-provided commands

Git, kubectl, and other ecosystems may discover external plugins.

Treat dynamically discovered subcommands carefully.

The user should be able to refresh.

---

## 72.9 Secrets in the current shell line

Since the shell buffer can contain:

```sh
--token SECRET
```

Flagpick must:

- keep it in process memory only;
- not log it;
- not cache it;
- not send it anywhere;
- avoid crash dumps where practical.

---

## 72.10 Agent integrations expanding trust surface

MCP/OpenCode/Codex/Claude integrations must expose introspection, not hidden execution.

The coding agent already has its own execution/approval model. Do not bypass it.

---

# 73. Differentiation

Flagpick should avoid becoming another:

- cheatsheet manager,
- AI command translator,
- autocomplete generator,
- snippet picker.

Its distinct product shape is:

```text
installed CLI
    ↓
discover local command schema
    ↓
interactive transient UI
    ↓
edit existing shell buffer
    ↓
human review
```

That combination is the product.

---

# 74. Example workflows

## Docker

Input:

```sh
docker run nginx
```

Search:

```text
port
```

Select:

```text
-p, --publish
```

Value:

```text
8080:80
```

Return:

```sh
docker run nginx -p 8080:80
```

---

## Git

Input:

```sh
git log
```

Search:

```text
graph
```

Return:

```sh
git log --graph
```

No full Git man page opens.

---

## FFmpeg

Input:

```sh
ffmpeg -i input.mov output.mp4
```

Search:

```text
quality
```

The adapter can expose `-crf` from extended help.

Return:

```sh
ffmpeg -i input.mov -crf 20 output.mp4
```

Position-aware insertion policy needs care because FFmpeg option placement can be semantically significant. The FFmpeg adapter should distinguish input/output options where known; otherwise append conservatively and warn.

---

## curl

Input:

```sh
curl https://example.com
```

Search:

```text
retry
```

Return:

```sh
curl https://example.com --retry 3
```

Flagpick may source the option index from `curl --help all`, not only basic help.

---

# 75. Important semantic issue: argument order

Not all CLI options are order-insensitive.

Examples include:

- FFmpeg input/output scoping;
- `find` expressions;
- compiler/linker flags;
- `ssh` remote command boundaries;
- command wrappers.

Therefore v1 needs insertion policies:

```text
Append
BeforePositionals
BeforeTerminator
AtCursor
CommandSpecific
```

Default should be conservative.

For semantically complicated commands, show where the flag will be inserted.

---

# 76. “At cursor” mode

A useful advanced UX:

- user places cursor where option belongs;
- invokes Flagpick;
- Flagpick inserts selection at cursor.

This is safer for order-sensitive CLIs.

Potential binding:

```text
Ctrl-G      context/default insertion
Alt-G       insert exactly at cursor
```

Do not add multiple bindings until the primary flow is stable.

---

# 77. Documentation strategy

README should lead with a GIF and six lines:

```text
$ ffmpeg -i input.mov
  [Ctrl-G]

Search: quality
> -crf <value>   constant quality mode

$ ffmpeg -i input.mov -crf 20
```

Then:

```text
brew install flagpick
eval "$(flagpick shell-init zsh)"
```

Only afterward explain architecture.

The product is visual and should be understood in seconds.

---

# 78. Launch positioning

Recommended description:

> **Flagpick is an interactive `--help` for every CLI.**
>
> Start typing a command, press a key, search its flags and descriptions, and insert the result back into your shell. Local, offline, no AI, no command execution.

Avoid positioning as:

- “AI shell assistant,”
- “terminal copilot,”
- “new shell,”
- “command cheat sheet.”

Those categories obscure the narrow value.

---

# 79. Open questions to resolve during prototype

1. Which default keybinding has the fewest real-world conflicts?
2. Can Bash/Zsh multiline cursor positions be losslessly mapped with Unicode across supported versions?
3. How much generic Clap/Cobra/Click detection can be achieved without brittle framework fingerprints?
4. Which local completion metadata can be reused safely without evaluating scripts?
5. For Git, is concise `-h` sufficient for most subcommands or should man output supplement it?
6. How should FFmpeg option placement be modeled?
7. What exact no-exec discovery sources are available on macOS/Linux?
8. Should the cache be one JSON file per command node or an embedded database? Recommendation: files first; avoid SQLite unless evidence demands it.
9. Does MCP belong in the primary binary behind a feature, or a separate `flagpick-mcp` executable?
10. How should aliases be structurally resolved without introducing shell evaluation?

---

# 80. Recommended first implementation plan

## Phase A — prove the shell UX

Build:

- `flagpick shell-edit zsh`;
- tiny hardcoded option list;
- Ratatui picker;
- buffer round-trip;
- cancel.

Success:

```text
partial shell line → Ctrl-G → choose item → exact buffer updated
```

Do this before writing a universal parser.

---

## Phase B — build schema/discovery core

Implement:

- `CommandSpec`;
- generic probe runner;
- timeouts;
- stdout/stderr capture;
- ANSI sanitization;
- generic option parser;
- fixture tests;
- cache.

---

## Phase C — make five flagship demos excellent

Prioritize:

```text
git
curl
ffmpeg
docker
kubectl
```

These collectively exercise most difficult patterns and make strong demos.

---

## Phase D — distribution

Ship:

- macOS arm64;
- macOS x86_64;
- Linux x86_64;
- Linux arm64;
- Homebrew tap;
- Nix flake.

Get installation to one command before adding secondary integrations.

---

## Phase E — agent interface

Stabilize:

```sh
flagpick schema ... --format json
```

Then add:

- OpenCode plugin;
- Codex skill/plugin example;
- Claude Code MCP example;
- optional `flagpick mcp`.

---

# 81. Definition of done for v1

Flagpick v1 is done when:

- a clean macOS/Linux user can install a single native binary through a common package path;
- Zsh/Bash/Fish integration works without `eval` of untrusted data;
- the user's shell text is preserved on cancel and minimally patched on accept;
- common CLI help conventions are handled;
- unusual conventions have explicit adapters;
- real compatibility tests cover at least 20 important CLIs;
- help probes are bounded and transparent;
- ANSI/control-sequence injection is neutralized;
- generated commands are never automatically executed;
- `flagpick schema` provides stable structured output;
- agent integrations can inspect CLI syntax without bypassing their own execution permissions;
- cached invocation is fast enough to feel like shell completion;
- no account, daemon, runtime interpreter, or network service is required.

---

# 82. Product rule worth protecting

If future feature requests threaten the simplicity of the product, return to this rule:

> **Flagpick helps you construct a command. Your shell runs it.**

That boundary keeps the utility small, understandable, local, and trustworthy.

---

# 83. Research references

Pain-point and UX references:

- Ask HN — large and variable command lines:  
  https://news.ycombinator.com/item?id=40142791
- Reddit — forgetting shell flags/syntax:  
  https://www.reddit.com/r/devops/comments/1cetuur/remembering_shell_commands_that_you_are_not/
- Reddit — contextual CLI explanation discussion:  
  https://www.reddit.com/r/commandline/comments/ixzr2i
- GitHub — cmux large CLI/help wall:  
  https://github.com/manaflow-ai/cmux/issues/6616
- GitHub — Claude Code shell completion request:  
  https://github.com/anthropics/claude-code/issues/40503
- GitHub — Claude Code command discoverability:  
  https://github.com/anthropics/claude-code/issues/40538
- GitHub — quoting values with spaces:  
  https://github.com/cli/cli/issues/756

CLI behavior references:

- Git help:  
  https://git-scm.com/docs/git  
  https://git-scm.com/docs/git-help  
  https://git-scm.com/book/en/v2/Getting-Started-Getting-Help.html
- Docker CLI help:  
  https://docs.docker.com/reference/cli/docker/
- Kubectl:  
  https://kubernetes.io/docs/reference/kubectl/generated/kubectl/
- FFmpeg:  
  https://ffmpeg.org/ffmpeg.html
- curl:  
  https://curl.se/docs/manpage.html
- OpenSSL:  
  https://docs.openssl.org/3.4/man1/openssl/

Agent integration references:

- OpenCode plugins:  
  https://opencode.ai/v2/docs/plugins
- OpenCode CLI plugins:  
  https://opencode.ai/v2/docs/cli/plugins
- OpenCode custom commands:  
  https://dev.opencode.ai/docs/commands/
- Codex CLI/customization:  
  https://developers.openai.com/docs/codex/cli  
  https://developers.openai.com/docs/customization/overview  
  https://developers.openai.com/docs/build-skills
- Claude Code CLI/MCP:  
  https://docs.anthropic.com/en/docs/claude-code/cli-usage  
  https://docs.anthropic.com/en/docs/claude-code/mcp

---

# 84. One-sentence specification

**Flagpick is a dependency-free local Rust utility that safely discovers the argument surface of an installed CLI, presents it as a searchable transient TUI, and minimally edits the user's existing shell buffer without executing the resulting command.**
