---
title: ghzinga Resource Tabs Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-05
---

# ghzinga Resource Tabs Plan

## Goal

Add Herdr-inspired resource tabs so one ghzinga session can keep multiple PRs
or issues open without turning the first screen into a dashboard.

## Behavior

- A single open resource keeps the existing focused ghzinga layout.
- No extra resource tab bar appears above the header until at least two
  resources are open.
- The top-right plus button is always visible.
- Clicking plus, or pressing `n`, opens a centered modal.
- The modal accepts:
  - full GitHub PR and issue URLs,
  - `owner/repo#123`,
  - `owner/repo 123`,
  - `#123` or `123`, resolved relative to the active resource repository.
- Confirming a valid input starts a normal background GitHub fetch.
- When the fetch completes, ghzinga opens the resource in a tab and focuses it.
- If the resource is already open, ghzinga updates and focuses the existing tab.
- Resource tabs show kind, number, title, a new-resource button, and close
  affordances.
- When there are more resource tabs than fit, the visible tab window keeps the
  active resource tab reachable.
- Closing the last resource tab is ignored so the app always has a valid active
  resource.

## Architecture

`AppState` keeps the current `resource` field as the active resource mirror and
adds `resource_tabs` plus `active_resource_tab` for the tab layer. This keeps
the existing renderers and fetch code simple while allowing each resource tab to
snapshot its own active section, scroll offset, scroll limit, expanded rows,
navigation history, and refresh metadata.

The add-resource modal is normal app state, not a terminal side effect. While it
is open, keyboard input is routed to the modal first. Enter parses and returns
an `OpenResource(ResourceId)` intent. The runner handles that intent through the
same background fetch channel used by refresh, startup loading, and link
navigation. The prompt remains open until the runner accepts the background
fetch, so input is not lost if another fetch is already active.

`FetchAction::OpenTab` centralizes the async path. Successful results call
`open_resource_in_tab`, which appends or deduplicates tabs and restores the
active resource snapshot.

Background fetches carry a request id and originating resource-tab id. A fetch
completion is ignored if it does not match the current loading request, and
refresh/navigation completions apply to the tab that started the fetch. This
prevents a slow response from overwriting a different tab after the user clicks
away.

Mouse hit testing follows render stacking order: later registered hit areas win.
The modal registers a no-op overlay hit area before its buttons, so clicks
inside the dialog cannot reach underlying content. That also lets the tab close
affordance win over the broader tab body target.

## Herdr Inspiration

Herdr's tab bar keeps tabs compact, clickable, and visually subordinate to the
active pane. ghzinga borrows the same principles:

- render tab geometry explicitly,
- store click hit areas in app state,
- keep the active tab bold and high-contrast,
- put new-tab creation behind a focused modal,
- let modal input own keyboard handling until confirmed or cancelled.

ghzinga intentionally does not borrow Herdr's pane, workspace, PTY, or session
machinery. Resource tabs are only an in-process reading/navigation layer.

## Verification

- Parser tests cover URL, `owner/repo#number`, `owner/repo number`, `#number`,
  and bare-number input.
- Reducer tests cover opening the modal, typing, confirming, invalid input, and
  mouse hit targets.
- State tests cover opening, switching, deduplicating, closing, and preserving
  per-resource view state, history, and refresh status.
- Fetch tests cover `OpenTab` outcome application and fetch completion after
  switching away from the origin tab.
- Render and hit-target tests cover modal overlay blocking, tab-close overlap,
  and overflowed resource tabs keeping the active tab visible.
