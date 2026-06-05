---
title: ghzinga Session Restore Spec
author: Bob <dutifulbob@gmail.com>
date: 2026-06-05
---

# ghzinga Session Restore Spec

## Goal

`gzg` should be able to restart into the same PR/issue dashboard without the
user having to pass a session name. The feature should work inside Herdr, tmux,
plain terminals, and future launch environments without making the data model
Herdr-specific.

The restored state is ghzinga state: open resources, active resource tab, active
view, view order, selected expansions, UI settings, and cached GitHub data.
Terminal managers only provide launch context; they are not the source of truth
for GitHub resources.

## Non-goals

- Do not implement a Herdr-style headless renderer or multiplexer server.
- Do not make Herdr understand ghzinga resources.
- Do not depend on GitHub API availability before the first restored frame.
- Do not restore every scroll movement synchronously.
- Do not make restore mandatory; users need clear escape hatches.

## Storage Layout

Use XDG state and cache paths:

```text
$XDG_STATE_HOME/ghzinga/sessions/<session-id>/session.json
$XDG_STATE_HOME/ghzinga/session-index.json
$XDG_RUNTIME_DIR/ghzinga/<session-id>.sock
$XDG_CACHE_HOME/ghzinga/resources/<owner>/<repo>/<number>.json
```

Fallbacks:

```text
~/.local/state/ghzinga/...
/tmp/ghzinga-$UID/...
~/.cache/ghzinga/...
```

Test and scripting overrides:

```text
GZG_STATE_HOME=/tmp/gzg-state
GZG_CACHE_HOME=/tmp/gzg-cache
GZG_RUNTIME_HOME=/tmp/gzg-runtime
GZG_SESSION=<session-id>
```

`GZG_CONFIG_PATH` remains only for config. Session restore should not overload
the config file because sessions are runtime state, not user preference.

## Session File

Each session owns one `session.json`:

```json
{
  "schema_version": 1,
  "id": "s_8x9k2m",
  "name": null,
  "created_at": "2026-06-05T11:30:00Z",
  "updated_at": "2026-06-05T11:42:00Z",
  "launch": {
    "argv": ["gzg"],
    "cwd": "/home/bob/repos/ghzinga",
    "contexts": [
      {
        "provider": "herdr",
        "key": "socket=/home/bob/.config/herdr/herdr.sock;pane=p_12",
        "confidence": "strong",
        "metadata": {
          "socket_path": "/home/bob/.config/herdr/herdr.sock",
          "pane_id": "p_12"
        }
      },
      {
        "provider": "git",
        "key": "github.com/dutifuldev/ghzinga",
        "confidence": "weak",
        "metadata": {
          "remote": "https://github.com/dutifuldev/ghzinga.git"
        }
      }
    ]
  },
  "ui": {
    "theme": "default",
    "symbols": "emoji",
    "spacing": "comfortable",
    "width_mode": "fixed",
    "fixed_width": 118,
    "scrollbar": "on-scroll"
  },
  "resources": {
    "active_index": 0,
    "tabs": [
      {
        "id": "r_1",
        "resource": "dutifuldev/ghzinga#28",
        "kind_hint": "pull_request",
        "view": "overview",
        "scroll": 0,
        "reverse_chronological": false,
        "expanded_blocks": []
      }
    ]
  }
}
```

Rules:

- `schema_version` is required.
- `id` is stable for the lifetime of the session.
- `name` is optional and user-facing.
- `launch.contexts` is append-only enough to preserve useful history, but it
  should be deduplicated by `(provider, key)`.
- UI values mirror the current config schema so restore can preserve per-session
  overrides later. Global config still provides defaults.
- `resource` strings use ghzinga's existing canonical `owner/repo#number`
  format.
- `kind_hint` should be saved only when known from a URL or fetched payload.
  Loading placeholders must not invent a pull-request hint for ambiguous
  `owner/repo#number` input because the next restore still needs to try the
  issue fallback.
- Cached resource payloads live in cache, not inside the session file.

## Session Index

`session-index.json` maps launch contexts to sessions:

```json
{
  "schema_version": 1,
  "anchors": [
    {
      "provider": "herdr",
      "key": "socket=/home/bob/.config/herdr/herdr.sock;pane=p_12",
      "session_id": "s_8x9k2m",
      "confidence": "strong",
      "last_seen_at": "2026-06-05T11:42:00Z"
    },
    {
      "provider": "git",
      "key": "github.com/dutifuldev/ghzinga",
      "session_id": "s_8x9k2m",
      "confidence": "weak",
      "last_seen_at": "2026-06-05T11:42:00Z"
    }
  ]
}
```

