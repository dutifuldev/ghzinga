# ghzinga

`ghzinga` is a standalone terminal UI for watching one GitHub pull request or
issue.

It shows the conversation, metadata, linked resources, checks, commits, and
changed files in one keyboard- and mouse-friendly terminal view. It uses direct
GitHub API calls for data and reuses your existing GitHub CLI login through
`gh auth token`. You can also set `GH_TOKEN` or `GITHUB_TOKEN` to override that
token. For public repositories, ghzinga can fall back to unauthenticated GitHub
data when credentials are unavailable.

## Install

Install from crates.io:

```sh
cargo install ghzinga
```

Install from a local checkout:

```sh
cargo install --path .
```

That installs both commands:

- `gzg`, the short command
- `ghzinga`, the long command name

Both commands run the same TUI entrypoint. Cargo installs them as two executable
commands.

For a real filesystem link from `ghzinga` to `gzg`, use the repo installer
instead:

```sh
scripts/install.sh
```

That installs `gzg` and creates `ghzinga -> gzg` in the install bin directory.
Use `scripts/install.sh --root /path/to/root` to choose a different install
root.

Build from source without installing:

```sh
cargo build --release
```

Run from a local checkout:

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

## Configuration

Ghzinga reads a small TOML config file:

```text
~/.config/ghzinga/config.toml
```

When `XDG_CONFIG_HOME` is set, the path is
`$XDG_CONFIG_HOME/ghzinga/config.toml`. `GZG_CONFIG_PATH` can point at a
specific file for custom setups.

Default config:

```toml
[ui]
theme = "default"
symbols = "emoji"
spacing = "comfortable"
width_mode = "fixed"
fixed_width = 118
scrollbar = "on-scroll"
```

The app works without a config file. Invalid known values fall back to safe
defaults and show a warning in the status band. Unknown fields are ignored so
future config additions do not break older files.

Open settings inside the TUI with `s` or the footer `[⚙ settings]` control. Theme,
symbol, spacing, width mode, fixed width, and scrollbar changes apply live and
are saved back to `config.toml`; write errors are shown in the status band
without crashing the app.

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
- changed files, with summary rows and separately expandable in-TUI patch
  context when a file row is expanded; patch additions use a green background
  tint, deletions use a red background tint, hunk headers use an accent color by
  default, and patch code hides raw unified-diff `+` / `-` markers while
  preserving indentation
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
needed. Use the visible `[➕ more]` and `[➖ less]` controls to expand or collapse
content. The fixed bottom command bar shows `[➕ all]` or `[➖ all]` after
`[🔄 refresh]`, `[📋 copy]`, `[🌐 open]`, `[⚙ settings]`, `[❔ help]`, and
`[⏻ quit]` when the current tab has expandable rows; that control opens or
folds every expandable row in the active tab without requiring a scroll to the
bottom of the content. The footer message area is reserved for transient
loading, save, status, and error messages; shortcut help lives in the Help view
instead of an always-on footer cheat sheet. The rendered content window only registers hit
targets for the visible rows, so long paginated GitHub histories remain
scrollable without turning every off-screen row into an active terminal target.
If the normal economical API depth sees that GitHub has more pages behind a
first-page collection, the footer also shows `[⬇ full]` before the
expand/collapse-all control; clicking it refetches the current resource with
full supported pagination without restarting the TUI.

The TUI adapts to terminal width. Header metadata, tabs, the status band, and
footer controls wrap into extra rows on narrow terminals instead of silently
overlapping. Long content uses display-width-aware wrapping and truncation, so
emoji and wide characters do not corrupt the layout.

The top identity header uses a stronger background highlight behind the GitHub
link and PR/issue title so the current resource stays easy to spot while
switching tabs or scanning status changes.

