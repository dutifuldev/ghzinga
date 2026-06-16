# Ghzinga for Herdr

This Herdr plugin opens GitHub issue and pull request links in a ghzinga side
pane.

## Install

From the `ghzinga` repository:

```sh
herdr plugin install dutifuldev/ghzinga/plugins/herdr
```

For local development:

```sh
herdr plugin link /path/to/ghzinga/plugins/herdr
```

## Usage

Inside Herdr, Ctrl-click a GitHub issue or pull request URL:

```text
https://github.com/dutifuldev/ghzinga/pull/29
https://github.com/dutifuldev/ghzinga/issues/32
```

The plugin opens a right-side ghzinga pane next to the pane that contained the
link. Later Ctrl-clicks from the same source pane in the same Herdr session
reuse that side pane by running `gzg open --session ...`.

Ctrl-clicks from inside that ghzinga side pane also stay in the same ghzinga
pane, so following related issue/PR links does not create nested ghzinga panes.

Herdr currently routes plugin link handlers through Ctrl-click. Plain left-click
link handling would need Herdr itself to expose that behavior to plugins.

## How it works

The plugin manifest is only wiring. It registers a GitHub issue/PR link handler
and points Herdr at two ghzinga-owned entrypoints:

```text
herdr-plugin.toml -> gzg herdr-plugin open
herdr-plugin.toml -> gzg herdr-plugin viewer
```

`gzg herdr-plugin open` runs as the Herdr link action. It reads Herdr's clicked
URL and source pane environment, normalizes the GitHub issue or PR URL with
ghzinga's Rust parser, and opens a right-side Herdr plugin pane beside the source
pane.

`gzg herdr-plugin viewer` runs inside that side pane. It starts the normal
ghzinga TUI for the selected issue or PR.

The Rust entrypoint also keeps pane reuse state scoped by Herdr session/socket
and source pane. On later Ctrl-clicks from the same source pane, it focuses the
existing ghzinga pane if it is still alive and still belongs to this plugin, then
sends `gzg open --session ... <url>` into that ghzinga session. If the stored
pane is gone or belongs to something else, it opens a fresh side pane.

When the source pane is already a ghzinga plugin viewer, the entrypoint uses the
viewer-pane-to-session state written at pane creation time and opens the link in
that same ghzinga session.

## Requirements

- Herdr 0.7.0 or newer.
- `gzg` installed on `PATH`. The Herdr entrypoints call
  `gzg herdr-plugin open` and `gzg herdr-plugin viewer`.
- GitHub credentials through `gh auth token`, `GH_TOKEN`, or `GITHUB_TOKEN` for
  private repositories.

Set `GHZINGA_BIN` before launching Herdr if you need to use a non-default
ghzinga binary path for the viewer process. Normal installs do not need this.

## Verification

The production path has three layers of coverage:

- Rust unit tests for URL normalization, pane identity validation, session
  scoping, self-pane reuse, and the `gzg herdr-plugin open` entrypoint.
- Fake Herdr shell tests for open, reuse, self-pane reuse, stale-pane, and viewer
  launch behavior.
- `scripts/herdr-plugin-live-smoke.sh`, which starts a disposable Herdr session,
  links this plugin, opens a real right-side pane, verifies ghzinga rendered
  fixture content there, and verifies links from the ghzinga pane reuse that
  pane.
