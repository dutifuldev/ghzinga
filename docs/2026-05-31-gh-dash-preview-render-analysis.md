---
title: gh-dash Preview Render Analysis
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# gh-dash Preview Render Analysis

This is a detailed analysis of how `gh dash` renders PR and issue previews, using:

- source from `/home/bob/repos/gh-dash` at the installed `gh dash` extension version, v4.24.1
- terminal captures for `openclaw/openclaw#81834` in `captures/gh-dash-pr-81834/manual-preview/`
- the capture config in `captures/gh-dash-pr-81834/config.yml`

The captures are PR-focused because the manual session targeted `openclaw/openclaw#81834`. The issue preview analysis is source-backed from `internal/tui/components/issueview/`.

## Capture Set

The manual captures cover three terminal sizes:

| Size | Directory | Layout observed |
| --- | --- | --- |
| `80x24` | `captures/gh-dash-pr-81834/manual-preview/narrow/` | bottom preview |
| `120x36` | `captures/gh-dash-pr-81834/manual-preview/medium/` | bottom preview |
| `160x50` | `captures/gh-dash-pr-81834/manual-preview/large/` | manually toggled right preview |

Important frames:

| Area | Narrow | Medium | Large |
| --- | --- | --- | --- |
| Overview/body | `narrow/08_overview_down.txt` | `medium/01_overview_down.txt` | `large/00_overview_top.txt` |
| Activity/comments | `narrow/30_activity_top.txt` | `medium/20_activity_top.txt` | `large/20_activity_top.txt` |
| Commits | `narrow/60_commits_top.txt` | `medium/40_commits_top.txt` | `large/40_commits_top.txt` |
| Checks | `narrow/70_checks_top.txt` | `medium/50_checks_top.txt` | `large/50_checks_top.txt` |
| Files | `narrow/90_files_top.txt` | `medium/60_files_top.txt` | `large/60_files_top.txt` |

The manual flow was:

1. Start `gh dash --config captures/gh-dash-pr-81834/config.yml` inside tmux.
2. Press `p` to open preview.
3. Press `g` to force the first selected row into the preview.
4. Press `e` on Overview to expand the body.
5. Scroll with `Ctrl+d` and `Ctrl+u`.
6. Move PR preview tabs with `]`.
7. Toggle large layout with `P` to capture a right-side preview.

## Rendering Pipeline

The preview is a composition of four layers:

```text
Bubble Tea program
  -> selected section row
  -> domain preview renderer
  -> generic sidebar viewport
  -> root layout joins list + sidebar + footer
```

The main modules are:

| Module | Responsibility |
| --- | --- |
| `internal/tui/ui.go` | global update loop, key dispatch, layout dimensions, sidebar synchronization |
| `internal/tui/components/sidebar/sidebar.go` | scrollable viewport wrapper with bottom/right framing |
| `internal/tui/components/prview/prview.go` | PR preview shell, PR header, tabs, body/labels/reviewers |
| `internal/tui/components/prview/activity.go` | PR comments, review threads, reviews |
| `internal/tui/components/prview/checks.go` | aggregate review/check/merge box and individual checks |
| `internal/tui/components/prview/commits.go` | commit list |
| `internal/tui/components/prview/files.go` | changed-file list |
| `internal/tui/components/issueview/issueview.go` | issue preview shell, body, labels, author, state |
| `internal/tui/components/issueview/activity.go` | issue comments |
| `internal/data/prapi.go` | GraphQL PR primary and enriched data shapes |
| `internal/data/issueapi.go` | GraphQL issue data shape |

The central architectural choice is that `sidebar.Model` is intentionally generic. It stores a rendered string and a `bubbles/viewport.Model`. It does not know whether it is showing a PR, issue, branch, or notification subject. Domain-specific renderers build styled strings, then `ui.syncSidebar()` hands those strings to `sidebar.SetContent()`.

That gives gh-dash this shape:

```text
selected row changes
  -> ui.onViewedRowChanged()
  -> prView.SetSummaryViewLess()
  -> prView.GoToFirstTab()
  -> ui.syncSidebar()
  -> prView.EnrichCurrRow()
  -> sidebar.ScrollToTop()
```

When enrichment returns, the PR view is updated again with richer data. This explains why some tabs can briefly show `Loading...` even though the row itself is already visible.

## Data Model

PR preview has a split model:

| Layer | Purpose | Source |
| --- | --- | --- |
| Primary PR data | fast list row plus enough metadata for header/table | GraphQL search PR query |
| Enriched PR data | body, comments, review threads, reviews, files, commits, checks | lazy `data.FetchPullRequest(url)` |

The primary row includes title, number, repo, author, branches, state, labels, assignees, comments/review counts, last commit status, merge status, additions/deletions, and branch protection metadata.

The enriched PR fetch includes:

- body
- comments, last 50
- review threads, last 50, with first 20 comments per thread
- reviews, last 100
- review requests, last 100
- suggested reviewers
- commits, last 100
- latest commit status check rollup, last 100 contexts
- latest commit check suites, last 20
- changed files, first 20

Issue preview is simpler. The issue row data already carries body and recent comments, so `issueview.Model` renders a single document instead of a tabbed, lazy-enriched PR inspection surface.

This difference is important: gh-dash treats PR preview as a multi-panel dashboard artifact, while issue preview is closer to a rendered issue document.

## Layout Selection

The capture config used:

```yaml
defaults:
  preview:
    open: false
    width: 0.55
    height: 0.60
    position: auto
```

`ui.resolvePreviewPosition()` implements `auto` with one hard constraint: right preview is allowed only if the main table would keep at least 80 columns.

```text
previewWidth = width setting, either absolute columns or fraction of screen width
tableWidth = screenWidth - previewWidth
if tableWidth < 80:
  bottom preview
else:
  right preview
```

With `width: 0.55`:

| Terminal | Preview width | Remaining table width | Auto decision |
| --- | ---: | ---: | --- |
| `80x24` | about 44 | about 36 | bottom |
| `120x36` | about 66 | about 54 | bottom |
| `160x50` | about 88 | about 72 | bottom |

The large capture was manually toggled to right mode with `P`, so it shows what right preview looks like even though the automatic rule would still protect the table and choose bottom mode.

### Bottom Preview

Bottom mode keeps the list at full width and gives the preview a horizontal strip below it:

```text
top tabs / search / row list
horizontal separator
preview viewport
footer
```

This is visible in `medium/50_checks_top.txt`. The PR list remains readable, but preview height is limited. In `80x24`, the header and tab chrome consume most of the preview viewport; `narrow/70_checks_top.txt` shows the Checks tab header but not the checks body.

### Right Preview

Right mode keeps more preview height and makes the preview feel like an inspector pane:

```text
top tabs / search
row list | preview viewport
footer
```

`large/50_checks_top.txt` is the clearest frame. The preview has enough vertical room to show:

- the PR identity header
- the title
- state and branch line
- author line
- tab row
- aggregate checks box
- beginning of the detailed check list

The tradeoff is table compression. In the same frame, the left table truncates the repo/title heavily.

## Sidebar Viewport Mechanics

`sidebar.Model.View()` renders three cases:

1. Closed preview: empty string.
2. Open preview without data: centered `Nothing selected...`.
3. Open preview with data: `viewport.View()` plus a scroll percentage.

The scroll percentage at the bottom of captures (`0%`, `3%`, `13%`, `28%`, `100%`) is owned by the generic sidebar viewport. It is not PR-specific. Any content rendered into the sidebar gets the same pager behavior.

`Ctrl+d` and `Ctrl+u` map to `viewport.HalfPageDown()` and `viewport.HalfPageUp()`. This is why scrolling advances differently across terminal sizes: the viewport height is different, so a half-page scroll means a different number of rendered lines.

The captures show this clearly:

- `medium/01_overview_down.txt` is already at `3%` after one half-page scroll and exposes the expanded body.
- `narrow/08_overview_down.txt` is at `13%` and still only partway through the Overview document.
- `medium/60_files_top.txt` reaches `100%` because the Files tab is shorter than the viewport and the pager rounds to the end.

## PR Header

Every PR tab starts with the same `viewHeader()`:

```text
repo/name · #number
title block
state pill + base branch -> head branch
author + age + author association
tab carousel
bottom border
```

For the captured PR, the repeated identity chrome is:

- `openclaw/openclaw · #81834`
- `feat(senseaudio): add SenseAudio TTS provider`
- `Open`
- `main` to `feat/senseaudio-tts`
- `@KLilyZ`, `1mo ago`, `none`
- `Overview`, `Activity`, `Commits`, `Checks`, `Files Changed`

The header is valuable because it preserves context while switching tabs, but it is expensive in small terminals. In `narrow/70_checks_top.txt`, the bottom preview shows the row list, separator, PR header, title, state, author, and tab row, but the useful checks content is below the fold. In other words, the header is optimized for orientation, not dense reading.

