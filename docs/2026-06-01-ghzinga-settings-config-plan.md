---
title: ghzinga Settings and Config Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-01
---

# ghzinga Settings and Config Plan

`ghzinga` should have a small in-app settings surface for preferences that users
expect to change while reading a PR or issue. The first version should focus on
theme, symbol style, and spacing density because those options are safe to apply
live.

## Config Convention

Use a TOML config file:

```text
~/.config/ghzinga/config.toml
```

Respect `XDG_CONFIG_HOME` when set:

```text
$XDG_CONFIG_HOME/ghzinga/config.toml
```

Allow `GZG_CONFIG_PATH` to override the path for tests, scripts, and users who
keep dotfiles somewhere else. This mirrors Herdr's simple `config.toml` model
without requiring ghzinga to grow Herdr's full config system.

Default config:

```toml
[ui]
theme = "default"
symbols = "ascii"
spacing = "comfortable"
width_mode = "fixed"
fixed_width = 118
scrollbar = "on-scroll"
```

Supported values:

- `ui.theme`: Herdr-style built-ins: `default`, `catppuccin`,
  `catppuccin-latte`, `terminal`, `tokyo-night`, `tokyo-night-day`, `dracula`,
  `nord`, `gruvbox`, `gruvbox-light`, `one-dark`, `one-light`, `solarized`,
  `solarized-light`, `kanagawa`, `kanagawa-lotus`, `rose-pine`,
  `rose-pine-dawn`, and `vesper`
- `ui.symbols`: `ascii`, `emoji`
- `ui.spacing`: `comfortable`, `compact`
- `ui.width_mode`: `fixed`, `full`
- `ui.fixed_width`: fixed readable content width in terminal columns
- `ui.scrollbar`: `always`, `on-scroll`, `hidden`

Rules:

- Missing config file means defaults, with no warning.
- Unknown fields are ignored so future versions can add settings without
  breaking older configs.
- Invalid known values fall back to defaults and produce a visible startup
  warning in the status band.
- CLI flags override config for the current run only.
- In-app settings save the full current config atomically enough for a small
  local file: create the config directory, write TOML, and report errors in the
  status band without crashing the TUI.

## Settings UI

Open settings with:

- keyboard: `s`
- mouse: footer `[settings]`

Settings should render as a focused settings view over the normal content area,
not as a separate top-level resource tab. That keeps the resource tabs stable and
works for both issues and pull requests.

Initial controls:

- Theme rows: all built-in Herdr palettes
- Symbol rows: `ascii`, `emoji`
- Spacing rows: `comfortable`, `compact`, presented like a Gmail-style density
  choice where `comfortable` is the gh-dash-like reading mode and `compact` is
  the dense small-terminal mode.
- Width rows: `fixed`, `full`, and fixed-width presets. Files stay full-width
  because diffs need horizontal room.
- Scrollbar rows: `on-scroll`, `always`, `hidden`. Ratatui renders the
  scrollbar; ghzinga owns mouse click/drag mapping from the visible track to the
  current scroll offset.
- Current values are bold and accented.
- Mouse click applies a row immediately.
- Keyboard shortcuts while settings are open:
  - `t`: cycle theme
  - `y`: cycle symbol style
  - `p`: cycle spacing mode
  - `w`: cycle fixed/full width mode
  - `b`: cycle scrollbar visibility
  - `-` / `+`: decrease/increase fixed readable width
  - `s` or `Esc`: close settings
  - `?`: help remains available

Saving behavior:

- Applying a setting updates the live `AppState`.
- The app writes `config.toml` immediately.
- Success sets a short status message containing the config path.
- Failure keeps the live setting for the current session and shows the write
  error in the status band.

## Implementation Tasks

- Add a `config` module with `AppConfig`, `UiConfig`, load/save helpers, and
  config path resolution.
- Change CLI theme/symbol options from defaulted values to optional override
  flags.
- Initialize `AppState` from config first, then apply CLI overrides.
- Add settings state to `AppState`.
- Add settings hit targets and reducer intents for save requests.
- Render settings rows with existing Ratatui text, button, and wrapping helpers.
- Apply comfortable spacing by inserting breathing room after section rules and
  between repeated rows such as files, checks, commits, links, and timeline
  groups. Keep compact mode dense for small terminals.
- Add unit tests for config parsing/path overrides, CLI override behavior,
  settings keyboard changes, settings mouse hit targets, and persistence intent.
- Update capture evidence after source changes.
