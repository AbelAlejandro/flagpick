# Flagpick

Flagpick is a local, offline utility for interactive shell command editing.  
1. The shell supplies the current buffer on stdin.  
2. Flagpick opens its TUI on `/dev/tty`.  
3. Up/Down navigate available options.  
4. Typing searches the available options.  
5. Enter confirms a replacement; it is the only stdout output.  
6. The shell runs the constructed command; Flagpick never executes it.

## Current status

The current shell-edit command is:

`flagpick shell-edit <zsh|bash> --cursor <character-position> [--at-cursor]`

Flagpick discovers installed command help through bounded direct-argv probes, caches validated schemas, and presents discovered options in the picker. Git, curl, FFmpeg, Docker, and kubectl have specialized probe paths for representative workflows.

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

## Bash setup

From the repository root, put the development binary on `PATH` and source the Readline integration:

```bash
export PATH="$PWD/target/debug:$PATH"
source integrations/bash/flagpick.bash
```

Press **Ctrl-G** to invoke `_flagpick_widget` in an interactive Bash session.

## Nix

Run the current source directly with Nix:

```sh
nix run github:AbelAlejandro/flagpick
```

The flake exposes the default package and app for Linux and macOS on x86_64 and arm64:

```sh
nix build github:AbelAlejandro/flagpick
nix run .
```

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
bash tests/bash-integration.bash
zsh -f tests/zsh-integration.zsh
npm run secrets
npm run risk -- origin/main
```

## Limitations

This is not completed v1 or production-ready software. Use `flagpick inspect <command...>` for JSON diagnostics or `flagpick schema <command...> --format json` for the canonical schema. The public path accepts registered executable/subcommand paths and invokes only bounded help probes; it never forwards final-command arguments. `--no-exec-probe` permits cache-only inspection. Fish support, packaging, and releases remain unavailable.

The Zsh adapter currently assumes a UTF-8 locale when mapping ZLE character positions. Normal errors and cancellation restore the terminal; after an uncatchable termination such as `SIGKILL`, run `reset` if the terminal remains in raw or alternate-screen state.

## Project links

- [Linear work item](https://linear.app/comply-finops/issue/COM-62/deliver-flagpick-v1-from-the-approved-product-specification)
- [Approved product brief](flagpick-product-engineering-spec.md)

## License

Flagpick is licensed under the dual MIT/Apache-2.0 license.
