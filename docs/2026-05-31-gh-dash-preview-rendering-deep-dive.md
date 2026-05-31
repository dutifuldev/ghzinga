---
title: gh-dash PR and Issue Preview Rendering Deep Dive
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# gh-dash PR and Issue Preview Rendering Deep Dive

This document analyzes how `gh dash` renders its PR and issue preview panel.
It combines source inspection from `/home/bob/repos/gh-dash` with terminal
captures saved under `captures/gh-dash-pr-81834/manual-preview/`.

The capture target was:

```text
openclaw/openclaw#81834
feat(senseaudio): add SenseAudio TTS provider
```

The captures are PR-focused. Issue preview behavior is source-backed from
`internal/tui/components/issueview/` because the terminal session captured PR
preview in detail.

## Capture Evidence

The manual capture set covers three terminal sizes:

| Size | Geometry | Directory | Captured frames |
| --- | ---: | --- | ---: |
| Narrow | `80x24` | `captures/gh-dash-pr-81834/manual-preview/narrow/` | 73 |
| Medium | `120x36` | `captures/gh-dash-pr-81834/manual-preview/medium/` | 51 |
| Large | `160x50` | `captures/gh-dash-pr-81834/manual-preview/large/` | 33 |

Key frames:

| View | Narrow | Medium | Large |
| --- | --- | --- | --- |
| Overview/body | `narrow/04_overview_down.txt` through `narrow/20_overview_down.txt` | `medium/01_overview_down.txt` through `medium/12_overview_down.txt` | `large/00_overview_top.txt` through `large/08_overview_down.txt` |
| Activity | `narrow/30_activity_top.txt` through `narrow/50_activity_down.txt` | `medium/20_activity_top.txt` through `medium/32_activity_down.txt` | `large/20_activity_top.txt` through `large/28_activity_down.txt` |
| Commits | `narrow/60_commits_top.txt` through `narrow/66_commits_down.txt` | `medium/40_commits_top.txt` through `medium/44_commits_down.txt` | `large/40_commits_top.txt` through `large/42_commits_down.txt` |
| Checks | `narrow/70_checks_top.txt` through `narrow/80_checks_down.txt` | `medium/50_checks_top.txt` through `medium/58_checks_down.txt` | `large/50_checks_top.txt` through `large/54_checks_down.txt` |
| Files | `narrow/90_files_top.txt` through `narrow/100_files_down.txt` | `medium/60_files_top.txt` through `medium/68_files_down.txt` | `large/60_files_top.txt` through `large/64_files_down.txt` |

The captures show two different preview placements:

- Narrow and medium use bottom preview.
- Large was manually toggled to right preview with `P`.

That distinction matters more than the raw terminal size. Bottom preview
preserves list width but turns the resource preview into a shallow horizontal
strip. Right preview gives the resource enough height to read, but compresses
the main list.

## System Shape

The preview is not a separate route or page. It is a generic sidebar viewport
fed by PR-specific or issue-specific string renderers.

Core source files:

| File | Role |
| --- | --- |
| `/home/bob/repos/gh-dash/internal/tui/ui.go` | global Bubble Tea model, root view composition, preview placement, row-to-sidebar synchronization |
| `/home/bob/repos/gh-dash/internal/tui/components/sidebar/sidebar.go` | generic scrollable preview viewport |
| `/home/bob/repos/gh-dash/internal/tui/components/prview/prview.go` | PR preview shell, PR header, tab routing, Overview body |
| `/home/bob/repos/gh-dash/internal/tui/components/prview/activity.go` | PR comments, review comments, reviews |
| `/home/bob/repos/gh-dash/internal/tui/components/prview/checks.go` | review/check/merge summary and detailed check list |
| `/home/bob/repos/gh-dash/internal/tui/components/prview/files.go` | changed-file list and change overview |
| `/home/bob/repos/gh-dash/internal/tui/components/issueview/issueview.go` | issue header, body, labels, comments, input overlays |
| `/home/bob/repos/gh-dash/internal/data/prapi.go` | primary and enriched PR GraphQL data shapes |
| `/home/bob/repos/gh-dash/internal/data/issueapi.go` | issue GraphQL data shape |

The effective render pipeline is:

```text
Bubble Tea update loop
  -> current section owns selected row
  -> ui.syncSidebar() reads the selected row
  -> PR/issue renderer turns domain data into a styled ANSI string
  -> sidebar.SetContent() loads that string into a Bubbles viewport
  -> ui.View() joins section view and sidebar view
  -> Bubble Tea writes the final alt-screen frame
```