For a dedicated single-resource viewer, this argues for a sticky compact header rather than a large repeated header.

## Overview Tab

`viewOverviewTab()` composes:

1. requested reviewers
2. labels
3. summary/body
4. changes overview
5. checks overview
6. optional input box

The body renderer:

- strips HTML comments with `htmlCommentRegex`
- strips table-like lines with `lineCleanupRegex`
- trims whitespace
- renders Markdown through Glamour using the current content width
- folds to eight rendered lines unless `summaryViewMore` is true
- appends a centered `Press e to read more...` hint when folded

The manual capture pressed `e`, so `medium/01_overview_down.txt` shows an expanded summary:

```text
## Summary
• Problem: `senseaudio` bundled plugin only has ASR; no TTS.
• Why it matters: completes the round trip in the same plugin...
• What changed: registers a `speechProvider` in `extensions/senseaudio/`.
```

The Markdown rendering is width-sensitive. In medium mode, long bullets wrap over a couple of lines. In narrow mode, the same Overview becomes a long scroll document; `narrow/08_overview_down.txt` shows checklist items and linked issue/PR content much later in the scroll sequence.

The Overview tab also exposes an important UX pattern: it is not just the PR body. It is a compact decision surface. The body is followed by:

- changed file and commit counts
- additions/deletions
- last updated time
- review state
- check state
- mergeability

That makes Overview a good dashboard preview, but it also means long body reading competes with summary metadata inside the same viewport.

## Activity Tab

PR Activity is rendered by `prview/activity.go`. It builds a single chronological feed from three sources:

| Source | Included data |
| --- | --- |
| review threads | path, line, author, body, updated time |
| issue-style PR comments | author, body, updated time |
| reviews | reviewer, review state, body, updated time |

The renderer normalizes these into `RenderedActivity` values and sorts them ascending by `UpdatedAt`.

Each comment renders as:

```text
rounded author/time header
optional path#line for review-thread comments
Markdown body
```

Each review renders as:

```text
review decision glyph + author + reviewed time
Markdown review body
```

The capture `large/20_activity_top.txt` shows the resulting structure well:

- the tab heading says `7 comments`
- a `github-actions` comment appears first
- a long maintainer/review comment follows
- Markdown headings, bullets, code spans, and quoted text are preserved

This design creates a readable text feed, but it is intentionally flat. It does not expose a tree of review threads, does not provide jump anchors, and does not distinguish bot comments from human review comments beyond author text. For gh-dash, that is acceptable because it is a preview. For a dedicated inspector, the same data would benefit from sectioning and navigation.

## Commits Tab

`renderCommits()` uses enriched `AllCommits.Nodes`.

Each commit row renders:

```text
commit glyph + headline + horizontal fill + abbreviated SHA
vertical continuation glyph + author + relative committed time + optional check count
```

The implementation truncates the left headline with `ansi.Truncate()` after reserving space for the right-aligned SHA. It then fills the middle with a faint horizontal line.

This is one of the strongest terminal layout choices in the preview:

- the headline remains left-scannable
- the SHA remains right-scannable
- the row keeps a stable shape as width changes
- status metadata can attach beneath without disrupting the title line

The captures show this remains legible in medium and large sizes. In narrow bottom preview, it still requires scrolling because the repeated header consumes most of the initial viewport.

## Checks Tab

The Checks tab renders two layers:

```text
renderChecksOverview()
blank line
renderChecks()
```

### Aggregate Check Box

`renderChecksOverview()` combines:

1. review state
2. check state
3. merge state

It uses a rounded border whose color is semantic:

- failure if any category fails
- success if review, checks, and merge are all successful
- faint otherwise

For `#81834`, `large/50_checks_top.txt` shows:

```text
Reviews
  None requested

All checks have passed
  38 skipped, 2 neutral, 86 successful
  [proportional bar]

Merging is blocked
```

This is a dense and useful summary. It separates "checks passed" from "merge blocked", which is important: CI state and mergeability are related but not equivalent.

### Check Statistics

`getChecksStats()` derives counts from:

- `StatusCheckRollup.Contexts.CheckRunCountsByState`
- `StatusCheckRollup.Contexts.StatusContextCountsByState`
- `CheckSuites` not yet represented in the rollup, especially `ACTION_REQUIRED`, `QUEUED`, `PENDING`, and `WAITING`

The proportional bar is built from repeated `▃` glyphs. Sections are ordered:

