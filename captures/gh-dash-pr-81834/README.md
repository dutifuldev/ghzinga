# gh dash PR 81834 captures

Target: `openclaw/openclaw#81834`

Title: `feat(senseaudio): add SenseAudio TTS provider`

Tool: installed GitHub CLI extension `gh dash` (`dlvhdr/gh-dash v4.24.1`) running inside `tmux`.

## PR preview captures

The detailed PR preview captures are in `manual-preview/`.

Those captures include the PR body, Activity/comments, Commits, Checks/CI status, and Files Changed tabs across `80x24`, `120x36`, and `160x50` tmux windows. See `manual-preview/README.md` for the exact manual key flow and the key frames.

## Earlier list captures

- `narrow/`: `80x24`
- `medium/`: `120x36`
- `large/`: `160x50`

Each size directory contains:

- `00_initial.*`: first loaded dashboard view for the PR query.
- `01_after_j_down.*`: after pressing `j`.
- `02_after_k_up.*`: after pressing `k`.
- `03_after_ctrl_d_page_down.*`: after pressing `Ctrl-d`.
- `04_after_ctrl_u_page_up.*`: after pressing `Ctrl-u`.
- `05_after_page_down.*`: after pressing `PageDown`.
- `06_after_page_up.*`: after pressing `PageUp`.
- `07_after_G_end.*`: after pressing `G`.
- `08_after_g_home.*`: after pressing `g`.
- `tmux_history.*`: full tmux pane history after the sequence.
- `manifest.json`: exact requested and actual tmux dimensions.

For every capture:

- `.txt` is plain terminal text from `tmux capture-pane`.
- `.ansi` preserves ANSI styling from `tmux capture-pane -e`.
- `.ansi` preserves terminal styling for visual inspection in a terminal.

The capture config is `config.yml`, and the repeatable capture script is `capture_gh_dash.py`.
