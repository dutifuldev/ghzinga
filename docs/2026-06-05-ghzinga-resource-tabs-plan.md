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
- Clicking plus, or pressing `n`, opens a centered modal for a new tab.
- Pressing `o` opens the same parser modal in replace-current-tab mode.
- The modal accepts:
  - full GitHub PR and issue URLs,
  - `owner/repo#123`,
  - `owner/repo 123`,
  - `#123` or `123`, resolved relative to the active resource repository.
- While typing in the modal, `Ctrl-C` clears the input first. If the input is
  already empty, it closes the modal instead of quitting ghzinga. Plain `q`
  closes the modal.
- Confirming a valid input starts a normal background GitHub fetch and
  immediately shows a loading placeholder in the destination tab.
- In new-tab mode, ghzinga creates and focuses a loading resource tab before
  GitHub returns. In replace-current mode, ghzinga replaces the active tab with
  the loading placeholder before GitHub returns.
- When the fetch completes, ghzinga replaces the placeholder with the loaded
  resource.
- If the resource is already open, ghzinga updates and focuses the existing tab.
- Resource tabs show kind, number, title, a new-resource button, and close
  affordances.
- Pressing `x` in the normal view closes the current resource tab. It is a
  no-op when only one resource tab is open and while modals, help, or settings
  own the UI.
- Clicking a GitHub issue or pull-request link no longer immediately replaces
  the active resource. If the target is a different issue or PR, ghzinga asks
  whether to open it in the current tab or a new resource tab. If the link points
  to a comment, review, or discussion on the current resource, ghzinga focuses
  the matching Activity entry in place.
- Resource tabs first try to show their full kind, number, and title text. When
  the full labels do not fit, ghzinga reduces tab widths across the row so every
  resource tab stays visible.
- Shrinking must preserve the identity prefix, for example `PR #29` or
  `Issue #123`. Title text may be removed before that identity is truncated.
- If all identity-only tabs still cannot fit, ghzinga renders a visible window
  around the active tab and shows left/right arrow buttons for hidden tabs.
- Clicking a tab-bar arrow only scrolls the visible tab strip. It must not
  switch the active PR/issue, change the content view, or trigger a refresh.
- Activating a resource tab recenters the tab window on that resource; arrows
  can then page the tab strip away without changing the active resource.
- Closing the last resource tab is ignored so the app always has a valid active
  resource.
- The plus button uses a single `+` glyph inside its button chrome, keeps a
  clickable target even on narrow terminals, and truncates gracefully before it
  disappears.
- Comfortable spacing applies the same horizontal gutter to resource tabs as it
  does to the header, status band, navigation tabs, content, and footer. When
  resource tabs are visible, a blank spacer row separates them from the PR/issue
  title area.

## Architecture

`AppState` keeps the current `resource` field as the active resource mirror and
adds `resource_tabs` plus `active_resource_tab` for the tab layer. This keeps
the existing renderers and fetch code simple while allowing each resource tab to
snapshot its own active section, scroll offset, scroll limit, expanded rows,
navigation history, and refresh metadata.
Per-tab refresh/error footer messages are snapshotted with the same view state,
so failures from background fetches stay attached to the tab that started them
even if the user switches away before the request completes.

The add-resource modal is normal app state, not a terminal side effect. While it
is open, keyboard input is routed to the modal first. Enter parses and returns
an `OpenResource(ResourceId)` intent. The runner handles that intent through the
same background fetch channel used by refresh, startup loading, and link
navigation. The prompt remains open until the runner accepts the background
fetch, so input is not lost if another fetch is already active.

The resource-link choice modal is also normal app state. Rendered GitHub issue
and pull-request links carry a resource id plus their original URL when present.
Clicking a different resource opens the choice modal; "open here" uses the
existing navigation flow and "new tab" uses `OpenResource`. Clicking a comment
fragment for the active resource skips the modal, switches to Activity, expands
the matching entry, and records a pending focus request. The next render resolves
that request to a row index after wrapping and comfortable spacing have been
applied, then scrolls the viewport to the focused activity row.

`FetchAction::OpenTab` centralizes the async path. Successful results call
`open_resource_in_tab`, which appends or deduplicates tabs and restores the
active resource snapshot. Fetch completion does not close the add-resource
prompt; the runner closes the prompt when a request is accepted so a later prompt
opened while an older request is in flight cannot lose typed input.

Background fetches carry a request id and originating resource-tab id. A fetch
completion is ignored if it does not match the current loading request, and
refresh/navigation completions apply to the tab that started the fetch. This
prevents a slow response from overwriting a different tab after the user clicks
away.

Mouse hit testing follows render stacking order: later registered hit areas win.
The modal registers a no-op overlay hit area before its buttons, so clicks
inside the dialog cannot reach underlying content. That also lets the tab close
affordance win over the broader tab body target.
Modal geometry is clamped to the actual terminal frame before clearing or
drawing, so very small or freshly resized terminals do not panic.

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
- Reducer tests cover `Ctrl-C` clearing and closing modal input, plus resource
  link choice actions.
- State tests cover opening, switching, deduplicating, closing, and preserving
  per-resource view state, history, and refresh status.
- Fetch tests cover `OpenTab` outcome application and fetch completion after
  switching away from the origin tab.
- Render and hit-target tests cover modal overlay blocking, tab-close overlap,
  tiny terminal modal drawing, same-resource comment focusing, responsive plus
  button rendering, comfortable tab gutters, and overflowed resource tabs keeping
  the active tab visible.
- Render tests cover resource-tab shrink-to-fit behavior that keeps all tabs
  visible without truncating `PR #number` or `Issue #number`, plus overflow arrow
  behavior when even minimum labels cannot fit.
