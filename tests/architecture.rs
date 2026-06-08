use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[test]
fn domain_layer_stays_pure() {
    let forbidden = [
        "crate::app",
        "crate::github",
        "crate::input",
        "crate::render",
        "crate::terminal",
        "crossterm",
        "ratatui",
        "reqwest",
        "tokio",
        "std::fs",
        "std::process",
    ];

    assert_no_forbidden_text("src/domain", &forbidden);
}

#[test]
fn github_adapter_does_not_depend_on_tui_layers() {
    let forbidden = [
        "crate::app",
        "crate::input",
        "crate::render",
        "crate::terminal",
        "crossterm",
        "ratatui",
    ];

    assert_no_forbidden_text("src/github", &forbidden);
}

#[test]
fn render_layer_does_not_depend_on_external_adapters() {
    let forbidden = [
        "crate::github",
        "crate::terminal",
        "reqwest",
        "tokio",
        "std::fs",
        "std::process",
    ];

    assert_no_forbidden_text("src/render", &forbidden);
}

#[test]
fn input_layer_stays_small_and_adapter_free() {
    let forbidden = [
        "crate::github",
        "crate::render",
        "crate::terminal",
        "crossterm",
        "reqwest",
        "tokio",
        "std::fs",
        "std::process",
    ];

    assert_no_forbidden_text("src/input", &forbidden);
}

#[test]
fn app_reducer_does_not_call_concrete_io_adapters() {
    let forbidden = [
        "crate::github",
        "crate::terminal",
        "reqwest",
        "tokio::process",
    ];

    assert_no_forbidden_text("src/app", &forbidden);
}

#[test]
fn fetch_runtime_boundary_stays_out_of_terminal_and_rendering_layers() {
    let source = fs::read_to_string("src/fetch.rs").expect("read fetch runtime source");

    for forbidden in [
        "crate::input",
        "crate::render",
        "crate::terminal",
        "crossterm",
        "ratatui",
        "reqwest",
        "std::process",
        "tokio::process",
    ] {
        assert!(
            !source.contains(forbidden),
            "fetch runtime boundary contains forbidden dependency text `{forbidden}`"
        );
    }

    for expected in [
        "app::AppState",
        "domain::{Resource, ResourceId}",
        "github::{",
    ] {
        assert!(
            source.contains(expected),
            "fetch runtime boundary should document intentional dependency on `{expected}`"
        );
    }
}

#[test]
fn terminal_adapter_stays_out_of_domain_and_data_layers() {
    let forbidden = [
        "crate::app",
        "crate::domain",
        "crate::github",
        "crate::input",
        "crate::render",
        "reqwest",
        "tokio",
        "std::process",
    ];

    assert_no_forbidden_text("src/terminal", &forbidden);
}

#[test]
fn github_data_layer_does_not_shell_out_to_gh_view_or_api() {
    let source = rust_files(Path::new("src/github"))
        .into_iter()
        .map(|path| fs::read_to_string(path).expect("read GitHub adapter source"))
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(source.matches("Command::new(\"gh\")").count(), 1);
    assert!(source.contains(".args([\"auth\", \"token\"])"));

    for forbidden in [
        "gh pr view",
        "gh issue view",
        "gh api",
        ".args([\"pr\", \"view\"",
        ".args([\"issue\", \"view\"",
        ".args([\"api\"",
    ] {
        assert!(
            !source.contains(forbidden),
            "GitHub data adapter contains forbidden gh transport text: {forbidden}"
        );
    }
}

#[test]
fn complete_github_fetch_keeps_file_patch_enrichment() {
    let source = fs::read_to_string("src/github/api.rs").expect("read GitHub API source");
    let fetch_start = source
        .find("async fn fetch_resource(&self")
        .expect("fetch_resource implementation");
    let base_start = source[fetch_start..]
        .find("async fn fetch_resource_base(&self")
        .expect("fetch_resource_base implementation")
        + fetch_start;
    let fetch_resource = &source[fetch_start..base_start];

    assert!(fetch_resource.contains("self.fetch_resource_base(id).await?"));
    assert!(fetch_resource.contains("self.enrich_resource(resource).await?"));
    assert!(
        fetch_resource.contains("self.enrich_file_patches(resource).await"),
        "complete live fetches such as --once must keep REST diff patch context"
    );
}

