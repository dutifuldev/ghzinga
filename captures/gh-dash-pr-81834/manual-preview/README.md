# gh-dash PR preview captures

Target: `openclaw/openclaw#81834`

Captured with the installed `gh dash` extension in tmux, using the local capture config at `../config.yml`.

## Sizes

- `narrow/`: `80x24`
- `medium/`: `120x36`
- `large/`: `160x50`

Each directory contains:

- `*.ansi`: ANSI tmux capture for the same frame
- `*.txt`: plain-text tmux capture for the same frame
- `size.txt`: actual tmux window size
- `tmux_history.txt`: final tmux history capture

## Manual flow

The session was driven through tmux with the installed `gh dash` binary:

1. Start `gh dash --config ../config.yml` in a tmux window at the target size.
2. Send a dark background-color terminal response to the pane so markdown rendering initializes inside tmux.
3. Press `p` to open preview.
4. Press `g` to sync the selected PR into the preview.
5. Press `e` on Overview to expand the PR body.
6. Capture Overview while scrolling down and back up with `Ctrl+d` / `Ctrl+u`.
7. Press `]` through Activity, Commits, Checks, and Files Changed, capturing scroll states on each tab.
8. For `large/`, press `P` after opening the preview to capture right-preview layout.

## Key frames

- Body: `*/01_overview_down.txt` onward
- Comments/activity: `*/30_activity_top.txt` onward for narrow, `*/20_activity_top.txt` onward for medium/large
- CI status/checks: `narrow/70_checks_top.txt`, `medium/50_checks_top.txt`, `large/50_checks_top.txt`
- Changed files: `narrow/90_files_top.txt`, `medium/60_files_top.txt`, `large/60_files_top.txt`
