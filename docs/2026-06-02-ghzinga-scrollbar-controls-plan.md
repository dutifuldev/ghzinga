---
title: Scrollbar Controls Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-02
---

# Scrollbar Controls Plan

## Scope

Remove the verbose footer hint line permanently and make scroll orientation live
in the right-edge scrollbar instead.

The footer should only show:

- persistent command buttons
- transient status, loading, save, and error messages

It should not show the always-on `scroll 0/155 0% | tab Overview | ...` command
cheat sheet. Keyboard shortcuts already live in Help and README.

## Scrollbar Visibility

Add a persisted setting under `[ui]`:

```toml
scrollbar = "on-scroll"
```

Supported values:

- `always`: show the scrollbar whenever the content can scroll.
- `on-scroll`: show it while keyboard or wheel scrolling is active, then fade.
- `hidden`: never render the scrollbar.

Settings should expose all three rows and a keyboard shortcut:

- `b`: cycle `on-scroll` -> `always` -> `hidden`
- mouse click: choose an explicit scrollbar mode row

CLI should support the same override for capture and one-off use:

```bash
gzg openclaw/openclaw#81834 --scrollbar always
```

## Dragging

Ratatui supports rendering a `Scrollbar` with `ScrollbarState`; it does not own
mouse interaction. Dragging should be handled by the app:

- Register a hit area over the visible right-edge scrollbar track.
- A left-click on the track jumps to the corresponding content position.
- A left-button drag keeps scrolling as the pointer moves.
- Releasing the left button ends the drag.
- Dragging clamps above and below the track, so users can drag to exact top or
  bottom without overshooting.

The scrollbar hit area should only exist while the scrollbar is visible. Hidden
mode should not reserve an active mouse target.

## Verification

- Config parsing and saving cover the new `ui.scrollbar` setting.
- CLI parsing covers `--scrollbar`.
- Reducer tests cover settings shortcut, settings click, track click, drag, and
  release.
- Renderer tests cover no verbose footer hint, always/on-scroll/hidden
  visibility, and visible scrollbar hit target registration.
- Capture evidence is refreshed because footer/help/settings text changes.
