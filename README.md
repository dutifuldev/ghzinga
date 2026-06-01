# ghzinga

`ghzinga` is a standalone Rust terminal UI for monitoring one GitHub pull request
or issue.

It uses Ratatui and Crossterm for the TUI, and direct GitHub API calls for PR,
issue, and enrichment data. There is no separate login flow: if `GH_TOKEN` or
`GITHUB_TOKEN` is set, `ghzinga` uses it; otherwise it can reuse the token from an
existing `gh` login through `gh auth token`. The GitHub CLI is only a credential
fallback, not the data transport. When credentials are unavailable, clearly
rejected by GitHub, or rate-limited, public repositories fall back to an
unauthenticated REST view with warnings for richer GraphQL-only metadata; public
REST comments, PR commits, PR reviews, PR review comments, changed files,
timeline events, check runs, and status contexts are loaded without auth where
GitHub exposes them publicly. Private
repositories still need a token or existing `gh` login. The primary PR or issue
view is fetched first;
optional enrichment failures are shown as warnings instead of preventing the
resource from rendering.
Architecture guardrails are documented in
`docs/2026-06-01-ghzinga-slophammer-guardrails.md` and enforced by
`tests/architecture.rs`.

## Install

Install from this checkout:

```sh
cargo install --path .
```

That installs both commands:

- `gzg`, the short command
- `ghzinga`, the long command name

Both commands run the same TUI entrypoint. Cargo installs them as two executable
commands. For a real filesystem link, use the repo installer instead:

```sh
scripts/install.sh
```

That installs `gzg` and creates `ghzinga -> gzg` in the install bin directory.
Use `scripts/install.sh --root /path/to/root` to choose a different install
root.

Build without installing:

```sh
cargo build --release
```

Run the debug build during development:

```sh
cargo run --bin gzg -- openclaw/openclaw#81834
cargo run --bin ghzinga -- openclaw/openclaw#81834
```

Run the built binary:

```sh
target/release/gzg openclaw/openclaw#81834
target/release/ghzinga openclaw/openclaw#81834
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
gzg openclaw/openclaw#81834 --api-depth full
gzg openclaw/openclaw#81834 --no-mouse
gzg openclaw/openclaw#81834 --theme solarized-dark
gzg openclaw/openclaw#81834 --symbols emoji
gzg openclaw/openclaw#81834 --spacing compact
gzg openclaw/openclaw#81834 --width-mode full
gzg openclaw/openclaw#81834 --fixed-width 132
gzg openclaw/openclaw#81834 --once
gzg openclaw/openclaw#81834 --offline-fixture fixtures/pr-81834.json
gzg openclaw/openclaw#81834 --offline-fixture fixtures/pr-81834.json --offline-resource-fixture fixtures/issue-66943.json
```

