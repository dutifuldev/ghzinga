---
title: Progressive Resource Loading Spec
author: Bob <dutifulbob@gmail.com>
date: 2026-06-06
---

# Progressive Resource Loading Spec

## Goal

Make `gzg` load pull requests and issues progressively and much faster.

The first useful PR/issue view should appear as soon as base GitHub data is
available. Slower sections should load independently, update the TUI as they
arrive, and never block reading the title, body, status, branches, basic files,
comments, reviews, commits, or checks.

## Current problem

The startup placeholder already renders immediately, but the real resource is
still applied as one completed `Resource` after `GithubApiGateway::fetch_resource`
finishes.

For a normal PR load, the default path currently waits for:

- GraphQL rate-limit preflight, cached for 60 seconds per process
- base PR GraphQL query
- participants GraphQL enrichment
- review-thread GraphQL enrichment, paginated until complete
- timeline GraphQL enrichment, paginated until complete
- PR diff REST request for file patch text
- check-suite GraphQL enrichment

That means useful base data is ready earlier than the UI shows it. The TUI only
switches from the loading placeholder to the real PR after all default
enrichments complete.

## Design principle

Use a GitHub-shaped data layer and a UI-shaped render layer.

The data layer should preserve GitHub's model:

- resource identity: issue or pull request
- base object fields
- paginated connections with `pageInfo`
- section-level errors and freshness

The render layer should combine those sections into the current product shape:

- Overview chronological feed
- Activity feed
- Commits tab
- Checks tab
- Files tab
- Links tab

## Target behavior

Initial live load:

1. Parse the requested resource.
2. Render the existing loading shell immediately.
3. Fetch the base resource query.
4. Replace the loading shell with the base resource as soon as that query
   returns.
5. Schedule slow enrichments in the background.
6. Merge each completed section into the current resource and redraw.

Opening a new tab should follow the same behavior. The new tab should appear
immediately with a loading shell, then switch to the base snapshot, then fill in
sections.

Replacing the current tab should immediately replace the active tab with a
loading shell and then follow the same staged path.

## Resource snapshot model

Introduce a resource snapshot that can represent partial and complete data.

```rust
struct ResourceSnapshot {
    id: ResourceId,
    kind: ResourceKind,
    generation: u64,
    base: BaseResource,
    sections: ResourceSections,
    loaded_at: Option<SystemTime>,
    warnings: Vec<String>,
}

struct ResourceSections {
    comments: Section<Comment>,
    timeline: Section<TimelineItem>,
    reviews: Section<Review>,
    review_threads: Section<ReviewThread>,
    commits: Section<Commit>,
    files: Section<ChangedFile>,
    file_patches: Section<FilePatch>,
    status_rollup: Section<CheckRun>,
    check_suites: Section<CheckRun>,
    participants: Section<String>,
    linked_resources: Section<ResourceId>,
    metadata: Section<MetadataItem>,
}

struct Section<T> {
    state: SectionState,
    items: Vec<T>,
    page_info: PageInfo,
    loaded_at: Option<SystemTime>,
    error: Option<String>,
}

enum SectionState {
    NotStarted,
    Loading,
    LoadedPartial,
    LoadedComplete,
    Failed,
    Stale,
}
```

This does not need to replace the existing renderer-facing `Resource` in the
first slice. A bridge can convert `ResourceSnapshot` into the current `Resource`
shape while the renderer is migrated gradually.

## GitHub section mapping

Base PR query should provide the first useful screen:

- title, URL, author, state, body
- created/updated timestamps
- labels, assignees, reactions
- branches, merge status, draft/merge queue state
- review decision and basic review requests
- linked closing issues
- first page of comments
- first page of reviews
- first page of commits
- first page of changed files, without patch bodies
- first page of status check rollup

Follow-up section jobs should map to GitHub connections:

| Section | GitHub source | Default policy |
| --- | --- | --- |
| `comments` | `comments(first: 100, after)` | Base page first; full pagination only in full-depth mode or on demand |
| `reviews` | `reviews(first: 100, after)` | Base page first; full pagination only in full-depth mode or on demand |
| `review_threads` | `reviewThreads(first: 100, after)` | Background load after base; cap or page progressively |
| `timeline` | `timelineItems(first: 100, after)` | Background load after base; cap or page progressively |
| `commits` | `commits(first: 100, after)` | Base page first; full pagination on full-depth/load more |
| `files` | `files(first: 100, after)` | Base page first; full pagination on Files tab/full-depth |
| `file_patches` | REST PR diff | Lazy-load on Files tab or file expansion |
| `status_rollup` | `statusCheckRollup.contexts(first: 100, after)` | Base page first; full pagination on Checks tab/full-depth |
| `check_suites` | latest commit `checkSuites` | Background or Checks-tab priority |
| `participants` | `participants(first: 100, after)` | Low-priority background |
| `linked_resources` | closing/closed-by references | Base page first; full pagination on full-depth |
| `metadata` | projects, labels, assignees, review requests | Full pagination only when needed |

## Load manager

Loading should become a first-class subsystem instead of one monolithic
`fetch_pr()` call.

Responsibilities:

- schedule base and section jobs
- dedupe identical in-flight jobs
- prioritize the visible resource and visible tab
- deprioritize background tabs
- cancel or ignore stale jobs by generation
- serialize or throttle API calls when rate limits are low
- report section-level loading/error state to the app

Proposed job shape:

```rust
struct LoadJob {
    tab_id: u64,
    resource_id: ResourceId,
    generation: u64,
    section: LoadSection,
    priority: LoadPriority,
    page: LoadPage,
}

enum LoadSection {
    Base,
    Comments,
    Reviews,
    ReviewThreads,
    Timeline,
    Commits,
    Files,
    FilePatches,
    StatusRollup,
    CheckSuites,
    Participants,
    LinkedResources,
    Metadata,
    FullDepth,
}
```

