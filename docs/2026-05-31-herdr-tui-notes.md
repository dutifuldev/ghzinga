---
title: Herdr TUI Architecture Notes
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# Herdr TUI Architecture Notes

This note summarizes how the local `herdr` checkout uses Ratatui, Crossterm, and supporting Rust libraries for its terminal UI, input pipeline, mouse capture, and embedded terminal panes. It is written as design input for `ghzoom`, a smaller GitHub issue/PR viewer.

Source baseline: `/home/bob/repos/herdr` at commit `7781979`.

## Executive Summary

Herdr is built like a terminal multiplexer, not like a typical terminal dashboard. Ratatui is the renderer and layout system, Crossterm provides terminal mode commands and event types, Tokio drives async runtime work, and a vendored Ghostty terminal emulator handles real PTY panes. Mouse support is not a single flag; it is a policy that decides whether the host terminal should send mouse events to Herdr, whether Herdr should handle those events as application UI, and whether some events should be translated and forwarded into a child terminal app.

The most useful Herdr patterns for `ghzoom` are:

- Compute all layout geometry before rendering.
- Store hit areas in app state.
- Route mouse input against those hit areas.
- Keep terminal setup and restoration in a guard.
- Use Crossterm mouse capture only when the app has real clickable UI.
- Keep the first `ghzoom` design local and single-process.

The Herdr parts `ghzoom` should avoid at first are PTY management, Ghostty terminal emulation, raw byte parsing, remote frame streaming, and child app mouse forwarding. Those solve multiplexer problems that a focused issue/PR viewer probably does not have.

## Source Map

These are the Herdr files most relevant to the TUI stack:

| Area | Files | Purpose |
| --- | --- | --- |
| Dependencies | `Cargo.toml` | Declares Ratatui, Crossterm, Tokio, portable-pty, Serde, tracing, Ghostty vendor inputs. |
| Terminal setup | `src/main.rs`, `src/client/mod.rs` | Enters raw/alternate screen mode, toggles mouse capture, bracketed paste, focus events, keyboard enhancements, and restores terminal state. |
| App event loop | `src/app/mod.rs`, `src/app/runtime.rs` | Runs timers, input handling, API/internal events, render scheduling, and mouse capture synchronization. |
| UI geometry/rendering | `src/ui.rs`, `src/ui/*` | Computes view rectangles and draws widgets with Ratatui. |
| Mouse handling | `src/app/input/mouse.rs`, `src/app/input/mod.rs`, `src/app/input/*` | Routes click, drag, wheel, double-click, right-click, sidebar, tab, pane, modal, and selection behavior. |
| Raw input parsing | `src/raw_input.rs` | Converts raw terminal bytes into app events, including SGR mouse, bracketed paste, focus events, and modified keys. |
| Config | `src/config.rs`, `src/config/model.rs`, `src/config/keybinds.rs`, `src/config/io.rs` | Defines `ui.mouse_capture`, scroll behavior, keybindings, theme settings, and config reload. |
| Embedded terminals | `src/terminal/runtime.rs`, `src/pane/terminal.rs`, `src/pane/input.rs`, `src/ghostty/*` | Owns PTY-backed panes, terminal emulation, input state, and input encoding. |
| Headless/server mode | `src/server/headless.rs`, `src/server/render_stream.rs`, `src/protocol/wire.rs` | Renders virtual frames and streams them to thin clients, including mouse capture mode updates. |

## Dependency Shape

Core crates in `Cargo.toml`:

- `ratatui = "0.30"` with `unstable-rendered-line-info`: frame layout, widgets, buffers, rendering, and virtual frame rendering.
- `crossterm = "0.29"`: terminal mode commands and event types such as keys, mouse events, focus events, bracketed paste, and mouse capture.
- `tokio = "1"`: async runtime, timers, channels, server/client loops, and background tasks.
- `portable-pty = "0.9"`: pseudo-terminal management for real pane processes.
- `bytes`: efficient byte buffers for PTY and client/server input forwarding.
- `serde`, `serde_json`, `bincode`, `toml`: config, protocol messages, session state, and API payloads.
- `tracing`, `tracing-subscriber`: structured file logging.
- `regex`, `unicode-width`: terminal text parsing, detection heuristics, and width-aware rendering.
- Vendored Ghostty terminal code under `vendor/libghostty-vt`: terminal emulation, input state tracking, mouse encoding, ANSI/VT rendering, and scrollback.