Rules:

- Strong anchors can auto-restore.
- Weak anchors can restore only if they are the only match and the launch has no
  explicit resource argument.
- Stale anchors are not trusted if their provider can prove the context is gone.
- Writes should be atomic enough for local files: write temp file, then rename.
- If the index is corrupt, keep running, show one recoverable warning, and create
  a fresh index.

## Context Providers

Collect launch contexts at startup. The resolver should be generic:

1. Explicit:
   - `--session <id-or-name>`
   - `GZG_SESSION`
   - confidence: `explicit`
2. Herdr:
   - requires `HERDR_ENV=1`
   - uses `HERDR_SOCKET_PATH` and `HERDR_PANE_ID`
   - confidence: `strong`
   - no Herdr source change required; Herdr already injects these into panes
3. tmux:
   - uses `TMUX` and `TMUX_PANE`
   - confidence: `strong`
4. screen:
   - uses `STY` and `WINDOW`
   - confidence: `medium`
5. git:
   - uses current git remote owner/repo when available
   - confidence: `weak`
6. cwd:
   - canonical current working directory
   - confidence: `weak`
7. tty:
   - current tty path when available
   - confidence: `weak`

Provider keys must be deterministic strings, but the session schema should keep
provider-specific details in `metadata` so the format can grow.

## Herdr Integration Without Herdr Changes

Inside Herdr, `gzg` can read:

```text
HERDR_ENV=1
HERDR_SOCKET_PATH=/path/to/herdr.sock
HERDR_PANE_ID=p_12
```

The Herdr provider should use those as a strong anchor. That is enough for a
running pane.

For better restore after Herdr remaps internal pane ids, ghzinga may optionally
call Herdr's existing socket API to set a pane label that includes the ghzinga
session id:

```text
gzg:s_8x9k2m dutifuldev/ghzinga#28
```

This is still not Herdr-specific storage. It is only a provider marker that
helps recover the generic ghzinga session id. If the rename API fails, ghzinga
continues using its own session index.

## Startup Resolution

Startup chooses a session in this order:

1. `--no-restore`: create an ephemeral unsaved session.
2. `--new --session <id-or-name>` or `--new` with `GZG_SESSION`: create a new
   saved session using that exact normalized session id and bind it to the
   current contexts.
3. `--new`: create a new saved session with a generated id and bind it to the
   current contexts.
4. `--session` or `GZG_SESSION`: load or create that exact session.
5. Strong context match: load that session.
6. Medium context match: load only if exactly one match exists.
7. Weak context match: load only if no resource argument was passed and exactly
   one match exists.
8. No match: create a new saved session and bind current contexts.

Resource argument behavior:

- `gzg owner/repo#123` with a restored session opens or focuses that resource in
  the resolved session.
- It does not discard restored tabs unless `--new` or `--no-restore` is passed.
- If the resource argument already exists as a restored cached tab, focus that
  tab and refresh it in the background without replacing cached content with a
  loading placeholder.
- `--new` may use an explicit user-supplied `--session` or `GZG_SESSION` value
  as the new session id, but it must ignore provider-discovered labels such as
  Herdr's pane marker so the flag always creates a separate session.
- `gzg` with no resource argument restores the session tabs as-is.
- If a new saved session is created with no resource argument and no cached
  tabs, show the add-resource prompt immediately.
- The first real resource opened from an empty launch prompt replaces the
  placeholder tab instead of appending a second tab.

## Rendering During Restore

First frame rules:

- Show restored tabs immediately from `session.json`.
- If cached resource data exists, render it immediately with a stale marker.
- If no cache exists for a tab, render the existing loading placeholder for that
  resource.
- Start background refreshes after the TUI enters the alternate screen. Restored
  tabs refresh in place and preserve saved view, scroll, order, and expanded
  blocks; only brand-new placeholder tabs use initial replacement semantics.
- If an inactive restored tab has no cache entry, refresh it when the user
  focuses that tab so it cannot remain a loading placeholder indefinitely. If
  that fetch fails, keep the placeholder and error visible until the user
  explicitly refreshes instead of retrying every frame.
- Do not show a success message after refresh; only errors need status text.

This builds on the existing startup progressive loading behavior.

## Persistence Rules

Persist immediately for structural changes:

- tab opened or closed
- active resource tab changed
- active view changed
- chronological order changed
- settings changed

Persist with debounce:

- scroll position
- expanded block changes
- transient focus targets

Debounce target: 750ms after the last input change, plus immediate saves after
completed fetches and a final flush on normal exit.

Never persist:

- loading spinner frame
- temporary status messages
- API error text older than the current process
- modal input
- in-flight request ids

