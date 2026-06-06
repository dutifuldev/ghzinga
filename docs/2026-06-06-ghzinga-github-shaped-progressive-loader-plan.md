---
title: GitHub-Shaped Progressive Loader Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-06
---

# GitHub-Shaped Progressive Loader Plan

## Main goal

`gzg` should show useful PR/issue information as soon as GitHub returns it, then
fill in slower sections progressively without changing the overall TUI shape.

The loader should fit the shape of GitHub data:

- one resource identity: owner, repo, number, PR/issue kind
- one base object: title, URL, state, author, body, branches, summary counts
- many paginated connections: comments, timeline items, reviews, commits, files,
  checks, review threads, labels, assignees, linked resources
- section-level freshness, loading state, pagination, and errors

This is not a generic framework for arbitrary data sources. It is a
GitHub-aware loading system that keeps the renderer concrete and predictable.

## Product behavior

Opening `gzg owner/repo#number` should feel immediate:

1. The TUI opens right away with the normal shell.
2. The base resource replaces the loading placeholder as soon as the base
   GitHub response returns.
3. Overview becomes readable before slow timeline, review-thread, check-suite,
   participant, or diff-patch requests finish.
4. Each slower section updates the current tab when it arrives.
5. Stale results from old tabs, replaced tabs, closed tabs, or older refreshes
   are ignored.

Opening a new tab and replacing the current tab should follow the same rule:
the tab appears immediately, starts loading immediately, and never waits on the
previous resource screen before showing progress.

## Non-negotiables

- Use direct GitHub API calls. Do not shell out to `gh api` for resource data.
- Preserve the current TUI layout unless a later UI task explicitly changes it.
- Keep base data readable even when enrichment fails.
- Treat enrichment failures as warnings, not fatal resource errors.
- Lazy-load expensive diff patch text only when the Files tab needs it.
- Do not fetch every page of every connection just to render the first screen.
- Make request ownership explicit enough that stale async results cannot corrupt
  the current tab.

## Data model

The long-term model should separate the GitHub resource from its loaded
sections.

```rust
struct ResourceSnapshot {
    tab_id: u64,
    resource_id: ResourceId,
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

The first implementation can keep the existing renderer-facing `Resource` type
and bridge staged results into it. The important architectural rule is that
future code has a place to express partial section state instead of pretending a
resource is either completely absent or completely loaded.

## GitHub mapping

Base PR/issue data should include the first useful screen:

- title, URL, author, state, body
- created and updated timestamps
- labels, assignees, reactions
- branches, merge state, review decision, draft state
- first pages of comments, reviews, commits, changed files, and check rollup
- linked closing issues when available without expensive pagination

Background section jobs should map to GitHub connections:

| Section | GitHub source | Default policy |
| --- | --- | --- |
| Comments | GraphQL comments connection | First page in base; deeper pages on demand |
| Reviews | GraphQL reviews connection | First page in base; deeper pages on demand |
| Review threads | GraphQL reviewThreads connection | Background after base |
| Timeline | GraphQL timelineItems connection | Background after base |
| Commits | GraphQL commits connection | First page in base; paginate on demand |
| Files | GraphQL files connection | First page in base without patch bodies |
| File patches | REST pull request diff | Lazy after Files tab or file expansion |
| Status rollup | GraphQL statusCheckRollup contexts | First page in base; Checks tab priority |
| Check suites | Latest commit checkSuites | Background or Checks tab priority |
| Participants | GraphQL participants connection | Low-priority background |
| Linked resources | GraphQL closing/closed-by references | Base first; deeper pages on demand |

REST/core budget and GraphQL budget should be treated separately. REST diff
patches should stay lazy even when REST budget is healthy because they add
latency and payload size to the first screen.

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
    Enrichment,
    Timeline,
    ReviewThreads,
    CheckSuites,
    Participants,
    Files,
    FilePatches,
    FullDepth,
}
```

Every result must carry the same tab id, resource id, generation, and section.
Reducers must ignore results that no longer match the current tab generation.

The compatibility slice can use three fetch stages:

