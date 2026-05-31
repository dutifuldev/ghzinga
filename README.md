# ghzinga

`ghzinga` is a standalone Rust terminal UI for monitoring one GitHub pull request
or issue.

It uses Ratatui and Crossterm for the TUI, and direct GitHub API calls for PR,
issue, and enrichment data. There is no separate login flow: if `GH_TOKEN` or
`GITHUB_TOKEN` is set, `ghzinga` uses it; otherwise it can reuse the token from an
existing `gh` login through `gh auth token`. The GitHub CLI is only a credential
fallback, not the data transport. When credentials are unavailable, public
repositories fall back to an unauthenticated REST view with warnings for richer
GraphQL-only metadata; public REST comments, PR commits, and changed files are
paginated until GitHub returns a short page. Private repositories still need a
token or existing `gh` login. The primary PR or issue view is fetched first;
optional enrichment failures are shown as warnings instead of preventing the
resource from rendering.
Architecture guardrails are documented in
`docs/2026-06-01-ghzinga-slophammer-guardrails.md` and enforced by
`tests/architecture.rs`.

## Install

Build from this checkout:

```sh
cargo build --release
```

Run the debug build during development:

```sh
cargo run --bin gzg -- openclaw/openclaw#81834
```

Run the built binary:

```sh
target/release/gzg openclaw/openclaw#81834
```

## Usage

Accepted resource forms:

```sh
gzg https://github.com/openclaw/openclaw/pull/81834
gzg https://github.com/openclaw/openclaw/issues/88499
gzg openclaw/openclaw#81834
gzg openclaw/openclaw 81834
```

Useful options:

```sh
gzg openclaw/openclaw#81834 --tab checks
gzg openclaw/openclaw#81834 --refresh-seconds 30
gzg openclaw/openclaw#81834 --no-mouse
gzg openclaw/openclaw#81834 --theme solarized-dark
gzg openclaw/openclaw#81834 --symbols emoji
gzg openclaw/openclaw#81834 --spacing compact
gzg openclaw/openclaw#81834 --once
gzg openclaw/openclaw#81834 --offline-fixture fixtures/pr-81834.json
```

`--tab` accepts `overview`, `activity`, `commits`, `checks`, `files`, and
`links`. Issue views only show `overview`, `activity`, and `links`. `--theme`
accepts `default` and `solarized-dark`. `--symbols` accepts `ascii` and
`emoji`. `--spacing` accepts `comfortable` and `compact`, similar to Gmail's
density setting. Comfortable is the default and adds gh-dash-like breathing room
between repeated rows, a small content gutter, and hanging indents for wrapped
long lines; compact keeps more rows visible in small terminals. CLI theme,
symbol, and spacing flags override saved config for that run only.

## Configuration

Ghzinga reads a small TOML config file:

```text
~/.config/ghzinga/config.toml
```

When `XDG_CONFIG_HOME` is set, the path is
`$XDG_CONFIG_HOME/ghzinga/config.toml`. `GZG_CONFIG_PATH` can point at a
specific file for tests, scripts, or dotfile setups.

Default config:

```toml
[ui]
theme = "default"
symbols = "ascii"
spacing = "comfortable"
```

The app works without a config file. Invalid known values fall back to safe
defaults and show a warning in the status band. Unknown fields are ignored so
future config additions do not break older files.

Open settings inside the TUI with `s` or the footer `[settings]` control. Theme,
symbol, and spacing changes apply live and are saved back to `config.toml`;
write errors are shown in the status band without crashing the app.

## What It Shows

For pull requests:

- body, labels, reactions, author, state, base/head branches
- Overview starts with a GitHub-style chronological conversation timeline:
  opening body, commits, reviews, review comments, commit comments, regular
  comments, and timeline events are interleaved by timestamp instead of split
  into separate summary blocks first
- paginated labels, assignees, and requested reviewers
- GitHub metadata such as draft/cross-repository state, mergeability,
  changed-file count, milestones, projects, ref OIDs, and merge commits where
  available
- comments, reviews, review comments, and timeline events such as labels,
  references, assignments, locks, pins, duplicate markers, transfers, review
  requests, draft/ready state, branch changes, force-pushes, merge queue
  changes, review dismissals, auto-merge/rebase/squash changes, automatic base
  changes, merges, title changes, milestones, projects, project-v2 statuses,
  issue types, issue fields, sub-issues, parent issues, blocking relationships,
  user blocks, converted project notes, converted draft items, converted
  discussions, revision markers, and deployment events; comments, timeline
  events, reviews, review threads, review-thread comments, commit comment
  threads, and nested commit comments are paginated so long histories are not
  capped at the first page
- comment/review author association, edit/minimized flags, reactions,
  permalinks, commit-comment path/position, and review-thread resolved/outdated
  state when GitHub exposes it
- unresolved and outdated review-thread counts in the PR status summary
- paginated commits, with expandable commit bodies, authored/committed dates,
  paginated coauthor lists, and paginated deployment/environment lists where
  available
- paginated CI/check status grouped by state, including suite-level workflow
  status, GitHub Actions check runs, and legacy status contexts, with
  status/conclusion, timestamps, and details URLs on expanded check rows
- changed files, with gh-dash-style file summary rows and separately expandable
  in-TUI patch context when a file row is expanded; patch additions render green,
  deletions render red, and hunk headers use an accent color by default
- detected issue/PR links, including paginated GitHub relationship links