`--tab` accepts `overview`, `activity`, `commits`, `checks`, `files`, and
`links`. Issue views only show `overview`, `activity`, and `links`. `--theme`
accepts Herdr-style built-ins: `default`, `catppuccin`, `catppuccin-latte`,
`terminal`, `tokyo-night`, `tokyo-night-day`, `dracula`, `nord`, `gruvbox`,
`gruvbox-light`, `one-dark`, `one-light`, `solarized`, `solarized-light`,
`kanagawa`, `kanagawa-lotus`, `rose-pine`, `rose-pine-dawn`, and `vesper`.
`solarized-dark` remains an alias for `solarized`. `--symbols` accepts `ascii`
and `emoji`. `--spacing` accepts `comfortable` and `compact`, similar to
Gmail's density setting. Comfortable is the default and adds gh-dash-like
breathing room between repeated rows, a small content gutter, top/bottom chrome
padding, and hanging indents for wrapped long lines; compact keeps more rows
visible in small terminals. `--width-mode` accepts `fixed` and `full`;
`--fixed-width` sets the fixed readable width in columns. Files stay full width
so diffs have room. `--api-depth`
accepts `partial` and `full`. Partial is the default and keeps GraphQL usage
conservative; full follows all supported paginated GraphQL enrichment paths.
CLI theme, symbol, spacing, and width flags override saved config for that run
only.
`--offline-resource-fixture` can be repeated when an offline fixture run needs
click-through navigation to linked issues or PRs without calling GitHub.

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
width_mode = "fixed"
fixed_width = 118
```

The app works without a config file. Invalid known values fall back to safe
defaults and show a warning in the status band. Unknown fields are ignored so
future config additions do not break older files.

Open settings inside the TUI with `s` or the footer `[settings]` control. Theme,
symbol, spacing, width mode, and fixed width changes apply live and are saved
back to `config.toml`; write errors are shown in the status band without
crashing the app.

## What It Shows

For pull requests:

- body, labels, reactions, author, state, base/head branches
- Overview starts with a GitHub-style chronological conversation timeline:
  opening body, commits, reviews, review comments, commit comments, regular
  comments, and timeline events are interleaved by timestamp instead of split
  into separate summary blocks first
- labels, assignees, and requested reviewers from the base GitHub response;
  use `--api-depth full` or `GZG_API_DEPTH=full` to spend extra GraphQL calls on
  exhaustive pagination
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
  discussions, revision markers, and deployment events; timeline events, review
  threads, review-thread comments, commit comment threads, and nested commit
  comments are paginated, while base comments and reviews use the first page by
  default unless `--api-depth full` or `GZG_API_DEPTH=full` is set
- comment/review author association, edit/minimized flags, reactions,
  permalinks, commit-comment path/position, and review-thread resolved/outdated
  state when GitHub exposes it
- unresolved and outdated review-thread counts in the PR status summary
- commits from the base GitHub response, with expandable commit bodies and
  compact human-readable authored/committed dates such as `2w ago`;
  `--api-depth full` or `GZG_API_DEPTH=full` enables
  extra GraphQL calls for exhaustive commit pagination, coauthor pagination, and
  deployment metadata
- paginated CI/check status grouped by state, including suite-level workflow
  status, GitHub Actions check runs, and legacy status contexts, with
  status/conclusion, compact human-readable timestamps, and details URLs on
  expanded check rows; public unauthenticated fallback also shows public check
  runs and status contexts for the PR head commit, while marking GraphQL-only
  suite grouping as unavailable
- changed files, with gh-dash-style file summary rows and separately expandable
  in-TUI patch context when a file row is expanded; patch additions use a green
  background tint, deletions use a red background tint, and hunk headers use an
  accent color by default
- detected issue/PR links, including bare `#123` references, Markdown links, and
  paginated GitHub relationship links

For issues:

- body, labels, reactions, assignees, author, and state from the base GitHub
  response; `--api-depth full` or `GZG_API_DEPTH=full` enables extra GraphQL
  calls for exhaustive label, assignee, and comment pagination
- GitHub metadata such as pinned state, state reason, closed time, milestones,
  and projects where available
- comments and timeline events such as labels, references, assignments, title
  changes, locks, pins, duplicate markers, transfers, milestones, issue types,
  issue fields, projects, project-v2 statuses, sub-issues, parent issues,
  blocking relationships, user blocks, converted project notes, converted draft
  items, and converted discussions; comments and timeline events are paginated so
  long histories are not capped at the first page
- comment author association, edit/minimized flags, reactions, and permalinks
- detected issue/PR links, including bare `#123` references, Markdown links, and
  paginated GitHub relationship links

Long body text, comments, checks, and files are truncated by default where
needed. Use the visible `[+ more]` and `[- less]` controls to expand or collapse
content. The fixed bottom command bar also shows `[expand all]` or
`[collapse all]` after `[refresh]`, `[copy]`, `[open]`, `[settings]`, `[help]`,
and `[quit]` when the current tab has expandable rows; that control opens or
folds every expandable row in the active tab without requiring a scroll to the
bottom of the content. Scroll position stays in the footer message line rather
than between command buttons. The rendered content window only registers hit
targets for the visible rows, so long paginated GitHub histories remain
scrollable without turning every off-screen row into an active terminal target.
If the normal economical API depth sees that GitHub has more pages behind a
first-page collection, the footer also shows `[load full]` before the
expand/collapse-all control; clicking it refetches the current resource with
full supported pagination without restarting the TUI.

The TUI adapts to terminal width. Header metadata, tabs, the status band, and
footer controls wrap into extra rows on narrow terminals instead of silently
overlapping. Long content uses display-width-aware wrapping and truncation, so
emoji and wide characters do not corrupt the layout.

