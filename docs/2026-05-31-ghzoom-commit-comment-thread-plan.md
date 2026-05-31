---
title: ghzoom Commit Comment Thread Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# ghzoom Commit Comment Thread Plan

This plan adds one missing PR conversation surface: GitHub commit comment
threads that appear in the pull request timeline as
`PULL_REQUEST_COMMIT_COMMENT_THREAD`.

## Gap

ghzoom currently fetches and renders:

- issue and PR comments
- PR reviews
- PR review threads and nested review-thread comments
- generic issue/PR lifecycle timeline events
- PR commits as a dedicated tab and chronological overview entries

GitHub's PR timeline schema also has commit comment thread objects. These are
not review threads and are not ordinary issue comments. Without them, a PR that
has comments directly on commits can look complete in ghzoom while missing part
of the GitHub web conversation.

## Design

- Include `PULL_REQUEST_COMMIT_COMMENT_THREAD` only in PR timeline queries.
- Render each nested commit comment as an activity entry with a distinct
  `Commit comment` kind.
- Preserve the commit comment permalink, author association, reactions,
  minimized state, edit marker, file path, and best available position.
- Include the short commit OID in the activity body so the comment is clearly
  tied to a commit, not a review thread.
- Fetch nested commit comment pages until `hasNextPage` is false, using the
  thread node ID just like review-thread comment pagination.
- Keep review-thread summary counts scoped to actual review threads, not commit
  comment threads.

## Verification

- Query test proves PR timeline queries include
  `PULL_REQUEST_COMMIT_COMMENT_THREAD` while issue timeline queries do not.
- Mapper tests prove commit comment threads become clickable `Commit comment`
  activity entries with path, position, reactions, edit/minimized metadata, and
  thread ID preserved.
- Pagination test proves commit comment thread comment pages preserve
  `hasNextPage`, `endCursor`, and comment payloads.
- Live `cargo run -- openclaw/openclaw#81834 --tab activity --once` proves the
  updated GraphQL query is accepted by GitHub.
