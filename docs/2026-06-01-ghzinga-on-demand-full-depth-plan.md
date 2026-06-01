---
title: ghzinga On-Demand Full-Depth Loading Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-01
---

# ghzinga On-Demand Full-Depth Loading Plan

`ghzinga` should stay economical by default, but it should not make users
restart the app when the current PR or issue has more GitHub pages than the
default partial load fetched.

## Problem

The default `partial` API depth protects GraphQL quota and keeps refreshes fast.
When GitHub reports `hasNextPage` for expensive base connections, ghzinga adds a
warning that the user can rerun with `--api-depth full` or
`GZG_API_DEPTH=full`.

That is technically correct, but it is not ideal for a mouse-first monitor. The
TUI already knows the current resource is incomplete, so it should provide a
visible way to load the complete supported data set in place.

## Behavior

- Keep `partial` as the default startup and auto-refresh depth.
- Detect the normalized partial-depth warning on the current resource.
- When the warning is present, show a footer `[load full]` action before the
  active-tab bulk expand/collapse action.
- Add `f` as the keyboard shortcut for the same action.
- Starting the action launches one background fetch for the current resource
  with `ApiDepth::Full`.
- The usual single-flight rule still applies: if another fetch is running,
  report `still loading: ...` and do not queue another request.
- While full-depth loading is in flight, keep the existing resource on screen
  and show `Loading |: loading full data for owner/repo#number from GitHub`.
- On success, apply the refreshed resource in place, preserving the active tab
  and scroll behavior like normal refresh.
- On offline fixtures, show a clear skipped message instead of pretending to
  call GitHub.

## UI Rules

- `[load full]` is only visible when it can do something useful.
- `[expand all]` / `[collapse all]` remains the final footer command when both
  controls are present.
- The footer does not render an always-on shortcut hint line. The action remains
  discoverable through `[load full]`, the Help view, and the README shortcut
  list.
- The action is mouse-clickable and keyboard-accessible.

## Verification

- Unit test the resource partial-depth marker helper.
- Reducer tests for keyboard `f` and footer click intent.
- Render tests for conditional `[load full]` visibility and ordering before
  `[expand all]`.
- Runner tests for full-depth fetch outcome behavior and offline-fixture skip.
- Update README and verification matrix once implemented.
