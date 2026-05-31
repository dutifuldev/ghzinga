---
title: ghzoom Revision Marker Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# ghzoom Revision Marker Plan

This plan adds the remaining lightweight PR timeline object:
`PULL_REQUEST_REVISION_MARKER`.

## Gap

After adding commit comment threads, the live
`PullRequestTimelineItemsItemType` schema still has one PR-only timeline object
that ghzoom neither fetches directly nor delegates to a richer dedicated query:

- `PULL_REQUEST_REVISION_MARKER`

The other object-style timeline items are already covered:

- `PULL_REQUEST_COMMIT`: fetched through the paginated Commits tab and included
  in the chronological overview.
- `PULL_REQUEST_REVIEW`: fetched through the paginated reviews query.
- `PULL_REQUEST_REVIEW_THREAD`: fetched through paginated review threads and
  nested review-thread comments.
- `PULL_REQUEST_COMMIT_COMMENT_THREAD`: fetched from timeline and nested comment
  pages are followed by node ID.

## Design

- Add `PULL_REQUEST_REVISION_MARKER` to PR timeline item types only.
- Query `createdAt` and `lastSeenCommit { oid }`.
- Render it as a generic timeline row like `revision marker at <short-oid>`.
- Use the timeline index fallback ID because the GraphQL type does not expose an
  `id` field.

## Verification

- Query test proves PR timelines include `PULL_REQUEST_REVISION_MARKER` and
  issue timelines do not.
- Mapper test proves `PullRequestRevisionMarker` becomes a visible timeline row
  with a short commit OID.
- Live `cargo run -- openclaw/openclaw#81834 --tab activity --once` proves the
  updated GraphQL query is accepted by GitHub.