## CLI Surface

The current implementation scope includes both restore and live control. These
commands are required now, not deferred:

```text
gzg [RESOURCE]
gzg --new [RESOURCE]
gzg --no-restore [RESOURCE]
gzg --session <id-or-name> [RESOURCE]
gzg sessions
gzg session show <id-or-name>
gzg session delete <id-or-name>
gzg session rename <id-or-name> <name>
gzg open <RESOURCE>
gzg open --session <id-or-name> <RESOURCE>
gzg set --session <id-or-name> theme solarized
gzg set --session <id-or-name> spacing comfortable
gzg set --session <id-or-name> width-mode fixed
gzg set --session <id-or-name> fixed-width 118
gzg set --session <id-or-name> scrollbar on-scroll
```

Control command behavior:

- If the target session is running, send a local runtime command to that TUI and
  return after the command is accepted.
- If the target session is not running, update the saved session state so the
  next `gzg` launch restores the requested resource or setting.
- `gzg open <RESOURCE>` with no explicit session targets the best current
  session using the same explicit, Herdr, tmux, screen, git, cwd, and tty
  context resolver. If that is ambiguous, print the matching sessions and ask
  the user to pass `--session`.
- Opening a resource in a running TUI should behave like the in-app new-tab
  flow: focus the existing tab when it is already open, otherwise add a new
  resource tab, render cached content immediately when available, then refresh
  in the background.
- `gzg set` should use the same validation as the settings UI and persist the
  changed value to the session snapshot.
- Control commands should not steal terminal focus, enter alternate screen, or
  require keyboard/mouse input into the pane.

## Runtime Control Channel

Each running persistent TUI owns a local Unix socket:

```text
$XDG_RUNTIME_DIR/ghzinga/<session-id>.sock
```

Fallback when `XDG_RUNTIME_DIR` is absent:

```text
/tmp/ghzinga-$UID/<session-id>.sock
```

Rules:

- Socket paths are runtime-only and are not the session source of truth.
- On TUI startup, create the socket after the session id is resolved and remove
  stale sockets for the same session when the owning process is gone.
- The socket should accept newline-delimited JSON commands so it can evolve
  without breaking older clients.
- Commands must include a schema version and command id.
- Replies must include `ok`, the command id, and either a short result or an
  error.
- Unsupported command versions should fail cleanly with a user-facing error.
- A command sent to a stale socket should fail fast, remove the stale runtime
  marker if safe, and fall back to saved-state mutation when appropriate.
- Runtime control is local only. Do not expose a network listener.
- File and socket permissions should be owner-only where the platform permits
  it.

Initial command shapes:

```json
{"schema_version":1,"id":"c_1","method":"open","resource":"dutifuldev/ghzinga#29"}
{"schema_version":1,"id":"c_2","method":"set","key":"theme","value":"solarized"}
```

Initial reply shapes:

```json
{"schema_version":1,"id":"c_1","ok":true,"result":"opened dutifuldev/ghzinga#29"}
{"schema_version":1,"id":"c_2","ok":false,"error":"unknown theme"}
```

## Cache Rules

Cache normalized GitHub resources separately from session files:

```text
$XDG_CACHE_HOME/ghzinga/resources/dutifuldev/ghzinga/28.json
```

Cache metadata should include:

- fetched resource payload
- fetched_at
- api_depth
- source version/schema

Cache is best-effort. Corrupt cache should be ignored with a recoverable warning.

## Cleanup

Add a conservative cleanup command later:

```text
gzg sessions prune
```

Prune rules:

- remove index anchors whose sessions no longer exist
- remove sessions with no tabs and no recent activity
- optionally remove cache entries not referenced by any session and older than a
  configurable age

Do not auto-delete sessions silently in the first version.

## Implementation Plan

### Phase 1: state files and resolver

- Add `session` module with:
  - `SessionSnapshot`
  - `SessionIndex`
  - `LaunchContext`
  - path resolution helpers
  - atomic load/save helpers
- Add context collectors for explicit, Herdr, tmux, screen, git, cwd, and tty.
- Add resolver tests for priority, multi-match behavior, corrupt index, and
  resource-argument behavior.
- Add CLI flags `--new`, `--no-restore`, and `--session`.
- Keep `--once` unchanged and skip restore unless explicitly requested later.

### Phase 2: app restore and save

- Convert `AppState` resource tabs to and from `SessionSnapshot`.
- Restore active resource tab, active view, reverse order, scroll, and expanded
  blocks.
- Save structural changes synchronously enough for local files.
- Debounce scroll/expansion persistence.
- Flush pending session writes on normal quit.
- Add tests for tab restore, active tab restore, view restore, and save-on-change
  intents.