For `ghzoom`, the likely minimum is:

- `ratatui`: UI.
- `crossterm`: terminal setup, event polling, mouse capture.
- `tokio`: async GitHub API calls and refresh.
- `serde`, `serde_json`, `toml`: config and GitHub responses.
- `tracing`, `tracing-subscriber`: debug logs.
- `unicode-width`: correct title/comment truncation.

Avoid initially:

- `portable-pty`: needed only to host shell/terminal apps.
- Ghostty vendor code: needed only to emulate child terminals.
- `bincode`: useful for a binary client/server protocol, not needed for a local viewer.

## Runtime Modes

Herdr has several execution modes. Understanding them matters because some complexity exists only for remote or multiplexer behavior.

### Monolithic App Mode

`src/main.rs` creates a Tokio runtime, initializes a real terminal, creates `app::App`, and calls `app.run(&mut terminal).await`.

This mode:

- enters alternate screen/raw mode with `ratatui::init()`,
- enables or disables host mouse capture from config,
- enables bracketed paste and focus reporting,
- enables richer keyboard reporting where supported,
- handles input and render locally,
- restores terminal state on normal exit and panic.

This is closest to what `ghzoom` should start with.

### Headless Server Mode

`src/server/headless.rs` runs the Herdr app without controlling a real terminal. It does not enter raw mode or read stdin. Instead, clients send input over a socket and receive rendered frames.

The server:

- keeps the authoritative app state,
- parses raw input sent by clients,
- renders virtual Ratatui frames,
- streams frame diffs to clients,
- tells clients whether they should currently capture mouse input.

`ghzoom` should not start here unless remote attach is a hard requirement.

### Thin Client Mode

`src/client/mod.rs` controls the user-facing terminal while connected to a Herdr server.

The client:

- sets up the local terminal,
- reads raw stdin bytes,
- forwards those bytes to the server,
- receives frame messages,
- blits frames into the terminal,
- applies `ServerMessage::MouseCapture { enabled }` changes locally,
- restores terminal state on exit.

Again, this is unnecessary for an initial `ghzoom`, but it shows how mouse capture can be dynamically controlled by app state.

## Terminal Setup In Detail

The monolithic setup in `src/main.rs` imports these Crossterm commands:

- `EnableMouseCapture` / `DisableMouseCapture`
- `EnableBracketedPaste` / `DisableBracketedPaste`
- `EnableFocusChange` / `DisableFocusChange`
- `PushKeyboardEnhancementFlags` / `PopKeyboardEnhancementFlags`

The setup sequence is:

1. Compute terminal keyboard enhancement mode based on host environment.
2. Install a panic hook that restores terminal features before delegating to the original hook.
3. Build a Tokio multi-thread runtime.
4. Call `ratatui::init()`.
5. Enable or disable mouse capture from `config.ui.mouse_capture`.
6. Enable bracketed paste, focus reporting, and keyboard enhancement flags.
7. Optionally enable xterm `modifyOtherKeys`.
8. Run the app.
9. Reset `modifyOtherKeys`, clear Kitty graphics if needed, disable terminal features, and call `ratatui::restore()`.

The thin client has an analogous guard in `src/client/mod.rs`:

- `setup_terminal_with_capabilities(...)` enters terminal mode and applies features.
- `TerminalGuard` restores terminal state in `Drop`.
- `restore_terminal_state(...)` disables mouse capture, bracketed paste, focus reporting, keyboard enhancement flags, and restores Ratatui.

For `ghzoom`, use the guard pattern:

```rust
struct TerminalGuard;

impl TerminalGuard {
    fn enter(mouse: bool) -> anyhow::Result<Self> {
        ratatui::init();
        if mouse {
            crossterm::execute!(std::io::stdout(), EnableMouseCapture)?;
        }
        crossterm::execute!(std::io::stdout(), EnableBracketedPaste)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(
            std::io::stdout(),
            DisableBracketedPaste,
            DisableMouseCapture,
        );
        ratatui::restore();
    }
}
```

