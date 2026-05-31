---
title: ghzinga Settings and Config Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-01
---

# ghzinga Settings and Config Plan

`ghzinga` should have a small in-app settings surface for preferences that users
expect to change while reading a PR or issue. The first version should focus on
theme and symbol style because those already exist as CLI flags and are safe to
apply live.

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
```

Supported values:

- `ui.theme`: `default`, `solarized-dark`
- `ui.symbols`: `ascii`, `emoji`

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

- Theme rows: `default`, `solarized-dark`
- Symbol rows: `ascii`, `emoji`
- Current values are bold and accented.
- Mouse click applies a row immediately.
- Keyboard shortcuts while settings are open:
  - `t`: cycle theme
  - `y`: cycle symbol style
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
- Add unit tests for config parsing/path overrides, CLI override behavior,
  settings keyboard changes, settings mouse hit targets, and persistence intent.
- Update capture evidence after source changes.
