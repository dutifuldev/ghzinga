---
title: Linked Resource Pagination Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# Linked Resource Pagination Plan

`ghzinga` still reads explicit PR/issue relationships from the base query with
`closingIssuesReferences(first: 100)` for pull requests and
`closedByPullRequestsReferences(first: 100)` for issues. These relationships
feed the Links tab and clickable navigation, so they should not silently stop at
the first GitHub connection page.

## Goal

Fetch every explicit linked-resource page for both resource kinds:

- Pull requests: linked issues the PR closes.
- Issues: linked pull requests that close the issue.

The renderer should keep receiving `Resource.related_resources` as
`Vec<ResourceId>`, but the vector should represent the complete GitHub
connection when enrichment succeeds.

## Design

- Add a selector-aware `linked_resources_query(kind)`.
- Query `closingIssuesReferences(first: 100, after: $after)` for PRs and
  `closedByPullRequestsReferences(first: 100, after: $after)` for issues.
- Fetch `pageInfo.hasNextPage` and `pageInfo.endCursor`.
- Reuse the existing URL/number fallback mapping into `ResourceId`.
- Replace `Resource.related_resources` on successful enrichment, including valid
  empty lists.
- Keep the base resource usable and add an enrichment warning if the paginated
  query fails.

## Tests

- Query tests for PR/issue connection names, selectors, and pagination fields.
- Page parser tests for cursor state and URL/number fallback mapping.
- Empty-response test proving a valid empty page can replace base first-page
  data.