The footer shows the active tab and scroll position as current row, maximum row,
and percentage, so long PR conversations and diff views keep the same quick
orientation cue as a gh-dash preview pane.

When content is scrollable, ghzinga also shows a slim Ratatui scrollbar on the
right edge while you scroll with the keyboard or mouse wheel. The thumb reaches
the bottom at the final scroll position, including comfortable-mode bottom
padding, and endpoint rendering keeps the thumb contiguous at the top and
bottom edges. The scrollbar is transient: it appears during movement, including
edge-scroll attempts, then fades after a few render frames so it does not
permanently take reading space.

By default, ghzinga renders with plain ASCII symbols so it works in terminals
without special fonts or emoji support. Use `--symbols emoji` to opt into the
richer emoji labels.

## Controls

Mouse:

- click tabs to switch views
- click the underlined `owner/repo#123` identity in the header to open the
  current PR or issue on GitHub
- click bold `[+ more]` and `[- less]` controls to expand or collapse content
- click footer `[expand all]` and `[collapse all]` controls at the end of the
  bottom command bar to expand or collapse the current tab
- click file rows in the Files tab to expand or collapse per-file details, then
  click `[+ more patch]` or `[- less patch]` to reveal or fold long diffs
- click the header `owner/repo#number` identity to open the current GitHub
  issue or PR
- click GitHub issue/PR references to navigate
- click exact GitHub URLs, such as check runs, deployment logs, and comment
  permalinks, to open them in the browser; footer `[copy]` and `[open]`
  prefer the first visible URL before falling back to the current PR/issue URL
- click `[refresh]`, `[copy]`, `[open]`, `[settings]`, `[help]`, `[quit]`,
  `[load full]` when shown, and the active-tab expand/collapse control in the
  footer
- use the mouse wheel to scroll

Keyboard:

- `q` or `Ctrl-C`: quit
- `?`: toggle help
- `s`: open or close settings
- `t` / `y` / `p` / `w` while settings are open: cycle theme / symbol style /
  spacing / width mode
- `-` / `+` while settings are open: decrease or increase fixed content width
- `r`: refresh now
- `f`: load full supported GitHub pagination when a partial-depth warning is
  shown
- `y`: copy the first visible GitHub URL, or the current PR/issue URL if no
  visible link is available
- `o`: open the first visible GitHub URL, or the current PR/issue URL if no
  visible link is available
- `Tab`, `Shift+Tab`, `Left`, `Right`: switch tabs
- `1`-`6`: jump to the visible tab in that position. PRs expose Overview,
  Activity, Commits, Checks, Files, Links; issues expose Overview, Activity,
  Links.
- `Up`, `Down`, `PageUp`, `PageDown`, `Home`, `End`: scroll
- `Enter`: activate the first visible content action, such as a link or `[+ more]`
- `e`: expand or collapse the main body
- `a`: expand or collapse all expandable rows in the current tab
- `Backspace`: go back after following a linked issue or PR

The shortcuts avoid tmux prefix bindings. `Right` and `Left` are reliable
fallbacks when a terminal or multiplexer encodes Tab unusually.

## Refresh

Live GitHub mode refreshes automatically every 300 seconds by default. Change the
interval with `--refresh-seconds`; use `0` to disable automatic refresh. Manual
refresh is always available with `r` or the `[refresh]` footer control.
Clicking `[open]` or pressing `o` opens the first visible GitHub URL, such as a
comment permalink, check-run URL, or linked issue/PR. If no visible link is
available, it opens the current PR or issue URL. Clicking `[copy]` or pressing
`y` follows the same visible-link rule and copies the URL instead. Set
`GZG_COPY_COMMAND` to a command that reads clipboard text from stdin when the
default platform clipboard command is not available in tmux, SSH, or headless
sessions.

`ghzinga` checks the GraphQL rate-limit bucket before authenticated GraphQL
requests when its local decision cache is stale. If GraphQL is exhausted, it
skips GraphQL until GitHub's reset time and uses the public REST fallback for
public repositories instead of repeatedly spending failed GraphQL attempts.
Normal mode avoids duplicate first-page GraphQL enrichment; set
`--api-depth full` or `GZG_API_DEPTH=full` only when exhaustive pagination
matters more than quota.
When normal mode sees that a first-page collection has more than 100 items
behind it, the TUI shows a warning naming the partial sections and the full-depth
escape hatch. The same condition enables the footer `[load full]` action and
the `f` shortcut, which run a one-off full-depth refetch for the current
resource while keeping normal startup, auto-refresh, and manual refresh
economical.

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
All direct GitHub HTTP requests reuse the same client and carry a 30-second
per-request timeout, so a bad network path reports through the existing
recoverable error or warning UI instead of waiting indefinitely.

