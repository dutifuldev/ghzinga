---
title: ghzinga API Economy Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-06-01
---

# ghzinga API Economy Plan

`ghzinga` should spend GitHub API quota like a monitor, not like a crawler. A
single PR/issue screen needs fresh, high-signal data, but it should not keep
replaying every enrichment query when the already-fetched base response is good
enough.

## Current Problem

The authenticated account can have plenty of REST/core quota while GraphQL is
empty. In that state the app should not repeatedly try GraphQL and wait for
rate-limit errors before falling back to public REST.

The richer PR path also duplicates work. The base PR query already asks for the
first page of labels, assignees, review requests, linked issues, comments,
reviews, commits, status rollup, and changed files. The fetch path then runs
separate GraphQL queries for many of those same first pages. That burns GraphQL
points without changing the visible result for normal PRs.

## Fetch Policy

1. Check the GraphQL rate-limit bucket before authenticated GraphQL when the
   local cooldown is unknown or expired.
2. If GraphQL has `0` remaining, skip GraphQL until its reset time and use the
   existing public REST fallback for public repositories.
3. Treat the base GraphQL response as authoritative for first-page data.
4. Do not refetch first-page GraphQL surfaces that are already present in the
   base response.
5. Keep `GZG_API_DEPTH=full` as an explicit escape hatch for exhaustive
   pagination when the user is willing to spend more GraphQL quota.
6. Keep targeted default enrichments only where they add data the base query does not
   have:
   - PR review threads
   - chronological timeline events
   - check suite grouping
   - PR diff patches through REST
   - issue-only relationship and linked-branch metadata
   - participants metadata
7. Make background refresh conservative by default. Manual refresh remains
   immediate, but auto-refresh should default to five minutes instead of one
   minute.

## Expected API Shape

For a typical public PR with valid GraphQL quota, one load should use:

- one REST `/rate_limit` preflight when no cached GraphQL decision exists
- one base GraphQL query
- a small set of non-duplicative GraphQL enrichments
- one REST diff request for file patch bodies

If GraphQL is exhausted, one load should use:

- one REST `/rate_limit` preflight
- no GraphQL POSTs
- public REST calls for the fallback PR/issue view

Auto-refresh should reuse the same policy. If GraphQL is still in cooldown, it
should not probe GraphQL again until the reset time has passed.

## Verification

- Unit tests should cover GraphQL rate-limit preflight parsing and cooldown.
- Unit tests should cover the default refresh interval.
- Architecture tests should continue to ensure data transport uses direct HTTP
  and only shells out to `gh` for token fallback.
- Existing capture validation should still pass because the visible first-page
  data remains available from the base query or public REST fallback.
