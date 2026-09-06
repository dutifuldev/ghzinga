---
title: Make Herdr link actions survive stale executable paths
author: Onur Solmaz <2453968+osolmaz@users.noreply.github.com>
date: 2026-09-06
---

# Make Herdr link actions survive stale executable paths

A Herdr link action can keep an old `HERDR_BIN_PATH` after the executable at that path is replaced. Ghzinga must still open clicked GitHub links without changing or restarting Herdr.

## Requirements

- Try the command named by `HERDR_BIN_PATH` first.
- If that command cannot start because it is not found, retry with `herdr` from `PATH`.
- Preserve `HERDR_SOCKET_PATH` and the rest of the plugin environment during the retry.
- Do not retry after the configured command starts, exits with a nonzero status, or fails to start for another reason. This prevents a mutating command from running twice.
- Pass the configured path to the operating system unchanged. Do not special-case or remove a literal `(deleted)` suffix.

## Scope

Use one Ghzinga-owned Herdr command runner for pane lookup, pane focus, and pane creation. The runner will make the fallback decision from the process spawn result.

Update the Herdr plugin documentation to describe the fallback and its safety boundary.

## Non-goals

- Do not update, restart, stop, hand off, or modify Herdr.
- Do not add compatibility behavior to Herdr.
- Do not retry a command based on its exit status or output.
- Do not change Ghzinga session or pane reuse behavior.

## Acceptance criteria

- A valid configured Herdr path is used without a fallback.
- A missing or stale configured path falls back to `herdr` from `PATH`.
- A missing fallback command returns an error.
- A configured command that starts and fails is run once.
- Existing link action, pane reuse, and viewer tests continue to pass.

## Verification

Run:

```sh
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo llvm-cov --fail-under-lines 85 --summary-only
cargo audit
plugins/herdr/test/test-open.sh
plugins/herdr/test/test-viewer.sh
scripts/ci-local.sh
```

Use only the isolated named session created by `scripts/herdr-plugin-live-smoke.sh` for a live smoke test. Do not affect the active default Herdr session.
