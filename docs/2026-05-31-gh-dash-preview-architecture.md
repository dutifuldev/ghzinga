---
title: gh-dash PR and Issue Preview Architecture
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# gh-dash PR and Issue Preview Architecture

This analysis describes how `gh dash` renders the PR/issue preview panel, using the installed `gh dash` extension version captured in `captures/gh-dash-pr-81834/manual-preview/` and the matching `gh-dash` source tag `v4.24.1`.

The concrete capture target was `openclaw/openclaw#81834`, titled `feat(senseaudio): add SenseAudio TTS provider`.

## Capture Evidence

The preview captures are in:

- `captures/gh-dash-pr-81834/manual-preview/narrow/` (`80x24`)
- `captures/gh-dash-pr-81834/manual-preview/medium/` (`120x36`)
- `captures/gh-dash-pr-81834/manual-preview/large/` (`160x50`)

Useful reference frames:

- PR body: `medium/01_overview_down.png`, `large/00_overview_top.png`, `narrow/04_overview_down.png`
- Comments/activity: `medium/20_activity_top.png`, `large/20_activity_top.png`, `narrow/32_activity_down.png`
- Commits: `medium/40_commits_top.png`, `large/40_commits_top.png`, `narrow/62_commits_down.png`
- CI/checks: `medium/50_checks_top.png`, `large/50_checks_top.png`, `narrow/73_checks_down.png`
- Changed files: `medium/60_files_top.png`, `large/60_files_top.png`, `narrow/90_files_top.png`

The captures show both the rendered UI and the adaptation behavior:

- In `narrow/`, preview uses a bottom split with very little vertical room. Useful content appears only after repeated `Ctrl+d`.
- In `medium/`, preview also uses bottom split, but the body and checks are readable with fewer scroll steps.
- In `large/`, preview was toggled to a right split with `P`, showing the main PR list on the left and the preview on the right.

## Top-Level Model

The preview is not a separate screen. It is a sidebar component attached to the current list view.

The relevant source modules are:

- `internal/tui/ui.go`: owns global key dispatch, current section, preview open state, layout sizing, and composition.
- `internal/tui/components/sidebar/sidebar.go`: owns the scrollable preview viewport and its frame.
- `internal/tui/components/prview/prview.go`: renders PR preview content.
- `internal/tui/components/issueview/issueview.go`: renders issue preview content.
- `internal/tui/components/prssection/prssection.go`: owns the PR list and selected row.
- `internal/tui/components/issuessection/issuessection.go`: owns the issue list and selected row.
- `internal/data/prapi.go`: GraphQL data shape for list PRs and enriched PRs.
- `internal/data/issueapi.go`: GraphQL data shape for issues.

The architecture is:

```text
Bubble Tea update loop
  -> current section tracks selected row
  -> ui.syncSidebar() reads selected row data
  -> sidebar receives rendered string content
  -> sidebar viewport handles scrolling
  -> root view joins list + sidebar horizontally or vertically
```

The important design choice is that `sidebar.Model` is generic. It does not know much about PRs or issues. It just stores a rendered string, a viewport, dimensions, and open/closed state. PR-specific and issue-specific rendering happens before content is handed to the sidebar.

## Data Flow

### Initial list data

For PRs, `FetchPullRequests()` runs a GitHub GraphQL search query. The list row data includes enough fields to render the table and the first-level preview header:

- number, title, state, author
- repo, base/head branch names
- labels, assignees
- review status
- comment and review-thread counts
- last commit check rollup state
- additions/deletions
- changed-file count

The row model is `prrow.Data`, which wraps `data.PullRequestData`. `prssection.BuildRows()` converts each `prrow.Data` into a `prrow.PullRequest`, then into a table row with `ToTableRow()`.

For issues, `FetchIssues()` uses a similar GraphQL search path, returning `data.IssueData`. Unlike PRs, the issue list data already includes issue body and recent comments (`comments(last: 15)`), so there is less of a split between basic and enriched rendering.

### Selected row handoff

Selection is tracked by the table/list viewport. The section exposes `GetCurrRow()`:

- PR section returns `*prrow.Data`.
- Issue section returns `*data.IssueData`.

When selection changes, `ui.onViewedRowChanged()` runs. It:

- collapses the PR summary with `SetSummaryViewLess()`
- resets the PR tab carousel to Overview
- calls `syncSidebar()`
- starts PR enrichment with `prView.EnrichCurrRow()`
- scrolls preview to top

This explains one behavior visible during capture: opening preview alone can show `Nothing selected...` until the current row is explicitly synced. Pressing `g` after opening preview forced `onViewedRowChanged()` for row 0 and populated the preview.

### PR enrichment

The PR preview has a two-tier data model:

- `Primary`: list/search data already available from the PR table query.
- `Enriched`: fetched lazily by `data.FetchPullRequest(url)` when a PR becomes selected.

The enriched query fetches:

- full body fields
- all commits up to `last: 100`
- comments `last: 50`
- review threads `last: 50`, including review comments
- review requests and reviews
- suggested reviewers
- richer status check rollup contexts
- check suites, including queued or approval-required workflows
- first five changed files

The `prview.Model` stores `*prrow.PullRequest`, so it can render both the row-derived summary and the enriched details. If enriched data is missing, tabs such as Activity, Commits, and Checks render loading or partial states.

## Layout Engine

Preview placement is controlled in `ui.resolvePreviewPosition()` and `ui.syncMainContentDimensions()`.

`resolvePreviewPosition()` has three modes:

- explicit `right`
- explicit `bottom`
- `auto`

In `auto`, gh-dash computes whether a right-side preview would leave at least `80` columns for the main table. If not, it falls back to bottom preview.

For our config, preview width was `0.55`. That means:

- At `80x24`, right preview would leave about `36` columns for the list, below the `80` column threshold, so bottom preview is chosen.
- At `120x36`, right preview would leave about `54` columns, also below the threshold, so bottom preview is chosen.
- At `160x50`, right preview would leave about `72` columns, still below the threshold with that width. We manually pressed `P` to override to right preview for the large capture.

`syncMainContentDimensions()` sets:

- `MainContentWidth`
- `MainContentHeight`
- `DynamicPreviewWidth`
- `DynamicPreviewHeight`
- `SidebarOpen`

Bottom preview:

```text
tabs
search/list area
horizontal divider
preview viewport
footer
```

Right preview:

```text
tabs
list area | preview viewport
footer
```

The root view then composes the current section and sidebar:

- bottom mode uses `lipgloss.JoinVertical`
- right mode uses `lipgloss.JoinHorizontal`

That is exactly what the captures show. `medium/50_checks_top.png` has a full-width bottom preview under the row list. `large/50_checks_top.png` has the preview on the right with the PR list compressed to the left.

## Sidebar Viewport

`sidebar.Model` is a thin wrapper around `bubbles/viewport`.

State:

- `IsOpen`
- raw rendered `data` string
- `viewport.Model`
- current `ProgramContext`
- empty state text: `Nothing selected...`

Rendering:

- If closed, returns an empty string.
- If open with no data, renders centered `Nothing selected...`.
- If open with data, renders `viewport.View()` plus a scroll percentage.

Scrolling:

- `Ctrl+d` maps to `viewport.HalfPageDown()`.
- `Ctrl+u` maps to `viewport.HalfPageUp()`.

The footer percentage visible in captures (`0%`, `3%`, `38%`, `100%`) is the sidebar viewport scroll percentage, not a PR-specific component. This is why the same percentage behavior appears across Overview, Activity, Checks, and Files Changed.

## PR Preview Rendering

`prview.Model.View()` dispatches by selected tab:

- Overview
- Activity
- Commits
- Checks
- Files Changed

The tab bar is a `carousel.Model`; `[` and `]` move between tabs. In our manual capture, `]` was used to step from Overview through Activity, Commits, Checks, and Files Changed.

### Shared PR header

Every PR tab starts with `viewHeader()`:

1. Repo and PR number, rendered as a preview header.
2. Title block with highlighted background.
3. Status pill plus base/head branch line.
4. Author, relative age, and author association.
5. Tab carousel with a bottom border.

This is visible in nearly every capture. For `#81834`, the header displays:

- `openclaw/openclaw · #81834`
- `feat(senseaudio): add SenseAudio TTS provider`
- `Open`
- `main` to `feat/senseaudio-tts`
- author `@KLilyZ`