That gives `ghzoom` the important safety property without inheriting Herdr's full terminal protocol setup.

## Ratatui Rendering Model

Herdr separates geometry from drawing.

In `src/ui.rs`:

- `compute_view_with_runtime_registry(...)` derives layout rectangles from `AppState`, terminal size, and terminal runtimes.
- `compute_view_internal(...)` chooses desktop/mobile layout, sidebar width, tab bar area, terminal pane area, scroll bounds, and hit areas.
- `render_with_runtime_registry(...)` draws from the already-computed state.

In `src/app/mod.rs`, the render loop:

1. Synchronizes animation timers.
2. Synchronizes host mouse capture mode.
3. If rendering is needed and not rate-limited, calls `terminal.draw(...)`.
4. Inside draw, computes the view for the current frame area.
5. Calls `ui::render_with_runtime_registry(...)`.
6. Updates frame timing state.

Important Herdr practice:

- Ratatui widgets do not own app behavior.
- `AppState` owns behavior, mode, scroll offsets, selected item, hit areas, and drag state.
- Rendering is mostly a projection from `AppState` to a terminal buffer.

For `ghzoom`, design this as:

```text
AppState
  active_tab: Overview | Timeline | Files | Checks | Review
  selected_anchor: Option<AnchorId>
  issue_or_pr: LoadedItem
  timeline: Vec<TimelineEntry>
  files: Vec<FileEntry>
  checks: Vec<CheckEntry>
  scroll: PanelScrollState
  mouse_capture: bool
  view: ViewRects

ViewRects
  full: Rect
  header: Rect
  tab_bar: Rect
  left_index: Rect
  content: Rect
  details: Rect
  footer: Rect
  tab_hit_areas: Vec<(Tab, Rect)>
  anchor_hit_areas: Vec<(AnchorId, Rect)>
  action_hit_areas: Vec<(Action, Rect)>
```

The render loop should compute `ViewRects` once per frame, store it, then draw. Mouse handlers should use `state.view`.

## Layout Patterns

Herdr uses Ratatui layouts directly:

- `Layout::horizontal([Constraint::Length(sidebar_w), Constraint::Min(1)])`
- `Layout::vertical([Constraint::Length(1), Constraint::Min(1)])`
- fixed-height headers/menus,
- constrained inner rectangles for modals,
- explicit rectangles for scrollbars and buttons.

It does not treat layout as throwaway render code. Helpers such as sidebar, mobile, tabs, modals, and scrollbars compute rectangles that are later used by mouse handlers.

For `ghzoom`, use a similar deterministic layout:

```text
+--------------------------------------------------------------------------------+
| owner/repo #1234  title                                           state checks  |
+--------------------------------------------------------------------------------+
| Overview  Timeline  Files  Checks  Reviews                         actions ... |
+--------------------------+-----------------------------------------------------+
| index / metadata         | selected panel content                              |
| labels                   | comments, PR body, file list, check details         |
| assignees                |                                                     |
| participants             |                                                     |
+--------------------------+-----------------------------------------------------+
| q quit   tab next   o open in browser   r refresh   c comment                  |
+--------------------------------------------------------------------------------+
```

On narrow terminals:

```text
+------------------------------------------+
| owner/repo #1234 state                   |
| title                                    |
+------------------------------------------+
| Overview Timeline Files Checks Reviews   |
+------------------------------------------+
| selected panel full width                |
|                                          |
+------------------------------------------+
| compact footer                           |
+------------------------------------------+
```

Keep the layout model boring and predictable. The value is in good routing, fast rendering, and a clear issue/PR view.

## App State And Modes

Herdr uses explicit modes to decide how input should behave. Examples include terminal mode, settings mode, navigation mode, resize mode, context menu mode, onboarding, modals, keybinding help, and mobile switcher.

Mouse handlers check mode early:

- onboarding consumes onboarding clicks,
- settings routes to settings mouse logic,
- global menu routes to menu hover/click logic,
- keybinding help ignores some mouse paths,
- modal modes constrain input to modal buttons/scrollbars,
- terminal mode allows pane, sidebar, tab, and context menu behavior.

For `ghzoom`, likely modes:

- `Normal`: read and navigate issue/PR content.
- `Search`: typing into a filter/search prompt.
- `Command`: command palette or action menu.
- `Comment`: composing a comment.
- `Confirm`: confirming destructive actions.
- `Help`: keybinding overlay.

Mouse handling should branch by mode first, not by coordinates first. That avoids accidental clicks leaking through overlays into underlying content.

## Raw Input Pipeline

Herdr's raw input system is in `src/raw_input.rs`.

It defines:

- `RawInputEvent`: semantic event enum for key, mouse, paste, focus, color reply, unsupported, etc.
- `RawInputFramer`: converts byte chunks to semantic events.
- `RawInputByteFramer`: buffers incomplete byte sequences before they become events.

The byte framer solves real terminal edge cases:

- bracketed paste may arrive in chunks,
- host OSC color replies may be split,
- a lone ESC may mean Escape key or start of an Alt/control sequence,
- UTF-8 characters may arrive split across reads,
- terminal control strings should not be forwarded accidentally to child panes.

The semantic parser handles:

- bracketed paste delimiters,
- host foreground/background color responses,
- `ESC [ I` focus gained,
- `ESC [ O` focus lost,
- SGR mouse sequences,
- terminal key sequences,
- ordinary UTF-8 text.

The SGR mouse parser handles sequences of the form:

```text
ESC [ < cb ; column ; row M
ESC [ < cb ; column ; row m
```

It converts terminal 1-based coordinates to 0-based Crossterm coordinates and maps `cb` bits into:

- left/middle/right down,
- up/release,
- drag,
- moved,
- scroll up/down/left/right,
- shift/alt/control modifiers.

For `ghzoom`, use Crossterm's normal event APIs first. Herdr's raw parser is valuable only if `ghzoom` later needs:

- exact modified-key behavior across many terminals,
- host theme probing,
- remote client/server forwarding,
- robust paste/focus handling beyond Crossterm defaults,
- embedded child terminal apps.

## Input Event Flow

The monolithic Herdr input flow is roughly:

```text
stdin bytes
  -> RawInputFramer
  -> RawInputEvent
  -> App runtime
  -> mode-specific handler
  -> state mutation
  -> render requested
```

For mouse:

```text
RawInputEvent::Mouse(mouse)
  if state.mouse_capture:
      App::handle_mouse(mouse)
  else:
      AppState::handle_pane_mouse_only(mouse)
```

The `mouse_capture = false` branch matters because Herdr may have disabled host mouse capture for its own UI but then re-enabled it when a focused child pane requested mouse reporting. In that state, mouse events should be forwarded to the pane, not interpreted as Herdr chrome clicks.

For `ghzoom`, the flow can be:

```text
crossterm event
  -> Event::Key / Event::Mouse / Event::Resize
  -> input::handle_event(state, event)
  -> state mutation or async command request
  -> render
```

No forwarding branch is needed unless `ghzoom` embeds child terminal programs.

## Mouse Capture Policy

Herdr has a user-facing config:

```toml
[ui]
mouse_capture = true
mouse_scroll_lines = 3
```

`src/config/model.rs` makes `mouse_capture` default to `true`. `src/app/state.rs` stores it on `AppState`.

The key method is:

```rust
should_capture_host_mouse_from(terminal_runtimes)
```

It returns:

```text
state.mouse_capture || focused_pane_requests_mouse_capture
```

`focused_pane_requests_mouse_capture` checks whether the focused embedded terminal pane has an `InputState` whose mouse protocol mode has reporting enabled.

That gives Herdr three behaviors:

1. App mouse enabled: Herdr captures host mouse and handles clicks/drags/wheel for Herdr UI.
2. App mouse disabled, focused pane not requesting mouse: host terminal handles mouse normally.
3. App mouse disabled, focused pane requesting mouse: Herdr captures mouse only to forward it to the pane app.

