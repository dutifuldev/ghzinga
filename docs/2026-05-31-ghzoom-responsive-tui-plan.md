---
title: Responsive TUI Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# Responsive TUI Plan

`ghzoom` should stay usable from narrow split panes through large terminals.
The app is a focused PR/issue detail viewer, so responsive behavior should
preserve reading flow, clickable controls, and chronological context instead of
trying to show more panels.

## Design Context

- Audience: developers and maintainers reviewing one GitHub issue or pull
  request from a terminal.
- Use cases: scan status, read the chronological conversation, inspect checks,
  expand commits/files/comments, and click through to related GitHub targets.
- Tone: dense, utilitarian, quiet, and terminal-native, with color and bold used
  for emphasis instead of decorative panels.
- Devices: terminal windows from very narrow panes to large desktop terminals.
- Input: keyboard always works; mouse click targets should follow wrapped visual
  positions.

## gh-dash Ideas To Steal

`gh-dash` handles responsive UI by making dimensions explicit and re-rendering
children after terminal size changes:

- `tea.WindowSizeMsg` updates global width and height, then child components are
  resynchronized with explicit `SetWidth` and `SetHeight` calls.
- Preview placement has an `auto` mode that avoids right-side preview when it
  would starve the main table.
- Markdown rendering receives the current content width through
  `glamour.WithWordWrap(width)`.
- Small item groups, such as reviewer lists, are packed onto the current row and
  moved to the next row when the next item would exceed available width.
- Tests cover tiny or zero-width cases to prevent panics and broken layouts.

`ghzoom` is a full-screen detail view rather than a master/detail dashboard, so
the equivalent strategy is:

- Recompute every rectangle from `Frame::area()` on each render.
- Give chrome areas enough rows for wrapped controls on narrow terminals.
- Use content-driven breakpoints rather than fixed desktop assumptions.
- Treat the content viewport as a final wrapping boundary, so long rows from any
  tab cannot clip silently.
- Keep hit areas aligned with what the user sees after wrapping.

## Layout Rules

- Header, tabs, status, content, and footer are recomputed every render.
- The status area stays a horizontal band, but chips wrap across multiple rows.
- Tabs wrap across rows as complete labels; the active tab remains bold and
  colored.
- Footer buttons wrap across rows and keep one hit target per visible button.
- The content viewport owns vertical scrolling after all wrapping has happened.
- At least one content row should survive whenever the terminal is tall enough
  to show app chrome plus content.
- Overly small terminals should degrade by truncating low-priority chrome before
  losing the main content area.

## Content Wrapping Rules

- Each tab renderer should continue to wrap obvious long-form text with
  display-width-aware helpers.
- The content renderer must also perform a final pass over every `ContentRow`.
  This catches long URLs, metadata rows, branch names, file paths, check names,
  and future rows added without local wrapping.
- Wrapped rows should preserve style and interaction:
  - plain rows stay plain
  - styled rows keep their style on every visual continuation
  - clickable rows keep the same `HitTarget` on every visual continuation
- Wrapping uses terminal display width, not byte length, so emoji and wide
  Unicode characters do not shift layout.
- Long single tokens are split at character boundaries when they exceed the
  viewport width.

## Implementation Checklist

- Add a centralized `wrap_content_rows(rows, width)` pass before scroll math.
- Preserve `ContentRow.target` across wrapped continuations so clicking any
  visible part of a wrapped URL/details row works.
- Keep existing local wrapping for body text, comments, commits, patches, help,
  status chips, tabs, and footer controls.
- Add tests that render narrow content with long metadata, file paths, check
  names, URLs, and emoji without clipping or losing hit targets.
- Keep the existing `ViewRects` chrome tests and add assertions for cramped
  content behavior where needed.
