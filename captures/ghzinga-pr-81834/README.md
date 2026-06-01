# ghzinga PR captures

Target: `openclaw/openclaw#81834`

Captured with the local `target/debug/gzg` binary in tmux.

## Sizes

- `narrow/`: `80x24`
- `medium/`: `120x36`
- `large/`: `160x50`

Each directory contains:

- `*.ansi`: ANSI tmux capture for the same frame
- `*.txt`: plain-text tmux capture for the same frame
- `manifest.json`: git revision, commands, requested/actual size, frame list, tabs, and keys for that capture run
- `*.history.txt`: tmux history for the session used to produce that frame

## Driven Flow

The capture script starts a fresh tmux session for each frame, using
`gzg openclaw/openclaw#81834 --refresh-seconds 0` plus `--tab` where needed.
It captures:

1. Overview top.
2. Body expansion with `e`.
3. Overview page-down.
4. Activity top and page-down.
5. Commits top.
6. Checks top and page-down.
7. Files top.
8. Links top.
9. Help overlay with `?`.

The code test suite separately verifies mouse click routing for tabs, expansion
controls, refresh, quit, help, settings, and issue/PR navigation targets. The
`mouse-smoke/` capture set additionally drives the real TUI inside tmux with
xterm SGR mouse click events:

1. Click an Overview `[+ more]` control, then click `[- less]`.
2. Click the `Files` tab.
3. Click footer `[expand all]`.
4. Click footer `[collapse all]`.
5. Click the `Links` tab.
6. Click a linked issue row and verify the TUI replaces the current PR with the
   linked issue.
7. Press Backspace and verify the TUI returns to the original PR.
8. Click footer `[refresh]` and verify the fixture-mode refresh status appears.
9. Click footer `[copy]` and verify a capture-local copy command receives the
   current PR URL.
10. Click footer `[open]` and verify a capture-local opener receives the current
    PR URL.
11. Click footer `[help]`.
12. Click footer `[settings]`.
13. Click the `compact` settings row and verify the capture-local TOML is saved.
14. Click footer `[quit]` and verify the tmux session exits.
15. Start an isolated fixture-backed session with a partial-depth warning, click
    footer `[load full]`, and verify fixture mode reports that full-depth loading
    was skipped instead of pretending to call GitHub.

Regenerate mouse smoke captures:

```sh
python3 captures/ghzinga-pr-81834/capture_mouse_smoke.py
```

Validate saved mouse smoke captures:

```sh
python3 captures/ghzinga-pr-81834/capture_mouse_smoke.py --validate-only
```

Validate the saved PR captures:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py --validate-only
```

The same script can capture other resources:

```sh
python3 captures/ghzinga-pr-81834/capture_ghzinga.py \
  --root captures/ghzinga-issue-88499 \
  --target https://github.com/openclaw/openclaw/issues/88499 \
  --title 'openai-responses provider: 404 on previous_response_id when store=false (default)' \
  --load-needle openai-responses \
  --mode issue
```
