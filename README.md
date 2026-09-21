# Flagpick

Flagpick is a local, offline prototype for interactive shell command editing.  
1. The shell supplies the current buffer on stdin.  
2. Flagpick opens its TUI on `/dev/tty`.  
3. Up/Down navigate available options.  
4. Typing searches the available options.  
5. Enter confirms a replacement; it is the only stdout output.  
6. The shell runs the constructed command; Flagpick never executes it.

## Phase A prototype

Phase A proves the shell interaction before universal help discovery. The current command is:

`flagpick shell-edit zsh --cursor <character-position> [--at-cursor]`

The prototype options are hardcoded and currently limited to `--help`, `--verbose`, and `--version`.

## Build

```sh
cargo build
```

## Zsh setup

From the repository root, put the development binary on `PATH` and source the integration:

```zsh
export PATH="$PWD/target/debug:$PATH"
source integrations/zsh/flagpick.zsh
```

Press **Ctrl-G** to invoke `flagpick-widget`. `flagpick-widget-at-cursor` is registered but not bound.

The wrapper preserves cancel and error buffers, preserves output-ending newlines with a sentinel, and uses no `eval`.

## Controls

- **Up/Down**: navigate
- **Typing**: search
- **Enter**: confirm
- **Esc** or **Ctrl-C**: cancel

## Safety boundary

Flagpick is local and offline. It requires no account and does not execute the completed command. It constructs a command; the shell runs it.

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
expect -f tests/tui.expect
zsh -f tests/zsh-integration.zsh
npm run secrets
npm run risk -- origin/main
```

## Limitations

This is not completed v1 or production-ready software. The [`flagpick::schema` contract](docs/schema.md), bounded [`flagpick::probe` runner](docs/probing.md), generic help parser, bounded schema cache, and public discovery commands now exist as Phase B foundations. Use `flagpick inspect <command...>` for JSON diagnostics or `flagpick schema <command...> --format json` for the canonical schema. `--no-exec-probe` permits cache-only inspection. Docker and Git integration therefore still show the hardcoded prototype options. Schema/TUI integration, Bash or Fish support, packaging, and releases remain unavailable.

The Zsh adapter currently assumes a UTF-8 locale when mapping ZLE character positions. Normal errors and cancellation restore the terminal; after an uncatchable termination such as `SIGKILL`, run `reset` if the terminal remains in raw or alternate-screen state.

## Project links

- [Linear work item](https://linear.app/comply-finops/issue/COM-62/deliver-flagpick-v1-from-the-approved-product-specification)
- [Approved product brief](flagpick-product-engineering-spec.md)

## License

Flagpick is licensed under the dual MIT/Apache-2.0 license.