In monolithic mode, `src/app/mod.rs` keeps host mouse capture synchronized:

```text
desired = state.should_capture_host_mouse_from(...)
if desired changed:
    EnableMouseCapture or DisableMouseCapture
```

In server mode, `src/server/headless.rs` sends the desired state to clients as `ServerMessage::MouseCapture { enabled }`; `src/client/mod.rs` applies it locally.

For `ghzoom`, decide early:

- If mouse is central to the app, default `mouse_capture = true`.
- If keyboard-first terminal ergonomics matter more, default `mouse_capture = false` and provide `--mouse`.
- If disabled, document that terminal selection and Cmd-click URLs work normally.
- If enabled, document Shift-drag as the typical terminal bypass for selecting text.

## Mouse Routing In Detail

Herdr's full mouse handler is mostly in `src/app/input/mouse.rs`.

The ordering is important:

1. Onboarding and toast click handling.
2. Settings mode.
3. Global launcher/menu hover and click behavior.
4. Mobile-specific mouse handling.
5. Modal-specific handling for worktree/open/remove/confirm flows.
6. Main mouse match by event kind.

For left button down:

- clear selection and drag state,
- handle confirm/modal buttons,
- handle sidebar collapse/toggle,
- handle workspace list rows and scrollbars,
- handle agent panel rows and scrollbars,
- focus panes,
- forward click to pane app if requested,
- otherwise begin text selection in pane.

For left drag:

- update text selection if active,
- forward drag to pane app if needed,
- start or update workspace reorder,
- start or update tab reorder,
- drag scrollbars,
- drag pane split borders,
- drag sidebar divider.

For left up:

- finish selection and copy selected text,
- forward release to pane app if needed,
- complete workspace/tab reorder,
- switch workspace/tab if it was a click rather than drag.

For wheel:

- over tab bar: switch tabs,
- over panes: forward wheel to pane app if mouse reporting or alternate scroll says so, otherwise scroll Herdr scrollback,
- over sidebar: scroll workspace/agent lists or move visible selection.

For right click:

- open context menu for workspace, tab, or pane depending on hit area.

For middle click/drag:

- forward to pane app when relevant.

For `ghzoom`, use this ordering:

1. Overlay/modal first.
2. Action menu or command palette.
3. Tab bar.
4. Content anchor hit areas.
5. Side index/list.
6. Footer actions.
7. Wheel fallback to active panel scroll.

Example:

```rust
fn handle_mouse(state: &mut AppState, mouse: MouseEvent) -> Command {
    if state.overlay.is_some() {
        return input::overlay::handle_mouse(state, mouse);
    }

    if let Some(tab) = state.view.tab_at(mouse.column, mouse.row) {
        if left_down(mouse) {
            state.active_tab = tab;
            return Command::None;
        }
    }

    if let Some(anchor) = state.view.anchor_at(mouse.column, mouse.row) {
        if left_down(mouse) {
            state.focus_anchor(anchor);
            return Command::None;
        }
    }

    if wheel(mouse) {
        state.scroll_panel_under(mouse.column, mouse.row, mouse.kind);
        return Command::None;
    }

    Command::None
}
```

## Embedded Terminal Panes

Herdr's biggest complexity comes from the fact that each pane is a real terminal process.

The stack is:

```text
TerminalRuntime
  -> PaneRuntime
  -> PaneTerminal
  -> GhosttyPaneTerminal
  -> vendored Ghostty terminal emulator
  -> child PTY process
```

`src/pane/terminal.rs` defines `InputState`, which tracks:

- alternate screen,
- application cursor mode,
- bracketed paste,
- focus reporting,
- mouse protocol mode,
- mouse protocol encoding,
- mouse alternate scroll,
- modifyOtherKeys.

This lets Herdr answer questions like:

- Is the child app asking for mouse reporting?
- Should wheel events scroll Herdr scrollback or be sent to the child app?
- Is the child app in alternate screen?
- Should PageUp/PageDown be forwarded or used for host scrollback?
- Which mouse encoding should be used when forwarding events?