For issues:

- body, paginated labels, reactions, paginated assignees, author, and state
- GitHub metadata such as pinned state, state reason, closed time, milestones,
  and projects where available
- comments and timeline events such as labels, references, assignments, title
  changes, locks, pins, duplicate markers, transfers, milestones, issue types,
  issue fields, projects, project-v2 statuses, sub-issues, parent issues,
  blocking relationships, user blocks, converted project notes, converted draft
  items, and converted discussions; comments and timeline events are paginated so
  long histories are not capped at the first page
- comment author association, edit/minimized flags, reactions, and permalinks
- detected issue/PR links, including paginated GitHub relationship links

Long body text, comments, checks, and files are truncated by default where
needed. Use the visible `[+ more]` and `[- less]` controls to expand or collapse
content, or `[expand all]` and `[collapse all]` to open or fold every expandable
row in the current tab. The rendered content window only registers hit targets
for the visible rows, so long paginated GitHub histories remain scrollable
without turning every off-screen row into an active terminal target.

The TUI adapts to terminal width. Header metadata, tabs, the status band, and
footer controls wrap into extra rows on narrow terminals instead of silently
overlapping. Long content uses display-width-aware wrapping and truncation, so
emoji and wide characters do not corrupt the layout.

The footer shows the active tab and scroll position as current row, maximum row,
and percentage, so long PR conversations and diff views keep the same quick
orientation cue as a gh-dash preview pane.

By default, ghzinga renders with plain ASCII symbols so it works in terminals
without special fonts or emoji support. Use `--symbols emoji` to opt into the
richer emoji labels.

## Controls

Mouse:

- click tabs to switch views
- click bold `[+ more]` and `[- less]` controls to expand or collapse content
- click bold `[expand all]` and `[collapse all]` controls to expand or collapse
  the current tab
- click file rows in the Files tab to expand or collapse per-file details, then
  click `[+ more patch]` or `[- less patch]` to reveal or fold long diffs
- click GitHub issue/PR references to navigate
- click exact GitHub URLs, such as check runs, deployment logs, and comment permalinks, to open them in the browser
- click `[refresh]`, `[open]`, `[settings]`, `[help]`, and `[quit]`
- use the mouse wheel to scroll

Keyboard:

- `q`: quit
- `?`: toggle help
- `s`: open or close settings
- `t` / `y` / `p` while settings are open: cycle theme / symbol style / spacing
- `r`: refresh now
- `o`: open the current resource URL in the browser
- `Tab`, `Shift+Tab`, `Left`, `Right`: switch tabs
- `Up`, `Down`, `PageUp`, `PageDown`, `Home`, `End`: scroll
- `Enter`: activate the first visible content action, such as a link or `[+ more]`
- `e`: expand or collapse the main body
- `Backspace`: go back after following a linked issue or PR

The shortcuts avoid tmux prefix bindings. `Right` and `Left` are reliable
fallbacks when a terminal or multiplexer encodes Tab unusually.

## Refresh

Live GitHub mode refreshes automatically every 60 seconds by default. Change the
interval with `--refresh-seconds`; use `0` to disable automatic refresh. Manual
refresh is always available with `r` or the `[refresh]` footer control.

The horizontal status band shows the last refresh time and whether the fetched
resource changed. Change detection includes comment/review bodies and
review-thread state, not just top-level PR or issue fields. When a refresh
changes data, the status band lists the changed surfaces, such as `activity`,
`checks`, `files`, or `commits`. If an optional enrichment call fails, the
status and overview areas show a warning while keeping the base resource
visible.

Refresh, linked-resource navigation, and Backspace navigation run as background
GitHub fetches. The previous resource stays readable while the status band and
footer show a terminal-safe loading marker such as `Loading |: ...`; duplicate
fetch starts are ignored until the current one finishes, so rapid clicks or
auto-refresh ticks do not build a request queue.

## Verification

Run the normal local checks:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
npx -y @simpledoc/simpledoc check
```

GitHub Actions runs these same checks for pull requests and pushes to `main`.
It also runs the saved PR and issue capture validators so checked-in UX evidence
cannot silently drift behind the app rendering code. A tmux mouse-smoke capture
validator verifies that real terminal mouse clicks can switch to Files, expand
all rows, collapse them again, switch to Links, and activate a linked issue row.

The repository includes tmux capture artifacts for PR and issue views:

- `captures/ghzinga-pr-81834/`
- `captures/ghzinga-issue-88499/`

Reference docs:

- `docs/2026-05-31-ghzinga-implementation-plan.md`
- `docs/2026-05-31-gh-cli-reference-notes.md`

Regenerate PR captures:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py
```

Validate saved PR captures:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py --validate-only
```

Regenerate issue captures:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py \
  --root captures/ghzinga-issue-88499 \
  --target https://github.com/openclaw/openclaw/issues/88499 \
  --title 'openai-responses provider: 404 on previous_response_id when store=false (default)' \
  --load-needle openai-responses \
  --mode issue
```

Validate saved issue captures:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py \
  --root captures/ghzinga-issue-88499 \
  --mode issue \
  --validate-only
```

Regenerate mouse smoke captures:

```sh
python3 captures/ghzinga-pr-81834/capture_mouse_smoke.py
```

Validate saved mouse smoke captures:

```sh
python3 captures/ghzinga-pr-81834/capture_mouse_smoke.py --validate-only
```
