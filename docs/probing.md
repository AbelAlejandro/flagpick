# Bounded probing

## API

`flagpick::probe` exposes `ProbeConfig`, `ProbeRequest`, `ProbeRunner`, `ProbeOutput`, `ProbeStatus`, `CapturedOutput`, and `ProbeError`.

Execution is disabled by default. A request requires an explicit absolute executable and nonempty argument elements.

## Bounds and defaults

- Timeout: greater than zero through 30 seconds; default 2 seconds.
- Captured stdout: 1 byte through 16 MiB; default 4 MiB.
- Captured stderr: 1 byte through 16 MiB; default 1 MiB.

## Execution behavior

The runner invokes `std::process::Command` directly with the supplied argv and never uses a shell.

Before execution, the environment is cleared. Only these variables are set:

- `TERM=dumb`
- `NO_COLOR=1`
- `PAGER=cat`
- `GIT_PAGER=cat`
- `MANPAGER=cat`

stdin is null. stdout and stderr are piped and concurrently drained as raw bytes. The configured prefix of each stream is retained; total byte counts and truncation are reported. Nonzero and signaled exits are returned as data.

The overall timeout covers the direct child and inherited pipes. On Unix, the runner starts a new process group, kills that group with `SIGKILL` on timeout, and reaps the direct child. Cancellation-aware pipe readers keep runner completion bounded even if a descendant leaves the group while retaining a pipe.

## Safety boundary

This is bounded execution, not a portable sandbox. Probing executes the target, so a malicious or broken target can have side effects even when given a help-like argv. A descendant that deliberately leaves the process group may survive; bounded reader teardown does not claim universal process containment.

Only explicitly selected absolute paths may be passed. There is no speculative, background, `PATH`, alias, or shell-function probing. The disabled policy is the library basis for a future `--no-exec-probe` option.

Callers should treat both the target and captured bytes as untrusted. Captured output can be non-UTF-8 and truncated.

## Verification

Integration tests probe only their own absolute test executable. They cover disabled and invalid pre-spawn rejection, exact argv, environment and stdin behavior, simultaneous capped binary streams, nonzero exit, signal termination, timeout, Unix process-group descendant termination, and bounded return when a fixture descendant leaves the group while retaining pipes. No installed command is probed.

## Current limits

The public `flagpick inspect <command...>` and `flagpick schema <command...> --format json` commands resolve only the explicitly selected executable and named subcommands, then append `--help` when needed. They reject flags and paths after the executable so arbitrary command arguments are never forwarded to the target. Probe timeout, exit status, byte counts, truncation, and direct-probe provenance are observable in inspect diagnostics; incomplete probes are not cached. Captured bytes are sanitized to UTF-8 text with terminal controls removed before parsing. `--no-exec-probe` skips execution and reads only an existing schema cache entry. There is no allow/deny configuration, specialized help strategy, schema/TUI integration, or real Git/Docker discovery. Arbitrary Windows descendant termination is not established.