`src/pane/input.rs` converts Herdr/Crossterm input into Ghostty input:

- `ghostty_key_event_from_terminal_key(...)` maps key code, modifiers, press/release/repeat, and UTF-8 text.
- `ghostty_mouse_encoder_for_terminal(...)` creates an encoder configured from the child terminal state.
- `ghostty_mouse_event_from_button_kind(...)` maps Crossterm mouse button events to Ghostty mouse events.
- `ghostty_mouse_event_from_wheel_kind(...)` maps wheel events.

`src/app/input/mouse.rs` uses runtime helpers:

- `encode_mouse_button(...)`
- `encode_mouse_wheel(...)`
- `input_state()`
- `wheel_routing()`
- `try_send_bytes(...)`

This is not needed for `ghzoom` unless it grows a "run this command in a pane" feature.

## Scroll Behavior

Herdr treats wheel events as context-sensitive:

- Wheel over tab bar changes tabs.
- Wheel over sidebar scrolls workspace or agent lists.
- Wheel over a terminal pane either forwards to the child app or scrolls Herdr-managed scrollback.
- Wheel over overlays scrolls that overlay.

The `ui.mouse_scroll_lines` config defaults to `3` and is applied when Herdr scrolls pane scrollback itself.

For `ghzoom`, use panel-local scroll offsets:

```text
ScrollState
  overview: usize
  timeline: usize
  files: usize
  checks: usize
  reviews: usize
  sidebar: usize
```

Wheel routing should scroll the panel under the pointer. Keyboard routing should scroll the active panel. This makes mouse and keyboard behavior predictable without needing Herdr's child terminal routing.

## Selection And Copying

Herdr supports selecting text inside pane content because pane contents are terminal screens. Selection is not handled by the host terminal when Herdr captures mouse.

Selection behavior includes:

- left click anchors a selection in pane coordinates,
- drag updates selection,
- mouse-up copies selected text,
- double-click copies a token,
- copy feedback is rendered as a TUI overlay,
- Shift-drag can still be used by host terminals when mouse capture is disabled.

For `ghzoom`, decide whether app-level selection is worth implementing. Since GitHub content is structured data, a better first version is:

- keyboard command to copy URL,
- keyboard command to copy title,
- keyboard command to copy selected comment URL,
- click to focus comment/file/check,
- leave arbitrary text selection to the host terminal when mouse capture is off.

If mouse capture is on by default, app-level copy commands become more important because normal drag selection is intercepted.

## Theming

Herdr centralizes palette state in `AppState` and uses Ratatui `Style` everywhere. It supports built-in themes, custom colors, and a special `terminal` theme that can query host terminal default colors.

The advanced terminal theme path is tied to its terminal emulator and host color query machinery. That is too much for `ghzoom` initially.

For `ghzoom`, define a simple palette:

```rust
pub struct Palette {
    pub bg: Color,
    pub panel: Color,
    pub panel_subtle: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub link: Color,
    pub selected_bg: Color,
    pub selected_fg: Color,
}
```

Then map GitHub concepts:

- open issue: green/success,
- closed issue: purple or muted,
- merged PR: purple,
- draft PR: muted,
- failing checks: danger,
- pending checks: warning,
- passing checks: success,
- labels: color chips approximated in terminal colors.

## Config And Keybindings

Herdr has a real keybinding model:

- a configurable prefix key,
- direct bindings,
- prefix bindings,
- indexed bindings,
- custom command bindings,
- validation and diagnostics.

For `ghzoom`, do not start with a full prefix system unless it is meant to behave like tmux. A GitHub object viewer probably wants direct, discoverable bindings:

- `q`: quit
- `tab` / `shift+tab`: next/previous panel
- `j` / `k`: move selection
- `g` / `G`: top/bottom
- `/`: search within item
- `r`: refresh
- `o`: open selected thing in browser
- `y`: copy selected URL
- `c`: start comment
- `?`: help

Config can still allow overrides later:

```toml
[ui]
mouse_capture = true
theme = "default"

[keys]
quit = "q"
next_panel = "tab"
previous_panel = "backtab"
open_browser = "o"
copy_url = "y"
refresh = "r"
help = "?"
```

