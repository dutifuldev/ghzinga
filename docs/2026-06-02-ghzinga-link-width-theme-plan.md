---
title: Link Width Theme Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-02
---

# Link Width Theme Plan

## Scope

Finish the UI settings work around three user-facing preferences:

- The header identity is always a clickable GitHub link. When the terminal is
  wide enough, the visible label is the full `https://github.com/...` URL so
  terminal URL detection points at GitHub. When space is tight, the label falls
  back to `owner / repo #number` to avoid terminals auto-linking
  `owner/repo#number` as a bogus `http://owner/repo#number` URL.
- Reading content can use either a fixed readable width or the full terminal width.
- Theme choices include the full Herdr built-in palette set, not only the default and Solarized dark.

## Header Link

The header identity should link to a sanitized GitHub URL from `resource.url`
when GitHub supplied one and fall back to the resource id's GitHub URL
otherwise. The hit target must stay separate from normal visible URL actions so
footer `[copy]` and `[open]` still prefer visible content links and otherwise
fall back to the current resource URL.

## Width Settings

Add two config values under `[ui]`:

```toml
width_mode = "fixed"
fixed_width = 118
```

`width_mode` accepts:

- `fixed`: cap readable tabs to `fixed_width`, with comfortable-mode gutters.
- `full`: use the full available content width after gutters.

The Files tab stays full width in both modes because diffs need horizontal space. Compact spacing also stays full width because compact is intended for constrained terminals.

Settings should support live changes:

- `w` cycles fixed/full width mode while settings are open.
- `-` and `+` decrease/increase fixed width while settings are open.
- Clickable rows expose fixed/full plus a small set of width presets.

## Themes

Adopt Herdr's built-in palette names:

`default`, `catppuccin`, `catppuccin-latte`, `terminal`, `tokyo-night`, `tokyo-night-day`, `dracula`, `nord`, `gruvbox`, `gruvbox-light`, `one-dark`, `one-light`, `solarized`, `solarized-light`, `kanagawa`, `kanagawa-lotus`, `rose-pine`, `rose-pine-dawn`, and `vesper`.

`default` remains an alias for the current Tokyo Night style so existing configs do not change visually. `solarized-dark` remains accepted as an alias for `solarized`.

## Verification

- Unit tests cover config parsing/saving, CLI parsing, settings keyboard/click routing, header link URL choice, width-area calculation, and theme parsing.
- README documents the new config keys, CLI flags, settings shortcuts, and theme list.
- Run `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `npx -y @simpledoc/simpledoc check`.
