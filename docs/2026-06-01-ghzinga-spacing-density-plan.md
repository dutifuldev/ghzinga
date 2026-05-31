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

The default `comfortable` mode should read closer to `gh dash`: list rows still
fit in a terminal, but each review item, file, check, or commit has enough
breathing room that the eye can re-anchor while scanning. `compact` should keep
the previous dense behavior for small terminals and users who want maximum row
count.

## Density Rules

- `comfortable` remains the default config and CLI mode.
- `compact` keeps the old dense row output except for explicit blanks already
  required by a view.
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
- Click targets stay on the visible control row. Blank spacing rows are never
  clickable.

## Settings Copy

The settings view should explain the tradeoff directly:

- `comfortable`: gh-dash-like breathing room for long review sessions
- `compact`: dense rows for smaller terminals

## Verification

- Unit tests should compare comfortable and compact output for repeated rows.
- Render-to-click tests should keep passing because inserted blank rows must not
  shift hit areas away from the rows actually rendered.
- UX captures should be refreshed after source changes so the saved images show
  the default comfortable spacing.
