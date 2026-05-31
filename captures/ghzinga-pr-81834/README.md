# ghzinga PR captures

Target: `openclaw/openclaw#81834`

Captured with the local `target/debug/gzg` binary in tmux.

## Sizes

- `narrow/`: `80x24`
- `medium/`: `120x36`
- `large/`: `160x50`

Each directory contains:

- `*.png`: image render of the terminal frame
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
controls, refresh, quit, help, and issue/PR navigation targets. The
`mouse-smoke/` capture set additionally drives the real TUI inside tmux with
xterm SGR mouse click events:

1. Click the `Files` tab.
2. Click `[expand all]`.
3. Click `[collapse all]`.

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
