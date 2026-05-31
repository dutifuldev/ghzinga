---
title: Commit Pagination Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# Commit Pagination Plan

`ghzoom` currently reads PR commits from the base PR GraphQL query with
`commits(first: 100)`. That is enough for many PRs, but it does not satisfy the
monitoring goal for larger PRs because the Commits tab and chronological
Overview can silently omit older or newer commits past the first page.

## Goal

Load every PR commit exposed by GitHub's pull request commit connection and keep
the renderer unchanged: it should receive a complete `Vec<Commit>` and render it
with the existing truncation, expansion, deployment, and chronological timeline
behavior.

## Design

- Add a dedicated `commits_query(after)` next to the other paginated enrichment
  queries.
- Query the same commit fields as the base PR query:
  - OID
  - headline
  - body
  - committed/authored timestamps
  - authors
- Preserve the direct GitHub API boundary; do not shell out to `gh` for commit
  data.
- Fetch pages until `pageInfo.hasNextPage` is false.
- Replace the base query's first-page commit list with the complete paginated
  result when enrichment succeeds.
- If enrichment fails, keep the base PR usable and show an enrichment warning.
- Continue applying deployment enrichment after the final commit list is known.

## Tests

- Query string test: `commits(first: 100, after: $after)`, `pageInfo`,
  `hasNextPage`, and `endCursor`.
- Page parser test: commit nodes become `Commit` values and preserve pagination
  cursor state.
- Application test: replacing commits swaps in the full list without touching
  other PR fields.
- Existing render tests cover commit expansion and chronological rendering once
  the domain model has the complete commit list.
