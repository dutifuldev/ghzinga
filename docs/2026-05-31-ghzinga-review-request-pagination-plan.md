---
title: Review Request Pagination Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# Review Request Pagination Plan

`ghzinga` still reads PR review requests from the base query with
`reviewRequests(first: 100)`. Requested reviewers are visible in the PR overview
change summary, so they should follow the same paginated enrichment pattern as
labels, assignees, comments, reviews, commits, checks, files, and projects.

## Goal

Fetch every requested-reviewer page for pull requests. The renderer should keep
receiving `PullRequest.requested_reviewers` as a plain `Vec<String>`, but the
vector should represent the complete GitHub connection when enrichment
succeeds.

## Design

- Add a PR-only `review_requests_query()`.
- Fetch `pageInfo.hasNextPage` and `pageInfo.endCursor`.
- Preserve the existing requested-reviewer normalization for users and teams.
- Replace `PullRequest.requested_reviewers` on successful enrichment, including
  valid empty lists.
- Keep the base PR usable and add an enrichment warning if the paginated query
  fails.

## Tests

- Query test for pagination fields and requested-reviewer user/team fragments.
- Page parser test for cursor state and user/team display names.
- Empty-response test proving a valid empty page can replace base first-page
  data.