The important architectural boundary is this:

```text
sidebar.Model knows how to scroll and frame content.
prview.Model and issueview.Model know what content means.
```

That boundary is the cleanest part of the implementation. `sidebar.Model` does
not need to know whether the content is a PR, issue, notification subject, or
branch preview. It owns open/closed state, dimensions, a `viewport.Model`, and a
rendered string.

## Root Composition

`ui.Model.View()` creates the final terminal frame. It opts into:

- alternate screen
- focus reporting
- mouse cell motion

Then it renders:

1. top tabs, unless in repo view
2. the current section/list
3. the sidebar preview, if open
4. footer or error line
5. overlay layers for completions/input helpers

The key composition split is:

```text
if preview position is bottom and sidebar is open:
  JoinVertical(section.View(), sidebar.View())
else:
  JoinHorizontal(section.View(), sidebar.View())
```

This exact split is visible in the captures:

- `medium/50_checks_top.txt` shows a full-width list, a horizontal separator,
  then a bottom preview.
- `large/50_checks_top.txt` shows the PR list on the left and the preview on
  the right.

The preview is therefore layout-coupled to the dashboard. It is not rendered as
the primary document. That is the core reason it works well for quick peeking
but becomes cramped for deep reading.

## Preview Placement

Preview placement is resolved globally in `ui.resolvePreviewPosition()`.

The config used for captures had:

```yaml
defaults:
  preview:
    open: false
    width: 0.55
    height: 0.60
    position: auto
```

In `auto` mode, gh-dash simulates right-preview width and asks whether the main
table would keep at least 80 columns:

```text
previewWidth = configured preview width
tableWidth = screenWidth - previewWidth
if tableWidth < 80:
  use bottom
else:
  use right
```

With `width: 0.55`, all three terminal widths would automatically choose
bottom preview:

| Terminal | Preview width | Remaining table width | Auto placement |
| --- | ---: | ---: | --- |
| `80x24` | ~44 | ~36 | bottom |
| `120x36` | ~66 | ~54 | bottom |
| `160x50` | ~88 | ~72 | bottom |

The large capture was manually toggled with `P` so that right-preview behavior
could be documented.

This explains why `large/50_checks_top.txt` is much more useful than
`medium/50_checks_top.txt` despite both using the same PR and same tab. The
right preview gives the Checks tab enough vertical room to show identity,
aggregate status, and the beginning of detailed checks in one frame.

## Sidebar Viewport

`sidebar.Model` is a thin Bubbles viewport wrapper.

State:

- `IsOpen`
- rendered `data` string
- `viewport.Model`
- program context
- empty state text

Input handling is minimal:

- page down keybinding calls `viewport.HalfPageDown()`
- page up keybinding calls `viewport.HalfPageUp()`

Rendering has only three branches:

1. closed sidebar returns an empty string
2. open sidebar with no data renders `Nothing selected...`
3. open sidebar with content renders `viewport.View()` plus scroll percentage

The percent shown at the bottom of preview frames is owned by this generic
viewport. It is not PR logic. That is why the same `0%`, `10%`, `100%` style
appears across Overview, Activity, Checks, and Files.

The capture set shows the effect clearly:

- `narrow/70_checks_top.txt` shows `0%` and only reaches the PR header/tab row.
- `narrow/73_checks_down.txt` shows `10%` and finally reaches the aggregate
  checks body.
- `medium/60_files_top.txt` shows `100%` because the Files tab content is short
  enough that the viewport is effectively at the end.

## Selected Row Synchronization

The preview is populated by the currently selected dashboard row.

When the viewed row changes, `ui.onViewedRowChanged()` does several things:

```text
fold PR summary
move PR preview tab back to Overview
sync sidebar content from selected row
start PR enrichment
scroll sidebar to top
clear notification subject state
```

This is an intentional dashboard behavior. Moving to a different row resets the
preview to a predictable first state. It is less appropriate for a dedicated
single-resource viewer, where preserving tab, scroll, and expanded/collapsed
state is usually better.

The manual capture sequence exposed one practical detail: opening preview with
`p` does not necessarily populate useful content by itself. Pressing `g` after
opening forced the selected row to synchronize into the sidebar. The capture
flow therefore used:

```text
p
g
e
Ctrl+d / Ctrl+u
]
P for large right preview
```

## PR Data Model

PR preview uses a two-tier data model.

