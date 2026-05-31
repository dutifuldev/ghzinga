---
title: ghzoom Implementation Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# ghzoom Implementation Plan

`ghzoom` is a standalone Rust TUI for monitoring one GitHub pull request or issue. It is inspired by Herdr's Ratatui/Crossterm architecture and gh-dash's PR/issue preview layout, but it is not a dashboard side panel. It is a full-screen detail viewer optimized for reading, mouse use, refresh, and status monitoring.

## Product Goal

Open a PR or issue, render all useful GitHub status in a terminal UI, and keep it current.

Primary examples:

```sh
ghzoom https://github.com/openclaw/openclaw/pull/81834
ghzoom openclaw/openclaw#81834
ghzoom openclaw/openclaw 81834
```

The app must not implement its own login flow. It should use `GH_TOKEN` or
`GITHUB_TOKEN` when present, and may fall back to the installed `gh` CLI only to
read an existing credential with `gh auth token`.

## Requirements

- Full Rust implementation.
- Ratatui UI with Crossterm terminal setup and mouse capture.
- Standalone binary, not a `gh dash` plugin.
- No Nerd Font characters required. The default UI may use common emoji symbols
  as semantic markers, but controls must also include text labels so meaning is
  not color- or symbol-only.
- PR support: body, reactions, comments, review comments, commits, CI/check status, changed files, labels, author, branch/base metadata, mergeability/status.
- Issue support: body, reactions, comments, labels, author, state, linked PR/issue navigation where available.
- Mouse-first interaction:
  - click tabs
  - click visible expand/collapse controls
  - click comments/files/checks/linked references where rendered
  - scroll content with mouse wheel
  - click links to navigate to other PRs/issues when they point to GitHub issues/PRs
- Keyboard shortcuts supplement mouse and avoid conflicts with Herdr/tmux:
  - no tmux prefix bindings
  - avoid Ctrl-b, Ctrl-a, Ctrl-d/Ctrl-u as primary app shortcuts
  - use arrows/PageUp/PageDown/Home/End, Tab/Shift-Tab, Enter, Backspace, r, ?, q
- Auto-refresh when GitHub data changes.
- Long text must truncate by default and be expandable using visible UI elements.
- Tests must cover parsing, GitHub adapter command construction, state transitions, layout hit testing, rendering snapshots, mouse click routing, scrolling, refresh behavior, and CLI argument parsing.
- UX verification must exercise narrow, medium, and large terminal sizes and mouse/click interactions.

## Reference Findings

### Herdr

Use these patterns:

- Terminal setup/restoration guard.
- Ratatui render loop with all geometry computed before drawing.
- App state owns hit areas and scroll offsets.
- Mouse events route through stored rectangles, not through widget internals.
- Input handling is explicit and testable.
- Crossterm mouse capture is enabled only because the app has real clickable UI.

Avoid initially:

- PTY hosting.
- Ghostty terminal emulation.
- remote headless server/client mode.
- raw byte parser complexity unless Crossterm events prove insufficient.

### gh-dash

Use these UI ideas:

- stable header with repo/number/title/state metadata
- tabbed PR layout: Overview, Activity, Commits, Checks, Files
- issue layout: body plus activity/comments
- status summary box for review/check/merge state
- scroll percentage and clear loading/error states
- bottom/right adaptive preview lessons from captured sizes

Responsive implementation details observed in gh-dash:

- `tea.WindowSizeMsg` updates shared screen width/height and then resynchronizes
  child component dimensions.
- Preview placement uses an `auto` mode: right-side preview is used only when
  the remaining main table width stays above a minimum threshold, otherwise the
  preview moves below the list.
- Child components receive explicit widths through `SetWidth` / `SetHeight`.
- Rows use display-width measurement before joining horizontal segments.
- Wide horizontal rows truncate less important segments, and list-like rows
  wrap groups onto new lines when the next item would exceed the current width.

Change for `ghzoom`:

- full-screen detail view by default
- compact sticky header instead of large repeated preview chrome
- visible text controls with bold styling and emoji symbols instead of icon-only controls
- full detail depth, not first-five-files preview only
- stronger navigation by links and click targets

### GitHub Web Pull Request Conversation

The GitHub webapp's pull request conversation is a single timeline rooted under
`pull-discussion-timeline` and repeated `TimelineItem` / `timeline-comment`
elements. A public reference page inspected for this behavior was
`rust-lang/rust#140000`.

Observed ordering:

- the opening PR body renders first as the first comment-like item
- commit pushes render inline as timeline items
- assignment, label, branch, merge, and other system events render inline
- reviews render inline in the same chronological stream
- regular comments and review comments render inline in the same chronological
  stream
- later referenced commits or external references appear later in the same
  stream

`ghzoom` should follow that model in the Overview tab. Separate tabs can still
group commits, checks, files, and links for task-specific scanning, but Overview
should begin with the full chronological conversation so it feels like the
GitHub Conversation tab rather than a summary page.

### Slophammer

Use these code-quality conventions:

- pure core logic separated from adapters
- typed snapshot/data model
- app orchestration coordinates adapters and pure rules/state
- tests define behavior first where possible
- dependency boundaries are explicit
- no broad dynamic escape hatches in core code
- default test/lint/format commands documented and runnable

## Architecture

```text
src/
  main.rs                 CLI entrypoint
  lib.rs                  public module root for tests
  app/
    mod.rs                App orchestration
    state.rs              AppState, tabs, focus, refresh state
    update.rs             event reducer: key/mouse/tick/data events
    navigation.rs         link target and history handling
  github/
    mod.rs                GitHub gateway trait
    api.rs                resource fetch/enrichment orchestration and normalization
    auth.rs               token resolution from env or `gh auth token`
    transport.rs          mockable reqwest-backed GraphQL/REST transport
    queries.rs            GraphQL query strings
    types.rs              API response DTOs
    normalize.rs          DTO -> domain model
  domain/
    mod.rs                pure model exports
    resource.rs           ResourceId, Resource, PullRequest, Issue
    activity.rs           comments/reviews/timeline entries
    checks.rs             check rollups and status summaries
    reactions.rs          reaction counts
  render/
    mod.rs                Ratatui rendering entrypoint
    layout.rs             ViewRects and responsive geometry
    components.rs         buttons, tabs, badges, separators
    markdown.rs           plain terminal markdown simplifier/wrapper
    resource.rs           page renderers
    theme.rs              small role-based color palette inspired by Herdr
  input/
    mod.rs                key/mouse mapping
    hit.rs                HitArea, HitTarget, hit testing
  terminal/
    mod.rs                Crossterm/Ratatui terminal guard
  cli.rs                  Clap parser
```

Dependency direction:

```text
main -> cli/app/terminal
app -> github/domain/render/input
render -> domain/input(hit types)
github -> domain
domain -> no adapters, no terminal, no filesystem
```

`domain` must stay pure. `github` owns external API shape. `render` can be tested with Ratatui `TestBackend`. `input` maps terminal events to app intents.

## GitHub Data Strategy

Use direct GitHub API calls for data transport. The app can still reuse an
existing `gh` login as a credential source, but `gh` must not be the data API.

Advantages:

- no token storage
- no login UI
- respects `GH_TOKEN`, `GITHUB_TOKEN`, or the user's existing `gh` auth token
- keeps data fetching in typed HTTP/GraphQL adapters instead of shell commands
- easy to mock in tests by abstracting HTTP transport

GitHub CLI reference notes:

- A local reference checkout was created at `/home/bob/repos/gh-cli`.
- Detailed notes live in `docs/2026-05-31-gh-cli-reference-notes.md`.
- `api/http_client.go` is the relevant transport pattern: construct an HTTP
  client, set GitHub API headers, and let an auth-aware transport attach the
  token to API requests.
- `internal/config/config.go` is the relevant credential pattern: prefer
  explicit environment tokens before falling back to configured/keyring-backed
  credentials. `ghzoom` mirrors that at a smaller scale with `GH_TOKEN`,
  `GITHUB_TOKEN`, then `gh auth token`.
- `pkg/httpmock/stub.go` is the relevant test pattern for direct data access:
  tests should assert HTTP method/path/query shape instead of shell command
  arguments for GitHub data fetches.

Data loaded for PR:

- repo/name/owner/URL
- number, title, body, state, author, created/updated timestamps
- labels and assignees
- metadata such as draft/cross-repository state, mergeability,
  changed-file count, milestone, project membership, ref OIDs, and merge commits
- reactions counts
- base/head branch names
- changed files with additions/deletions/change type, paginated until complete or configured cap
- patch context from the direct REST pull-request diff media type, shown when a
  file row is expanded
  with separate `[more patch]` / `[less patch]` controls for long patches
