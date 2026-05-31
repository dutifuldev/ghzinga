---
title: Review Pagination Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# Review Pagination Plan

`ghzoom` currently gets pull request review summaries from the base PR query with
`reviews(first: 100)`. Regular comments, review-thread comments, timeline
events, commits, files, and checks now have paginated enrichment paths; reviews
should match that standard so large PRs do not silently lose review decisions or
review bodies past the first page.

## Goal

Fetch every `PullRequestReview` summary page exposed by GitHub and render those
reviews in the same Activity and chronological Overview flows as today.

## Design

- Add a dedicated `reviews_query(after)` for
  `pullRequest.reviews(first: 100, after: $after)`.
- Query the same fields used by the base PR query:
  - id
  - author
  - author association
  - body
  - state
  - submitted/updated timestamps
  - URL
  - reactions
- Convert each page with the existing `ReviewDto` -> `ActivityEntry` mapping.
- Replace only `ActivityKind::Review` entries when paginated review enrichment
  succeeds.
- Preserve comments, review-thread comments, and timeline events already added
  by other enrichment paths.
- Keep the base PR usable and show an enrichment warning if review pagination
  fails.

## Tests

- Query string test for pagination fields, review fields, and reactions.
- Page parser test for `hasNextPage` / `endCursor` and review activity
  conversion.
- Replacement test proving old review entries are swapped without removing
  comments, review comments, or timeline entries.