Primary PR data is available from the list/search query and is enough for:

- row display
- title, number, repo
- state and draft/merged status
- base/head branch names
- author and author association
- labels and assignees
- review decision
- comments/review-thread counts
- current status summary
- additions/deletions
- file count

Enriched PR data is fetched lazily for the selected PR and powers the deep
preview tabs:

- full PR body
- issue-style PR comments
- review threads and review-thread comments
- reviews and review requests
- suggested reviewers
- commits
- latest commit status/check rollup
- check suites
- changed files

This is why Activity, Commits, Checks, and Files can render richer content after
selection while the table remains fast. If enrichment has not arrived yet, the
renderer can show loading states.

## PR Preview Shell

`prview.Model.View()` does two jobs:

1. Select a tab body from the PR tab carousel.
2. Join the shared PR header above that tab body.

The PR tab set is:

```text
Overview
Activity
Commits
Checks
Files Changed
```

Every PR tab repeats the same header:

1. repo/name and PR number
2. title block
3. status pill with base/head branch line
4. author/age/author-association line
5. tab carousel with bottom border

For `#81834`, the repeated identity is:

```text
openclaw/openclaw · #81834
feat(senseaudio): add SenseAudio TTS provider
Open main -> feat/senseaudio-tts
by @KLilyZ, 1mo ago, none
Overview Activity Commits Checks Files Changed
```

This stable header is useful for orientation, especially when the preview is on
the right. It is also the main small-screen cost. In `narrow/70_checks_top.txt`,
the bottom preview is so short that the header consumes the visible preview
area and the actual Checks content is below the fold.

## Overview Rendering

The Overview tab is not just a PR body. It is a compact decision surface:

1. requested reviewers
2. labels
3. PR summary/body
4. change overview
5. checks overview
6. optional input box

The summary/body path:

```text
full enriched body
  -> strip HTML comments
  -> strip table-like lines
  -> trim whitespace
  -> render Markdown through Glamour at content width
  -> fold to 8 rendered lines unless expanded
  -> add "Press e to read more..." hint if folded
```

The capture flow pressed `e` before scrolling. That is why the body is visible
in the Overview scroll sequence instead of staying folded. `medium/01_overview_down.txt`
shows the expanded body starting with:

```text
## Summary
* Problem: senseaudio bundled plugin only has ASR; no TTS.
* Why it matters: completes the round trip in the same plugin...
* What changed: registers a speechProvider in extensions/senseaudio/.
```

The UI implication is subtle: Overview tries to answer both "what is this PR?"
and "can I act on it?" in one scrollable document. That is right for a dashboard
preview. For a single-resource inspector, it creates competition between body
reading and decision metadata.

## Activity Rendering

Activity normalizes three PR data sources into one chronological feed:

| Source | Rendered as |
| --- | --- |
| review-thread comments | comment cards with optional path#line metadata |
| issue-style PR comments | comment cards |
| reviews | review decision row plus Markdown body |

The algorithm:

```text
collect review-thread comments
collect PR comments
render each comment with Markdown
collect reviews
render each review with Markdown
sort all rendered items by UpdatedAt ascending
prepend "<n> comments" heading
```

The renderer is text-first. A comment card has:

- rounded author/time header
- optional file path and line
- Markdown-rendered body

The captures show both strengths and limitations:

- `medium/20_activity_top.txt` shows the tab starting cleanly with `7 comments`
  and the first `github-actions` card.
- Later Activity frames show that a single long review/comment can dominate the
  viewport.
- The source preserves path/line metadata for review comments, but the UI does
  not make threads, resolved state, bot comments, or review decisions into
  separately navigable objects.

This is appropriate for a preview. A deeper PR viewer should preserve the
chronological feed but add structure: grouping, comment jump targets, bot/human
distinction, and file/thread navigation.

## Commits Rendering

The Commits tab renders enriched commit nodes.

Each commit row uses a strong terminal layout pattern:

```text
commit icon + headline + horizontal filler + short SHA
metadata line with author, relative time, optional status summary
```

For `#81834`, the captured content is a single commit:

```text
feat(senseaudio): add SenseAudio TTS provider ... fb948c9
```

The important UI idea is the right-aligned SHA. It keeps the commit identifier
scannable while letting the title consume the flexible middle space. This
pattern should be reused in a full single-resource viewer.

## Checks Rendering

The Checks tab is the strongest preview component.

It renders:

1. aggregate review/check/merge status box
2. detailed `All Checks` list

