# ghzoom

`ghzoom` is a standalone Rust terminal UI for monitoring one GitHub pull request
or issue.

It uses Ratatui and Crossterm for the TUI, and direct GitHub API calls for PR,
issue, and enrichment data. There is no separate login flow: if `GH_TOKEN` or
`GITHUB_TOKEN` is set, `ghzoom` uses it; otherwise it can reuse the token from an
existing `gh` login through `gh auth token`. The GitHub CLI is only a credential
fallback, not the data transport. If credentials are missing, `ghzoom` reports
the failed credential step and the `gh auth status` / `gh auth login` next step.
The primary PR or issue view is fetched first; optional enrichment failures are
shown as warnings instead of preventing the resource from rendering.

## Install

Build from this checkout:

```sh
cargo build --release
```

Run the debug build during development:

```sh
cargo run -- openclaw/openclaw#81834
```

Run the built binary:

```sh
target/release/ghzoom openclaw/openclaw#81834
```

## Usage

Accepted resource forms:

```sh
ghzoom https://github.com/openclaw/openclaw/pull/81834
ghzoom https://github.com/openclaw/openclaw/issues/88499
ghzoom openclaw/openclaw#81834
ghzoom openclaw/openclaw 81834
```

Useful options:

```sh
ghzoom openclaw/openclaw#81834 --tab checks
ghzoom openclaw/openclaw#81834 --refresh-seconds 30
ghzoom openclaw/openclaw#81834 --no-mouse
ghzoom openclaw/openclaw#81834 --once
ghzoom openclaw/openclaw#81834 --offline-fixture fixtures/pr-81834.json
```

`--tab` accepts `overview`, `activity`, `commits`, `checks`, `files`, and
`links`. Issue views only show `overview`, `activity`, and `links`.

## What It Shows

For pull requests:

- body, labels, reactions, author, state, base/head branches
- assignees and requested reviewers
- GitHub metadata such as draft/cross-repository state, mergeability,
  changed-file count, milestones, projects, ref OIDs, and merge commits where
  available
- comments, reviews, review comments, and timeline events such as labels,
  references, assignments, locks, pins, duplicate markers, transfers, review
  requests, draft/ready state, branch changes, force-pushes, merge queue
  changes, review dismissals, auto-merge/rebase/squash changes, automatic base
  changes, merges, title changes, milestones, issue types, sub-issues, parent
  issues, blocking relationships, and converted discussions; comments, timeline
  events, review threads, and review-thread comments are paginated so long
  histories are not capped at the first page
- comment/review author association, edit/minimized flags, reactions,
  permalinks, and review-thread resolved/outdated state when GitHub exposes it
- unresolved and outdated review-thread counts in the PR status summary
- commits, with expandable commit bodies, authored/committed dates, coauthor
  lists, and deployment/environment status where available
- CI/check status grouped by state, including paginated suite-level workflow
  status, GitHub Actions check runs, and legacy status contexts, with
  status/conclusion, timestamps, and details URLs on expanded check rows
- changed files, with separately expandable patch context when a file row is expanded
- detected issue/PR links, including GitHub relationship links

For issues:

- body, labels, reactions, assignees, author, and state
- GitHub metadata such as pinned state, state reason, closed time, milestones,
  and projects where available
- comments and timeline events such as labels, references, assignments, title
  changes, locks, pins, duplicate markers, transfers, milestones, issue types,
  sub-issues, parent issues, blocking relationships, and converted discussions;
  comments and timeline events are paginated so long histories are not capped at
  the first page
- comment author association, edit/minimized flags, reactions, and permalinks
- detected issue/PR links, including GitHub relationship links

Long body text, comments, checks, and files are truncated by default where
needed. Use the visible `[more]` and `[less]` controls to expand or collapse
content.

## Controls

Mouse:

- click tabs to switch views
- click `[more]` and `[less]` controls to expand or collapse content
- click GitHub issue/PR references to navigate
- click exact GitHub URLs, such as check runs, deployment logs, and comment permalinks, to open them in the browser
- click `[refresh]`, `[open]`, `[help]`, and `[quit]`
- use the mouse wheel to scroll

Keyboard:

- `q`: quit
- `?`: toggle help
- `r`: refresh now
- `o`: open the current resource in the browser through `gh`
- `Tab`, `Shift+Tab`, `Left`, `Right`: switch tabs
- `Up`, `Down`, `PageUp`, `PageDown`, `Home`, `End`: scroll
- `Enter`: activate the first visible content action, such as a link or `[more]`
- `e`: expand or collapse the main body
- `Backspace`: go back after following a linked issue or PR

The shortcuts avoid tmux prefix bindings. `Right` and `Left` are reliable
fallbacks when a terminal or multiplexer encodes Tab unusually.

## Refresh

Live GitHub mode refreshes automatically every 60 seconds by default. Change the
interval with `--refresh-seconds`; use `0` to disable automatic refresh. Manual
refresh is always available with `r` or the `[refresh]` footer control.

The status panel shows the last refresh time and whether the fetched resource
changed. Change detection includes comment/review bodies and review-thread
state, not just top-level PR or issue fields. When a refresh changes data, the
status panel lists the changed surfaces, such as `activity`, `checks`, `files`,
or `commits`. If an optional enrichment call fails, the status and overview
areas show a warning while keeping the base resource visible.

## Verification

Run the normal local checks:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
npx -y @simpledoc/simpledoc check
```

The repository includes tmux capture artifacts for PR and issue views:

- `captures/ghzoom-pr-81834/`
- `captures/ghzoom-issue-88499/`

Regenerate PR captures:

```sh
python3 captures/ghzoom-pr-81834/capture_ghzoom.py
```

Regenerate issue captures:

```sh
python3 captures/ghzoom-pr-81834/capture_ghzoom.py \
  --root captures/ghzoom-issue-88499 \
  --target https://github.com/openclaw/openclaw/issues/88499 \
  --title 'openai-responses provider: 404 on previous_response_id when store=false (default)' \
  --load-needle openai-responses \
  --mode issue
```
