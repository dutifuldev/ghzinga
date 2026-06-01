---
title: ghzinga Verification Matrix
author: Bob <dutifulbob@gmail.com>
date: 2026-05-31
---

# ghzinga Verification Matrix

This matrix maps the requested `ghzinga` behavior to current implementation
evidence. It is intentionally concrete so future changes can see which tests,
captures, and smoke checks prove each behavior.

## Current Scope

`ghzinga` is a standalone Rust TUI for viewing one GitHub PR or issue. It uses
Ratatui for rendering, Crossterm for terminal input/mouse capture, Tokio for the
async loop, and direct GitHub GraphQL/REST requests for GitHub data. The
installed `gh` CLI is used only as a fallback credential source via
`gh auth token`.

## Requirement Evidence

| Requirement | Evidence |
| --- | --- |
| Full Rust standalone app | `Cargo.toml`, `src/main.rs`, `src/bin/ghzinga.rs`, `src/lib.rs`, Rust-only app modules under `src/` |
| Ratatui/Crossterm architecture | `src/render/`, `src/terminal/mod.rs`, `src/app/update.rs` |
| Slophammer-style architecture guardrails | `AGENTS.md` and `docs/2026-06-01-ghzinga-slophammer-guardrails.md` map the Slophammer/Uncle Bob conventions onto this Rust TUI; `tests/architecture.rs` verifies domain purity, GitHub adapter isolation from TUI layers, render/input/app/terminal adapter boundaries, dedicated public REST fallback ownership, dedicated HTTP transport test ownership, no `gh pr view` / `gh issue view` / `gh api` data transport regressions, and that `Command::new("gh")` appears only in the auth-token fallback |
| Automated quality gate | `.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `scripts/verify-install.sh`, `npx -y @simpledoc/simpledoc check`, `scripts/verify-no-png-captures.sh`, validate-only PR/issue capture evidence checks, and validate-only tmux mouse-smoke evidence checks on pull requests and pushes to `main`; the PNG guard rejects both tracked and generated PNG artifacts under `captures/` so UX evidence stays in terminal text and ANSI transcripts |
| Install command aliases | `Cargo.toml` declares both `gzg` and `ghzinga` binary targets; `tests/cli_once.rs` verifies both command names render the PR fixture; `scripts/install.sh` installs `gzg` and links `ghzinga -> gzg`; `scripts/verify-install.sh` verifies plain `cargo install --path .` exposes both executable commands and the repo installer creates a real `ghzinga` symlink that runs the same TUI |
| Mouse capture with opt-out | `TerminalGuard::enter(mouse_enabled)` enables `EnableMouseCapture`; CLI exposes `--no-mouse`; `TerminalGuard` tracks raw mode, alternate screen, and mouse capture independently so partial setup failures still restore only the terminal features that were enabled |
| Uses existing auth, no app login | Base PR/issue fetches, paginated GraphQL enrichment, PR diff patch context, and the `src/github/public_rest.rs` unauthenticated fallback use direct HTTPS requests with `GH_TOKEN` / `GITHUB_TOKEN` or the token from `gh auth token`; auth tests verify token precedence, whitespace trimming, blank-token fallback from `GH_TOKEN` to `GITHUB_TOKEN`, successful trimmed `gh auth token` output, empty `gh auth token` rejection, unavailable-auth classification for GitHub CLI login failures, and public REST fallback classification for clear token-rejection responses such as HTTP 401, Bad credentials, SAML SSO authorization rejection, resource-not-accessible token errors, and GitHub rate-limit errors; public PRs/issues fall back to unauthenticated REST when local credentials are unavailable, clearly rejected, or rate-limited, and public REST comments, PR reviews, PR review comments, timeline events, PR commits, changed files, public check runs, public check suites, head-commit deployments, and legacy status contexts are loaded without auth where GitHub exposes them publicly; normal GraphQL mode requests cheap `hasNextPage` markers on first-page base collections and warns when a section needs `GZG_API_DEPTH=full` for exhaustive pagination; `graphql_transport_receives_post_shape_and_returns_body`, `rest_transport_receives_get_shape_and_returns_body`, `rest_transport_can_omit_authorization_for_public_requests`, `public_rest_pr_fallback_renders_core_monitoring_surfaces`, `pr_fallback_normalizer_tolerates_missing_optional_fields`, `public_rest_fallback_warning_describes_auth_unavailable`, `public_rest_fallback_warning_describes_token_api_errors`, `base_pr_query_requests_partial_depth_markers`, `base_issue_query_requests_partial_depth_markers`, `base_pr_partial_depth_warning_lists_paginated_sections`, `base_issue_partial_depth_warning_lists_paginated_sections`, `public_rest_issue_fallback_renders_core_monitoring_surfaces`, `pages_until_short_page_without_auth`, `public_rest_fallback_includes_rate_limited_token_errors`, `public_rest_reviews_without_auth`, `public_rest_review_comments_without_auth`, `public_rest_timeline_events_without_auth`, `public_rest_check_runs_page_without_auth_and_preserve_urls`, `public_rest_check_suites_page_without_auth_and_preserve_suite_rollups`, `public_rest_head_deployments_without_auth_and_preserve_latest_status_urls`, `public_rest_status_contexts_without_auth`, `graphql_transport_errors_on_graphql_errors_payload`, and `rest_transport_includes_status_and_body_on_http_failure` verify the mockable HTTP transport boundary and public fallback |
| PR body, labels, reactions, author, state, branches | Overview/status render tests verify the fixed status band highlights PR/issue state with a background badge and shows branch direction, checks, file count, and additions/deletions without comment/review/reaction/thread/warning counters; live `cargo run --bin gzg -- openclaw/openclaw#81834 --once`; `labels_query_uses_selector_and_pagination_state`, `labels_page_preserves_pagination_state_and_filters_blank_values`, `assignees_query_uses_selector_and_pagination_state`, and `assignees_page_preserves_pagination_state_and_display_names` verify paginated label and assignee enrichment for PR/issue selectors |
| PR metadata such as draft/cross-repo/mergeability/locked state/merge queue/rebaseability/total comments/milestone/projects/ref OIDs | `pr_view_preserves_extra_github_metadata`, `base_pr_query_requests_current_status_metadata`, `project_items_query_uses_selector_and_pagination_state`, `project_items_page_preserves_pagination_state`, `apply_project_metadata_replaces_existing_value_and_dedupes`, `project_scope_errors_are_suppressed_for_optional_metadata`, `review_requests_query_requests_pagination_state_and_reviewer_fragments`, `review_requests_page_preserves_pagination_state_and_display_names`, `renders_resource_and_pr_metadata`, live overview smoke |
| PR comments, reviews, review comments, commit comments, timeline events | `pr_activity_includes_reviews_with_state`, `comments_query_uses_selector_and_pagination_state`, `comment_activity_page_preserves_pagination_state`, `replace_comment_activity_keeps_other_activity`, `reviews_query_requests_pagination_state_and_reaction_fields`, `review_activity_page_preserves_pagination_state`, `replace_review_activity_keeps_comments_review_comments_and_timeline`, `review_threads_query_requests_pagination_state`, `review_thread_comments_query_requests_comment_pagination_state`, `review_thread_activity_keeps_path_and_line`, `review_thread_activity_page_preserves_pagination_state`, `review_thread_comments_page_preserves_pagination_state`, `commit_comment_thread_comments_page_preserves_pagination_state`, `public_rest_reviews_without_auth`, `public_rest_review_comments_without_auth`, `public_rest_timeline_events_without_auth`, `timeline_activity_maps_commit_comment_threads`, `review_thread_activity_shows_thread_state`, `review_threads_summary_counts_unique_unresolved_and_outdated_threads`, `renders_review_thread_summary_in_pr_status`, `timeline_query_accounts_for_current_github_schema_item_types`, `timeline_activity_maps_github_events`, `timeline_activity_maps_schema_coverage_events`, `timeline_activity_page_preserves_pagination_state`, PR activity captures; activity rows preserve author association, edit/minimized state, reactions, permalinks, labels, references, assignments, pins, locks, duplicate markers, transfers, connected/disconnected references, review requests, draft/ready state, branch changes, force-pushes, merge queue changes, review dismissals, auto-merge/rebase/squash changes, automatic base changes, merges, deployments, title changes, milestones, project movements, project-v2 status changes, issue types, issue fields, user blocks, converted project notes, revision markers, sub-issues, parent issues, blocking relationships, converted discussions, close, and reopen events; ordinary comment, review summary, review-thread, nested review-thread comment, commit-comment-thread, nested commit-comment, and timeline GraphQL pages are fetched until `hasNextPage` is false; unauthenticated public PR fallback additionally loads public review summaries, review comments, and REST issue timeline events, while GraphQL-only review-thread resolution/outdated state remains unavailable in that fallback |
| PR commits | `commits_query_requests_pagination_state_and_full_commit_fields`, `commits_page_preserves_pagination_state_and_author_fallbacks`, `commit_authors_query_requests_commit_object_and_pagination_state`, `commit_authors_page_preserves_pagination_state_and_display_names`, `replace_pr_commits_keeps_base_commits_when_paginated_list_is_empty`, `commit_from_dto_preserves_body_dates_and_authors`, `commit_deployments_query_requests_commit_pagination_state_and_status_fields`, `commit_deployment_items_query_requests_commit_object_and_pagination_state`, `commit_deployments_page_preserves_pagination_state`, `commit_deployment_items_page_preserves_pagination_state_and_status_mapping`, `commit_deployments_from_response_maps_environment_status_and_urls`, `applies_commit_deployments_to_matching_commits`, `public_rest_head_deployments_without_auth_and_preserve_latest_status_urls`, `commit_rows_are_click_expandable`, `expanded_commit_rows_show_full_commit_body`, `expanded_commit_rows_show_deployments`, PR commits captures under `captures/ghzinga-pr-81834/*/20_commits_top.*`, live commits smoke; PR commit GraphQL pages, nested commit-author pages, commit deployment enrichment pages, and nested deployment item pages are fetched until `hasNextPage` is false; unauthenticated public fallback enriches the PR head commit with public REST deployment statuses |
| PR checks/CI | `checks_are_grouped_by_status`, `check_from_dto_preserves_github_metadata`, `check_from_dto_handles_status_context_fields`, `status_rollup_query_requests_pagination_state_and_context_fields`, `status_rollup_page_preserves_pagination_state_and_context_types`, `status_rollup_checks_from_response_handles_null_rollup`, `check_suites_query_requests_pagination_state`, `check_suite_from_dto_maps_workflow_status_and_urls`, `check_suites_page_preserves_pagination_state`, `check_suites_from_response_keeps_latest_suite_by_name`, `apply_check_suites_dedupes_existing_names`, `public_rest_check_runs_page_without_auth_and_preserve_urls`, `public_rest_check_suites_page_without_auth_and_preserve_suite_rollups`, `public_rest_status_contexts_without_auth`, `maps_github_check_states_and_conclusions`, `check_rows_are_click_expandable`, live checks smoke, PR checks captures; status rollup and latest-commit check suite GraphQL pages are fetched until `hasNextPage` is false, and unauthenticated public fallback loads public check runs, public check suites, and legacy status contexts for the PR head SHA |
| PR changed files and patch context | `changed_files_from_graphql_keep_change_type`, `pr_diff_uses_rest_pull_diff_path`, `parses_unified_diff_patches_by_file_path`, `expanded_file_rows_show_patch_context`, `long_patch_rows_are_click_expandable`, `file_rows_style_diff_lines_by_change_kind`, PR files captures; patch context uses the direct REST pull-request diff media type, file rows are individually expandable, long patches have their own `[+ more patch]` / `[- less patch]` controls, and default diff styling renders additions green, deletions red, and hunk headers in an accent color |
| Issue body, reactions, comments, timeline events, labels, author, state | issue fixture integration test and issue captures under `captures/ghzinga-issue-88499/`; labels, assignees, participants, and issue relationships use shared-style paginated enrichment covered by `labels_query_uses_selector_and_pagination_state`, `labels_page_preserves_pagination_state_and_filters_blank_values`, `assignees_query_uses_selector_and_pagination_state`, `assignees_page_preserves_pagination_state_and_display_names`, `participants_query_uses_selector_and_pagination_state`, `participants_page_preserves_pagination_state_and_display_names`, `issue_parent_query_requests_parent_reference`, `issue_duplicate_query_requests_duplicate_reference`, `issue_relationships_query_requests_connection_and_pagination_state`, and `issue_relationships_page_preserves_pagination_state_and_kind_mapping`; `timeline_query_accounts_for_current_github_schema_item_types` guards current GitHub issue timeline enum coverage, with ordinary issue comments handled by the dedicated paginated comments query; comment and timeline metadata is normalized through the shared activity model, including pins, locks, duplicate markers, transfers, connected/disconnected references, project movements, project-v2 status changes, issue types, issue fields, user blocks, converted project notes, sub-issues, parent issues, blocking relationships, and converted discussions; ordinary comment and timeline GraphQL pages are fetched until `hasNextPage` is false, and unauthenticated public issue fallback additionally loads REST issue timeline events |
| Issue metadata such as pinned/locked/issue type/state reason/closed time/last edit/tracked counts/dependency summaries/milestone/projects/participants/current relationships/duplicate target/linked branches | `issue_view_preserves_extra_github_metadata`, `base_issue_query_requests_current_status_metadata`, `project_items_query_uses_selector_and_pagination_state`, `project_items_page_preserves_pagination_state`, `apply_project_metadata_replaces_existing_value_and_dedupes`, `apply_participant_metadata_replaces_existing_value_and_dedupes`, `apply_issue_relationship_metadata_replaces_existing_value_and_dedupes`, `issue_duplicate_from_response_maps_url_or_number`, `issue_linked_branches_query_requests_ref_repository_and_pagination_state`, `issue_linked_branches_page_preserves_pagination_state_and_labels`, `apply_linked_branch_metadata_replaces_existing_value_and_dedupes`, `project_scope_errors_are_suppressed_for_optional_metadata`, issue overview smoke |
| Linked issue/PR navigation targets | `render_registers_github_link_hit_area`, `render_registers_relative_issue_link_hit_area`, `render_registers_owner_repo_hash_link_hit_area`, `render_registers_markdown_relative_issue_link_hit_area`, `links_tab_registers_markdown_absolute_pr_link_hit_area`, `links_tab_detects_owner_repo_hash_references`, `rendered_visible_link_hit_area_can_be_clicked_to_navigate`, and `navigation_loads_target_and_back_restores_previous_resource` verify rendered link targets, reducer intents, gateway-backed navigation, history push, and back navigation for bare URLs, bare `#123` references, plain `owner/repo#123` references, and Markdown link targets |
| Exact detail URL open targets | `render_registers_exact_comment_url_as_open_url`, `activity_permalink_details_are_clickable_without_expansion`, `check_rows_are_click_expandable`, `expanded_commit_rows_show_deployments`, `keyboard_enter_opens_first_visible_url_action`, `mouse_click_on_url_target_requests_open_url`, `url_open_command_uses_browser_env_when_available` |
| Explicit GitHub relationship links | `related_resource_ids_parse_urls_and_number_fallbacks`, `linked_resources_query_uses_selector_connection_and_pagination_state`, `linked_resources_page_preserves_pagination_state_and_kind_mapping`, `linked_resources_page_allows_valid_empty_pages`, `links_tab_renders_explicit_related_resources_once`; PR closing-issue and issue closed-by-PR relationship links are fetched until `hasNextPage` is false |
| Auto-refresh | `auto_refresh_due_requires_live_mode_positive_interval_and_elapsed_time`, `automatic_refresh_starts_background_fetch_and_records_completed_changes`, `automatic_refresh_waits_until_interval_is_due`, and `automatic_refresh_throttles_due_attempt_while_fetch_is_in_progress` verify the event-loop interval predicate, single-flight throttling, and the gateway-backed refresh path without live network calls; `apply_refreshed_resource` tests preserve tab/scroll and record changed/no-change state; `renders_last_refresh_changed_sections` verifies changed surfaces render in the status panel; `loading_indicator_advances_through_ascii_frames` and `renders_loading_state_in_status_and_footer` verify the terminal-safe loading marker; `fingerprint_changes_when_activity_content_or_metadata_changes` verifies activity body and metadata changes are part of change detection |
| Manual refresh | reducer tests for `r` and `[refresh]`, footer render-to-click tests |
| Visible truncation/expansion | `[more]` / `[less]` render tests for body, activity, commits, checks, and files; `[expand all]` / `[collapse all]` reducer and render-to-click tests verify tab-level expansion uses visible bottom controls that do not displace the first feed item |
| Mouse click routing | synthetic Crossterm `MouseEvent` tests in `src/app/update.rs`, render-to-click tests in `src/render/resource.rs`, and `captures/ghzinga-pr-81834/mouse-smoke/` evidence generated by sending real xterm SGR mouse events into the app inside tmux; the smoke run uses fixture-backed linked-resource loading to prove a clicked issue row replaces the current TUI view and Backspace restores the original PR |
| Keyboard shortcuts avoid tmux/herdr conflicts | reducer supports arrows, PageUp/PageDown, Home/End, Tab/Shift-Tab, Ctrl-i fallback, visible-position tab jumps with `1`-`6`, Enter for first visible content action, `r`, `?`, `q`, `o`, `y`, `v` for feed order, Backspace; plain-letter shortcuts ignore Control/Alt modifiers, with tests proving Ctrl-a/Ctrl-b/Ctrl-d/Ctrl-u/Ctrl-y and other control-letter variants are inert while Ctrl-c and Ctrl-i remain the only intentional control-key paths; numbered shortcuts ignore Control/Alt and stay inactive while settings owns input |
| Settings and config persistence | `docs/2026-06-01-ghzinga-settings-config-plan.md` documents `~/.config/ghzinga/config.toml`, `XDG_CONFIG_HOME`, `GZG_CONFIG_PATH`, CLI override precedence, and the in-app settings flow; config tests verify missing-file defaults, TOML parsing, invalid-value diagnostics, and save output; reducer/render tests verify `s`, footer `[settings]`, theme/symbol/spacing selection rows, and save intents |
| Comfortable and compact spacing modes | `docs/2026-06-01-ghzinga-spacing-density-plan.md` documents the Gmail-style density rules and gh-dash-like comfortable default; `ui.spacing`, `--spacing`, settings keyboard `p`, and settings rows support `comfortable` and `compact`; renderer tests verify compact keeps dense full-width rows while comfortable inserts section breathing room, repeated-row gaps, a gh-dash-like content gutter, equal chrome/content padding, fixed nav padding plus a continuous separator below the tab buttons, top/bottom content padding, and a wide-terminal readable-column cap for read-heavy tabs while leaving Files/diffs full-width; hit rectangles stay aligned to the visible rows |
| No special UI fonts required | renderer defaults to `--symbols ascii`, with ASCII labels such as `[+ more]`, `[- less]`, `[refresh]`, `[copy]`, `[open]`, `[quit]`, `[help]`, `OK`, `!!`, `..`, and `*`; continuous separator rules use standard terminal line glyphs and do not require Nerd Font icons; the default fixture smoke remains readable in ordinary terminals, and `--symbols emoji` is opt-in with `once_can_render_emoji_symbols_when_requested` |
| Responsive TUI wrapping | `docs/2026-05-31-ghzinga-responsive-chrome-plan.md` documents the gh-dash-inspired wrapping strategy; `ViewRects::compute` tests verify narrow chrome row reservations, status-before-tabs ordering, and cramped content preservation; `narrow_render_wraps_chrome_without_losing_click_targets` verifies wrapped tabs/footer controls keep hit areas; status render tests verify loading messages use a separate detail row and do not displace the main summary; `oversized_status_pieces_wrap_without_early_ellipsis` verifies long status chips wrap before truncation; `header_wrap_keeps_identity_state_updated_and_title_visible` verifies wrapped header metadata remains visible; `extremely_narrow_tabs_fit_visible_width` and `extremely_narrow_footer_controls_fit_visible_width` verify clipped labels keep hit areas aligned with visible controls; `content_rows_wrap_long_text_and_preserve_click_targets` verifies the final content wrapping pass keeps wrapped clickable rows clickable; `narrow_content_wraps_metadata_without_clipping` verifies long metadata wraps in a narrow viewport; `wrap_display_text` tests preserve markup characters while splitting wide/emoji text by display width |
| gh-dash-style scroll orientation | `footer_renders_scroll_position_cue_when_space_allows` verifies the footer exposes the scroll cue in the rendered chrome; `scroll_summary_reports_current_limit_and_percent` verifies current row, maximum row, clamping, and percentage math |
| Transient content scrollbar | `scrolling_reveals_transient_scrollbar` verifies scroll input makes the scrollbar state visible and that it fades after the configured render countdown; `content_scrollbar_is_transient_after_scroll_input` verifies the Ratatui-rendered content pane has no thumb at rest, shows the thumb after scroll input, and hides it again after subsequent renders; `scrollbar_content_length_tracks_rows_not_only_scroll_limit` and `content_scrollbar_reaches_bottom_at_scroll_limit` verify the Ratatui scrollbar uses rendered content length plus viewport height and maps the scroll offset onto Ratatui's position range so the thumb reaches the bottom at the last scroll position; PR/issue PageDown capture validators assert saved scroll captures include a visible scrollbar thumb |
| Narrow/medium/large UX rendering | regenerated tmux captures for `80x24`, `120x36`, `160x50` in PR and issue capture directories; `captures/ghzinga-pr-81834/capture_ghzinga.py --validate-only` verifies PR frame coverage, markers, footer controls, generated files, actual tmux dimensions, PR body/comment/review/commit/check/file/link content markers, and that app/rendering source paths have not changed since the manifest revision; the same script with `--root captures/ghzinga-issue-88499 --mode issue --validate-only` verifies issue frames plus body/comment/link content markers; CI runs both validate-only checks |
| Visible URL browser open and copy | reducer tests for `o`, `y`, `[open]`, and `[copy]`; `ResourceId::web_url` tests verify PR and issue URL construction; `keyboard_o_opens_first_visible_url`, `keyboard_o_falls_back_to_current_resource_url`, `keyboard_y_copies_first_visible_open_url`, `keyboard_y_copies_first_visible_navigation_target`, and `keyboard_y_falls_back_to_current_resource_url` verify app-level browser/copy behavior for visible content links and current-resource fallback; `url_open_command_uses_browser_env_when_available` and `url_open_command_preserves_browser_arguments` verify the direct browser adapter; `clipboard_command_uses_explicit_env_command`, `clipboard_command_prefers_wayland_when_available`, and `clipboard_command_uses_xclip_for_x11` verify the direct clipboard adapter; no `gh` shell-out is used for browser opening or clipboard copy |