### Aggregate Box

`renderChecksOverview()` combines three independent states:

- review state
- check state
- merge state

The border color is derived from the worst category:

- any failure -> error border
- all successful -> success border
- otherwise -> faint border

For `#81834`, `large/50_checks_top.txt` shows the full status story in one
frame:

```text
Reviews
  None requested

All checks have passed
  38 skipped, 2 neutral, 86 successful

Merging is blocked
```

This is the key insight: "checks passed" and "mergeable" are not the same
thing. The preview makes that distinction visible without requiring the user to
open GitHub.

The status bar is proportional. It counts failed, awaiting approval, in
progress, skipped/neutral, and successful checks, then renders width-weighted
segments. For this PR, the bar is mostly success with a visible skipped/neutral
segment.

### Detailed Checks

The detailed list is built from the latest commit:

- status check rollup contexts
- check runs
- status contexts
- check suites not represented in the rollup
- required branch-protection contexts that have not reported yet

Ordering is:

1. awaiting approval workflows
2. pending workflows
3. failures
4. waiting checks
5. everything else

The list works as a preview, but it becomes long quickly. The captured PR has
`38 skipped`, `2 neutral`, and `86 successful` checks in the aggregate summary.
`large/50_checks_top.txt` can show the beginning of the detailed list, while
`narrow/70_checks_top.txt` cannot even reach the aggregate body without
scrolling. That is a direct layout consequence, not a data issue.

For a dedicated PR viewer, Checks should keep the aggregate box but make the
detailed list grouped and collapsible by state/workflow/app.

## Files Changed Rendering

Files Changed is intentionally shallow.

Each file row is:

```text
+additions  -deletions  change-type icon  path
```

`medium/60_files_top.txt` shows:

```text
+1   -1    docs/plugins/plugin-inventory.md
+1   -1    docs/plugins/reference.md
+3   -3    docs/plugins/reference/senseaudio.md
+74  -14   docs/providers/senseaudio.md
+3   -1    extensions/senseaudio/index.ts
```

The renderer reserves fixed-width columns for additions/deletions and wraps
long paths manually when needed. There is no diff rendering and no file-level
expansion. That is correct for a dashboard preview: the tab communicates change
footprint, not implementation detail.

For a `ghzoom`-style viewer, Files should become a real navigation surface:

- all changed files, not just a preview page
- filtering and grouping
- optional diff hunks
- review-thread anchors

## Issue Preview Rendering

Issue preview reuses the same sidebar viewport but has a simpler document
renderer.

`issueview.Model.View()` renders:

1. issue number/repo header
2. title block
3. state pill
4. author/age/author-association line
5. labels
6. body
7. comments/activity
8. optional input box

There is no tab carousel. That matches the underlying information shape:
issues are primarily body plus comments and metadata. PRs require separate
surfaces for checks, commits, files, reviews, and review threads.

The issue body follows the same cleanup/Markdown path:

```text
body
  -> strip HTML comments
  -> strip table-like lines
  -> trim
  -> render Markdown through Glamour
  -> fallback to "No description provided." if empty
```

Issue comments are rendered as a chronological list with author/time headers and
Markdown bodies. Architecturally, issue preview is a single rendered document;
PR preview is a tabbed inspection surface.

That difference should be preserved in ghzoom. PRs and issues should share a
shell, but they should not be forced into the same information architecture.

## Styling System

gh-dash uses Bubble Tea for the event loop, Lip Gloss for styled string
composition, Bubbles viewport for scrolling, Glamour for Markdown, and zone
scanning for mouse-aware output.

The preview styling vocabulary:

- selected-background resource header
- highlighted title block
- semantic status pills
- label pills
- underlined section headings
- rounded comment and status boxes
- faint text for timestamps/metadata
- semantic pass/fail/waiting glyphs
- scroll percentage

The style is coherent and dense. It works best when the preview has enough
height. In the narrow captures, the same styling becomes chrome-heavy because
the repeated header, tab row, borders, footer, and scroll percentage leave only
a handful of content lines.

## Mouse And Interaction Notes

The root view enables mouse cell motion. gh-dash uses Bubble Tea key/mouse
messages and zone scanning in the final rendered output.

The relevant preview keys from the capture flow are:

| Key | Effect |
| --- | --- |
| `p` | toggle preview open/closed |
| `P` | toggle preview position |
| `g` | move to first row and sync preview |
| `j` / `k` | move selected row |
| `Ctrl+d` / `Ctrl+u` | half-page preview scroll |
| `]` / `[` | next/previous PR preview tab |
| `e` | expand folded PR summary |

