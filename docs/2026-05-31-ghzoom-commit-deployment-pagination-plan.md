---
title: Commit Deployment Pagination Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# Commit Deployment Pagination Plan

`ghzoom` paginates the PR commit list, but deployment enrichment still reads
deployments through `commits(last: 100)`. That means a long-running PR can show
every commit while only the newest 100 commits receive deployment/environment
status.

## Goal

Fetch deployment enrichment for every commit page that GitHub returns for a pull
request. The renderer should keep receiving deployments on each `Commit`, but
deployment data should cover the same commit set as the paginated Commits tab.

## Design

- Add `$after` pagination to `commit_deployments_query()`.
- Fetch `commits(first: 100, after: $after)` with `pageInfo`.
- Keep the existing deployment DTO and latest-status mapping.
- Merge each commit page into a single `HashMap<oid, Vec<Deployment>>`.
- Keep the PR usable and preserve the existing enrichment warning if deployment
  fetching fails.

## Tests

- Query test for commit pagination fields and deployment status fields.
- Page parser test for cursor state and deployment mapping.
- Empty-response test proving valid empty pages produce an empty deployment map.
