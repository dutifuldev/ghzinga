---
title: GitHub CLI Reference Notes
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# GitHub CLI Reference Notes

These notes record how `ghzinga` should use the local GitHub CLI checkout as a
reference without turning GitHub CLI into the data transport.

Reference checkout:

- Source: `https://github.com/cli/cli.git`
- Local path: `/home/bob/repos/gh-cli`
- Branch inspected: `trunk`
- Commit inspected: `9a2f33078`

## Boundary

`ghzinga` fetches PR and issue data with direct GitHub API calls. It must not run
`gh pr view`, `gh issue view`, or `gh api` to gather resource data.

Allowed GitHub CLI use:

- `gh auth token` as a credential fallback after `GH_TOKEN` and `GITHUB_TOKEN`
- `gh auth status` / `gh auth login` in human-facing recovery text

This keeps the app standalone, mockable, and independent from the output shape
of another CLI.

## Patterns To Borrow

### Auth Resolution

Reference: `/home/bob/repos/gh-cli/internal/config/config.go`

GitHub CLI's `AuthConfig.ActiveToken` resolves credentials in layers:

- explicit token sources first
- persisted host config next
- keyring-backed credentials last

`ghzinga` uses a smaller version of the same idea:

- `GH_TOKEN`
- `GITHUB_TOKEN`
- `gh auth token`

The difference is intentional. `ghzinga` should not read or migrate GitHub CLI
config internals directly; the CLI remains the owner of its stored credential
format.

### HTTP Transport

Reference: `/home/bob/repos/gh-cli/api/http_client.go`

GitHub CLI builds an HTTP client with default GitHub API headers, then wraps its
transport to attach the active token per request. The useful design idea for
`ghzinga` is the separation between request construction and credential
attachment.

`ghzinga` mirrors that with an internal transport boundary:

- GraphQL requests are POSTs to `https://api.github.com/graphql`
- REST enrichment requests are GETs under `https://api.github.com`
- each request records method, URL, accept header, token, and optional body
- tests can replace the transport without spawning processes

`ghzinga` currently uses `reqwest` for the concrete transport and keeps the
request shape testable before JSON normalization runs.

### API Header Shape

Reference: `/home/bob/repos/gh-cli/api/http_client.go`

The GitHub CLI sends a stable user agent and GitHub API headers through its
client setup. `ghzinga` should keep doing the same in Rust:

- stable `User-Agent`
- GitHub JSON accept header for REST and GraphQL JSON
- diff media type only for pull-request file patches
- bearer authorization attached by the direct HTTP layer

Future support for GitHub Enterprise should extend this boundary by making the
API base URL host-aware rather than shelling out to `gh api`.

### HTTP Tests

Reference: `/home/bob/repos/gh-cli/pkg/httpmock/stub.go`

GitHub CLI tests API commands by matching HTTP method, path, GraphQL body, query
parameters, and response payloads. The equivalent `ghzinga` rule is:

- adapter tests assert direct HTTP request shape
- adapter tests feed fixture GraphQL/REST responses
- architecture tests block regressions to `gh pr view`, `gh issue view`, and
  `gh api`

This is already represented in `tests/architecture.rs`, which permits only the
`gh auth token` shell-out in the GitHub data adapter.

### Browser Abstraction

Reference: `/home/bob/repos/gh-cli/internal/browser/browser.go`

GitHub CLI hides browser launching behind a tiny interface and tests callers
with a stub. `ghzinga` follows the same boundary without shelling out to `gh` for
browser opening:

- route URL opening through a small adapter
- keep render and input layers unaware of process spawning
- test clicked URL targets with a stub, not a real browser

## Patterns To Avoid

Do not copy these parts of GitHub CLI into `ghzinga`:

- the full command factory and command tree
- interactive auth flows
- config migration or keyring access
- extension/plugin machinery
- generic `gh api` command behavior
- Go-specific HTTP client abstractions that do not map cleanly to Rust

`ghzinga` is a focused TUI for one resource. The reference value of `cli/cli` is
its mature API/auth/test boundaries, not its broad command architecture.

## Documentation Implications

When documenting GitHub data access, use precise language:

- say "direct GitHub API calls" for PR/issue data
- say "`gh auth token` credential fallback" for GitHub CLI integration
- avoid saying "gh CLI gateway" as if GitHub CLI owns data fetching

The direct GitHub adapter now keeps token resolution in `src/github/auth.rs`,
HTTP request execution in `src/github/transport.rs`, GraphQL query text in
`src/github/queries.rs`, and resource orchestration and normalization in
`src/github/api.rs`. A later code cleanup can split the remaining normalization
code further without changing the product behavior.
