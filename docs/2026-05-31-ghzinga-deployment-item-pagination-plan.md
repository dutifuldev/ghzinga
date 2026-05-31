---
title: Deployment Item Pagination Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# Deployment Item Pagination Plan

`ghzinga` now fetches deployment enrichment across every PR commit page, but each
commit still reads deployments with `deployments(last: 10)`. Expanded commit
rows render deployment/environment status, so a commit with many deployments can
still be capped.

## Goal

Fetch every deployment page for commits that report more deployments after the
first deployment page. The renderer should keep receiving deployments on each
`Commit`, but the deployment list should represent the complete GitHub
connection when enrichment succeeds.

## Design

- Change deployment enrichment to request `deployments(first: 100, after: null)`
  with `pageInfo`.
- Track commits whose deployment connection reports `hasNextPage`.
- Add a commit-object deployment query by commit OID with `$after`.
- Append remaining deployment pages to the matching commit's deployment list in
  the deployment-enrichment map.
- Reuse the existing deployment latest-status mapping.
- Preserve the current enrichment warning behavior if deployment pagination
  fails.

## Tests

- Query tests for nested deployment pagination fields and the commit deployment
  item query.
- Page parser tests for deployment cursor state and status mapping.
- Commit deployment page tests proving continuations are detected.
