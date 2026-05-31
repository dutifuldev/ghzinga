---
title: Commit Author Pagination Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# Commit Author Pagination Plan

`ghzinga` paginates pull-request commits, but each commit still reads authors
with `authors(first: 100)`. The expanded Commits tab renders the coauthor list,
so very large coauthored commits can still be capped even when the commit list
itself is complete.

## Goal

Fetch every commit-author page for commits that report more authors after the
first page. The renderer should keep receiving a plain `Vec<String>` on each
`Commit`, but the vector should represent the complete GitHub author connection
when enrichment succeeds.

## Design

- Add `pageInfo` to the nested `authors(first: 100)` connection in the commit
  list query.
- Track commits whose author connection reports `hasNextPage`.
- Add a commit-object author query by commit OID with `$after`.
- Append remaining author pages to the matching `Commit.authors` list.
- Reuse the existing author display-name fallback for anonymous commit authors.
- Keep the existing commit-list warning behavior if author pagination fails.

## Tests

- Query tests for nested author pagination fields and the commit-author query.
- Page parser tests for author cursor state and display-name fallback.
- Commit page tests proving continuations are detected without changing the
  existing commit domain shape.
