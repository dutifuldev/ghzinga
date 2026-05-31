# ghzoom issue captures

Target: `https://github.com/openclaw/openclaw/issues/88499`

Captured with the local `target/debug/ghzoom` binary in tmux.

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

## Captured Views

The issue capture set uses a real issue with comments and captures:

1. Overview top.
2. Body expansion with `e`.
3. Overview page-down.
4. Activity top and page-down.
5. Links top.
6. Help overlay with `?`.

Validate the saved issue captures:

```sh
python3 captures/ghzoom-pr-81834/capture_ghzoom.py \
  --root captures/ghzoom-issue-88499 \
  --mode issue \
  --validate-only
```
