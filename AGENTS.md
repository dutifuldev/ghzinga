# AGENTS.md

These instructions apply to this repository.

## Local Checks

Run these before finishing code changes:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
npx -y @simpledoc/simpledoc check
python3 captures/ghzinga-pr-81834/capture_ghzinga.py --validate-only
python3 captures/ghzinga-pr-81834/capture_ghzinga.py --root captures/ghzinga-issue-88499 --mode issue --validate-only
python3 captures/ghzinga-pr-81834/capture_mouse_smoke.py --validate-only
```

If app rendering, fixtures, `Cargo.toml`, or `Cargo.lock` change, regenerate the
capture artifacts after committing the source/docs change, then commit the
capture refresh separately.

## Architecture Boundaries

- Keep `src/domain` pure: no app, GitHub, TUI, terminal, filesystem, process, or
  network dependencies.
- Keep `src/github` as the data adapter: it may use HTTP and auth, but it must
  not depend on Ratatui, Crossterm, input, render, terminal, or app layers.
- Keep `src/render` as TUI rendering: it may depend on domain/app state and hit
  targets, but it must not call GitHub, terminal setup, network, filesystem, or
  process APIs.
- Keep `src/input` tiny: hit areas and hit targets only, with no terminal event
  loop or external adapter logic.
- Keep `src/terminal` as the terminal adapter only.
- `src/main.rs` is the orchestration edge where terminal, GitHub loading, and
  app update/render loops are wired together.

The architecture tests in `tests/architecture.rs` enforce the important import
boundaries. Add tests there when a new boundary matters.

## GitHub Data

Use direct GitHub API calls for data. The installed `gh` CLI may only be used as
a credential fallback through `gh auth token`; do not use `gh pr view`,
`gh issue view`, or `gh api` as the data transport.

## Rust Style

- Prefer small modules with explicit typed data at boundaries.
- Add or update the nearest behavior tests for changes.
- Keep external JSON and terminal behavior validated at adapter boundaries,
  then convert into typed domain data.
- Do not add dependencies unless they remove real complexity.
