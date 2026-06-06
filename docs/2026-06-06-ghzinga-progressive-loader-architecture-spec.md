---
title: Progressive Loader Architecture Spec
author: Bob <dutifulbob@gmail.com>
date: 2026-06-06
---

# Progressive Loader Architecture Spec

## Main goal

`gzg` should make opening a GitHub PR or issue feel immediate while still
eventually showing the deep GitHub data that makes the app useful.

The user should see the TUI shell right away, then see the PR/issue identity,
title, body, state, branch/status summary, and first useful content as soon as
the base GitHub response returns. Slower data should fill in progressively by
section without blocking the first readable screen.

The long-term target is not a giant generic data-loading framework. It is a
GitHub-shaped loader that understands how GitHub actually exposes PRs and
issues:

- one resource identity: owner, repo, number, and kind
- one base object: title, URL, state, author, body, branches, summary counts
- many paginated connections: comments, reviews, timeline, commits, checks,
  files, review threads, labels, assignees, projects, and linked resources
- section-level loading state, pagination state, freshness, errors, and retry
  policy

The renderer should remain product-shaped. It should render Overview, Activity,
Commits, Checks, Files, Links, settings, tabs, and footer controls. It should
not expose raw loader machinery to the user.

## User-visible behavior

Opening a resource should happen in stages:

1. Create or replace the tab immediately.
2. Render the normal header, nav selectors, content area, footer controls, and
   loading detail on the first frame.
3. Apply base PR/issue data as soon as it returns.
4. Clear the blocking loading state when base data is visible.
5. Continue fetching slower sections in the background.
6. Redraw only the affected tab sections as each result arrives.
7. Show warnings for failed optional sections without discarding already visible
   base content.

Opening a new tab must not wait on the currently visible tab. Replacing the
current tab must not leave the old resource on screen while the new one is
loading. The target tab should appear immediately and then fill in.

Successful loads should be quiet. The app should not show `info loaded ...`
messages after success. Status text should be reserved for active loading,
explicit user actions, saved settings, and errors or warnings.

## Data shape

The loader should preserve the shape of GitHub data internally.

```rust
struct ResourceSnapshot {
    tab_id: u64,
    resource_id: ResourceId,
    generation: u64,
    base: Option<BaseResource>,
    sections: ResourceSections,
    warnings: Vec<ResourceWarning>,
    loaded_at: Option<SystemTime>,
}

struct ResourceSections {
    comments: Section<Comment>,
    reviews: Section<Review>,
    review_threads: Section<ReviewThread>,
    timeline: Section<TimelineItem>,
    commits: Section<Commit>,
    files: Section<ChangedFile>,
    file_patches: Section<FilePatch>,
    status_rollup: Section<CheckRun>,
    check_suites: Section<CheckSuite>,
    labels: Section<Label>,
    assignees: Section<User>,
    participants: Section<User>,
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

The current renderer-facing `Resource` can remain as an adapter target while
this model lands incrementally. The architectural rule is that the app should
stop treating a resource as either absent or fully loaded. A resource can be
readable while some sections are still loading, partially loaded, stale, or
failed.

## GitHub section mapping

Base data should return the first useful screen:

- title, URL, state, author, body, created/updated timestamps
- PR branches, draft state, mergeability, review decision, and merge queue state
- issue state reason, linked branch summary, and basic relationship metadata
- labels, assignees, reactions, and concise summary counts
- first page of comments
- first page of reviews
- first page of commits for PRs
- first page of changed files for PRs, without patch bodies
- first page of check rollup contexts for PRs
- cheap linked-resource references when available without expensive pagination

Background jobs should map to GitHub connections and REST surfaces:

| Section | GitHub source | Default behavior |
| --- | --- | --- |
| Comments | GraphQL comments connection | Base first page, deeper pages on demand or full-depth |
| Reviews | GraphQL reviews connection | Base first page, deeper pages on demand or full-depth |
| Review threads | GraphQL reviewThreads connection | Background after base, visible-tab priority |
| Timeline | GraphQL timelineItems connection | Background after base, chronological merge |
| Commits | GraphQL commits connection | Base first page, deeper pages on demand or full-depth |
| Files | GraphQL files connection | Base first page without patch bodies |
| File patches | REST pull-request diff | Lazy-load only from Files tab or file expansion |
| Status rollup | GraphQL statusCheckRollup contexts | Base first page, Checks tab priority |
| Check suites | GraphQL checkSuites from head commit | Background or Checks tab priority |
| Labels and assignees | GraphQL connections | Base first page, paginate only when needed |
| Projects | GraphQL projectItems connection | Optional enrichment, suppress missing scope warnings |
| Linked resources | GraphQL closing/closed-by references | Base cheap references, paginate on demand |

REST/core and GraphQL budgets are separate. The loader should exploit that
separation, but it should not spend REST budget on diff patches during the first
screen unless the Files tab is visible. Diff patch text is large and should stay
lazy even when REST budget is healthy.

## Load manager

Loading should be a first-class subsystem with explicit job ownership.

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
    Labels,
    Assignees,
    Projects,
    Participants,
    LinkedResources,
    Metadata,
    FullDepth,
}
```

