---
title: ghzoom Verification Matrix
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# ghzoom Verification Matrix

This matrix maps the requested `ghzoom` behavior to current implementation
evidence. It is intentionally concrete so future changes can see which tests,
captures, and smoke checks prove each behavior.

## Current Scope

`ghzoom` is a standalone Rust TUI for viewing one GitHub PR or issue. It uses
Ratatui for rendering, Crossterm for terminal input/mouse capture, Tokio for the
async loop, and direct GitHub GraphQL/REST requests for GitHub data. The
installed `gh` CLI is used only as a fallback credential source via
`gh auth token`.

## Requirement Evidence

| Requirement | Evidence |
| --- | --- |
| Full Rust standalone app | `Cargo.toml`, `src/main.rs`, `src/lib.rs`, Rust-only app modules under `src/` |
| Ratatui/Crossterm architecture | `src/render/`, `src/terminal/mod.rs`, `src/app/update.rs` |
| Slophammer-style architecture guardrails | `tests/architecture.rs` verifies domain purity, GitHub adapter isolation from TUI layers, and no `gh pr view` / `gh issue view` / `gh api` data transport regressions |
| Automated quality gate | `.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `npx -y @simpledoc/simpledoc check` on pull requests and pushes to `main` |
| Mouse capture with opt-out | `TerminalGuard::enter(mouse_enabled)` enables `EnableMouseCapture`; CLI exposes `--no-mouse` |
| Uses existing auth, no app login | Base PR/issue fetches, paginated GraphQL enrichment, and PR diff patch context use direct HTTPS requests with `GH_TOKEN` / `GITHUB_TOKEN` or the token from `gh auth token`; `graphql_transport_receives_post_shape_and_returns_body`, `rest_transport_receives_get_shape_and_returns_body`, `graphql_transport_errors_on_graphql_errors_payload`, and `rest_transport_includes_status_and_body_on_http_failure` verify the mockable HTTP transport boundary; auth failures point to `gh auth status` / `gh auth login` |
| PR body, labels, reactions, author, state, branches | Overview/status render tests and live `cargo run -- openclaw/openclaw#81834 --once` |
| PR metadata such as draft/cross-repo/mergeability/milestone/projects/ref OIDs | `pr_view_preserves_extra_github_metadata`, `project_items_query_uses_selector_and_pagination_state`, `project_items_page_preserves_pagination_state`, `apply_project_metadata_replaces_existing_value_and_dedupes`, `project_scope_errors_are_suppressed_for_optional_metadata`, `renders_resource_and_pr_metadata`, live overview smoke |
| PR comments, reviews, review comments, timeline events | `pr_activity_includes_reviews_with_state`, `comments_query_uses_selector_and_pagination_state`, `comment_activity_page_preserves_pagination_state`, `replace_comment_activity_keeps_other_activity`, `review_threads_query_requests_pagination_state`, `review_thread_comments_query_requests_comment_pagination_state`, `review_thread_activity_keeps_path_and_line`, `review_thread_activity_page_preserves_pagination_state`, `review_thread_comments_page_preserves_pagination_state`, `review_thread_activity_shows_thread_state`, `review_threads_summary_counts_unique_unresolved_and_outdated_threads`, `renders_review_thread_summary_in_pr_status`, `timeline_activity_maps_github_events`, `timeline_activity_page_preserves_pagination_state`, PR activity captures; activity rows preserve author association, edit/minimized state, reactions, permalinks, labels, references, assignments, pins, locks, duplicate markers, transfers, connected/disconnected references, review requests, draft/ready state, branch changes, force-pushes, merge queue changes, review dismissals, auto-merge/rebase/squash changes, automatic base changes, merges, title changes, milestones, issue types, sub-issues, parent issues, blocking relationships, converted discussions, close, and reopen events; ordinary comment, review-thread, nested review-thread comment, and timeline GraphQL pages are fetched until `hasNextPage` is false |
| PR commits | `commit_from_dto_preserves_body_dates_and_authors`, `commit_deployments_from_response_maps_environment_status_and_urls`, `applies_commit_deployments_to_matching_commits`, `commit_rows_are_click_expandable`, `expanded_commit_rows_show_deployments`, PR commits captures under `captures/ghzoom-pr-81834/*/20_commits_top.*`, live commits smoke |
| PR checks/CI | `checks_are_grouped_by_status`, `check_from_dto_preserves_github_metadata`, `check_from_dto_handles_status_context_fields`, `check_suites_query_requests_pagination_state`, `check_suite_from_dto_maps_workflow_status_and_urls`, `check_suites_page_preserves_pagination_state`, `check_suites_from_response_keeps_latest_suite_by_name`, `apply_check_suites_dedupes_existing_names`, `check_rows_are_click_expandable`, live checks smoke, PR checks captures; latest-commit check suite GraphQL pages are fetched until `hasNextPage` is false |
| PR changed files and patch context | `changed_files_from_graphql_keep_change_type`, `pr_diff_uses_rest_pull_diff_path`, `parses_unified_diff_patches_by_file_path`, `expanded_file_rows_show_patch_context`, `long_patch_rows_are_click_expandable`, PR files captures; patch context uses the direct REST pull-request diff media type |
| Issue body, reactions, comments, timeline events, labels, author, state | issue fixture integration test and issue captures under `captures/ghzoom-issue-88499/`; comment and timeline metadata is normalized through the shared activity model, including pins, locks, duplicate markers, transfers, connected/disconnected references, issue types, sub-issues, parent issues, blocking relationships, and converted discussions; ordinary comment and timeline GraphQL pages are fetched until `hasNextPage` is false |
| Issue metadata such as pinned/state reason/closed time/milestone/projects | `issue_view_preserves_extra_github_metadata`, `project_items_query_uses_selector_and_pagination_state`, `project_items_page_preserves_pagination_state`, `apply_project_metadata_replaces_existing_value_and_dedupes`, `project_scope_errors_are_suppressed_for_optional_metadata`, issue overview smoke |
| Linked issue/PR navigation targets | `render_registers_github_link_hit_area`, `render_registers_relative_issue_link_hit_area`, `rendered_visible_link_hit_area_can_be_clicked_to_navigate` |
| Exact detail URL open targets | `render_registers_exact_comment_url_as_open_url`, `check_rows_are_click_expandable`, `expanded_commit_rows_show_deployments`, `keyboard_enter_opens_first_visible_url_action`, `mouse_click_on_url_target_requests_open_url`, `url_open_command_uses_browser_env_when_available` |
| Explicit GitHub relationship links | `related_resource_ids_parse_urls_and_number_fallbacks`, `links_tab_renders_explicit_related_resources_once` |
| Auto-refresh | `run_tui()` interval path plus `apply_refreshed_resource` tests preserving tab/scroll and recording changed/no-change state; `renders_last_refresh_changed_sections` verifies changed surfaces render in the status panel; `fingerprint_changes_when_activity_content_or_metadata_changes` verifies activity body and metadata changes are part of change detection |
| Manual refresh | reducer tests for `r` and `[refresh]`, footer render-to-click tests |
| Visible truncation/expansion | `[more]` / `[less]` render tests for body, activity, checks, and files |
| Mouse click routing | synthetic Crossterm `MouseEvent` tests in `src/app/update.rs` and render-to-click tests in `src/render/resource.rs` |
| Keyboard shortcuts avoid tmux/herdr conflicts | reducer supports arrows, PageUp/PageDown, Home/End, Tab/Shift-Tab, Ctrl-i fallback, Enter for first visible content action, `r`, `?`, `q`, `o`, Backspace; no Ctrl-b/Ctrl-a/Ctrl-d/Ctrl-u primary shortcuts |
| No special UI fonts required | renderer uses ASCII chrome: `[more]`, `[less]`, `[refresh]`, `[open]`, `[quit]`, `[help]`, `-` rules, text status labels |
| Narrow/medium/large UX rendering | regenerated tmux captures for `80x24`, `120x36`, `160x50` in PR and issue capture directories |
| Current-resource browser open | reducer tests for `o` and `[open]`; smoke checks with `BROWSER=echo gh pr view ... --web` and `gh issue view ... --web` |

