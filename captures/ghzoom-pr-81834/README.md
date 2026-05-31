# ghzoom PR captures

Target: `openclaw/openclaw#81834`

Captured with the local `target/debug/ghzoom` binary in tmux.

## Sizes

- `narrow/`: `80x24`
- `medium/`: `120x36`
- `large/`: `160x50`

Each directory contains:

- `*.png`: image render of the terminal frame
- `*.ansi`: ANSI tmux capture for the same frame
- `*.txt`: plain-text tmux capture for the same frame
- `manifest.json`: commands, size, and frame list for that capture run
- `*.history.txt`: tmux history for the session used to produce that frame

## Driven Flow

The capture script starts a fresh tmux session for each frame, using
`ghzoom openclaw/openclaw#81834 --refresh-seconds 0` plus `--tab` where needed.
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

The code test suite separately verifies mouse click routing for tabs, expansion controls, refresh, quit, help, and issue/PR navigation targets.

The same script can capture other resources:

```sh
python3 captures/ghzoom-pr-81834/capture_ghzoom.py \
  --root captures/ghzoom-issue-88499 \
  --target https://github.com/openclaw/openclaw/issues/88499 \
  --title 'openai-responses provider: 404 on previous_response_id when store=false (default)' \
  --load-needle openai-responses \
  --mode issue
```