## Capture Evidence

PR captures:

```text
captures/ghzinga-pr-81834/
```

Issue captures:

```text
captures/ghzinga-issue-88499/
```

Each size directory contains `.txt` and `.ansi` frames. PNG capture artifacts
are rejected by `scripts/verify-no-png-captures.sh` and CI. Manifests record
the source revision, target, command, tab, keys, and actual tmux dimensions for
each frame. The current marker and content check verifies:

- PR: Activity, Commits, Checks, Files, Links, Help at narrow/medium/large sizes
- Issue: Overview, Activity, Links, Help at narrow/medium/large sizes
- footer controls `[refresh]`, `[copy]`, `[open]`, `[help]`, and `[quit]` appear in every checked size set
- PR captures include opening body text, dependency-warning comment content,
  review activity, commits, aggregate checks, changed files, and detected links
- Issue captures include the issue body, activity comments, and detected
  issue/comment links
- Mouse smoke captures include real tmux clicks on the Files and Links tabs,
  real clicks on `[expand all]` and `[collapse all]`, and a real click on a
  linked issue row that reaches the navigation path

## Click Coverage

Click behavior is verified at two levels:

1. Reducer-level tests construct Crossterm `MouseEvent` values and assert the
   returned `AppIntent` or state mutation.
2. Render-to-click tests draw with Ratatui `TestBackend`, use the actual
   registered hit rectangles, synthesize a mouse click inside the rectangle, and
   assert the resulting behavior.