### Phase 3: cache-backed first frame

- Add normalized resource cache load/save.
- On startup, use cached resources before placeholders.
- Mark cached resources as stale until refresh completes.
- Refresh live resources in the existing background fetch path.
- Add tests for cache hit, cache miss, corrupt cache, and refresh replacement.

### Phase 4: provider polish

- Add Herdr pane-label marker support as a best-effort provider hook.
- Add `gzg sessions` and basic session management commands.
- Add docs/README usage examples.
- Add capture or smoke coverage for restored multi-tab startup if render output
  changes.

### Phase 5: live CLI control

- Add runtime path resolution using `XDG_RUNTIME_DIR`, `/tmp/ghzinga-$UID`, and
  `GZG_RUNTIME_HOME` for tests.
- Add a local Unix socket listener owned by the running TUI session.
- Add newline-delimited JSON command parsing and replies.
- Implement `gzg open <RESOURCE>` and `gzg open --session <id-or-name>
  <RESOURCE>` against a running TUI.
- Implement saved-state fallback for `gzg open` when the target session is not
  currently running.
- Implement `gzg set --session <id-or-name> <key> <value>` for theme, spacing,
  width mode, fixed width, and scrollbar visibility.
- Reuse existing app actions for opening/focusing tabs and applying settings so
  runtime commands behave the same as keyboard/mouse UI commands.
- Add tests for running-session command delivery, stale socket fallback,
  ambiguous session resolution, saved-state mutation, and invalid setting
  errors.

## Verification Checklist

Automated CI should use offline fixtures and temporary state/cache directories.
Live GitHub and terminal-manager behavior should be covered by local smoke tests
because they depend on auth, rate limits, terminal state, and installed tools.

- Unit tests for session path resolution and env overrides.
- Unit tests for all context provider keys.
- Unit tests for resolver priority and ambiguous matches.
- Unit tests for session snapshot round trips.
- Unit tests for corrupt session/index/cache recovery.
- Reducer/state tests for save triggers.
- Startup tests for restored tabs with and without cache.
- Fixture smoke test:
  - run with `GZG_STATE_HOME` and `GZG_CACHE_HOME` pointing at temporary dirs
  - start from offline fixtures
  - open multiple resources
  - quit
  - restart with the same state dir
  - verify restored tabs and active view from a test backend frame
- Real GitHub smoke test:
  - run `gzg dutifuldev/ghzinga#28` with normal auth
  - open another real PR or issue tab from a link
  - quit
  - restart plain `gzg` from the same launch context
  - verify tabs restore before the live refresh completes
  - verify the background refresh replaces stale cached data without a success
    status message
- Real Herdr smoke test:
  - open a Herdr pane
  - run plain `gzg`
  - open multiple PR/issue tabs
  - from another shell, run `gzg open --session <id-or-name>
    dutifuldev/ghzinga#29`
  - verify the running Herdr-pane TUI opens or focuses that resource without
    restarting
  - from another shell, run a `gzg set --session <id-or-name> ...` command and
    verify the running TUI updates its setting
  - quit `gzg`
  - run plain `gzg` again in the same Herdr pane
  - verify it resolves the session from `HERDR_ENV`, `HERDR_SOCKET_PATH`, and
    `HERDR_PANE_ID`
  - if the best-effort Herdr pane-label marker is implemented, verify the label
    contains the ghzinga session id and restore still works when the raw Herdr
    pane id changes after a Herdr restart
- Real tmux smoke test:
  - run plain `gzg` inside a tmux pane
  - from outside the pane, run `gzg open --session <id-or-name>
    dutifuldev/ghzinga#29`
  - verify the running tmux-pane TUI opens or focuses that resource
  - open tabs, quit, and relaunch in the same pane
  - verify it resolves the session from `TMUX` and `TMUX_PANE`
- Runtime control smoke test:
  - start `gzg` with temporary state/cache/runtime directories
  - run `gzg sessions` and capture the session id
  - run `gzg open --session <id> <RESOURCE>` from a separate process
  - verify the already-running TUI shows a new resource tab
  - run `gzg set --session <id> theme solarized`
  - verify the already-running TUI updates and the session file persists the
    setting
  - kill the TUI, leave the session file, and run `gzg open --session <id>
    <RESOURCE>` again
  - verify saved-state fallback updates the session without needing a running
    socket
- Escape-hatch smoke test:
  - `gzg --new` creates a separate saved session even in the same pane
  - `gzg --no-restore` ignores saved state and does not bind a new persistent
    session
- Local CI.
- SimpleDoc check.
