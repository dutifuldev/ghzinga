---
title: Progressive Loader Main Goal
author: Bob <dutifulbob@gmail.com>
date: 2026-06-06
---

# Progressive Loader Main Goal

## Main Goal

`gzg` should open GitHub pull requests and issues with the same feeling of
immediacy as GitHub web: show the shell right away, show the core resource as
soon as the first useful GitHub response returns, then fill in slower sections
progressively.

The user should never wait for every timeline page, review thread, check suite,
participant list, or diff patch before reading the PR or issue title, body,
state, branches, and first useful activity.

This is a loading architecture goal, not a visual redesign. The existing TUI
shape should stay stable while the data arrives in better stages.

## Product Target

Opening or replacing a resource should behave like this:

1. The tab appears immediately with the normal header, title area, nav row,
   content area, and bottom controls.
2. The base PR/issue data replaces the placeholder as soon as the base request
   completes.
3. The Overview is readable before slow optional sections finish.
4. Activity, Checks, Files, Links, and other sections improve as their data
   arrives.
5. Failed optional sections become warnings on the visible resource, not fatal
   load failures.
6. Successful loads stay quiet; no `info loaded ...` message is shown after a
   normal success.

The app should feel busy only while it is actually doing work. Loading details
belong in the existing loading/status area and should not push title, link,
state, branch, or tab controls sideways.

## GitHub-Shaped Data

The loader should fit how GitHub exposes PRs and issues.

Every resource has:

- one identity: owner, repo, number, and PR/issue kind
- one base object: title, URL, state, author, body, timestamps, branches, and
  summary counts
- many slower connections: comments, reviews, timeline items, commits, changed
  files, checks, review threads, labels, assignees, participants, linked
  resources, and project metadata

That shape should be represented explicitly in the app. A resource can be
readable while some sections are still loading, partially loaded, stale, or
failed.

## Loading Stages

The near-term implementation can keep the existing renderer-facing `Resource`
type, but the fetch path should use explicit stages:

| Stage | Purpose |
| --- | --- |
| `Base` | Fetch enough GitHub data to render the first useful PR/issue screen. |
| `Enrichment` | Fetch slower timeline, review-thread, check-suite, participant, and metadata data. |
| `FilePatches` | Fetch REST diff patch text only when the Files tab or file expansion needs it. |
| `Complete` | Preserve deterministic full loads for `--once`, fixture mode, full-depth operations, and tests. |

Base data must end the blocking loading state. Enrichment and file-patch results
must merge into the current tab only if they still belong to the same resource
and request generation.

## Request Ownership

Every async result needs enough ownership information to prove it still belongs
on screen:

- tab id
- resource id
- request id or generation
- load stage or section

Reducers must ignore stale results from replaced tabs, closed tabs, older
refreshes, or resources that are no longer active in that tab.

## API Policy

Use direct GitHub API calls. Do not shell out to `gh api` for resource data.

REST/core and GraphQL budgets are separate and should be treated separately.
GraphQL work should be prioritized for visible data. REST diff patch fetching
should stay lazy because patch text is large and should not block the first
readable screen.

Optional enrichment failures should preserve base content. For example, if REST
diff patch fetching fails, the PR should still load and the Files tab should
show a warning or fallback row instead of failing the whole resource.

## Acceptance Criteria

- Opening a live PR or issue renders the TUI shell immediately.
- Base title, link, state, body, and first useful content become visible before
  slow enrichment finishes.
- Opening a new tab starts loading that tab immediately without waiting on the
  previous resource.
- Replacing the current tab immediately replaces the visible resource with the
  target loading state.
- Stale enrichment from old requests cannot overwrite the active tab.
- File patch text is loaded on Files-tab demand, not as a first-screen
  prerequisite.
- Complete loads still include file patch context when available.
- Complete loads do not fail solely because optional file patch context is
  unavailable.
- Successful loads do not leave `info loaded ...` status noise.
- Tests cover staged loading, stale-result rejection, lazy file patches,
  optional patch warnings, and complete-load compatibility.

## Related Docs

- [Progressive Resource Loading Spec](2026-06-06-ghzinga-progressive-resource-loading-spec.md)
- [GitHub-Shaped Progressive Loader Plan](2026-06-06-ghzinga-github-shaped-progressive-loader-plan.md)
- [Progressive Loader Architecture Spec](2026-06-06-ghzinga-progressive-loader-architecture-spec.md)