#[test]
fn public_rest_fallback_stays_in_dedicated_rest_adapter() {
    let source =
        fs::read_to_string("src/github/public_rest.rs").expect("read public REST adapter source");
    let api_source = fs::read_to_string("src/github/api.rs").expect("read GitHub API source");

    assert!(source.contains("fetch_public_rest_pr"));
    assert!(source.contains("fetch_public_rest_issue"));
    assert!(source.contains("run_rest_get_with"));
    for public_rest_detail in [
        "RestCommentDto",
        "RestPullDto",
        "fetch_public_rest_pages_with",
        "public_rest_page_path",
    ] {
        assert!(
            !api_source.contains(public_rest_detail),
            "GraphQL API orchestration should not know public REST detail `{public_rest_detail}`"
        );
    }
    assert!(
        !source.contains("run_graphql_query"),
        "public REST fallback should not grow GraphQL enrichment logic"
    );
}

#[test]
fn github_http_transport_tests_stay_with_transport_adapter() {
    let api_source = fs::read_to_string("src/github/api.rs").expect("read GitHub API source");
    let transport_source =
        fs::read_to_string("src/github/transport.rs").expect("read GitHub transport source");

    for transport_detail in [
        "graphql_transport_receives_post_shape_and_returns_body",
        "graphql_transport_errors_on_graphql_errors_payload",
        "graphql_transport_summarizes_scope_errors",
        "rest_transport_receives_get_shape_and_returns_body",
        "rest_transport_can_omit_authorization_for_public_requests",
        "rest_transport_includes_status_and_body_on_http_failure",
    ] {
        assert!(
            transport_source.contains(transport_detail),
            "transport adapter should own test `{transport_detail}`"
        );
        assert!(
            !api_source.contains(transport_detail),
            "GitHub API orchestration should not own transport test `{transport_detail}`"
        );
    }
}

#[test]
fn ci_workflow_delegates_to_full_local_gate() {
    let workflow = fs::read_to_string(".github/workflows/ci.yml").expect("read CI workflow");
    let local_gate = fs::read_to_string("scripts/ci-local.sh").expect("read local CI gate");

    assert!(workflow.contains("workflow_dispatch:"));
    assert!(workflow.contains("scripts/ci-local.sh"));

    for expected_check in [
        "cargo fmt --check",
        "cargo check",
        "cargo test",
        "cargo clippy --all-targets --all-features -- -D warnings",
        "cargo llvm-cov --fail-under-lines 85",
        "cargo audit",
        "cargo mutants --list",
        "slophammer-rs dry . --format json",
        "slophammer-rs check . --format json",
        "scripts/verify-install.sh",
        "sh -n scripts/live-smoke.sh",
        "GZG_LIVE_SELF_TEST=1 scripts/live-smoke.sh",
        "npx -y @simpledoc/simpledoc check",
        "scripts/verify-no-png-captures.sh",
        "capture_ghzinga.py --self-test",
        "capture_ghzinga.py --validate-only",
        "capture_mouse_smoke.py --self-test",
        "capture_mouse_smoke.py --validate-only",
    ] {
        assert!(
            local_gate.contains(expected_check),
            "local CI gate is missing `{expected_check}`"
        );
    }
}

