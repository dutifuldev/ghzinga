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

## Requirements

- Herdr 0.7.0 or newer.
- `gzg` installed on `PATH`. The Herdr entrypoints call
  `gzg herdr-plugin open` and `gzg herdr-plugin viewer`.
- GitHub credentials through `gh auth token`, `GH_TOKEN`, or `GITHUB_TOKEN` for
  private repositories.

Set `GHZINGA_BIN` before launching Herdr if you need to use a non-default
ghzinga binary path for the viewer process. Normal installs do not need this.
