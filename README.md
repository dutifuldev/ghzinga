# ghzinga

`ghzinga` is a small terminal UI for keeping GitHub pull requests and issues
open on the side while you work.

It is not trying to be a full GitHub client. It is for maintainers who want a
faster-than-the-web-UI view of the current status, comments, checks, files, and
links for a PR or issue, with automatic refresh. It is similar in spirit to
[`gh dash`](https://github.com/dlvhdr/gh-dash), but focused on a single item
first instead of a dashboard list.

`ghzinga` is inspired by [`Herdr`](https://herdr.dev/) and built with
[`Ratatui`](https://ratatui.rs/) and
[`Crossterm`](https://github.com/crossterm-rs/crossterm), so the terminal UI is
interactive: click tabs and links, expand rows, scroll with the mouse wheel,
drag the scrollbar, or use keyboard shortcuts.



https://github.com/user-attachments/assets/1078941e-f83f-4d6f-aceb-695a22015580



## Install

Install from crates.io:

```sh
cargo install ghzinga
```

Install from a local checkout:

```sh
cargo install --path .
```

This installs two equivalent commands:

- `gzg`
- `ghzinga`

## Usage

Open a pull request or issue:

```sh
gzg openclaw/openclaw#81834
gzg https://github.com/openclaw/openclaw/pull/81834
gzg https://github.com/openclaw/openclaw/issues/88499
gzg 81834
```

When you run `gzg` from inside a git checkout with a GitHub remote, a bare
number uses that repository. For example, `gzg 81834` inside a checkout of
`openclaw/openclaw` opens `openclaw/openclaw#81834`.

Run `gzg` again from the same terminal context to restore the last ghzinga
dashboard for that pane, tmux pane, Herdr pane, working tree, or named session.
Use `--new` to start a separate saved session, `--no-restore` to ignore saved
state for one run, or `--session <name>` to pick a specific saved session.

`ghzinga` reuses your existing GitHub CLI login through `gh auth token`. You can
also set `GH_TOKEN` or `GITHUB_TOKEN` to override that token. Public repositories
can fall back to unauthenticated GitHub data when credentials are unavailable.

Useful launch options:

```sh
gzg --tab files openclaw/openclaw#81834
gzg --theme solarized --spacing compact openclaw/openclaw#81834
gzg --width-mode full --scrollbar always openclaw/openclaw#81834
gzg --api-depth full openclaw/openclaw#81834
gzg --refresh-seconds 0 openclaw/openclaw#81834
```

## What You Get

For pull requests, ghzinga shows:

- overview and conversation
- author, labels, branches, review state, and merge/check status
- activity, reviews, review comments, and linked issues or PRs
- commits
- checks and status contexts
- changed files with expandable patch context

For issues, ghzinga shows:

- overview and conversation
- author, labels, assignees, state, milestones, and projects
- timeline activity and comments
- linked issues or PRs

Resources load progressively. The TUI shell appears immediately, core PR/issue
data replaces the loading placeholder as soon as GitHub returns it, and slower
details such as timeline pages, checks, review threads, and file patches fill in
afterward. File diffs are fetched lazily when the Files tab needs them.

## Interaction

The UI is built for active terminal use:

- click tabs to switch views
- click the top-right plus button, or press `n`, to open another PR or issue in a new tab
- when multiple resources are open, click the resource tabs to switch or close them
- click GitHub issue/PR links to choose between opening here or in a new tab
- click same-resource comment links to focus the matching Activity entry
- click rows and `[more]` controls to expand details
- click footer actions for refresh, expand/collapse, settings, help, and quit
- scroll with the mouse wheel or keyboard
- drag the scrollbar when content is long
- press `?` in the app for the full keyboard help

Common keys:

- `q`: close the active modal/help/settings layer, then ask before quitting
- `Ctrl-C`: quit immediately
- `r`: refresh
- `n`: open another PR or issue in a resource tab
- `o`: open a PR or issue in the current tab
- `x`: close the current resource tab
- `Ctrl-C` in the open-resource modal: clear input, then close when empty
- `Left`, `Right`, `h`, `l`: switch Overview, Activity, and other content tabs
- `Tab`, `Shift+Tab`, `Shift+Left`, `Shift+Right`: switch PR/issue tabs
- `Up`, `Down`, `j`, `k`, `PageUp`, `PageDown`, `Home`, `End`: scroll
- `Enter`: activate the first visible link or action
- `y`: copy the first visible GitHub URL
- `s`: settings
- `?`: help

## Configuration

The config file is:

```text
~/.config/ghzinga/config.toml
```

Default config:

```toml
[ui]
theme = "default"
symbols = "emoji"
spacing = "comfortable"
width_mode = "fixed"
fixed_width = 118
scrollbar = "on-scroll"
```

You can change theme, symbols, spacing, width, and scrollbar behavior from the
settings view inside the app.

Supported themes:

```text
default, catppuccin, catppuccin-latte, terminal, tokyo-night,
tokyo-night-day, dracula, nord, gruvbox, gruvbox-light, one-dark,
one-light, solarized, solarized-light, kanagawa, kanagawa-lotus,
rose-pine, rose-pine-dawn, vesper
```

Supported setting values:

- `symbols`: `emoji`, `ascii`
- `spacing`: `comfortable`, `compact`
- `width_mode`: `fixed`, `full`
- `fixed_width`: clamped between `72` and `180`
- `scrollbar`: `always`, `on-scroll`, `hidden`

## Sessions

`ghzinga` saves open PR/issue tabs and UI state under:

```text
~/.local/state/ghzinga
```

Cached GitHub resource snapshots are stored separately under:

```text
~/.cache/ghzinga
```

Environment overrides:

- `GZG_STATE_HOME`: alternate session state directory
- `GZG_CACHE_HOME`: alternate resource cache directory
- `GZG_RUNTIME_HOME`: alternate runtime socket directory for live session control
- `GZG_SESSION`: default named session

Session commands:

```sh
gzg sessions
gzg session show <id-or-name>
gzg session rename <id-or-name> <name>
gzg session delete <id-or-name>
```

Control a running session from another shell:

```sh
gzg open dutifuldev/ghzinga#29
gzg open --session <id-or-name> dutifuldev/ghzinga#29
gzg open --session <id-or-name> dutifuldev/ghzinga#29 dutifuldev/ghzinga#32
gzg set --session <id-or-name> theme solarized
gzg set --session <id-or-name> symbols emoji
gzg set --session <id-or-name> spacing comfortable
gzg set --session <id-or-name> width-mode fixed
gzg set --session <id-or-name> fixed-width 118
gzg set --session <id-or-name> scrollbar on-scroll
```

`gzg sessions` includes each session's running/saved status, active resource,
and resource count. If the session is running, control commands update the live
TUI without stealing terminal focus. `gzg open` adds or focuses resource tabs in
that session, including while another resource is still loading. If the target
session is not running, `gzg open` updates the saved session so the resources
appear on the next restore.

## Refresh

`ghzinga` refreshes automatically every 300 seconds by default. Use
`--refresh-seconds 0` to disable automatic refresh, or press `r` to refresh
manually.

When a resource has more data than the normal GitHub API depth loads, ghzinga
shows a full-depth action so you can fetch the rest without restarting the app.
Normal successful refreshes stay quiet; status messages are reserved for active
loading, changed sections, warnings, and errors.

## License

[MIT](LICENSE)