- commits with SHA, headline, full body, primary author, coauthors,
  authored/committed timestamps, check state when available, and
  deployment/environment status where GitHub exposes it
- status check rollup contexts and latest-commit check suites, including raw
  GitHub status/conclusion, started/completed timestamps, and details URLs
  where available; suite rows are prefixed with `suite/` and shown before
  job/context rows so top-level workflow status is visible first, and suite
  pages are fetched until GitHub reports no next page
- comments, reviews, and review threads, including author association,
  comment/review reactions, edit/minimized metadata, permalinks, and ordinary
  comments paginated until GitHub reports no next page
  review-thread resolved/outdated state; review thread pages are fetched until
  GitHub reports no next page, and threads with more than 100 comments fetch
  their remaining comment pages through the thread node
- timeline events for labels, references, assignments, pins, locks, duplicate
  markers, transfers, connected/disconnected references, review requests,
  draft/ready state, branch changes, force-pushes, merge queue changes, review
  dismissals, auto-merge/rebase/squash changes, automatic base changes, merges,
  title changes, milestones, issue types, sub-issues, parent issues, blocking
  relationships, converted discussions, close, and reopen events; timeline
  pages are fetched until GitHub reports no next page
- reviews and review states
- mergeability and review decision where exposed
- project metadata through optional paginated enrichment when the token has the
  GitHub `read:project` scope; missing project scope does not block the main
  PR view

Data loaded for issue:

- repo/name/owner/URL
- number, title, body, state, author, created/updated timestamps
- labels and assignees
- metadata such as pinned state, state reason, closed time, milestone, and
  project membership
- reactions counts
- comments and timeline events, including author association, comment
  reactions, edit/minimized metadata, permalinks, labels, references,
  assignments, pins, locks, duplicate markers, transfers,
  connected/disconnected references, title changes, milestones, issue types,
  sub-issues, parent issues, blocking relationships, converted discussions,
  close, and reopen events; comment and timeline pages are fetched until GitHub
  reports no next page
- project metadata through optional paginated enrichment when the token has the
  GitHub `read:project` scope; missing project scope does not block the main
  issue view
- timeline-ish linked references when available

Refresh:

- default interval: 60 seconds
- manual refresh: `r`
- display last refreshed time and whether content changed
- include activity bodies and review-thread state in the change fingerprint
- show the changed surfaces after refresh, such as activity, checks, files, or
  commits
- preserve selected tab and scroll position where possible
- if the resource number/title/state changes, update header immediately
- if current resource becomes inaccessible, show a recoverable error state

## UI Layout

Desktop/medium-large:

```text
+----------------------------------------------------------------------------+
| openclaw/openclaw #81834 [PR OPEN]  updated 1m ago  refreshed 12:40:10     |
| feat(senseaudio): add SenseAudio TTS provider                              |
| [Overview] [Activity] [Commits] [Checks] [Files] [Links]                    |
| ✅ open  👤 author  💬 7  👍 3  ✅ checks passed  🧵 2  🔁 refreshed 12:40 |
+----------------------------------------------------------------------------+
| Scrollable selected tab                                                     |
| bold section headings, colored status words, clickable 🔗 links             |
| visible [➕ more] / [➖ less] controls for expansion                         |
+----------------------------------------------------------------------------+
| 🔄 [refresh]  🌐 [open]  ❔ [help]  ⏻ [quit]   tab next | arrows scroll     |
+----------------------------------------------------------------------------+
```

Narrow:

```text
openclaw/openclaw #81834 [PR OPEN]
feat(senseaudio): add SenseAudio TTS provider
[Overview] [Activity] [Checks] [Files]
✅ open | checks passed | 💬 7 | 👍 3 | files 5
----------------------------------------------------------------
Scrollable selected tab
----------------------------------------------------------------
🔄 [refresh] | 🌐 [open] | ❔ [help] | ⏻ [quit]
```

Visual style:

- The status section is a horizontal band below the tabs, not a left sidebar.
- Buttons and expandable controls use bold styling and text labels, for example
  `[➕ more]`, `[➖ less]`, `[🔄 refresh]`, `[🌐 open]`.
- Emoji symbols are semantic markers only; keep nearby text labels so the UI is
  usable when emoji width/color rendering varies.
- Status badges use both symbols and words: `✅ PASS`, `❌ FAIL`, `⏳ PENDING`,
  `OPEN`, `CLOSED`, `MERGED`.