## Capture Evidence

PR captures:

```text
captures/ghzoom-pr-81834/
```

Issue captures:

```text
captures/ghzoom-issue-88499/
```

Each size directory contains `.txt`, `.ansi`, and `.png` frames. The current
marker check verifies:

- PR: Activity, Commits, Checks, Files, Links, Help at narrow/medium/large sizes
- Issue: Overview, Activity, Links, Help at narrow/medium/large sizes
- footer control `[open]` appears in every checked frame

## Click Coverage

Click behavior is verified at two levels:

1. Reducer-level tests construct Crossterm `MouseEvent` values and assert the
   returned `AppIntent` or state mutation.
2. Render-to-click tests draw with Ratatui `TestBackend`, use the actual
   registered hit rectangles, synthesize a mouse click inside the rectangle, and
   assert the resulting behavior.

Covered click targets:

- tabs
- body `[more]` / `[less]`
- activity/comment `[more]` / `[less]`
- check rows
- file rows and long patch expansion controls
- visible body/activity links
- exact check run, deployment, and comment URLs
- Links-tab navigation rows
- Enter activation for the first visible content action
- `[refresh]`
- `[open]`
- `[quit]`
- `[help]`

## Live Smoke Checks

Recent live checks through direct GitHub API data fetches:

```sh
cargo run -- openclaw/openclaw#81834 --tab links --once
cargo run -- https://github.com/openclaw/openclaw/issues/88499 --tab links --once
cargo run -- openclaw/openclaw#81834 --tab checks --once
```

Browser-open command smoke checks:

```sh
BROWSER=echo gh pr view 81834 -R openclaw/openclaw --web
BROWSER=echo gh issue view 88499 -R openclaw/openclaw --web
```

These proved that the open action must use `gh pr view --web` or
`gh issue view --web`, not `gh browse <full-url>`. The implementation was fixed
accordingly.

## Remaining Risk

The app now covers the requested core behavior, plus additional high-value
GitHub metadata exposed by the current fetch path: milestones, optional project
membership, issue state reason, pinned state, PR draft/cross-repository flags,
mergeability, changed-file totals, ref OIDs, and merge-commit metadata.

The phrase "all the info available from GitHub" is still broader than any
practical first version. Current coverage includes the requested body/reactions/
comments/review-comments/timeline/commits/deployments/check-suites/CI/files/status/link
surfaces, expanded-file patch context, and the metadata above. It now covers a
broad set of high-signal issue/PR lifecycle timeline events, but it still does
not attempt to render every possible GitHub timeline event.

That is the main remaining product-scope risk if the bar is interpreted as
literally every GitHub field rather than the requested monitoring dashboard
surfaces.

Project metadata is requested as optional paginated GraphQL enrichment because
`projectItems.project.title` requires the broader `read:project` token scope.
If the token lacks that scope, ghzoom suppresses that optional enrichment error
and keeps the PR or issue view usable.