## Verification

Run the normal local checks:

```sh
scripts/ci-local.sh
```

That script runs formatting, tests, clippy, install verification, SimpleDoc,
PNG rejection, and the saved PR and issue capture validators. GitHub Actions
delegates to the same script for pull requests, pushes to `main`, and manual
dispatches when Actions is enabled for the repository, so checked-in UX
evidence cannot silently drift behind the app rendering code. The tmux mouse-smoke
validators verify that real terminal mouse clicks can expand and collapse
visible content rows, switch PR and issue tabs, expand all rows, collapse them
again, press keyboard `a` to expand and collapse the current tab, activate
linked issue rows, replace the current TUI view with that issue, navigate back,
click footer `[refresh]` until the fixture-mode refresh status is visible, click
activity `[details]` permalinks, click footer `[copy]` and `[open]` through
capture-local adapter commands for visible permalinks, click footer
`[load full]` in an isolated partial-depth fixture session, open the help and
settings overlays through the footer, click a settings row until the
capture-local config save is visible, and click `[quit]` until the tmux session
exits. CI also rejects tracked or generated
PNG files under `captures/`; UX evidence is kept as terminal text and ANSI
transcripts only.

The repository includes tmux capture artifacts for PR and issue views. Captures
are stored as terminal text and ANSI transcripts; PNG screenshots are not
tracked or allowed under `captures/`. Capture scripts pin `GZG_CONFIG_PATH` to
a missing capture-local config file so saved user preferences do not change the
checked-in UX evidence.

- `captures/ghzinga-pr-81834/`
- `captures/ghzinga-issue-88499/`

Reference docs:

- `docs/2026-05-31-ghzinga-implementation-plan.md`
- `docs/2026-05-31-gh-cli-reference-notes.md`

Regenerate PR captures:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py \
  --offline-fixture fixtures/pr-81834.json \
  --offline-resource-fixture fixtures/issue-66943.json
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
  --mode issue \
  --offline-fixture fixtures/issue-88499.json
```

Validate saved issue captures:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py \
  --root captures/ghzinga-issue-88499 \
  --mode issue \
  --validate-only
```

Regenerate PR mouse smoke captures:

```sh
python3 captures/ghzinga-pr-81834/capture_mouse_smoke.py
```

Validate saved PR mouse smoke captures:

```sh
python3 captures/ghzinga-pr-81834/capture_mouse_smoke.py --validate-only
```

Regenerate issue mouse smoke captures:

```sh
python3 captures/ghzinga-issue-88499/capture_mouse_smoke.py
```

Validate saved issue mouse smoke captures:

```sh
python3 captures/ghzinga-issue-88499/capture_mouse_smoke.py --validate-only
```

Run opt-in live GitHub smoke checks:

```sh
scripts/live-smoke.sh
```

Run the no-network live-smoke harness self-test:

```sh
GZG_LIVE_SELF_TEST=1 scripts/live-smoke.sh
```

This calls GitHub with the normal direct HTTP path and validates high-signal PR
Overview, Activity, Commits, Checks, Files, and Links tabs plus live issue
Overview, Activity, and Links tabs. It also reruns all public PR tabs and public
issue Overview, Activity, and Links with `GH_TOKEN` and `GITHUB_TOKEN` blank and
`PATH` pointed at an empty directory, proving public fallback rendering works
without a usable token or `gh` executable. If GitHub's shared unauthenticated
quota for the current IP is exhausted, those public fallback cases are skipped
by default after the first rate-limit response; set
`GZG_LIVE_REQUIRE_PUBLIC_FALLBACK=1` to make that condition fatal. The script is
intentionally not part of CI because it depends on network availability and
current GitHub API quota, but CI does run the syntax check and no-network
self-test for the harness. Override the public targets with
`GZG_LIVE_PR_TARGET` and `GZG_LIVE_ISSUE_TARGET`.
