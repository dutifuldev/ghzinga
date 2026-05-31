---
title: Status Rollup Pagination Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# Status Rollup Pagination Plan

`ghzoom` already paginates latest-commit check suites, but the primary PR status
rollup still comes from the base PR query with `statusCheckRollup.contexts(first:
100)`. Large PRs can exceed that first page, which means the Checks tab and
status summary can silently miss check runs or legacy status contexts.

## Goal

Load every status rollup context GitHub exposes for the pull request and keep the
renderer unchanged. The domain model should receive the complete `Vec<CheckRun>`
before suite-level enrichment is merged in.

## Design

- Add a dedicated `status_rollup_query(after)` for
  `statusCheckRollup.contexts(first: 100, after: $after)`.
- Query both supported context node types:
  - `CheckRun`: name, status, conclusion, details URL, timestamps, workflow name
  - `StatusContext`: context, state, target URL
- Normalize workflow names the same way the base query does.
- Fetch pages until `pageInfo.hasNextPage` is false.
- Replace the base first-page rollup checks when paginated rollup enrichment
  succeeds.
- Keep the base PR usable and render a warning if enrichment fails.
- Apply suite enrichment after the complete rollup is loaded so suite-only rows
  can still be added without hiding direct rollup contexts.

## Tests

- Query string test for pagination fields and both context node fragments.
- Page parser test that preserves `hasNextPage` / `endCursor`.
- Parser test that maps nested workflow names onto check names.
- Existing Checks tab tests continue to verify grouping, expansion, and click
  targets from the domain model.
