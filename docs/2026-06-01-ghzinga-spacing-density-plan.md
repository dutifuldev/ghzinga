---
title: ghzinga Spacing Density Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-01
---

# ghzinga Spacing Density Plan

`ghzinga` already supports `comfortable` and `compact` spacing. The next step is
to make those modes visibly different in the content area instead of only adding
one blank row after section rules.

## Design Goal

The spacing control should behave like Gmail's density setting: users can pick a
readable default or a denser screen without changing what the TUI can render.
The default `comfortable` mode should read closer to `gh dash`: list rows still
fit in a terminal, but each review item, file, check, or commit has enough
breathing room that the eye can re-anchor while scanning. `compact` should keep
the previous dense behavior for small terminals and users who want maximum row
count.

## Density Rules

- `comfortable` remains the default config and CLI mode.
- `compact` keeps the old dense row output except for explicit blanks already
  required by a view.
- Comfortable content gets a two-column horizontal gutter when the terminal is
  wide enough. This follows the gh-dash preview pattern of giving preview text
  left/right padding instead of starting every readable line at column zero.
  Compact preserves full-width output for smaller terminals and maximum density.
- Comfortable chrome gets the same two-column horizontal gutter as content for
  the header, tab selector row, status band, and footer controls. The left and
  right padding must be equal so the upper navigation, title, status, and body
  align as one readable column.
- Comfortable tab selectors reserve fixed chrome space for the nav buttons, one
  blank breathing row below those buttons, and then a continuous separator line.
  The separator belongs to the navbar, not to the scrollable document, so it
  remains stable while the user scrolls.
- Comfortable content normalizes accidental leading/trailing blank rows, then
  adds exactly one top padding row after the navbar separator and exactly one
  bottom padding row after the last item. The first and last content items
  should never press directly against fixed chrome, and builders should not need
  to hard-code terminal padding rows.
- Repeated comfortable items keep vertical breathing room between entries. The
  padding scale should preserve scannability without making compact mode less
  dense.
- Comfortable read-heavy tabs cap the content column on very wide terminals.
  `gh dash` usually renders previews in a bounded split pane, so prose and
  comment threads do not stretch across the entire terminal. `ghzinga` is a
  full-screen single-resource viewer, so comfortable mode should recreate that
  bounded reading column for Overview, Activity, Commits, Checks, Links, Help,
  and Settings. The Files tab is the exception: changed-file summaries and patch
  diffs keep the full available width so code remains readable.
- Wrapped continuation lines in comfortable mode get a two-column hanging
  indent when there is enough width. This makes long comments and PR bodies
  scan more like the gh-dash preview pane: the first line anchors the item, and
  later lines visually belong to it instead of restarting as separate rows.
- Compact mode keeps wrapped lines flush-left so it preserves the maximum
  amount of horizontal space in narrow terminals.
- Section rules get a blank row after them when the next line has content.
- Repeated content groups get one blank row between groups:
  - chronological overview entries
  - activity entries
  - commit rows
  - check rows inside a status group
  - changed file rows
  - link rows
- Existing blank rows are reused; the renderer must not stack multiple blank
  rows just because a builder already included one.
- Scrollbar state represents the real rendered content length and viewport
  height. The current scroll offset is mapped to Ratatui's full scrollbar
  position range, so the thumb reaches the bottom when the last scrollable
  position is reached, including the comfortable bottom padding row. At the top
  and bottom endpoints, the renderer repaints one contiguous endpoint thumb so
  rounding cannot leave a visually stranded thumb above or below the edge.
- Repeated-row builders should prefer semantic gap markers over hard-coded
  blank rows. A hard blank row is still valid for a deliberate internal break,
  such as separating an expanded detail block from the next heading, but row
  density should otherwise be owned by the spacing mode.
- Click targets stay on the visible control row. Blank spacing rows are never
  clickable. Comfortable gutters shift the hit rectangles with the visible
  content so the clickable target still matches what the user sees.
- Tab-level controls such as `[expand all]` and `[collapse all]` should be
  fixed footer commands, appended as the final visible action after refresh,
  copy, open, settings, help, and quit when the active tab has expandable
  content. They apply to every expandable row in the current tab/view and must
  never require scrolling through the document to find them. The scroll cue
  stays in footer message text, not in the command row, so it cannot separate
  expand/collapse from the other buttons.

## Settings Copy

The settings view should explain the tradeoff directly:

- `comfortable`: Gmail-style comfortable density with gh-dash-like row spacing
  and content gutter for long review sessions
- `compact`: dense rows for smaller terminals

## Verification

- Unit tests should compare comfortable and compact output for repeated rows,
  content gutters, and the wide-terminal readable-column cap.
- Render-to-click tests should keep passing because inserted blank rows must not
  shift hit areas away from the rows actually rendered.
- UX captures should be refreshed after source changes so the saved images show
  the default comfortable spacing.