3. The tmux mouse-smoke capture sends xterm SGR mouse click sequences to the
   running TUI and validates the captured terminal frame after each click.

Covered click targets:

- tabs
- body `[more]` / `[less]`
- tab-level `[expand all]` / `[collapse all]`
- activity/comment `[more]` / `[less]`
- activity/comment `[details]` permalinks
- check rows
- file rows and long patch expansion controls
- visible body/activity links
- Links-tab navigation rows
- exact check run, deployment, and comment URLs
- Enter activation for the first visible content action
- `[refresh]`
- `[copy]`
- `[open]`
- `[quit]`
- `[help]`

## Live Smoke Checks

Recent live checks through direct GitHub API data fetches:

```sh
cargo run --bin gzg -- openclaw/openclaw#81834 --tab links --once
cargo run --bin gzg -- https://github.com/openclaw/openclaw/issues/88499 --tab links --once
cargo run --bin gzg -- openclaw/openclaw#81834 --tab checks --once
cargo run --bin gzg -- openclaw/openclaw#81834 --tab activity --once
```

Browser-open and clipboard evidence:

```sh
cargo test url_open_command
cargo test clipboard_command
cargo test builds_kind_aware_web_urls
cargo test gh_cli_shell_out_is_only_for_auth_token_fallback
```

