# ghzinga

`ghzinga` is a terminal UI for viewing one GitHub pull request or issue.

It is similar in spirit to `gh dash`, but focused on a single PR or issue
instead of a dashboard list. It uses Ratatui and Crossterm, so the interface is
not just text output: you can click tabs, click links, expand rows, scroll with
the mouse wheel, drag the scrollbar, and use keyboard shortcuts.

<img width="1089" height="672" alt="ghzinga-demo" src="https://github.com/user-attachments/assets/d44c0f11-b15a-4ff0-aec3-8ddc58a83f5d" />


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
```

`ghzinga` reuses your existing GitHub CLI login through `gh auth token`. You can
also set `GH_TOKEN` or `GITHUB_TOKEN` to override that token. Public repositories
can fall back to unauthenticated GitHub data when credentials are unavailable.

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

## Interaction

The UI is built for active terminal use:

- click tabs to switch views
- click GitHub links to open or navigate
- click rows and `[more]` controls to expand details
- click footer actions for refresh, copy, open, settings, help, and quit
- scroll with the mouse wheel or keyboard
- drag the scrollbar when content is long
- press `?` in the app for the full keyboard help

Common keys:

- `q` or `Ctrl-C`: quit
- `r`: refresh
- `Tab`, `Shift+Tab`, `Left`, `Right`: switch tabs
- `Up`, `Down`, `PageUp`, `PageDown`, `Home`, `End`: scroll
- `Enter`: activate the first visible link or action
- `y`: copy the first visible GitHub URL
- `o`: open the first visible GitHub URL
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

## Refresh

`ghzinga` refreshes automatically every 300 seconds by default. Use
`--refresh-seconds 0` to disable automatic refresh, or press `r` to refresh
manually.

When a resource has more data than the normal GitHub API depth loads, ghzinga
shows a full-depth action so you can fetch the rest without restarting the app.