The important state split:

- table row selection changes preview content
- sidebar viewport offset controls scroll
- PR carousel controls active tab
- PR summary expansion controls body folding

These state machines are coordinated but not persisted per resource. Row change
resets tab and body expansion. That is appropriate for a dashboard; a
single-resource tool should preserve more local state.

## Capture-Backed UI Findings

### 1. The preview is excellent for peeking

At medium/large sizes, a user can stay in the PR list while reading enough
context to decide what to do next. The list remains visible, and the preview
communicates title, author, labels, body, checks, commits, files, and comments.

The `large/50_checks_top.txt` frame is the best example: the left side retains
the selected PR row, while the right side shows the PR's decision state.

### 2. It is not optimized for deep reading

Long bodies and long comment threads are just strings in one viewport. The
scroll percentage helps, but there is no outline, comment index, file index, or
thread tree.

The narrow Overview and Activity sequences make this visible: reaching body or
comment content requires repeated scroll actions because list and header chrome
consume so much of the screen.

### 3. Bottom preview favors list scanning over resource inspection

Bottom preview preserves the full-width table, which is the right default for
gh-dash. But it makes the preview content shallow.

Evidence:

- `narrow/70_checks_top.txt` shows only identity/header/tab content for Checks.
- `narrow/73_checks_down.txt` finally reaches the aggregate checks body at
  `10%` scroll.
- `medium/50_checks_top.txt` reaches the start of the aggregate box but cannot
  show as much as the right preview.

### 4. Right preview is much better for PR inspection

Right preview gives enough vertical space for a tab to read like an inspector.
`large/50_checks_top.txt` shows header, aggregate status, and detailed check
rows together.

The tradeoff is visible too: the left table truncates aggressively. This is the
correct tradeoff for a preview mode but not for a full-screen single-resource
tool.

### 5. The repeated header is useful but expensive

The header keeps identity stable across tabs and scroll states. It also costs
many rows:

- repo/number
- title block
- blank lines
- status/branch line
- author line
- tab row
- divider

In a dedicated viewer, this should become a compact sticky header or collapse
after scroll.

### 6. Checks are the best reusable design pattern

The aggregate Checks box is compact, semantic, and action-oriented. It captures
review state, check state, and mergeability separately.

This should be directly carried into ghzoom, with deeper interaction added
below it.

### 7. Activity needs more structure for a deeper tool

The chronological Activity feed is readable but flat. It does not elevate:

- unresolved review threads
- file/line comments
- bot comments versus human reviews
- approvals versus comments versus change requests
- comment anchors and jump targets

For gh-dash, that is fine because the preview is not the main product. For
ghzoom, those distinctions should become navigation and filtering axes.

## Implications For ghzoom

gh-dash is a dashboard with a preview panel. ghzoom should be an individual
resource viewer. That changes the default assumptions.

Recommended architecture:

```text
resource resolver
  -> GitHub gateway
  -> resource store with refresh/fingerprint
  -> shell layout
  -> resource-specific tab/section renderer
  -> scroll state and hit-area registry
  -> terminal event loop
```

What to reuse conceptually:

- generic viewport separated from domain rendering
- primary/enriched data split
- stable resource identity
- visible scroll position
- aggregate PR decision box
- Markdown-preserving body/comment rendering
- tabbed PR inspection
- simpler issue document model

What to change:

- default to full-screen resource detail instead of list-plus-preview
- use compact headers, especially at narrow sizes
- preserve tab, scroll, and expansion state per resource
- expose comments, checks, files, and links as navigable objects
- group checks by state/workflow/app
- support full changed-file lists and optional diffs
- make linked issues/PRs first-class navigation targets
- avoid requiring special terminal fonts for core meaning

## Summary

gh-dash renders PR/issue previews by composing domain-specific styled strings
into a generic scrollable sidebar viewport. PR preview is tabbed and
lazy-enriched. Issue preview is a simpler single-document render. Layout is
global and adapts between bottom and right placement, with `auto` protecting the
main table from becoming too narrow.

The terminal captures show the design tradeoff clearly. The preview is strong
when the task is "peek at a selected row without leaving the dashboard." It is
weaker when the task is "deeply inspect one PR or issue." For ghzoom, the right
move is to borrow the architectural separation and the strongest UI components,
then shift the product shape from dashboard preview to full-detail inspector.