Browser open and URL copy use the first visible GitHub URL or navigation target
when one is available, fall back to the current PR/issue URL, and use direct
adapters. The clipboard adapter can be overridden with `GZG_COPY_COMMAND`. The
installed `gh` CLI remains limited to the `gh auth token` credential fallback.

## Remaining Risk

The app now covers the requested core behavior, plus additional high-value
GitHub metadata exposed by the current fetch path: milestones, optional project
membership, issue state reason, pinned state, PR draft/cross-repository flags,
mergeability, changed-file totals, ref OIDs, and merge-commit metadata.

The phrase "all the info available from GitHub" is still broader than any
practical first version. Current coverage includes the requested body/reactions/
comments/review-comments/timeline/commits/deployments/check-suites/CI/files/status/link
surfaces, expanded-file patch context, and the metadata above. It covers the
current GitHub issue and PR timeline enum sets as of the latest live schema
audit: comments, PR commits, reviews, and review-thread objects are fetched
through dedicated paginated queries instead of the generic `timelineItems` event
query, and all other known issue/PR timeline enum values are either requested in
`timelineItems` or covered by PR commit-comment-thread enrichment. PR commit
comment thread objects are fetched from the timeline query and their nested
comment pages are followed by node ID until `hasNextPage` is false.

That is the main remaining product-scope risk if the bar is interpreted as
literally every GitHub field, every future schema addition, or every private
preview field rather than the requested monitoring dashboard surfaces.

Project metadata is requested as optional paginated GraphQL enrichment because
`projectItems.project.title` requires the broader `read:project` token scope.
If the token lacks that scope, ghzinga suppresses that optional enrichment error
and keeps the PR or issue view usable.
