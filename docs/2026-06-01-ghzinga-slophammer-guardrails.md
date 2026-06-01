---
title: ghzinga Slophammer Guardrails
author: Bob <dutifulbob@gmail.com>
date: 2026-06-01
---

# ghzinga Slophammer Guardrails

The original goal asks for code quality inspired by `dutifuldev/slophammer` and
Uncle Bob conventions. `slophammer` does not currently ship a Rust checker, so
`ghzinga` applies the documented standards manually and enforces the parts that
fit this Rust TUI through local tests and CI.

## Source Patterns Reviewed

The relevant Slophammer references are:

- `/home/bob/repos/slophammer/AGENTS.md`
- `/home/bob/repos/slophammer/docs/AGENT_ENTRYPOINT.md`
- `/home/bob/repos/slophammer/docs/IMPLEMENTATION_MODEL.md`
- `/home/bob/repos/slophammer/docs/STRUCTURAL_REVIEW.md`
- `/home/bob/repos/slophammer/docs/DEPENDENCY_CHECKS.md`
- `/home/bob/repos/slophammer/docs/UNCLE_BOB_CONCEPTS.md`

The transferable standards are:

- keep business/domain behavior away from IO and framework plumbing
- isolate external systems behind adapter boundaries
- make dependency direction executable, not just documented
- prefer small typed modules and behavior-named tests
- run fast local checks and the same checks in CI
- keep agent instructions short, concrete, and enforceable

## ghzinga Mapping

`ghzinga` maps those standards onto the Rust TUI like this:

```text
src/main.rs
  -> orchestration edge: terminal loop, async loading, refresh, open-url command

src/app/
  -> state and event reducer; no concrete GitHub or terminal adapter calls

src/domain/
  -> typed PR/issue model and parsing; no IO, TUI, network, or process code

src/github/
  -> direct GitHub API adapter, public REST fallback, auth token fallback, DTO normalization

src/render/
  -> Ratatui rendering and hit-area registration; no network, process, or fs IO

src/input/
  -> hit targets and hit testing only

src/terminal/
  -> Crossterm/Ratatui terminal setup and teardown only
```

The current dependency direction is:

```text
main -> app/domain/github/render/input/terminal/config
app -> domain/input/render/config
render -> app/domain/input
github -> domain
input -> app/domain
terminal -> no product layer
domain -> no product layer
```

This is deliberately not a purist clean-architecture diagram. A TUI renderer
needs app view state and hit targets. The important Slophammer rule is that
domain and policy stay away from concrete IO, while external adapters do not
reach back into TUI layers.

## Executable Guardrails

`tests/architecture.rs` now checks:

- `src/domain` has no app, GitHub, input, render, terminal, TUI, network,
  process, or filesystem dependencies
- `src/github` has no app, input, render, terminal, Ratatui, or Crossterm
  dependency
- `src/render` has no GitHub, terminal, network, process, or filesystem
  dependency
- `src/input` stays small and adapter-free
- `src/app` does not call concrete GitHub, terminal, network, or process
  adapters
- `src/terminal` stays out of domain, data, input, render, and app layers
- GitHub data transport does not regress to `gh pr view`, `gh issue view`, or
  `gh api`; the only allowed `gh` use in the GitHub adapter is `gh auth token`
- unauthenticated public REST fallback code lives behind the GitHub adapter
  boundary, not in the runner or renderer
- `TerminalGuard` must be safe during partial setup: raw mode, alternate screen,
  and mouse capture are tracked independently so a setup error unwinds through
  the same restoration path as a normal exit

CI runs these checks through `cargo test`.

## Repository Guardrails

The repo-local `AGENTS.md` documents the checks agents must run and the boundary
rules they must preserve. The CI workflow runs the same core checks:

- `cargo fmt --check`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `npx -y @simpledoc/simpledoc check`
- PR, issue, and mouse-smoke capture validators

That gives the project the Slophammer shape even without a Rust-specific
Slophammer executable: the rules are written down, executable where possible,
and part of the normal review gate.