When content is scrollable, ghzinga also shows a slim Ratatui scrollbar on the
right edge while you scroll with the keyboard or mouse wheel. The thumb reaches
the bottom at the final scroll position, including comfortable-mode bottom
padding, and endpoint rendering keeps the thumb contiguous at the top and
bottom edges. The scrollbar mode is configurable: `on-scroll` appears during
movement and fades, `always` keeps it visible whenever content can scroll, and
`hidden` disables it. When the scrollbar is visible, click or drag its right-edge
track to jump through the content.

By default, ghzinga renders with emoji symbols in status badges, controls, and
the top navigation selectors. Use `--symbols ascii` for plain terminal-safe text
labels such as `[+ more]`, `[refresh]`, and `[expand all]` when a terminal or
font cannot render emoji cleanly.

## Controls

Mouse:

- click tabs to switch views
- click the underlined header identity to open the current PR or issue on
  GitHub; wide terminals show the full `https://github.com/...` URL so terminal
  URL detection points at GitHub, while narrow terminals use a non-autolink
  fallback label
- click bold `[➕ more]` and `[➖ less]` controls to expand or collapse content
- click footer `[➕ all]` and `[➖ all]` controls at the end of the
  bottom command bar to expand or collapse the current tab
- click file rows in the Files tab to expand or collapse per-file details, then
  click `[➕ more patch]` or `[➖ less patch]` to reveal or fold long diffs
- click the header identity to open the current GitHub issue or PR
- click GitHub issue/PR references to navigate
- click exact GitHub URLs, such as check runs, deployment logs, and comment
  permalinks, to open them in the browser; footer `[📋 copy]` and `[🌐 open]`
  prefer the first visible URL before falling back to the current PR/issue URL
- click `[🔄 refresh]`, `[📋 copy]`, `[🌐 open]`, `[⚙ settings]`, `[❔ help]`,
  `[⏻ quit]`, `[⬇ full]` when shown, and the active-tab expand/collapse control
  in the footer
- use the mouse wheel to scroll
- click or drag the visible right-edge scrollbar to scroll

Keyboard:

- `q` or `Ctrl-C`: quit
- `?`: toggle help
- `s`: open or close settings
- `t` / `y` / `p` / `w` while settings are open: cycle theme / symbol style /
  spacing / width mode
- `b` while settings are open: cycle scrollbar visibility
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
- `Enter`: activate the first visible content action, such as a link or
  `[➕ more]`
- `e`: expand or collapse the main body
- `a`: expand or collapse all expandable rows in the current tab
- `Backspace`: go back after following a linked issue or PR

The shortcuts avoid tmux prefix bindings. `Right` and `Left` are reliable
fallbacks when a terminal or multiplexer encodes Tab unusually.

## Refresh

Live GitHub mode refreshes automatically every 300 seconds by default. Change the
interval with `--refresh-seconds`; use `0` to disable automatic refresh. Manual
refresh is always available with `r` or the `[🔄 refresh]` footer control.
Clicking `[🌐 open]` or pressing `o` opens the first visible GitHub URL, such as a
comment permalink, check-run URL, or linked issue/PR. If no visible link is
available, it opens the current PR or issue URL. Clicking `[📋 copy]` or pressing
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
escape hatch. The same condition enables the footer `[⬇ full]` action and
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

Live startup, refresh, linked-resource navigation, and Backspace navigation run
as background GitHub fetches in the TUI. On startup, `gzg owner/repo#number`
enters the terminal UI immediately with a lightweight loading placeholder, then
replaces it with the fetched resource when GitHub responds. Successful loads are
quiet: the status detail line does not show `info loaded owner/repo#number`, so
the chrome settles back to resource metadata instead of keeping stale completion
text. During refresh or navigation, the previous resource stays readable while
the status band and footer show a terminal-safe loading marker such as
`Loading |: ...`; duplicate fetch starts are ignored until the current one
finishes, so rapid clicks or auto-refresh ticks do not build a request queue.
`--once` loads before rendering so it can produce deterministic static output.
All direct GitHub HTTP requests reuse the same client and carry a 30-second
per-request timeout, so a bad network path reports through the existing
recoverable error or warning UI instead of waiting indefinitely.