The header is repeated because the sidebar viewport captures the full visible preview frame at each scroll position. When scrolled, the viewport content can start below the full header; in narrow captures, the header often consumes most visible space.

### Overview tab

Overview renders:

1. Requested reviewers, if present.
2. Labels.
3. Summary/body.
4. Changes overview.
5. Checks overview.
6. Optional input box if an action mode is active.

For `#81834`, `medium/01_overview_down.txt` shows the expanded body:

```text
## Summary
* Problem: `senseaudio` bundled plugin only has ASR; no TTS.
* Why it matters: completes the round trip in the same plugin...
* What changed: registers a `speechProvider` in `extensions/senseaudio/`.
```

The body is Markdown rendered by Glamour through `markdown.GetMarkdownRenderer(width)`. Before rendering, gh-dash strips HTML comments and aggressively removes table-like lines using regex cleanup. The summary normally folds to eight rendered lines (`foldBodyHeight = 8`). Pressing `e` calls `SetSummaryViewMore()` through PR action handling, which is why the capture flow explicitly expanded the summary before scrolling.

UI effect:

- The folded body is good for quick review.
- The expanded body behaves like normal viewport content; the sidebar scroll percentage becomes the main way to understand position.
- In narrow mode, the body becomes a long document with many scroll stops.

### Activity tab

Activity combines:

- PR review-thread comments from enriched review threads.
- PR issue-style comments from enriched comments.
- PR reviews from primary review nodes.

It normalizes them into `RenderedActivity` items and sorts them ascending by `UpdatedAt`. Each comment gets:

- rounded border header with author and relative time
- optional file path and line for review-thread comments
- Markdown-rendered body

For `#81834`, captures show `7 comments`, starting with a `github-actions` dependency-change comment. Later scroll frames show longer review text and code/file references.

The tab is good at creating a chronological feed, but the model is still mostly text-first:

- There is no nested conversation outline.
- Review-thread path/line metadata is present, but not deeply interactive.
- Long review comments dominate the viewport; this is clear in `large/28_activity_down.txt`, where one comment fills most of the right pane.

### Commits tab

Commits renders `Enriched.AllCommits.Nodes`.

Each commit row includes:

- commit icon
- message headline
- abbreviated SHA aligned to the right
- author
- relative commit age
- optional status check summary for that commit

For `#81834`, the captures show one commit:

```text
feat(senseaudio): add SenseAudio TTS provider ... fb948c9
```

The renderer uses horizontal line fill between the truncated headline and the SHA. This is an effective terminal layout pattern because it keeps the SHA scannable even as width changes.

### Checks tab

Checks renders two layers:

1. `renderChecksOverview()`: an aggregate review/check/merge box.
2. `renderChecks()`: detailed individual checks.

The aggregate box combines:

- review state
- check state
- mergeability/conflict state

The checks section computes counts from:

- `StatusCheckRollup.Contexts.CheckRunCountsByState`
- `StatusCheckRollup.Contexts.StatusContextCountsByState`
- `CheckSuites` for suites not present in the rollup, especially queued or approval-required workflows

The capture `medium/50_checks_top.txt` shows:

```text
Reviews
  None requested

All checks have passed
  38 skipped, 2 neutral, 86 successful
```

The bar underneath is proportionally rendered with repeated block characters. This communicates the ratio of skipped/neutral/success states, but it is also expensive in horizontal space. In narrow mode, the same check summary requires scrolling before the useful lines appear, as shown by `narrow/73_checks_down.txt`.

### Files Changed tab

Files Changed renders only `m.pr.Data.Primary.Files.Nodes`, which comes from the first page of changed files (`files(first: 5)`) in both primary and enriched data structures.

Each file row includes:

- additions in success color
- deletions in error color
- change-type icon
- path, wrapping manually if it exceeds the remaining width

For `#81834`, captures show:

```text
+1   -1    docs/plugins/plugin-inventory.md
+1   -1    docs/plugins/reference.md
+3   -3    docs/plugins/reference/senseaudio.md
+74  -14   docs/providers/senseaudio.md
+3   -1    extensions/senseaudio/index.ts
```