Every result must carry `tab_id`, `resource_id`, `generation`, and `section`.
Reducers must ignore any result that does not match the active tab generation.
This is the core production-safety rule for tabs, replacement opens, refresh,
restore, and stale background work.

The first implementation can use a smaller staged model:

- `Base`: high-priority base response that ends blocking loading
- `Enrichment`: background details merged into the same generation
- `Complete`: full resource load for `--once`, fixture mode, and explicit
  full-depth operations

The staged model is acceptable only as a compatibility bridge. The long-term
shape should still move toward section jobs so pagination, retries, caching, and
visible-tab priority become precise.

## Scheduling rules

Default live open:

1. Create a loading tab immediately.
2. Schedule `Base`.
3. Apply `Base` immediately when it returns.
4. Schedule high-value enrichment for the active tab.
5. Schedule hidden-tab enrichment only when the active tab is idle.
6. Schedule file patches only when Files is visible or a file row is expanded.

Priority order:

1. Active tab base load
2. Active tab visible-section load
3. User-triggered refresh or replace-current load
4. Active tab background enrichment
5. Hidden tab base load
6. Hidden tab enrichment
7. Low-value optional metadata

The loader should dedupe identical in-flight jobs. It should also suppress noisy
automatic retries after a section fails until the user explicitly refreshes or
reopens the resource.

## Rendering rules

The UI shape should stay stable while data loads:

- Base success replaces the loading placeholder with real PR/issue content.
- Background loading detail should not push header metadata sideways.
- Section placeholders belong inside the relevant tab, not in the global header.
- Overview should remain chronological and readable while timeline enrichment is
  still loading.
- File rows can appear before patch text is available.
- Diff patch rows should show a small loading row until patch context arrives.
- Scroll should not jump when enrichment lands unless the user is anchored to the
  end of a chronological feed.
- Failed optional sections should render as warnings or section-level failures,
  not as total resource failures.

## Cache and restore

Persistent restore should save enough metadata to recover a useful session after
a restart:

- open tabs and active tab
- resource identity and kind
- tab generation or restore generation
- scroll positions, selected tab, feed order, expansion state
- cached base resource snapshot
- cached section pages with freshness metadata
- section failure state only when it is useful to show, not as a permanent block

On restore, cached base content should render immediately. Visible sections
should refresh first. Stale or missing sections should load in the background.
The app should never restore into a blank screen when a cached base snapshot is
available.

## Testing plan

Unit and integration tests should cover:

- startup enters the TUI before GitHub data returns
- base data replaces the placeholder before enrichment completes
- enrichment errors add warnings without failing the resource
- stale enrichment is ignored after tab replacement
- stale enrichment is ignored after a newer request for the same resource
- lazy file patch loading only starts when Files needs patch text
- failed lazy file patches do not retry on every frame
- `--once` still waits for complete data
- fixture mode stays deterministic
- current layout is preserved during progressive updates

Real-world verification should include:

- a live public PR opened through direct GitHub API calls
- a live public issue opened through direct GitHub API calls
- a multi-tab session where one tab is replaced while another is still loading
- a Files-tab run that proves patch text loads after the base changed-file list
- tmux captures at narrow, medium, and large sizes after source changes that can
  affect rendering
- CI running the full local script before merge

## Implementation checklist

- [ ] Keep direct GitHub API calls as the only data source.
- [ ] Keep `gh` limited to credential fallback through `gh auth token`.
- [ ] Preserve the existing TUI layout while changing loading behavior.
- [ ] Split resource loading into base, enrichment, and complete paths.
- [ ] Emit base results before enrichment completes.
- [ ] Make reducers generation-aware for all staged results.
- [ ] Convert optional enrichment failures into warnings.
- [ ] Move diff patch loading behind Files-tab demand.
- [ ] Add section-level state once the staged bridge is stable.
- [ ] Store cache and restore metadata by resource section.
- [ ] Verify with unit tests, local CI, and real GitHub/tmux evidence.