1. failed
2. awaiting approval
3. in progress
4. skipped plus neutral
5. succeeded

For the captured PR, the useful aggregate line is:

```text
38 skipped, 2 neutral, 86 successful
```

The bar visually emphasizes that most checks succeeded, while a large skipped/neutral segment exists. It is informative at medium/large widths. At narrow widths, it becomes expensive and can push useful detail out of view.

### Detailed Checks

`renderChecks()` starts with `All Checks`, then lists checks in this order:

1. awaiting approval workflows
2. pending workflows
3. failures
4. waiting checks
5. everything else

It gets individual checks from the latest commit's status rollup contexts:

- `CheckRun`
- `StatusContext`

It also checks branch protection rules for required contexts that have not reported yet and displays those as pending.

The medium capture `medium/54_checks_down.txt` shows a typical detailed section:

```text
 KLilyZ/Labeler/backfill-pr-labels
 clawsweeper[bot]/Mantis Telegram Desktop Proof/Resolve Mantis request
 KLilyZ/CI/security-fast
 KLilyZ/Workflow Sanity/actionlint
```

This is useful but long. `#81834` has 126 aggregate checks in the captured status summary. A linear list works for a preview, but a single-PR viewer should group and collapse checks by state and workflow.

## Files Changed Tab

`renderChangedFiles()` lists enriched changed files. Each row renders:

```text
+additions  -deletions  change icon  path
```

`renderFile()` reserves fixed widths for addition and deletion counts, then wraps long paths manually when the path exceeds remaining width.

`medium/60_files_top.txt` shows:

```text
+1   -1    docs/plugins/plugin-inventory.md
+1   -1    docs/plugins/reference.md
+3   -3    docs/plugins/reference/senseaudio.md
+74  -14   docs/providers/senseaudio.md
+3   -1    extensions/senseaudio/index.ts
```

This tab is a preview, not a file browser. It shows file names and change footprint, but no diff hunks, no file filtering, and no way to jump from a review-thread comment to a file. That is a reasonable gh-dash tradeoff because the main product is a dashboard. For a `ghzoom`-style tool, the Files area should probably be promoted to a navigable file list with optional diff preview.

## Issue Preview

Issue previews reuse the same sidebar and viewport but use a simpler renderer.

`issueview.Model.View()` creates one document:

```text
issue number + repo
title block
state pill
author + age + author association
labels
body
comments
optional input box
```

There is no tab carousel for issues. That matches the data shape: an issue is primarily body plus comments and metadata. PRs need tabs because checks, commits, files, reviews, and review threads are all first-class dimensions.

Issue body rendering follows the same cleanup and Markdown path:

- strip HTML comments
- strip table-like lines
- trim
- render Markdown through Glamour
- show `No description provided.` if empty

Issue comments are sorted ascending by updated time and rendered with the same author/time rounded header pattern used by PR comments.

The issue preview is therefore structurally simpler but still shares these shell mechanics:

- same generic `sidebar.Model`
- same scroll percentage
- same bottom/right layout logic
- same footer and root composition
- same input-box overlay pattern for comments/labels/assignment

## Styling Analysis

The UI is built from styled terminal strings, not retained widgets:

- Bubble Tea owns the event loop.
- Lip Gloss builds strings with width, padding, borders, colors, and joining.
- Bubbles viewport clips and scrolls the final string.
- Glamour renders Markdown into ANSI-styled terminal text.
- `zone.Scan()` enables mouse zones around the final output where needed elsewhere in the UI.

The preview has a consistent visual language:

| Pattern | Purpose |
| --- | --- |
| highlighted header line | stable resource identity |
| highlighted title block | title prominence |
| pill styles | state, labels |
| underlined section headings | body/check/activity sections |
| rounded bordered boxes | grouped status and comments |
| faint text | metadata and timestamps |
| semantic glyphs/colors | pass/fail/waiting/review states |
| pager percentage | scroll affordance |

The captures show that this visual system is strongest when there is enough width and height for both identity and content. In `large/50_checks_top.txt`, the Checks tab reads as a coherent inspector. In `narrow/70_checks_top.txt`, the same design is context-heavy and content-light.

## Interaction Analysis

Relevant keys from the capture flow:

| Key | Effect |
| --- | --- |
| `p` | open/close preview |
| `P` | toggle preview position |
| `g` | jump to first row, which also syncs sidebar content |
| `j` / `k` | move selected row |
| `Ctrl+d` / `Ctrl+u` | preview half-page down/up |
| `]` / `[` | next/previous PR preview tab |
| `e` | expand folded PR summary |