- No Nerd Font icons.
- Use color and bold to emphasize hierarchy:
  - accent color for active tabs, buttons, and links
  - green for success/open/passed
  - red for failed/blocked/error
  - yellow/peach for pending/warnings
  - dim overlay colors for metadata and separators
- Theme mechanism mirrors Herdr's role-based palette shape, but only exposes a
  small set:
  - `default`: dark, high-contrast developer dashboard palette
  - `solarized-dark`: Solarized Dark colors from Herdr's palette
- Line and separator rendering borrows Herdr's understated approach:
  - fill the terminal background with the active palette's panel color
  - use single-line separators such as `─` in dim surface colors instead of
    boxed panels around every region
  - use left guide markers such as `▏` for preformatted code or dense quoted
    blocks when that helps scanning
  - keep borders structural and quiet, with color and bold carrying most of the
    emphasis

Responsive behavior:

- Recompute all rectangles from the terminal width and height on every render.
- Let the header, tabs, horizontal status band, and footer reserve extra rows on
  narrow terminals instead of clipping into each other.
- Keep the status band horizontal, but wrap status chips across multiple rows
  when the terminal is narrow.
- Wrap tab labels and footer controls using display width; click hit areas must
  follow the wrapped visual position.
- Truncate only low-priority metadata when a fixed-height region cannot grow
  further.
- Use display-width-aware wrapping for long words, emoji, and wide characters.
- Preserve at least one content row whenever the terminal is tall enough to show
  app chrome plus content.

## Tabs

PR tabs:

- Overview: GitHub-style chronological conversation timeline first, with the
  opening body, comments, reviews, review comments, timeline events, and commits
  interleaved by timestamp; compact metadata and change summary follow it
- Activity: comments, reviews, review comments, bot comments
- Commits: commit list with SHA, message, author, timestamp, status
- Checks: aggregate status and grouped detailed checks
- Files: changed files, additions/deletions, click file to expand summary
- Links: detected issue/PR links from body/comments

Issue tabs:

- Overview: GitHub-style chronological conversation timeline first, with the
  opening body, comments, and timeline events interleaved by timestamp; compact
  metadata follows it
- Activity: comments and timeline entries
- Links: detected issue/PR links from body/comments

## Chronological Overview Rules

- The first Overview section is `Conversation`.
- The opening body is rendered as an authored timeline card using the resource
  author and created timestamp.
- PR commits are inserted into the same stream using `committed_at`, or
  `authored_at` when commit time is missing.
- Activity entries use their existing `updated_at` value.
- Items sort ascending by timestamp, with stable tie-breaking that keeps the
  opening body first, then commits, then activity entries when timestamps match.
- Each item starts with a bold colored heading and a quiet separator so it is
  clear where one item ends and the next begins.
- Expand/collapse controls stay per item. Long comments and the opening body
  are truncated by default; expanded state reuses existing `BlockId`s.
- GitHub API pagination remains an adapter concern. The renderer receives the
  normalized full resource and virtualizes the terminal work by only registering
  hit targets for visible rows after scroll clipping.
- If a timestamp is relative or not parseable, preserve the adapter order and
  display the timestamp text as-is rather than hiding the item.

## Input Model

Keyboard:

- `q`: quit
- `?`: help overlay
- `r`: refresh now
- `Tab` / `Shift+Tab`: next/previous tab
- `Left` / `Right`: next/previous tab
- `Up` / `Down`: scroll line
- `PageUp` / `PageDown`: scroll page
- `Home` / `End`: top/bottom
- `Enter`: activate the first visible content action, such as a link or `[➕ more]`
- `Backspace`: navigate back after following a link
- `o`: open current resource in browser through `gh`

Mouse:

- wheel up/down scrolls current content
- left click tab activates tab
- left click visible button activates it
- left click link navigates to linked PR/issue
- left click file/check/comment selects it

Avoid as primary shortcuts:

- `Ctrl-b` and `Ctrl-a` because of tmux/screen
- `Ctrl-d`/`Ctrl-u` because they are common in shells and gh-dash but conflict-prone inside nested TUIs
- raw escape sequences that Herdr may use for pane routing

## Text Expansion

Long text behavior:

- Body starts collapsed to a configurable rendered-line limit.
- Each long comment starts collapsed.
- Visible controls:
  - `[➕ more]` expands one block
  - `[➖ less]` collapses it
  - `[expand all]` for tab-level expansion
- Mouse and keyboard activation use the same `HitTarget::ToggleBlock`.
- Truncation must be tested against line wrapping, terminal width, and Unicode width.