Every result must carry the same `tab_id`, `resource_id`, `generation`, and
`section`. The reducer must ignore results that do not match the current tab
generation.

## Scheduling policy

Default PR open:

1. `Base` with high priority.
2. After `Base`, schedule:
   - `ReviewThreads`
   - `Timeline`
   - `CheckSuites`
   - `Participants`
3. Do not schedule `FilePatches` until the Files tab is opened or a file is
   expanded.
4. Do not schedule exhaustive `Comments`, `Reviews`, `Commits`, `Files`, or
   `StatusRollup` pagination until full-depth/load-more behavior asks for it.

Visible-tab priority:

- Overview prioritizes `Timeline` and `ReviewThreads`.
- Activity prioritizes `Timeline`, `ReviewThreads`, `Comments`, and `Reviews`.
- Checks prioritizes `StatusRollup` and `CheckSuites`.
- Files prioritizes `Files` and `FilePatches`.
- Commits prioritizes `Commits`.

## UI behavior

The UI should make partial data obvious but calm.

- Header, title, status, branch, and body should appear after base load.
- Tabs should render with whatever section data is available.
- Section-specific loading rows should appear only inside affected tabs.
- Errors should be section-local when possible, not resource-fatal.
- The footer loading message should describe active background work only when it
  helps, not constantly overwrite useful status.
- Cached data should render immediately with a subtle stale/refresh indicator
  while fresh section jobs run.

Examples:

- Files tab before patches load: `Loading diffs... file list is available.`
- Checks tab before suites load: `Loading check suites... status rollup is shown.`
- Overview before timeline load: show body/comments/reviews already available,
  plus `Loading timeline events...`

## Cache behavior

Cache base snapshots and sections separately.

Cache keys should include:

- owner
- repo
- number
- kind
- section
- section pagination state or completeness

Session restore should be able to show cached base data instantly, then refresh
the base and stale sections in the background.

Completed sections should survive restarts. Failed or loading sections should
restore as `NotStarted` or `Stale`, not as permanently failed.

## Rate-limit behavior

The existing GraphQL rate-limit preflight should remain, but section loading
allows better API economy:

- Avoid expensive jobs until their tab or action needs them.
- Skip or pause background jobs when GraphQL remaining budget is low.
- Prefer visible-tab jobs over background-tab jobs.
- Avoid repeated refreshes of unchanged hidden sections.
- Keep REST diff fetching lazy because it is expensive in latency and payload
  size even when REST rate limit is available.

## Migration plan

1. Add `SectionState`, `Section<T>`, and `ResourceSnapshot` types behind the
   existing domain model.
2. Split the base PR/issue query path from enrichment.
3. Add a `Base` load outcome that immediately replaces the loading placeholder.
4. Add section outcomes and merge logic with generation checks.
5. Move `check_suites`, `timeline`, `review_threads`, and `participants` to
   background section jobs.
6. Make file patch loading lazy on Files tab open or file expansion.
7. Convert full-depth loading into section pagination jobs.
8. Store base and section cache entries separately.
9. Gradually make renderers read section state directly instead of relying on a
   fully materialized `Resource`.

## Compatibility implementation slice

The first production slice keeps the existing renderer-facing `Resource` model
and introduces staged fetch outcomes:

- `Base` applies the base GraphQL PR/issue result and ends the blocking loading
  state.
- `Enrichment` merges slower background details into the same tab only if the
  current tab still shows the same resource.
- `Complete` preserves the existing blocking path for full-depth loads, offline
  fixtures, and file-patch requests.

This avoids a renderer rewrite while still satisfying the most important
latency rule: a normal PR renders base content without waiting for timeline,
review threads, participants, check suites, or diff patch text.

PR diff patch text is no longer part of default PR enrichment. It is loaded on
demand once the Files tab is active and a file lacks patch context. The Files tab
continues to render the existing `patch: not loaded` row until the diff request
returns.

## Testing plan

Unit tests:

- base load replaces the loading placeholder before enrichment results arrive
- stale section results are ignored after tab replacement or refresh
- duplicate section jobs are deduped
- Files tab schedules `FilePatches`
- Checks tab prioritizes `CheckSuites`
- section failures render warnings without discarding base data
- cached base snapshot restores immediately and schedules refresh

Transport tests:

- fake GitHub transport returns base quickly and delayed enrichments later
- delayed diff response does not block base render
- GraphQL rate-limit exhaustion prevents GraphQL section jobs but preserves
  already cached/base content

TUI smoke tests:

- start a real PR in tmux and capture the loading shell, base-loaded frame, and
  later enriched frame
- open Files and verify diff loading happens after tab activation
- open multiple resource tabs and verify visible-tab jobs win priority

Live validation:

- use a public PR with comments, review threads, checks, and files
- compare startup perceived latency before and after
- verify no `gh api` shell-out is introduced

## Non-goals

- Do not build a plugin system for arbitrary resource providers yet.
- Do not make every section generic at the UI layer; section types should remain
  concrete and GitHub-aware.
- Do not block this work on renderer rewrites. A compatibility bridge to the
  existing `Resource` shape is acceptable during migration.
- Do not fetch all pages by default just because the loader can represent them.

## Success criteria

- A normal PR renders useful base content after the base GraphQL query, without
  waiting for timeline, review threads, diff patches, participants, or check
  suites.
- Slow section loads update the current TUI without flicker or scroll resets.
- Files diffs are not fetched until Files is opened or a file is expanded.
- Multiple tabs can load independently without stale results corrupting the
  active tab.
- Restored sessions show cached content immediately and refresh progressively.
- API calls are fewer on startup for users who only read overview/status.
