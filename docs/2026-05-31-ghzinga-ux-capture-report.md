---
title: ghzinga UX Capture Report
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# ghzinga UX Capture Report

This report records tmux verification runs for `ghzinga` against a live PR and a
live issue.

PR capture directory:

```text
captures/ghzinga-pr-81834/
```

Issue capture directory:

```text
captures/ghzinga-issue-88499/
```

The PR captures were generated with:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py
```

The issue captures were generated with:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py \
  --root captures/ghzinga-issue-88499 \
  --target https://github.com/openclaw/openclaw/issues/88499 \
  --title 'openai-responses provider: 404 on previous_response_id when store=false (default)' \
  --load-needle openai-responses \
  --mode issue
```

## Sizes

The run covers the required terminal sizes:

| Size | Geometry | Directory |
| --- | ---: | --- |
| Narrow | `80x24` | `captures/ghzinga-pr-81834/narrow/` |
| Medium | `120x36` | `captures/ghzinga-pr-81834/medium/` |
| Large | `160x50` | `captures/ghzinga-pr-81834/large/` |

Each size directory includes `.txt` and `.ansi` frames, plus
`manifest.json`. The manifests record the source revision, target resource,
command, requested and actual tmux size, active tab, and keys used for each
frame.

## PR Captured Views

The capture set proves that the core PR dashboard surfaces render at all three
sizes:

| Frame | Purpose |
| --- | --- |
| `00_overview_top` | resource header, status summary, body start, reactions |
| `01_overview_expanded` | visible body expansion control path |
| `02_overview_pagedown` | long body scroll path |
| `10_activity_top` | comments and bot activity |
| `11_activity_pagedown` | long activity scroll path |
| `20_commits_top` | commit list |
| `30_checks_top` | CI/check aggregate and detailed check rows |
| `31_checks_pagedown` | check-list scroll path |
| `40_files_top` | changed files and file expansion controls |
| `50_links_top` | detected linked issue/PR navigation targets |
| `60_help` | built-in keyboard and mouse help |

## Validation

The PR capture validation command is:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py --validate-only
```

It checks that every size contains:

- `[Activity]`
- `[Commits]`
- `[Checks]`
- `[Files]`
- `[Links]`
- `Help`
- `[refresh]`
- `[open]`
- `[help]`
- `[quit]`

It also checks that no app/rendering source paths changed since the manifest
revision. Use `--allow-stale-revision` only when intentionally inspecting
historical captures.

For this checked-in PR evidence set, the validator also checks content markers
in every terminal size: opening body text, dependency-warning comment content,
review activity, commits, aggregate checks, changed files, and detected links.

The PR rendered frames also show:

- status summary with PR state, author, reactions, review, merge, checks, files,
  and line counts
- assignee and requested-reviewer summaries
- body truncation with visible `[more]`
- activity entries with visible `[more]`
- check rows grouped under `Passing`
- file rows with visible `[more]`
- the linked issue `openclaw/openclaw#66943`
- footer controls `[refresh] [copy] [open] [help] [quit]`

## Issue Captured Views

The issue capture set uses `openclaw/openclaw#88499`, a live issue with four
comments at the time of capture.

| Frame | Purpose |
| --- | --- |
| `00_overview_top` | issue header, labels, reactions, body start |
| `01_overview_expanded` | body expansion path |
| `02_overview_pagedown` | long issue body scroll path |
| `10_activity_top` | issue comments |
| `11_activity_pagedown` | long activity scroll path |
| `20_links_top` | detected linked issue/PR navigation targets |
| `30_help` | built-in keyboard and mouse help |

The issue capture validation command is:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py \
  --root captures/ghzinga-issue-88499 \
  --mode issue \
  --validate-only
```

It checks that every size contains:

- `[Overview]`
- `[Activity]`
- `[Links]`
- `Help`
- `[refresh]`
- `[open]`
- `[help]`
- `[quit]`

Like the PR validator, it rejects captures when app/rendering code has changed
since the recorded manifest revision. For this checked-in issue evidence set,
it also checks content markers for the issue body, activity comments, and
detected issue/comment links in every terminal size.

The rendered frames also show:

- issue-only tab set: Overview, Activity, Links
- issue state, author, reactions, and comment count
- assignee summary
- comment rendering with visible `[more]`
- detected links including `openclaw/openclaw#84904` and
  `https://github.com/openclaw/openclaw/issues/87310#issuecomment-4585747111`
- footer controls `[refresh] [copy] [open] [help] [quit]`

## tmux Key Finding

During capture work, synthetic `tmux send-keys Tab` did not reliably advance the
TUI tabs. A direct tmux smoke test confirmed that `Right` advances tabs, and the
capture script now starts fresh sessions with `--tab` for deterministic tab
frames.

The app was still updated for tmux compatibility:

- `KeyCode::Tab`
- literal `'\t'`
- `Ctrl+i`

all advance the active tab in the reducer. This behavior is covered by unit
tests, and `Right` remains a documented fallback that works under tmux.

## Mouse Coverage

The capture runs verify rendered views, keyboard-driven navigation evidence, and
real tmux mouse-click evidence. The `mouse-smoke` capture starts the TUI in tmux,
sends xterm SGR mouse events to the running process, and saves the resulting
terminal frames for:

- tab switching
- file expansion
- tab-level expand all
- tab-level collapse all
- Links-tab navigation target clicks
- footer refresh clicks through the fixture-mode status path
- footer help and settings overlay clicks

The broader mouse routing matrix is still covered through render-to-click
integration tests that render the actual Ratatui UI, click the registered hit
rectangles, and verify body expansion, file expansion, issue/PR link navigation
intent, refresh, copy, help, settings, quit, exact URL opening, and footer
controls.