## Testing Patterns Worth Copying

Herdr tests rendering by using Ratatui in-memory backends and checking buffers. It tests raw input parsers and mouse routing with constructed `MouseEvent` values.

For `ghzoom`, useful test layers:

- unit-test layout computation for known terminal sizes,
- unit-test hit-area lookup,
- unit-test mouse routing with synthetic `MouseEvent`,
- unit-test key routing,
- snapshot-test rendered buffers for small and wide terminals,
- mock GitHub API responses and test state transitions.

Avoid relying only on manual TUI testing. Mouse routing breaks easily when layout changes.

## Practical Architecture For Ghzoom

Suggested module layout:

```text
src/
  main.rs
  app.rs
  config.rs
  github/
    mod.rs
    gh_cli.rs
    models.rs
  input/
    mod.rs
    key.rs
    mouse.rs
  ui/
    mod.rs
    layout.rs
    render.rs
    theme.rs
    widgets.rs
```

Suggested data flow:

```text
main
  -> load config
  -> enter terminal
  -> create App
  -> event loop
       -> poll crossterm events
       -> receive async GitHub results
       -> update state
       -> draw Ratatui frame
  -> terminal guard restores
```

Suggested `AppState`:

```rust
pub struct AppState {
    pub repo: RepoRef,
    pub item: Option<IssueOrPullRequest>,
    pub active_tab: Tab,
    pub loading: LoadingState,
    pub error: Option<String>,
    pub selection: Selection,
    pub scroll: ScrollState,
    pub view: ViewRects,
    pub palette: Palette,
    pub mouse_capture: bool,
}
```

Suggested `ViewRects`:

```rust
pub struct ViewRects {
    pub full: Rect,
    pub header: Rect,
    pub tab_bar: Rect,
    pub sidebar: Rect,
    pub content: Rect,
    pub details: Rect,
    pub footer: Rect,
    pub tabs: Vec<(Tab, Rect)>,
    pub anchors: Vec<(AnchorId, Rect)>,
    pub actions: Vec<(Action, Rect)>,
}
```

Suggested event commands:

```rust
pub enum Command {
    None,
    Quit,
    Refresh,
    OpenBrowser(Url),
    Copy(String),
    SubmitComment(String),
}
```

This keeps input handlers pure-ish: they mutate state and return commands for side effects.

## What To Copy From Herdr

Copy these ideas:

- geometry-first rendering,
- explicit hit areas,
- mode-first input routing,
- terminal guard for setup/restore,
- configurable mouse capture,
- wheel routing based on pointer location,
- synthetic mouse/key tests,
- snapshot rendering tests,
- palette struct with semantic colors,
- simple full redraw on focus gain if terminal corruption appears.

## What Not To Copy Yet

Avoid these unless a concrete feature demands them:

- raw byte input parser,
- Ghostty terminal emulation,
- PTY pane runtime,
- child app mouse forwarding,
- thin client/server frame protocol,
- dynamic mouse capture from child app input state,
- host terminal color query machinery,
- complex prefix keybinding model,
- session persistence and handoff.

## Suggested First Mouse Behavior

For a polished `ghzoom` first version:

- left-click tab names to switch panels,
- left-click comments/files/checks to focus them,
- wheel scrolls the panel under the pointer,
- click footer actions if visible,
- right-click opens a small action menu only if it is easy,
- `--no-mouse` disables mouse capture for host terminal selection,
- Shift-drag remains the documented escape hatch in terminals that support it.

The mouse implementation should start from hit areas, not text matching. Every visible clickable thing should have a matching `Rect` stored during layout.

## Bottom Line

Herdr's TUI quality comes less from Ratatui itself and more from its architecture:

- state owns layout and behavior,
- rendering is deterministic,
- input routing is explicit,
- mouse capture is a policy,
- and terminal restoration is treated as critical infrastructure.

`ghzoom` should adopt those patterns, but not Herdr's multiplexer machinery. A single-process Ratatui app with Crossterm mouse capture, async GitHub loading, explicit hit areas, and a small config model is the right starting point.
