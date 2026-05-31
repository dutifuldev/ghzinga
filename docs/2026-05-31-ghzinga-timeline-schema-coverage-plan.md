---
title: ghzinga Timeline Schema Coverage Plan
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# ghzinga Timeline Schema Coverage Plan

This plan closes another gap between ghzinga's chronological Activity view and
GitHub's current GraphQL timeline schema. The source of truth for this slice is
live GraphQL introspection on 2026-05-31 for
`IssueTimelineItemsItemType` and `PullRequestTimelineItemsItemType`.

## Current Gap

ghzinga already fetches and renders the high-signal issue and PR lifecycle
events: labels, assignees, locks, pins, references, duplicates, transfers,
title/milestone changes, issue type changes, sub-issues, blocking relationships,
review requests, draft/ready state, ref changes, force-pushes, merge queue,
auto-merge/rebase/squash changes, review dismissals, and merges.

The live schema still exposes additional timeline item types that are not in the
current `timelineItems` fetch list:

- classic project events: `ADDED_TO_PROJECT_EVENT`,
  `MOVED_COLUMNS_IN_PROJECT_EVENT`, `REMOVED_FROM_PROJECT_EVENT`
- project-v2 events: `ADDED_TO_PROJECT_V2_EVENT`,
  `REMOVED_FROM_PROJECT_V2_EVENT`,
  `PROJECT_V2_ITEM_STATUS_CHANGED_EVENT`
- draft and note conversion events: `CONVERTED_FROM_DRAFT_EVENT`,
  `CONVERTED_NOTE_TO_ISSUE_EVENT`
- moderation event: `USER_BLOCKED_EVENT`
- issue fields: `ISSUE_FIELD_ADDED_EVENT`,
  `ISSUE_FIELD_REMOVED_EVENT`, `ISSUE_FIELD_CHANGED_EVENT`
- PR deployment events: `DEPLOYED_EVENT`,
  `DEPLOYMENT_ENVIRONMENT_CHANGED_EVENT`

The schema also includes object timeline items such as `ISSUE_COMMENT`,
`PULL_REQUEST_COMMIT`, `PULL_REQUEST_REVIEW`, and review-thread objects. ghzinga
already fetches those through dedicated paginated comment, commit, review, and
review-thread queries so they can carry richer domain-specific rendering.

## Implementation Plan

- Add the missing event item types to `timeline_query`.
- Add fragments for scalar fields that are safe in the required timeline query:
  actor, timestamp, issue-field names/values, blocked user, deployment
  environment/status, and status/log URLs.
- Avoid requiring project title fields in the core timeline query. Project
  metadata can require broader `read:project` scope; ghzinga already handles
  project membership as optional enrichment. Timeline project events should stay
  visible even when the token cannot read project details.
- Map each new event to a clear single-line activity body.
- Preserve clickable URLs where GitHub exposes a useful target, especially
  deployment environment and log URLs.
- Keep a fallback body for future timeline event types so unknown events remain
  visible instead of crashing the Activity view.

## Verification

- Unit-test that issue queries include the new issue/project/field events and PR
  queries include the deployment events.
- Extend `timeline_activity_maps_github_events` with representative project-v2,
  issue-field, blocked-user, converted-note, and deployment payloads.
- Run `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets
  --all-features -- -D warnings`, and `npx -y @simpledoc/simpledoc check`.
- Run one live `--once` smoke check against `openclaw/openclaw#81834` after the
  GraphQL query changes so invalid fragments are caught against GitHub itself.