This is a good preview, not a full files browser. It is intentionally limited and does not show file diffs.

## Issue Preview Rendering

Issue previews reuse the same `sidebar.Model`, but do not use the PR tab carousel. `issueview.Model.View()` renders a single document:

1. Issue number/repo header.
2. Title block.
3. State pill.
4. Author line.
5. Labels.
6. Body.
7. Comments/activity.
8. Optional input box if an action mode is active.

Issue comments are simpler than PR activity:

- only issue comments are included
- comments are sorted by updated time
- each comment has a rounded author/time header
- bodies are Markdown-rendered

This means issue preview is architecturally closer to a document view, while PR preview is a tabbed inspection surface.

That difference matters for a tool like `ghzoom`: if the goal is to display one individual issue/PR deeply, PR and issue should probably share a high-level "document viewport" shell but not force issue content into PR-style tabs. PRs naturally need tabs because they have checks, commits, files, reviewers, and review-thread activity. Issues mostly need body, comments, labels, assignees, and state transitions.

## Styling System

The preview is built with Lip Gloss:

- blocks are strings with ANSI style metadata
- layout uses `JoinVertical`, `JoinHorizontal`, width constraints, padding, borders, and foreground/background styles
- the final renderer is still a terminal string

Notable shared helpers:

- `common.RenderPreviewHeader()`: full-width line with selected background and secondary text
- `common.RenderPreviewTitle()`: fixed three-line highlighted title block
- `common.RenderLabels()`: pill layout with wrapping
- `PrView.PillStyle`: status and label pill styling
- `CommonStyles`: glyphs and semantic colors for success, failure, waiting, comments, people, merged state

The visual vocabulary is consistent:

- faint border boxes for grouped metadata
- bright semantic glyphs for status
- selected-background header/title blocks
- underlined section headings
- rounded borders around comments and aggregate checks

The captures show that the style works best in medium and large terminals. In narrow bottom mode, the fixed header and tab row consume a high percentage of visible space, so reading body/comments requires many scroll actions.

## Interaction Model

Global keys handled by `ui.go`:

- `p`: toggle preview open/closed
- `P`: toggle preview position
- `g`: first item
- `G`: last item
- `j/k`: row navigation
- `Ctrl+d` / `Ctrl+u`: preview half-page scroll
- `[` / `]`: PR preview tabs
- `e`: expand PR summary body

The important architectural point is that list navigation and preview scrolling are different layers:

- row navigation changes the section selection and re-renders preview content
- preview scrolling only changes `sidebar.viewport` offset
- tab navigation changes `prview.carousel`

This makes the UI predictable, but it also creates state coordination issues. For example:

- `p` opens the sidebar but does not itself call `syncSidebar()` in v4.24.1.
- Row movement or `g` does call `onViewedRowChanged()`, which syncs the selected row.
- The PR summary expansion state is reset on row changes, because `onViewedRowChanged()` calls `SetSummaryViewLess()`.

This is why the capture sequence was:

```text
p
g
e
Ctrl+d...
]
Ctrl+d...
```

## Terminal Capture Note

In tmux, the installed `gh dash` v4.24.1 preview could panic if Markdown rendered before the terminal background-color response initialized markdown styles. The manual capture flow fed a dark background-color response to the pane before opening the preview.

That detail is not part of normal end-user rendering, but it is relevant for automated or pseudo-automated terminal capture:

```text
ESC ] 11 ; rgb:0000/0000/0000 ESC \
```

After that, preview rendering worked consistently inside tmux.

## UI Strengths

The preview does several things well:

- It keeps the PR list visible while inspecting details.
- It adapts between right and bottom placement.
- The PR header gives stable identity across all tabs.
- The Checks tab has an effective aggregate status summary.
- Comments and reviews preserve Markdown, which keeps long review feedback readable.
- The Files Changed tab gives a quick change footprint without needing a diff.
- The Commit tab uses strong terminal layout: headline left, SHA right, metadata underneath.

The captured `#81834` checks frame is especially strong: it compresses review state, CI status, and mergeability into one bordered object, then follows with individual checks.

## UI Weaknesses

The preview also has constraints that matter for a dedicated individual issue/PR viewer.

### Header cost is high in small terminals

