---
title: Label And Assignee Pagination Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# Label And Assignee Pagination Plan

`ghzoom` still reads labels and assignees from the base PR/issue query with
`labels(first: 100)` and `assignees(first: 100)`. Those fields appear in the
status band, Overview details, and dashboard summary, so they should follow the
same paginated enrichment pattern as comments, reviews, timeline events,
commits, files, checks, and projects.

## Goal

Fetch every label and assignee page for both pull requests and issues. The
renderer should keep receiving plain `Vec<String>` values on `Resource`, but the
vectors should represent the complete GitHub connections when enrichment
succeeds.

## Design

- Add selector-aware `labels_query(kind)` and `assignees_query(kind)`.
- Fetch `pageInfo.hasNextPage` and `pageInfo.endCursor`.
- Replace `Resource.labels` and `Resource.assignees` on successful enrichment,
  including valid empty lists.
- Keep the base resource usable and add an enrichment warning if either query
  fails.
- Reuse the existing label and user display-name normalization paths.

## Tests

- Query tests for PR/issue selector choice and pagination fields.
- Page parser tests for label and assignee cursor state.
- Enrichment replacement helpers that prove valid empty lists can replace base
  first-page data.