#[test]
fn terminal_guard_restores_before_panic_output() {
    let source = fs::read_to_string("src/terminal/mod.rs").expect("read terminal adapter source");

    assert!(source.contains("panic::set_hook"));
    assert!(source.contains("restore_terminal_state();"));
    assert!(source.contains("default_hook(info);"));
    let hook_start = source.find("panic::set_hook").expect("panic hook");
    let hook_source = &source[hook_start..];
    assert!(
        hook_source
            .find("restore_terminal_state();")
            .expect("restore call")
            < hook_source
                .find("default_hook(info);")
                .expect("default hook call"),
        "panic hook should restore terminal state before default panic output"
    );
    assert!(source.contains("snapshot_and_clear"));
}

#[test]
fn gh_cli_shell_out_is_only_for_auth_token_fallback() {
    let matches = rust_files(Path::new("src"))
        .into_iter()
        .filter_map(|path| {
            let source = fs::read_to_string(&path).expect("read source file");
            source
                .contains("Command::new(\"gh\")")
                .then_some(path.display().to_string())
        })
        .collect::<Vec<_>>();

    assert_eq!(matches, ["src/github/auth.rs"]);
}

#[test]
fn timeline_query_accounts_for_current_github_schema_item_types() {
    let source = fs::read_to_string("src/github/queries.rs").expect("read GitHub query source");
    let issue_schema_types = BTreeSet::from([
        "ISSUE_COMMENT",
        "CROSS_REFERENCED_EVENT",
        "ADDED_TO_PROJECT_EVENT",
        "ADDED_TO_PROJECT_V2_EVENT",
        "ASSIGNED_EVENT",
        "CLOSED_EVENT",
        "COMMENT_DELETED_EVENT",
        "CONNECTED_EVENT",
        "CONVERTED_FROM_DRAFT_EVENT",
        "CONVERTED_NOTE_TO_ISSUE_EVENT",
        "CONVERTED_TO_DISCUSSION_EVENT",
        "DEMILESTONED_EVENT",
        "DISCONNECTED_EVENT",
        "LABELED_EVENT",
        "LOCKED_EVENT",
        "MARKED_AS_DUPLICATE_EVENT",
        "MENTIONED_EVENT",
        "MILESTONED_EVENT",
        "MOVED_COLUMNS_IN_PROJECT_EVENT",
        "PINNED_EVENT",
        "PROJECT_V2_ITEM_STATUS_CHANGED_EVENT",
        "REFERENCED_EVENT",
        "REMOVED_FROM_PROJECT_EVENT",
        "REMOVED_FROM_PROJECT_V2_EVENT",
        "RENAMED_TITLE_EVENT",
        "REOPENED_EVENT",
        "SUBSCRIBED_EVENT",
        "TRANSFERRED_EVENT",
        "UNASSIGNED_EVENT",
        "UNLABELED_EVENT",
        "UNLOCKED_EVENT",
        "USER_BLOCKED_EVENT",
        "UNMARKED_AS_DUPLICATE_EVENT",
        "UNPINNED_EVENT",
        "UNSUBSCRIBED_EVENT",
        "ISSUE_COMMENT_PINNED_EVENT",
        "ISSUE_COMMENT_UNPINNED_EVENT",
        "ISSUE_TYPE_ADDED_EVENT",
        "ISSUE_TYPE_REMOVED_EVENT",
        "ISSUE_TYPE_CHANGED_EVENT",
        "ISSUE_FIELD_ADDED_EVENT",
        "ISSUE_FIELD_REMOVED_EVENT",
        "ISSUE_FIELD_CHANGED_EVENT",
        "SUB_ISSUE_ADDED_EVENT",
        "SUB_ISSUE_REMOVED_EVENT",
        "PARENT_ISSUE_ADDED_EVENT",
        "PARENT_ISSUE_REMOVED_EVENT",
        "BLOCKED_BY_ADDED_EVENT",
        "BLOCKING_ADDED_EVENT",
        "BLOCKED_BY_REMOVED_EVENT",
        "BLOCKING_REMOVED_EVENT",
    ]);
    let pr_schema_types = issue_schema_types
        .iter()
        .copied()
        .chain([
            "PULL_REQUEST_COMMIT",
            "PULL_REQUEST_COMMIT_COMMENT_THREAD",
            "PULL_REQUEST_REVIEW",
            "PULL_REQUEST_REVIEW_THREAD",
            "PULL_REQUEST_REVISION_MARKER",
            "ADDED_TO_MERGE_QUEUE_EVENT",
            "AUTOMATIC_BASE_CHANGE_FAILED_EVENT",
            "AUTOMATIC_BASE_CHANGE_SUCCEEDED_EVENT",
            "AUTO_MERGE_DISABLED_EVENT",
            "AUTO_MERGE_ENABLED_EVENT",
            "AUTO_REBASE_ENABLED_EVENT",
            "AUTO_SQUASH_ENABLED_EVENT",
            "BASE_REF_CHANGED_EVENT",
            "BASE_REF_FORCE_PUSHED_EVENT",
            "BASE_REF_DELETED_EVENT",
            "CONVERT_TO_DRAFT_EVENT",
            "DEPLOYED_EVENT",
            "DEPLOYMENT_ENVIRONMENT_CHANGED_EVENT",
            "HEAD_REF_DELETED_EVENT",
            "HEAD_REF_FORCE_PUSHED_EVENT",
            "HEAD_REF_RESTORED_EVENT",
            "MERGED_EVENT",
            "READY_FOR_REVIEW_EVENT",
            "REMOVED_FROM_MERGE_QUEUE_EVENT",
            "REVIEW_DISMISSED_EVENT",
            "REVIEW_REQUESTED_EVENT",
            "REVIEW_REQUEST_REMOVED_EVENT",
        ])
        .collect::<BTreeSet<_>>();

    let issue_dedicated_queries = BTreeSet::from(["ISSUE_COMMENT"]);
    let pr_dedicated_queries = BTreeSet::from([
        "ISSUE_COMMENT",
        "PULL_REQUEST_COMMIT",
        "PULL_REQUEST_REVIEW",
        "PULL_REQUEST_REVIEW_THREAD",
    ]);
    let mut pr_query_item_types = issue_query_item_types(&source);
    pr_query_item_types.extend(pr_only_query_item_types(&source));

    assert_eq!(
        issue_query_item_types(&source),
        issue_schema_types
            .difference(&issue_dedicated_queries)
            .copied()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(
        pr_query_item_types,
        pr_schema_types
            .difference(&pr_dedicated_queries)
            .copied()
            .collect::<BTreeSet<_>>()
    );
}

fn issue_query_item_types(source: &str) -> BTreeSet<&str> {
    source_item_types(source, "timelineItems(first: 100", "]) {{")
}

fn pr_only_query_item_types(source: &str) -> BTreeSet<&str> {
    source_item_types(
        source,
        "let pr_timeline_items = match kind",
        "let pr_timeline_fragments = match kind",
    )
}

fn source_item_types<'a>(source: &'a str, start: &str, end: &str) -> BTreeSet<&'a str> {
    let start = source.find(start).expect("timeline item type start");
    let end = source[start..]
        .find(end)
        .map(|offset| start + offset)
        .expect("timeline item type end");
    source[start..end]
        .split(|ch: char| !(ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_'))
        .filter(|token| token.contains('_') && token.chars().any(|ch| ch.is_ascii_uppercase()))
        .collect()
}

fn assert_no_forbidden_text(root: &str, forbidden: &[&str]) {
    for path in rust_files(Path::new(root)) {
        let source = fs::read_to_string(&path).expect("read source file");
        for text in forbidden {
            assert!(
                !source.contains(text),
                "{} contains forbidden dependency text `{}`",
                path.display(),
                text
            );
        }
    }
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files.sort();
    files
}

fn collect_rust_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path.to_path_buf());
        }
        return;
    }

    for entry in fs::read_dir(path).expect("read source directory") {
        let entry = entry.expect("read source directory entry");
        collect_rust_files(&entry.path(), files);
    }
}