In `80x24`, the preview header, title block, branch/author lines, tab row, divider, footer, and scroll percentage leave only a few lines for actual content. Narrow captures make this obvious: the user often sees identity chrome instead of the body/comment content they are trying to inspect.

For a dedicated `ghzoom`-style viewer, the header should probably compact after initial scroll or offer a dense mode.

### Bottom preview is document-hostile

Bottom preview preserves the list width, but it turns long bodies/comments into a small scrolling strip. This is good for quick peeking, bad for deep reading.

For individual PR/issue display, the default should likely be a full-screen document layout rather than a list-plus-preview layout.

### PR tabs are useful but shallow

Tabs are the right top-level IA for PRs, but each tab is still a linear string in a single viewport.

This limits:

- jumping between comments
- expanding/collapsing long comments
- differentiating review comments from general comments
- navigating individual checks
- viewing more than the first five changed files

### Files tab is only a list

Files Changed shows names and line counts, not diffs. That is a good sidebar preview, but an individual PR inspector should probably support:

- full changed-file pagination
- file filters
- inline diff previews
- jump-to-review-thread anchors

### Checks need grouping

The aggregate check summary is excellent. The detailed list can become very long. A deeper viewer would benefit from grouping by:

- failing
- required
- waiting
- skipped/neutral
- successful
- workflow/app

gh-dash already partially groups awaiting-approval and pending checks. A dedicated viewer could make this grouping first-class and collapsible.

## Implications for ghzoom

If `ghzoom` is meant to display one individual issue or PR, the gh-dash preview is the right reference point, but not the final shape.

Recommended architecture:

```text
App shell
  -> resource resolver: issue or PR by URL/number/search
  -> resource store: primary + enriched data
  -> document viewport
  -> resource-specific panels/tabs
  -> action composer overlays
```

Recommended PR views:

- Summary
- Activity
- Checks
- Files
- Commits
- Metadata

Recommended issue views:

- Body
- Activity
- Metadata
- Linked PRs or timeline, if available

Recommended UI differences from gh-dash:

- Use full-screen detail layout by default.
- Keep identity header compact and sticky, not repeated as large chrome.
- Make tabs persistent but small.
- Treat body/comments/checks as navigable sections, not just a single string.
- Include a command palette or jump list for comments, files, and checks.
- Preserve gh-dash's strong semantic status language: success/failure/waiting glyphs, pills, faint grouping borders, and scroll percentage.

## Concrete Design Lessons

1. Keep the generic viewport separate from resource-specific rendering.

   gh-dash gets this right. `sidebar.Model` only scrolls and frames strings. `prview` and `issueview` own domain rendering.

2. Separate primary and enriched data.

   PR list data should be fast and cheap. Deep tabs can load later. For `ghzoom`, this should become explicit loading state per section rather than one resource-wide enrichment flag.

3. Preserve a stable resource header.

   The header is useful, but its size should be responsive. Full title blocks are good at large sizes and too expensive at narrow sizes.

4. Make scroll state visible.

   The sidebar percentage is simple and helpful. A deeper tool should keep it, but may also need a section outline so users know what content lies below.

5. PR and issue previews should share shell mechanics, not identical content models.

   PRs need tabs for checks/commits/files. Issues are closer to a body plus comments timeline.

6. The Checks overview is the strongest reusable component.

   Its aggregate box is compact, semantic, and useful before the full check list. A dedicated viewer should retain that pattern and add grouping/collapse for detail.

7. Activity needs structure beyond chronological text.

   gh-dash's chronological text feed works for previewing. For deep inspection, comments, review threads, bot comments, and reviews should be distinguishable and navigable.

## Summary

gh-dash implements PR/issue preview as a generic sidebar viewport fed by domain-specific renderers. PR preview is tabbed and enrichment-driven; issue preview is a simpler single-document render. Layout is controlled globally by preview position and terminal dimensions, with bottom split preferred when right split would starve the main table.

The captures show that this is excellent for peeking from a dashboard, especially at medium/large sizes, but it becomes cramped for deep reading in narrow terminals. For a dedicated `ghzoom` tool, the winning approach is to reuse the conceptual split of generic viewport plus resource renderers, while switching the default experience from "dashboard preview" to "full-detail document inspector."