## Testing Plan

Unit tests:

- parse resource ids from URL, `owner/repo#number`, and argument forms
- normalize GitHub GraphQL DTOs into domain models
- classify check states into PASS/FAIL/PENDING/NEUTRAL/SKIPPED
- detect issue/PR links in Markdown-ish text
- markdown simplifier strips tables safely and wraps text
- expansion reducer toggles blocks without affecting other blocks
- refresh reducer preserves tab/scroll state
- hit testing returns the expected tab/button/link target

Rendering tests:

- Ratatui `TestBackend` render snapshots for:
  - narrow overview
  - medium PR checks
  - large PR activity
  - issue overview
  - loading state
  - error state
- Chrome assertion: rendered buffers contain no heavy box-drawing or Nerd Font
  chrome from our components; common emoji markers and quiet `─`/`▏`
  separators are allowed.

Interaction tests:

- simulated mouse click on tab changes active tab
- simulated mouse wheel changes scroll offset
- simulated click on `[➕ more]` expands body/comment
- simulated click on PR link navigates to new resource id
- keyboard `Tab`, arrows, `PageDown`, `Backspace`, `r`, `q`

Adapter tests:

- GitHub gateway builds direct HTTP GraphQL and REST requests with expected
  method, URL, headers, token, and body
- mocked HTTP transport returns fixture JSON or diff bytes
- GraphQL error payloads fail before normalization
- HTTP failure status and body are preserved in errors
- auth failure maps to a friendly error
- API rate-limit or network error is displayed but does not crash UI

Architecture tests:

- domain modules do not import app, GitHub, terminal, render, TUI, network, or
  process APIs
- GitHub adapter modules do not import app, input, render, terminal, Ratatui, or
  Crossterm
- GitHub data fetching does not regress to `gh pr view`, `gh issue view`, or
  `gh api`; the only `gh` shell-out in the data adapter is `gh auth token`

End-to-end/manual verification:

- run `cargo test`
- run `cargo fmt --check`
- run `cargo clippy --all-targets --all-features -- -D warnings`
- run `npx -y @simpledoc/simpledoc check`
- keep `.github/workflows/ci.yml` aligned with these local gates
- run `ghzoom openclaw/openclaw#81834` in tmux at:
  - `80x24`
  - `120x36`
  - `160x50`
- capture frames showing:
  - body
  - comments
  - commits
  - checks
  - files
  - click tab behavior
  - click `[➕ more]`
  - refresh status

## Implementation Phases

### Phase 1: Foundation

- Create Rust crate.
- Add dependencies and dev tooling.
- Define module boundaries.
- Implement CLI parsing and resource id parsing.
- Implement domain model and fixture data.
- Add first unit tests.

### Phase 2: GitHub adapter

- Implement `GithubGateway` trait.
- Implement direct GitHub API transport with credential reuse.
- Add GraphQL queries for PR and issue.
- Normalize API JSON into domain models.
- Add fixture-driven adapter tests.

### Phase 3: Static TUI rendering

- Implement terminal guard.
- Implement app state and pure update reducer.
- Implement layout rectangles.
- Render header, tabs, horizontal status band, content region, footer.
- Render Overview/Activity/Commits/Checks/Files/Links from fixtures.
- Add Ratatui render tests.

### Phase 4: Input and mouse

- Enable mouse capture.
- Store hit areas during render.
- Route mouse clicks/wheel to app intents.
- Add tab, expansion, link navigation, and file/check selection.
- Add interaction tests.

### Phase 5: Refresh and navigation

- Add Tokio event loop with refresh ticker.
- Preserve state across refreshes.
- Add history stack and Backspace navigation.
- Add visible changed/error/loading states.
- Add tests for refresh reducer and navigation reducer.

### Phase 6: Verification

- Run unit, render, adapter, and interaction tests.
- Run fmt/clippy.
- Run live GitHub checks through direct API requests.
- Run tmux capture verification at narrow/medium/large sizes.
- Fix UX issues discovered by captures.

## First Build Slice

The first coherent build slice should produce:

- `cargo test` passing
- `ghzoom --help`
- `ghzoom openclaw/openclaw#81834 --offline-fixture fixtures/pr-81834.json`
- static Ratatui display with keyboard tab switching and scroll
- mouse click on tabs and `[➕ more]`

This slice proves the architecture without depending on live GitHub availability. Live `gh` integration follows immediately after.