There are three separate state machines interacting:

1. section table selection
2. sidebar viewport scroll offset
3. PR tab carousel and summary expansion

When the selected row changes, `onViewedRowChanged()` resets the PR view to Overview, folds the summary, synchronizes the sidebar, starts enrichment, and scrolls to top. This keeps row navigation predictable, but it also means per-PR preview state is not preserved when moving around the list.

That is correct for a dashboard. It may not be correct for a deep single-resource viewer, where preserving tab, scroll, expanded comments, and focused section is more valuable.

## Capture-Backed UI Findings

### 1. Preview reads like a dashboard, not a document

The Overview tab is a blend of document body and decision metadata. That is ideal when the user is scanning rows and wants quick context. It is less ideal when the user wants to read a long body or comment thread in full.

Evidence:

- `medium/01_overview_down.txt` starts body reading after the list, separator, resource header, labels, and Summary heading.
- `narrow/08_overview_down.txt` shows that long body content becomes a many-step scroll sequence.

### 2. Right preview is much better for detail

The large right-preview capture shows the Checks tab as a coherent panel. It can show summary and details together. Bottom preview cannot do this unless the terminal is very tall.

Evidence:

- `large/50_checks_top.txt` shows aggregate checks and the beginning of `All Checks`.
- `medium/50_checks_top.txt` only reaches the first few lines of the aggregate box.
- `narrow/70_checks_top.txt` does not reach the checks body at all.

### 3. The repeated header is the main small-screen cost

The repeated PR header provides identity, but in bottom preview it consumes a large fraction of the available area.

In `80x24`, the visible preview strip starts after the list and separator. The PR identity, title, branch line, author line, and tab row leave almost no room for tab content. For a single-resource app, the header should shrink after orientation is established.

### 4. Checks overview is the strongest reusable component

The aggregate check box cleanly separates review, check, and merge states. It exposes a subtle but important state: "All checks have passed" while "Merging is blocked".

For `ghzoom`, this pattern should be preserved, but the detailed check list should be grouped by state and workflow.

### 5. Activity needs richer structure for deep review

The flat chronological Activity feed is readable, but it hides useful axes:

- bot vs human
- issue comment vs code review comment
- resolved vs unresolved review thread
- file path and line
- approval/change request/comment-only review

The captured `large/20_activity_top.txt` demonstrates both the benefit and limitation: the Markdown body is readable, but one long comment dominates the viewport.

## Implications for ghzoom

gh-dash is optimized for "peek at selected row while staying in the dashboard." A dedicated issue/PR viewer should keep the good parts but change the default layout.

Recommended architecture:

```text
Resource resolver
  -> GitHub data store
  -> resource shell
  -> tab/section router
  -> scrollable document viewport
  -> action/input overlays
```

Recommended shared shell:

- compact resource header
- state/review/check/merge summary line
- tabs for PRs
- simple section navigation for issues
- persistent scroll indicator
- mouse hit areas for tabs, comments, checks, files, and links

Recommended PR views:

- Overview: compact body plus metadata summary
- Activity: grouped comments, reviews, review threads
- Checks: aggregate box plus grouped check list
- Files: full changed-file list and optional diff preview
- Commits: commit list with per-commit status
- Links: detected issue/PR references

Recommended issue views:

- Body
- Activity
- Metadata
- Links/timeline

Key differences from gh-dash:

- Default to full-screen detail, not list-plus-preview.
- Preserve tab and scroll state per resource.
- Treat comments/checks/files as navigable items, not just strings in a viewport.
- Use compact headers on small terminals.
- Keep aggregate decision state visible without sacrificing content.
- Make long sections collapsible or jumpable.

## Summary

gh-dash renders previews by turning domain-specific PR/issue data into styled strings, then handing those strings to a generic scrollable sidebar viewport. PR preview is tabbed and lazy-enriched; issue preview is a simpler single-document render. Layout is controlled globally and adapts between bottom and right placement, with `auto` protecting the main table from becoming too narrow.

The captures of `openclaw/openclaw#81834` show a strong dashboard preview experience at medium and large sizes, especially for aggregate CI and PR metadata. They also show the limits of that design for deep reading: the repeated header, bottom split, flat activity feed, and linear checks list are all reasonable dashboard tradeoffs but should be redesigned for a dedicated individual issue/PR viewer.
