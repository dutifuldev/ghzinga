---
name: ghzinga
description: Use when an agent should open, inspect, or monitor a single GitHub issue or pull request with the ghzinga terminal UI.
---

# Ghzinga

Ghzinga is a terminal UI for keeping one GitHub issue or pull request open while
working. It is focused on a single resource, not a dashboard of many resources.

Use it when the user wants a sidecar view of an issue or PR, wants to monitor
comments/checks/files/activity, or asks to open a GitHub item in `ghzinga` or
`gzg`.

## Commands

The installed commands are equivalent:

```bash
gzg openclaw/openclaw#81834
ghzinga openclaw/openclaw#81834
gzg https://github.com/openclaw/openclaw/pull/81834
gzg https://github.com/openclaw/openclaw/issues/88499
```

If `ghzinga` or `gzg` is missing but the source checkout exists, install from
the checkout:

```bash
cd /Users/onur/repos/ghzinga
cargo install --path .
```

## Authentication

Ghzinga reuses the GitHub CLI token via `gh auth token`. `GH_TOKEN` or
`GITHUB_TOKEN` can override it. Public repositories can fall back to
unauthenticated GitHub data when credentials are unavailable.

## Useful Options

Render once and exit, useful for captures or quick checks:

```bash
gzg openclaw/openclaw#81834 --once
```

Choose the initial tab:

```bash
gzg openclaw/openclaw#81834 --tab checks
```

Disable auto-refresh:

```bash
gzg openclaw/openclaw#81834 --refresh-seconds 0
```

Use a local normalized fixture:

```bash
gzg --offline-fixture fixtures/pr-81834.json --once
```

## What It Shows

For pull requests, ghzinga shows overview, conversation, author, labels,
branches, reviews, merge/check status, activity, commits, checks, changed files,
patch context, and links.

For issues, ghzinga shows overview, conversation, author, labels, assignees,
state, milestones, projects, timeline activity, comments, and links.

## Controls

Common keys:

- `q` or `Ctrl-C`: quit.
- `r`: refresh.
- `Tab`, `Shift+Tab`, `Left`, `Right`: switch tabs.
- `Up`, `Down`, `PageUp`, `PageDown`, `Home`, `End`: scroll.
- `Enter`: activate the first visible link or action.
- `y`: copy the first visible GitHub URL.
- `o`: open the first visible GitHub URL.
- `s`: settings.
- `?`: help.

Mouse interaction is supported for tabs, links, rows, footer actions, scrolling,
and scrollbar dragging. Use `--no-mouse` if mouse capture would interfere with
the surrounding terminal workflow.

## Configuration

The config file is:

```text
~/.config/ghzinga/config.toml
```

UI settings include theme, symbol style, spacing, width mode, fixed width, and
scrollbar behavior. These can also be changed in the in-app settings view.
