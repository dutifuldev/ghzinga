---
title: ghzoom Responsive Chrome Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# ghzoom Responsive Chrome Plan

## Goal

Make the fixed TUI chrome behave elegantly across narrow, medium, and large
terminals. The body content already uses display-width-aware wrapping; this
slice focuses on the chrome around it: header, tabs, status band, and footer
controls.

## gh-dash Ideas To Reuse

`gh-dash` treats preview chrome as width-sensitive units rather than one long
string. The useful patterns for `ghzoom` are:

- measure rendered width with terminal display width, not byte length
- wrap pill-like labels onto new rows before they overflow
- truncate only the one item that cannot fit on a row
- keep the scrollable preview/content viewport independent from fixed chrome
- reserve extra rows for narrow terminals instead of letting controls overlap

`ghzoom` should keep its Ratatui implementation simpler than `gh-dash`, but the
same rules apply: controls and status chips are measured, placed, and clipped
before Ratatui renders them.

## Current Gap

Tabs and footer controls already wrap between rows, and the layout reserves more
rows at narrow widths. Status chips also wrap between chips.

The remaining weak spot is oversized individual chips and control labels. For
example, a long assignee list, long refresh status, or an extremely narrow
terminal can still force truncation at the chip boundary. That avoids overlap,
but it hides useful information too early and can make the chrome feel abrupt.

## Plan

1. Keep the existing `ViewRects::compute` breakpoints and row reservations.
2. Add a reusable displayed-label helper for tabs and footer controls:
   - use the full label when it fits
   - truncate the label to the available row width when it is the first item on
     a row and cannot fit
   - register hit areas against the displayed width
3. Improve status wrapping:
   - wrap oversized status chips by display width before falling back to
     truncation
   - keep the style of every continuation line
   - preserve two-space separation between chips on the same row
   - let the existing status-area height clipping show an ellipsis only when the
     whole status band runs out of reserved rows
4. Wrap status detail messages instead of truncating them immediately.
5. Add focused rendering tests for:
   - oversized status chips staying within the terminal width without ellipsis
   - tab hit areas fitting extremely narrow terminals
   - footer controls fitting extremely narrow terminals

## Non-Goals

- No new pane layout or sidebars.
- No data-fetching changes.
- No new dependency. The existing `unicode-width` and local markdown wrapper are
  enough for this slice.

## Expected Result

At normal sizes the UI should look unchanged. At narrow sizes the chrome should
wrap into reserved rows, keep click targets aligned with visible labels, and
only replace content with an ellipsis when the terminal is genuinely too small
to show the reserved chrome.