- `Base`: high-priority base response that ends the blocking load state.
- `Enrichment`: background details merged into the same resource generation.
- `Complete`: existing full resource load for fixtures, `--once`, full-depth
  loads, and specialized operations.

This gives most of the latency win before the larger section-state migration.

## Scheduling policy

Default open:

1. Create or replace the tab with a loading placeholder immediately.
2. Schedule `Base`.
3. Apply `Base` as soon as it returns.
4. Schedule `Enrichment` for timeline, review threads, check suites, and
   participants.
5. Schedule file patch loading only after the Files tab is active and at least
   one visible file lacks patch context.

Priority rules:

- Active tab beats background tabs.
- Visible tab section beats hidden sections.
- User-triggered refresh beats passive refresh.
- Base jobs beat enrichment jobs.
- Low API budget pauses non-visible enrichment before it blocks visible content.

## UI rules

- The initial frame should render the normal header, title area, nav selectors,
  content area, and bottom controls.
- Base data should remove the main loading placeholder.
- Background enrichment can show a small loading detail, but it should not push
  or rewrite important header information.
- Successful loads should not leave an `info loaded ...` message. Only active
  loading and errors need status text.
- Tabs should render with whatever section data exists.
- Section-specific placeholders belong inside the relevant tab, not as global
  noise.
- File diffs should show a clear loading row until patches arrive.
- Scroll position should not jump when enrichment updates a section unless the
  user is anchored to the bottom of a feed and new chronological content arrives.

## Cache and restore

The cache should eventually store base and sections separately:

- base resource by owner/repo/number/kind
- section data by owner/repo/number/kind/section/page
- section completeness and freshness

Session restore should show cached base content immediately, mark stale sections
as refreshable, and schedule visible-tab refreshes first. Failed or in-flight
sections should not restore as permanently failed.

## Implementation plan

1. Document this GitHub-shaped progressive loader target.
2. Split GitHub fetches into base, enrichment, and complete paths.
3. Emit a base fetch outcome before enrichment completes.
4. End the blocking loading state on base success.
5. Merge enrichment only when the active tab still matches the request id and
   resource id.
6. Convert enrichment errors into warnings when base content is already visible.
7. Move PR diff patch loading behind Files-tab demand.
8. Add retry suppression for failed lazy diff patch loads until the user
   explicitly refreshes.
9. Keep `--once` and fixture mode on complete loads so static output stays
   deterministic.
10. Add tests for staged loading, stale-result rejection, warning handling, and
    lazy file patches.
11. Run local CI, SimpleDoc, and real GitHub/herdr validation before merging.

Later slices:

- Replace the compatibility `Resource` bridge with first-class section state.
- Add explicit section pagination/load-more actions.
- Cache base and sections separately.
- Add low-budget scheduling when GraphQL rate limit is constrained.

## Test plan

Unit tests:

- base outcome replaces the placeholder before enrichment
- stale base/enrichment outcomes are ignored after refresh or tab replacement
- enrichment failure adds a warning without clearing base data
- file patch load starts only when Files needs it
- failed file patch load is marked unavailable and does not retry every frame

Integration tests:

- fake transport returns base quickly and delayed enrichment later
- delayed diff patch response does not block base render
- GraphQL rate-limit exhaustion pauses GraphQL enrichment while preserving
  already loaded content

Real validation:

- run `gzg` against a live PR with comments, checks, files, and reviews
- confirm the base PR appears before slower enrichment finishes
- open the Files tab and confirm diff patches load on demand
- use the CLI session command to open several tabs into a running `gzg`
- confirm stale background responses do not overwrite the focused tab
- exercise the flow in a herdr pane and normal tmux panes

## Acceptance criteria

- A normal PR renders useful base content without waiting for timeline, review
  threads, participants, check suites, or diff patch text.
- The UI shape remains stable.
- GitHub API usage is lower for users who only read the overview.
- Multiple tabs can load independently without stale result corruption.
- Background failures are visible but do not erase readable content.
- Real GitHub and herdr-pane validation pass before merge.
