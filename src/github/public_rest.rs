use anyhow::Context;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    domain::{
        ActivityEntry, ActivityKind, ChangedFile, CheckRun, CheckStatus, Commit, MetadataItem,
        PullRequest, ReactionCounts, Resource, ResourceId, ResourceKind,
    },
    github::transport::{
        run_rest_get_with, GithubHttpTransport, ReqwestGithubHttpTransport, GITHUB_JSON_ACCEPT,
    },
};

const REST_PAGE_SIZE: usize = 100;

pub(super) async fn fetch_public_rest_pr(
    id: &ResourceId,
    auth_error: anyhow::Error,
) -> anyhow::Result<Resource> {
    let pull: RestPullDto = run_public_rest_json(&rest_pull_path(id))
        .await
        .context("public REST fallback could not load pull request")?;
    let issue: RestIssueDto = run_public_rest_json(&rest_issue_path(id))
        .await
        .context("public REST fallback could not load pull request issue metadata")?;
    let mut warnings = vec![format!(
        "using public REST fallback because no GitHub token is available: {auth_error}"
    )];
    let comments = match fetch_public_rest_comments(id).await {
        Ok(comments) => comments,
        Err(error) => {
            warnings.push(format!("public comments unavailable: {error}"));
            Vec::new()
        }
    };
    let reviews = match fetch_public_rest_reviews(id).await {
        Ok(reviews) => reviews,
        Err(error) => {
            warnings.push(format!("public reviews unavailable: {error}"));
            Vec::new()
        }
    };
    let review_comments = match fetch_public_rest_review_comments(id).await {
        Ok(review_comments) => review_comments,
        Err(error) => {
            warnings.push(format!("public review comments unavailable: {error}"));
            Vec::new()
        }
    };
    let timeline = match fetch_public_rest_timeline_events(id).await {
        Ok(timeline) => timeline,
        Err(error) => {
            warnings.push(format!("public timeline events unavailable: {error}"));
            Vec::new()
        }
    };
    let commits = match fetch_public_rest_commits(id).await {
        Ok(commits) => commits,
        Err(error) => {
            warnings.push(format!("public commit list unavailable: {error}"));
            Vec::new()
        }
    };
    let files = match fetch_public_rest_files(id).await {
        Ok(files) => files,
        Err(error) => {
            warnings.push(format!("public file list unavailable: {error}"));
            Vec::new()
        }
    };
    let head_sha = pull
        .head
        .as_ref()
        .and_then(|reference| reference.sha.clone())
        .or_else(|| commits.first().map(|commit| commit.oid.clone()));
    let checks = match head_sha.as_deref() {
        Some(sha) => fetch_public_rest_checks(id, sha, &mut warnings).await,
        None => {
            warnings.push("public check status unavailable: pull request head SHA missing".into());
            Vec::new()
        }
    };
    let mut activity = comments;
    activity.extend(reviews);
    activity.extend(review_comments);
    activity.extend(timeline);
    sort_activity(&mut activity);
    let pull_request = rest_pull_request(&pull, commits, checks, files);

    let mut resource = rest_issue_resource(issue, id, activity);
    resource.id.kind_hint = Some(ResourceKind::PullRequest);
    resource.url = pull.html_url;
    resource.title = pull.title;
    resource.state = pull.state.to_ascii_uppercase();
    resource.author = display_rest_author(pull.user);
    resource.created_at = pull.created_at;
    resource.updated_at = pull.updated_at;
    resource.pull_request = Some(pull_request);
    resource.warnings.extend(warnings);
    resource.warnings.push(
        "public REST fallback omits GraphQL-only enrichment such as review-thread resolution state, projects, participants, relationship links, and check-suite workflow grouping".into(),
    );
    Ok(resource)
}

pub(super) async fn fetch_public_rest_issue(
    id: &ResourceId,
    auth_error: anyhow::Error,
) -> anyhow::Result<Resource> {
    let issue: RestIssueDto = run_public_rest_json(&rest_issue_path(id))
        .await
        .context("public REST fallback could not load issue")?;
    let mut warnings = vec![format!(
        "using public REST fallback because no GitHub token is available: {auth_error}"
    )];
    let comments = match fetch_public_rest_comments(id).await {
        Ok(comments) => comments,
        Err(error) => {
            warnings.push(format!("public comments unavailable: {error}"));
            Vec::new()
        }
    };
    let timeline = match fetch_public_rest_timeline_events(id).await {
        Ok(timeline) => timeline,
        Err(error) => {
            warnings.push(format!("public timeline events unavailable: {error}"));
            Vec::new()
        }
    };
    let mut activity = comments;
    activity.extend(timeline);
    sort_activity(&mut activity);
    let mut resource = rest_issue_resource(issue, id, activity);
    resource.warnings.extend(warnings);
    resource.warnings.push(
        "public REST fallback omits GraphQL-only enrichment such as projects, participants, issue relationships, duplicate issue targets, linked branches, relationship links, and review data".into(),
    );
    Ok(resource)
}

async fn run_public_rest_json<T: serde::de::DeserializeOwned>(path: &str) -> anyhow::Result<T> {
    run_public_rest_json_with(&ReqwestGithubHttpTransport, path).await
}

async fn run_public_rest_json_with<T: serde::de::DeserializeOwned>(
    transport: &impl GithubHttpTransport,
    path: &str,
) -> anyhow::Result<T> {
    let output = run_rest_get_with(transport, None, path, GITHUB_JSON_ACCEPT).await?;
    serde_json::from_slice(&output)
        .with_context(|| format!("failed to parse public REST JSON from {path}"))
}

async fn fetch_public_rest_pages<T: serde::de::DeserializeOwned>(
    base_path: &str,
) -> anyhow::Result<Vec<T>> {
    fetch_public_rest_pages_with(&ReqwestGithubHttpTransport, base_path).await
}

async fn fetch_public_rest_pages_with<T: serde::de::DeserializeOwned>(
    transport: &impl GithubHttpTransport,
    base_path: &str,
) -> anyhow::Result<Vec<T>> {
    let mut page = 1;
    let mut items = Vec::new();
    loop {
        let path = public_rest_page_path(base_path, page);
        let mut current_page: Vec<T> = run_public_rest_json_with(transport, &path).await?;
        let is_last_page = current_page.len() < REST_PAGE_SIZE;
        items.append(&mut current_page);
        if is_last_page {
            return Ok(items);
        }
        page += 1;
    }
}

fn public_rest_page_path(base_path: &str, page: u64) -> String {
    let separator = if base_path.contains('?') { '&' } else { '?' };
    format!("{base_path}{separator}per_page={REST_PAGE_SIZE}&page={page}")
}

async fn fetch_public_rest_comments(id: &ResourceId) -> anyhow::Result<Vec<ActivityEntry>> {
    let comments = fetch_public_rest_pages::<RestCommentDto>(&format!(
        "/repos/{}/{}/issues/{}/comments",
        id.owner, id.repo, id.number
    ))
    .await?;
    Ok(comments
        .into_iter()
        .enumerate()
        .map(rest_comment_activity)
        .collect())
}

async fn fetch_public_rest_timeline_events(id: &ResourceId) -> anyhow::Result<Vec<ActivityEntry>> {
    fetch_public_rest_timeline_events_with(&ReqwestGithubHttpTransport, id).await
}

async fn fetch_public_rest_timeline_events_with(
    transport: &impl GithubHttpTransport,
    id: &ResourceId,
) -> anyhow::Result<Vec<ActivityEntry>> {
    let events = fetch_public_rest_pages_with::<RestTimelineEventDto>(
        transport,
        &format!(
            "/repos/{}/{}/issues/{}/timeline",
            id.owner, id.repo, id.number
        ),
    )
    .await?;
    Ok(events
        .into_iter()
        .enumerate()
        .filter_map(rest_timeline_event_activity)
        .collect())
}

async fn fetch_public_rest_commits(id: &ResourceId) -> anyhow::Result<Vec<Commit>> {
    let commits = fetch_public_rest_pages::<RestCommitDto>(&format!(
        "/repos/{}/{}/pulls/{}/commits",
        id.owner, id.repo, id.number
    ))
    .await?;
    Ok(commits.into_iter().map(rest_commit).collect())
}

async fn fetch_public_rest_files(id: &ResourceId) -> anyhow::Result<Vec<ChangedFile>> {
    let files = fetch_public_rest_pages::<RestFileDto>(&format!(
        "/repos/{}/{}/pulls/{}/files",
        id.owner, id.repo, id.number
    ))
    .await?;
    Ok(files.into_iter().map(rest_file).collect())
}

async fn fetch_public_rest_reviews(id: &ResourceId) -> anyhow::Result<Vec<ActivityEntry>> {
    fetch_public_rest_reviews_with(&ReqwestGithubHttpTransport, id).await
}

async fn fetch_public_rest_reviews_with(
    transport: &impl GithubHttpTransport,
    id: &ResourceId,
) -> anyhow::Result<Vec<ActivityEntry>> {
    let reviews = fetch_public_rest_pages_with::<RestReviewDto>(
        transport,
        &format!(
            "/repos/{}/{}/pulls/{}/reviews",
            id.owner, id.repo, id.number
        ),
    )
    .await?;
    Ok(reviews
        .into_iter()
        .enumerate()
        .map(rest_review_activity)
        .collect())
}

async fn fetch_public_rest_review_comments(id: &ResourceId) -> anyhow::Result<Vec<ActivityEntry>> {
    fetch_public_rest_review_comments_with(&ReqwestGithubHttpTransport, id).await
}

async fn fetch_public_rest_review_comments_with(
    transport: &impl GithubHttpTransport,
    id: &ResourceId,
) -> anyhow::Result<Vec<ActivityEntry>> {
    let comments = fetch_public_rest_pages_with::<RestReviewCommentDto>(
        transport,
        &format!(
            "/repos/{}/{}/pulls/{}/comments",
            id.owner, id.repo, id.number
        ),
    )
    .await?;
    Ok(comments
        .into_iter()
        .enumerate()
        .map(rest_review_comment_activity)
        .collect())
}

async fn fetch_public_rest_checks(
    id: &ResourceId,
    sha: &str,
    warnings: &mut Vec<String>,
) -> Vec<CheckRun> {
    let mut checks = Vec::new();
    match fetch_public_rest_check_runs(id, sha).await {
        Ok(check_runs) => checks.extend(check_runs),
        Err(error) => warnings.push(format!("public check runs unavailable: {error}")),
    }
    match fetch_public_rest_status_contexts(id, sha).await {
        Ok(statuses) => checks.extend(statuses),
        Err(error) => warnings.push(format!("public status contexts unavailable: {error}")),
    }
    deduped_public_checks(checks)
}

async fn fetch_public_rest_check_runs(id: &ResourceId, sha: &str) -> anyhow::Result<Vec<CheckRun>> {
    fetch_public_rest_check_runs_with(&ReqwestGithubHttpTransport, id, sha).await
}

async fn fetch_public_rest_check_runs_with(
    transport: &impl GithubHttpTransport,
    id: &ResourceId,
    sha: &str,
) -> anyhow::Result<Vec<CheckRun>> {
    let mut page = 1;
    let mut checks = Vec::new();
    loop {
        let path = format!(
            "/repos/{}/{}/commits/{}/check-runs?per_page={REST_PAGE_SIZE}&page={page}",
            id.owner, id.repo, sha
        );
        let page_dto: RestCheckRunsPageDto = run_public_rest_json_with(transport, &path).await?;
        let is_last_page = page_dto.check_runs.len() < REST_PAGE_SIZE;
        checks.extend(page_dto.check_runs.into_iter().map(rest_check_run));
        if is_last_page {
            return Ok(checks);
        }
        page += 1;
    }
}

async fn fetch_public_rest_status_contexts(
    id: &ResourceId,
    sha: &str,
) -> anyhow::Result<Vec<CheckRun>> {
    fetch_public_rest_status_contexts_with(&ReqwestGithubHttpTransport, id, sha).await
}

async fn fetch_public_rest_status_contexts_with(
    transport: &impl GithubHttpTransport,
    id: &ResourceId,
    sha: &str,
) -> anyhow::Result<Vec<CheckRun>> {
    let status: RestCombinedStatusDto = run_public_rest_json_with(
        transport,
        &format!("/repos/{}/{}/commits/{}/status", id.owner, id.repo, sha),
    )
    .await?;
    Ok(status
        .statuses
        .into_iter()
        .map(rest_status_context)
        .collect())
}

fn rest_pull_path(id: &ResourceId) -> String {
    format!("/repos/{}/{}/pulls/{}", id.owner, id.repo, id.number)
}

fn rest_issue_path(id: &ResourceId) -> String {
    format!("/repos/{}/{}/issues/{}", id.owner, id.repo, id.number)
}

#[derive(Debug, Clone, Deserialize)]
struct RestUserDto {
    login: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RestLabelDto {
    name: String,
}

#[derive(Debug, Deserialize, Default)]
struct RestReactionsDto {
    #[serde(rename = "+1", default)]
    thumbs_up: u64,
    #[serde(rename = "-1", default)]
    thumbs_down: u64,
    #[serde(default)]
    laugh: u64,
    #[serde(default)]
    hooray: u64,
    #[serde(default)]
    confused: u64,
    #[serde(default)]
    heart: u64,
    #[serde(default)]
    rocket: u64,
    #[serde(default)]
    eyes: u64,
}

#[derive(Debug, Deserialize)]
struct RestIssueDto {
    number: u64,
    title: String,
    html_url: String,
    state: String,
    user: Option<RestUserDto>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    labels: Vec<RestLabelDto>,
    #[serde(default)]
    assignees: Vec<RestUserDto>,
    #[serde(default)]
    reactions: RestReactionsDto,
    body: Option<String>,
    closed_at: Option<String>,
    state_reason: Option<String>,
    #[serde(default)]
    locked: bool,
    active_lock_reason: Option<String>,
    milestone: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct RestPullDto {
    title: String,
    html_url: String,
    state: String,
    user: Option<RestUserDto>,
    created_at: String,
    updated_at: String,
    base: Option<RestRefDto>,
    head: Option<RestRefDto>,
    #[serde(default)]
    requested_reviewers: Vec<RestUserDto>,
    mergeable: Option<bool>,
    additions: Option<u64>,
    deletions: Option<u64>,
    changed_files: Option<u64>,
    #[serde(default)]
    draft: bool,
    merged_at: Option<String>,
    merge_commit_sha: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestRefDto {
    #[serde(rename = "ref")]
    reference: String,
    sha: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RestCommentDto {
    id: u64,
    user: Option<RestUserDto>,
    body: Option<String>,
    created_at: String,
    updated_at: String,
    html_url: Option<String>,
    author_association: Option<String>,
    #[serde(default)]
    reactions: RestReactionsDto,
}

#[derive(Debug, Deserialize)]
struct RestReviewDto {
    id: u64,
    user: Option<RestUserDto>,
    body: Option<String>,
    state: Option<String>,
    submitted_at: Option<String>,
    html_url: Option<String>,
    author_association: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RestReviewCommentDto {
    id: u64,
    user: Option<RestUserDto>,
    body: Option<String>,
    created_at: String,
    updated_at: String,
    html_url: Option<String>,
    author_association: Option<String>,
    #[serde(default)]
    reactions: RestReactionsDto,
    path: Option<String>,
    line: Option<u64>,
    position: Option<u64>,
    original_line: Option<u64>,
    original_position: Option<u64>,
    pull_request_review_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RestTimelineEventDto {
    id: Option<Value>,
    event: Option<String>,
    actor: Option<RestUserDto>,
    created_at: Option<String>,
    url: Option<String>,
    html_url: Option<String>,
    commit_id: Option<String>,
    label: Option<RestLabelDto>,
    assignee: Option<RestUserDto>,
    assigner: Option<RestUserDto>,
    requested_reviewer: Option<RestUserDto>,
    review_requester: Option<RestUserDto>,
    dismissed_review: Option<RestDismissedReviewDto>,
    rename: Option<RestRenameDto>,
    lock_reason: Option<String>,
    milestone: Option<Value>,
    source: Option<Value>,
    subject: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct RestDismissedReviewDto {
    state: Option<String>,
    dismissal_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RestRenameDto {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RestCommitDto {
    sha: String,
    commit: RestCommitInnerDto,
    author: Option<RestUserDto>,
}

#[derive(Debug, Deserialize)]
struct RestCommitInnerDto {
    message: String,
    author: Option<RestCommitPersonDto>,
    committer: Option<RestCommitPersonDto>,
}

#[derive(Debug, Deserialize)]
struct RestCommitPersonDto {
    name: Option<String>,
    date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RestFileDto {
    filename: String,
    additions: u64,
    deletions: u64,
    status: String,
    patch: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RestCheckRunsPageDto {
    check_runs: Vec<RestCheckRunDto>,
}

#[derive(Debug, Deserialize)]
struct RestCheckRunDto {
    name: String,
    status: Option<String>,
    conclusion: Option<String>,
    html_url: Option<String>,
    details_url: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
    output: Option<RestCheckRunOutputDto>,
}

#[derive(Debug, Deserialize)]
struct RestCheckRunOutputDto {
    title: Option<String>,
    summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RestCombinedStatusDto {
    statuses: Vec<RestStatusDto>,
}

#[derive(Debug, Deserialize)]
struct RestStatusDto {
    context: String,
    state: Option<String>,
    target_url: Option<String>,
    description: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

fn rest_reaction_counts(reactions: RestReactionsDto) -> ReactionCounts {
    ReactionCounts {
        thumbs_up: reactions.thumbs_up,
        thumbs_down: reactions.thumbs_down,
        laugh: reactions.laugh,
        hooray: reactions.hooray,
        confused: reactions.confused,
        heart: reactions.heart,
        rocket: reactions.rocket,
        eyes: reactions.eyes,
    }
}

fn rest_check_run(check: RestCheckRunDto) -> CheckRun {
    let raw_status = check.status.filter(|value| !value.is_empty());
    let raw_conclusion = check.conclusion.filter(|value| !value.is_empty());
    let summary = check.output.and_then(|output| {
        output
            .summary
            .filter(|value| !value.trim().is_empty())
            .or_else(|| output.title.filter(|value| !value.trim().is_empty()))
    });
    CheckRun {
        name: check.name,
        status: CheckStatus::from_github(raw_status.as_deref(), raw_conclusion.as_deref()),
        summary,
        details_url: check.details_url.or(check.html_url),
        started_at: check.started_at,
        completed_at: check.completed_at,
        raw_status,
        raw_conclusion,
    }
}

fn rest_status_context(status: RestStatusDto) -> CheckRun {
    let raw_status = status.state.filter(|value| !value.is_empty());
    CheckRun {
        name: status.context,
        status: CheckStatus::from_github(raw_status.as_deref(), None),
        summary: status.description.filter(|value| !value.trim().is_empty()),
        details_url: status.target_url,
        started_at: status.created_at,
        completed_at: status.updated_at,
        raw_status,
        raw_conclusion: None,
    }
}

fn deduped_public_checks(checks: Vec<CheckRun>) -> Vec<CheckRun> {
    let mut by_name = std::collections::HashMap::new();
    for check in checks {
        by_name.insert(check.name.clone(), check);
    }
    let mut checks = by_name.into_values().collect::<Vec<_>>();
    checks.sort_by(|left, right| left.name.cmp(&right.name));
    checks
}

fn rest_issue_resource(
    issue: RestIssueDto,
    requested: &ResourceId,
    activity: Vec<ActivityEntry>,
) -> Resource {
    let metadata = rest_issue_metadata(&issue);
    Resource {
        id: ResourceId {
            owner: requested.owner.clone(),
            repo: requested.repo.clone(),
            number: issue.number,
            kind_hint: Some(ResourceKind::Issue),
        },
        title: issue.title,
        url: issue.html_url,
        state: issue.state.to_ascii_uppercase(),
        author: display_rest_author(issue.user),
        created_at: issue.created_at,
        updated_at: issue.updated_at,
        labels: issue.labels.into_iter().map(|label| label.name).collect(),
        assignees: issue
            .assignees
            .into_iter()
            .map(|user| display_rest_author(Some(user)))
            .filter(|name| name != "unknown")
            .collect(),
        reactions: rest_reaction_counts(issue.reactions),
        body: issue.body.unwrap_or_default(),
        activity,
        related_resources: Vec::new(),
        metadata,
        warnings: Vec::new(),
        pull_request: None,
    }
}

fn rest_pull_request(
    pull: &RestPullDto,
    commits: Vec<Commit>,
    checks: Vec<CheckRun>,
    files: Vec<ChangedFile>,
) -> PullRequest {
    let base_ref = pull
        .base
        .as_ref()
        .map(|reference| reference.reference.clone())
        .unwrap_or_default();
    let head_ref = pull
        .head
        .as_ref()
        .map(|reference| reference.reference.clone())
        .unwrap_or_default();
    PullRequest {
        base_ref,
        head_ref,
        requested_reviewers: pull
            .requested_reviewers
            .iter()
            .cloned()
            .map(|user| display_rest_author(Some(user)))
            .filter(|name| name != "unknown")
            .collect(),
        review_decision: None,
        merge_state: pull.mergeable.map(|mergeable| {
            if mergeable {
                "MERGEABLE".to_string()
            } else {
                "CONFLICTING".to_string()
            }
        }),
        additions: pull.additions.unwrap_or_default(),
        deletions: pull.deletions.unwrap_or_default(),
        commits,
        checks,
        files,
        metadata: rest_pr_metadata(pull),
    }
}

fn display_rest_author(author: Option<RestUserDto>) -> String {
    author
        .and_then(|author| author.login)
        .filter(|login| !login.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn rest_issue_metadata(issue: &RestIssueDto) -> Vec<MetadataItem> {
    let mut items = Vec::new();
    push_nonempty_metadata(&mut items, "State reason", issue.state_reason.as_deref());
    push_nonempty_metadata(&mut items, "Closed at", issue.closed_at.as_deref());
    push_bool_metadata(&mut items, "Locked", issue.locked);
    push_nonempty_metadata(
        &mut items,
        "Lock reason",
        issue.active_lock_reason.as_deref(),
    );
    push_nonempty_metadata(
        &mut items,
        "Milestone",
        value_title(issue.milestone.as_ref()).as_deref(),
    );
    items
}

fn rest_pr_metadata(pr: &RestPullDto) -> Vec<MetadataItem> {
    let mut items = Vec::new();
    push_bool_metadata(&mut items, "Draft", pr.draft);
    push_nonempty_metadata(
        &mut items,
        "Changed files",
        pr.changed_files.map(|count| count.to_string()).as_deref(),
    );
    push_nonempty_metadata(&mut items, "Merged at", pr.merged_at.as_deref());
    push_nonempty_metadata(&mut items, "Merge commit", pr.merge_commit_sha.as_deref());
    items
}

fn rest_comment_activity((index, comment): (usize, RestCommentDto)) -> ActivityEntry {
    let includes_created_edit = comment.updated_at != comment.created_at;
    ActivityEntry {
        id: format!("rest-comment-{}", comment.id),
        kind: ActivityKind::Comment,
        author: display_rest_author(comment.user),
        body: comment.body.unwrap_or_default(),
        updated_at: comment.updated_at,
        path: None,
        line: None,
        url: comment.html_url,
        author_association: comment.author_association,
        reactions: rest_reaction_counts(comment.reactions),
        includes_created_edit,
        is_minimized: false,
        minimized_reason: None,
        thread_id: Some(format!("public-rest-comment-{index}")),
        thread_resolved: None,
        thread_outdated: None,
    }
}

fn rest_review_activity((index, review): (usize, RestReviewDto)) -> ActivityEntry {
    let state = review
        .state
        .as_deref()
        .map(|state| state.to_ascii_uppercase())
        .unwrap_or_else(|| "REVIEW".to_string());
    let submitted_at = review.submitted_at.unwrap_or_else(|| "unknown".to_string());
    let body = review.body.unwrap_or_default();
    let body = if body.trim().is_empty() {
        state.clone()
    } else {
        format!("{state}: {body}")
    };
    ActivityEntry {
        id: format!("rest-review-{}", review.id),
        kind: ActivityKind::Review,
        author: display_rest_author(review.user),
        body,
        updated_at: submitted_at,
        path: None,
        line: None,
        url: review.html_url,
        author_association: review.author_association,
        reactions: ReactionCounts::default(),
        includes_created_edit: false,
        is_minimized: false,
        minimized_reason: None,
        thread_id: Some(format!("public-rest-review-{index}")),
        thread_resolved: None,
        thread_outdated: None,
    }
}

fn rest_review_comment_activity((index, comment): (usize, RestReviewCommentDto)) -> ActivityEntry {
    let includes_created_edit = comment.updated_at != comment.created_at;
    ActivityEntry {
        id: format!("rest-review-comment-{}", comment.id),
        kind: ActivityKind::ReviewComment,
        author: display_rest_author(comment.user),
        body: comment.body.unwrap_or_default(),
        updated_at: comment.updated_at,
        path: comment.path,
        line: comment
            .line
            .or(comment.position)
            .or(comment.original_line)
            .or(comment.original_position),
        url: comment.html_url,
        author_association: comment.author_association,
        reactions: rest_reaction_counts(comment.reactions),
        includes_created_edit,
        is_minimized: false,
        minimized_reason: None,
        thread_id: Some(
            comment
                .pull_request_review_id
                .map(|id| format!("public-rest-review-{id}"))
                .unwrap_or_else(|| format!("public-rest-review-comment-{index}")),
        ),
        thread_resolved: None,
        thread_outdated: None,
    }
}

fn rest_timeline_event_activity(
    (index, event): (usize, RestTimelineEventDto),
) -> Option<ActivityEntry> {
    let event_name = event.event.as_deref()?.trim();
    if should_skip_rest_timeline_event(event_name) {
        return None;
    }
    let body = rest_timeline_event_body(event_name, &event);
    Some(ActivityEntry {
        id: format!(
            "rest-timeline-{}",
            event
                .id
                .as_ref()
                .map(value_id)
                .filter(|id| !id.is_empty())
                .unwrap_or_else(|| index.to_string())
        ),
        kind: ActivityKind::Timeline,
        author: display_rest_author(event.actor),
        body,
        updated_at: event
            .created_at
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string()),
        path: None,
        line: None,
        url: event.html_url.or(event.url),
        author_association: None,
        reactions: ReactionCounts::default(),
        includes_created_edit: false,
        is_minimized: false,
        minimized_reason: None,
        thread_id: Some(format!("public-rest-timeline-{index}")),
        thread_resolved: None,
        thread_outdated: None,
    })
}

fn should_skip_rest_timeline_event(event_name: &str) -> bool {
    matches!(
        event_name,
        // These are already represented by richer public REST endpoints.
        "commented" | "committed" | "reviewed" | "review_comment"
    )
}

fn rest_timeline_event_body(event_name: &str, event: &RestTimelineEventDto) -> String {
    match event_name {
        "assigned" => format!(
            "assigned {}",
            display_rest_author(event.assignee.clone().or_else(|| event.assigner.clone()))
        ),
        "unassigned" => format!(
            "unassigned {}",
            display_rest_author(event.assignee.clone().or_else(|| event.assigner.clone()))
        ),
        "labeled" => format!(
            "added label {}",
            event
                .label
                .as_ref()
                .map(|label| label.name.as_str())
                .unwrap_or("unknown")
        ),
        "unlabeled" => format!(
            "removed label {}",
            event
                .label
                .as_ref()
                .map(|label| label.name.as_str())
                .unwrap_or("unknown")
        ),
        "milestoned" => format!(
            "added milestone {}",
            value_title(event.milestone.as_ref()).unwrap_or_else(|| "unknown".to_string())
        ),
        "demilestoned" => format!(
            "removed milestone {}",
            value_title(event.milestone.as_ref()).unwrap_or_else(|| "unknown".to_string())
        ),
        "renamed" => match &event.rename {
            Some(rename) => format!(
                "renamed title from {} to {}",
                rename.from.as_deref().unwrap_or("unknown"),
                rename.to.as_deref().unwrap_or("unknown")
            ),
            None => "renamed title".to_string(),
        },
        "closed" => rest_timeline_commit_body("closed", event),
        "reopened" => "reopened this".to_string(),
        "merged" => rest_timeline_commit_body("merged", event),
        "locked" => event
            .lock_reason
            .as_ref()
            .filter(|reason| !reason.trim().is_empty())
            .map(|reason| format!("locked this as {reason}"))
            .unwrap_or_else(|| "locked this".to_string()),
        "unlocked" => "unlocked this".to_string(),
        "referenced" => rest_timeline_reference_body("referenced", event),
        "cross-referenced" => rest_timeline_reference_body("cross-referenced", event),
        "connected" => rest_timeline_reference_body("connected", event),
        "disconnected" => rest_timeline_reference_body("disconnected", event),
        "review_requested" => format!(
            "requested review from {}",
            display_rest_author(
                event
                    .requested_reviewer
                    .clone()
                    .or_else(|| event.review_requester.clone())
            )
        ),
        "review_request_removed" => format!(
            "removed review request for {}",
            display_rest_author(
                event
                    .requested_reviewer
                    .clone()
                    .or_else(|| event.review_requester.clone())
            )
        ),
        "review_dismissed" => {
            let review = event.dismissed_review.as_ref();
            let state = review
                .and_then(|review| review.state.as_deref())
                .unwrap_or("review");
            match review.and_then(|review| review.dismissal_message.as_deref()) {
                Some(message) if !message.trim().is_empty() => {
                    format!("dismissed {state} review: {message}")
                }
                _ => format!("dismissed {state} review"),
            }
        }
        "ready_for_review" => "marked ready for review".to_string(),
        "converted_to_draft" => "converted to draft".to_string(),
        "head_ref_deleted" => "deleted head branch".to_string(),
        "head_ref_restored" => "restored head branch".to_string(),
        "subscribed" => "subscribed".to_string(),
        "unsubscribed" => "unsubscribed".to_string(),
        "mentioned" => "mentioned this".to_string(),
        other => humanize_rest_timeline_event(other),
    }
}

fn rest_timeline_commit_body(action: &str, event: &RestTimelineEventDto) -> String {
    event
        .commit_id
        .as_ref()
        .filter(|commit| !commit.trim().is_empty())
        .map(|commit| format!("{action} this with {commit}"))
        .unwrap_or_else(|| format!("{action} this"))
}

fn rest_timeline_reference_body(action: &str, event: &RestTimelineEventDto) -> String {
    let reference = event
        .source
        .as_ref()
        .or(event.subject.as_ref())
        .and_then(value_reference);
    match reference {
        Some(reference) => format!("{action} {reference}"),
        None => format!("{action} this"),
    }
}

fn value_reference(value: &Value) -> Option<String> {
    value_url(value)
        .or_else(|| value_title(Some(value)))
        .or_else(|| {
            value
                .get("issue")
                .and_then(value_reference)
                .or_else(|| value.get("pull_request").and_then(value_reference))
                .or_else(|| value.get("pullRequest").and_then(value_reference))
        })
}

fn value_url(value: &Value) -> Option<String> {
    ["html_url", "url"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .map(str::to_string)
}

fn value_id(value: &Value) -> String {
    value
        .as_u64()
        .map(|id| id.to_string())
        .or_else(|| value.as_str().map(str::to_string))
        .unwrap_or_else(|| value.to_string())
}

fn humanize_rest_timeline_event(event_name: &str) -> String {
    event_name.replace('_', " ")
}

fn sort_activity(activity: &mut [ActivityEntry]) {
    activity.sort_by(|left, right| {
        left.updated_at
            .cmp(&right.updated_at)
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn rest_commit(commit: RestCommitDto) -> Commit {
    let headline = commit
        .commit
        .message
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    let body = commit
        .commit
        .message
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    let author = {
        let login = display_rest_author(commit.author);
        if login == "unknown" {
            commit
                .commit
                .author
                .as_ref()
                .and_then(|author| author.name.clone())
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            login
        }
    };
    let authored_at = commit
        .commit
        .author
        .as_ref()
        .and_then(|author| author.date.clone());
    let committed_at = commit
        .commit
        .committer
        .and_then(|committer| committer.date)
        .or_else(|| authored_at.clone())
        .unwrap_or_else(|| "unknown".to_string());
    Commit {
        oid: commit.sha,
        message: headline,
        body,
        author: author.clone(),
        authors: if author == "unknown" {
            Vec::new()
        } else {
            vec![author]
        },
        authored_at,
        committed_at,
        status: CheckStatus::Unknown,
        deployments: Vec::new(),
    }
}

fn rest_file(file: RestFileDto) -> ChangedFile {
    ChangedFile {
        path: file.filename,
        additions: file.additions,
        deletions: file.deletions,
        change_type: file.status.to_ascii_uppercase(),
        patch: file.patch,
    }
}

fn push_bool_metadata(items: &mut Vec<MetadataItem>, label: &str, value: bool) {
    items.push(MetadataItem {
        label: label.to_string(),
        value: if value { "yes" } else { "no" }.to_string(),
    });
}

fn push_nonempty_metadata(items: &mut Vec<MetadataItem>, label: &str, value: Option<&str>) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    items.push(MetadataItem {
        label: label.to_string(),
        value: value.to_string(),
    });
}

fn value_title(value: Option<&Value>) -> Option<String> {
    let value = value?;
    ["title", "name", "login", "oid"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .map(str::to_string)
        .or_else(|| {
            value
                .get("project")
                .and_then(|project| value_title(Some(project)))
        })
        .or_else(|| {
            value
                .get("repository")
                .and_then(|repository| value_title(Some(repository)))
        })
        .or_else(|| {
            value
                .get("owner")
                .and_then(|owner| value_title(Some(owner)))
        })
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use anyhow::Context;
    use serde_json::{json, Value};

    use super::*;
    use crate::github::transport::{
        GithubHttpFuture, GithubHttpMethod, GithubHttpRequest, GithubHttpResponse,
    };

    #[derive(Debug)]
    struct FakeGithubHttpTransport {
        requests: Mutex<Vec<GithubHttpRequest>>,
        responses: Mutex<VecDeque<GithubHttpResponse>>,
    }

    impl FakeGithubHttpTransport {
        fn from_responses(responses: Vec<GithubHttpResponse>) -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
                responses: Mutex::new(responses.into()),
            }
        }

        fn requests(&self) -> Vec<GithubHttpRequest> {
            self.requests.lock().expect("requests lock").clone()
        }
    }

    impl GithubHttpTransport for FakeGithubHttpTransport {
        fn execute<'a>(&'a self, request: GithubHttpRequest) -> GithubHttpFuture<'a> {
            Box::pin(async move {
                self.requests.lock().expect("requests lock").push(request);
                self.responses
                    .lock()
                    .expect("responses lock")
                    .pop_front()
                    .context("fake response queue is empty")
            })
        }
    }

    #[tokio::test]
    async fn pages_until_short_page_without_auth() {
        let first_page = (1..=100).map(rest_comment_json).collect::<Vec<_>>();
        let second_page = (101..=102).map(rest_comment_json).collect::<Vec<_>>();
        let transport = FakeGithubHttpTransport::from_responses(vec![
            GithubHttpResponse {
                status: reqwest::StatusCode::OK,
                body: serde_json::to_vec(&first_page).unwrap(),
            },
            GithubHttpResponse {
                status: reqwest::StatusCode::OK,
                body: serde_json::to_vec(&second_page).unwrap(),
            },
        ]);

        let comments = fetch_public_rest_pages_with::<RestCommentDto>(
            &transport,
            "/repos/openclaw/openclaw/issues/88499/comments",
        )
        .await
        .expect("paginated public REST comments");

        assert_eq!(comments.len(), 102);
        assert_eq!(comments[0].id, 1);
        assert_eq!(comments[101].id, 102);
        let requests = transport.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method, GithubHttpMethod::Get);
        assert_eq!(
            requests[0].url,
            "https://api.github.com/repos/openclaw/openclaw/issues/88499/comments?per_page=100&page=1"
        );
        assert_eq!(
            requests[1].url,
            "https://api.github.com/repos/openclaw/openclaw/issues/88499/comments?per_page=100&page=2"
        );
        assert_eq!(requests[0].token, None);
        assert_eq!(requests[1].token, None);
    }

    #[test]
    fn page_path_uses_ampersand_when_query_already_exists() {
        assert_eq!(
            public_rest_page_path("/repos/owner/repo/issues/1/comments?since=2026-01-01", 3),
            "/repos/owner/repo/issues/1/comments?since=2026-01-01&per_page=100&page=3"
        );
    }

    #[tokio::test]
    async fn public_rest_check_runs_page_without_auth_and_preserve_urls() {
        let id = ResourceId::from_owner_repo_number("openclaw/openclaw", "81834").unwrap();
        let transport = FakeGithubHttpTransport::from_responses(vec![GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: serde_json::to_vec(&json!({
                "check_runs": [
                    {
                        "name": "ci / test",
                        "status": "completed",
                        "conclusion": "failure",
                        "html_url": "https://github.com/openclaw/openclaw/runs/1",
                        "details_url": "https://ci.example.test/build/1",
                        "started_at": "2026-05-31T00:00:00Z",
                        "completed_at": "2026-05-31T00:01:00Z",
                        "output": {
                            "title": "tests failed",
                            "summary": "one test failed"
                        }
                    }
                ]
            }))
            .unwrap(),
        }]);

        let checks = fetch_public_rest_check_runs_with(&transport, &id, "abc123")
            .await
            .expect("public check runs");

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, "ci / test");
        assert_eq!(checks[0].status, CheckStatus::Failure);
        assert_eq!(checks[0].summary.as_deref(), Some("one test failed"));
        assert_eq!(
            checks[0].details_url.as_deref(),
            Some("https://ci.example.test/build/1")
        );
        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, GithubHttpMethod::Get);
        assert_eq!(
            requests[0].url,
            "https://api.github.com/repos/openclaw/openclaw/commits/abc123/check-runs?per_page=100&page=1"
        );
        assert_eq!(requests[0].token, None);
    }

    #[tokio::test]
    async fn public_rest_status_contexts_without_auth() {
        let id = ResourceId::from_owner_repo_number("openclaw/openclaw", "81834").unwrap();
        let transport = FakeGithubHttpTransport::from_responses(vec![GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: serde_json::to_vec(&json!({
                "statuses": [
                    {
                        "context": "legacy/build",
                        "state": "success",
                        "target_url": "https://ci.example.test/status/1",
                        "description": "build passed",
                        "created_at": "2026-05-31T00:00:00Z",
                        "updated_at": "2026-05-31T00:02:00Z"
                    }
                ]
            }))
            .unwrap(),
        }]);

        let checks = fetch_public_rest_status_contexts_with(&transport, &id, "abc123")
            .await
            .expect("public status contexts");

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, "legacy/build");
        assert_eq!(checks[0].status, CheckStatus::Success);
        assert_eq!(checks[0].summary.as_deref(), Some("build passed"));
        assert_eq!(
            checks[0].details_url.as_deref(),
            Some("https://ci.example.test/status/1")
        );
        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url,
            "https://api.github.com/repos/openclaw/openclaw/commits/abc123/status"
        );
        assert_eq!(requests[0].token, None);
    }

    #[tokio::test]
    async fn public_rest_reviews_without_auth() {
        let id = ResourceId::from_owner_repo_number("openclaw/openclaw", "81834").unwrap();
        let transport = FakeGithubHttpTransport::from_responses(vec![GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: serde_json::to_vec(&json!([
                {
                    "id": 44,
                    "user": {"login": "maintainer"},
                    "body": "looks good",
                    "state": "approved",
                    "submitted_at": "2026-05-31T00:03:00Z",
                    "html_url": "https://github.com/openclaw/openclaw/pull/81834#pullrequestreview-44",
                    "author_association": "MEMBER"
                }
            ]))
            .unwrap(),
        }]);

        let reviews = fetch_public_rest_reviews_with(&transport, &id)
            .await
            .expect("public reviews");

        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].kind, ActivityKind::Review);
        assert_eq!(reviews[0].author, "maintainer");
        assert_eq!(reviews[0].body, "APPROVED: looks good");
        assert_eq!(reviews[0].updated_at, "2026-05-31T00:03:00Z");
        assert_eq!(
            reviews[0].url.as_deref(),
            Some("https://github.com/openclaw/openclaw/pull/81834#pullrequestreview-44")
        );
        assert_eq!(reviews[0].author_association.as_deref(), Some("MEMBER"));
        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url,
            "https://api.github.com/repos/openclaw/openclaw/pulls/81834/reviews?per_page=100&page=1"
        );
        assert_eq!(requests[0].token, None);
    }

    #[tokio::test]
    async fn public_rest_review_comments_without_auth() {
        let id = ResourceId::from_owner_repo_number("openclaw/openclaw", "81834").unwrap();
        let transport = FakeGithubHttpTransport::from_responses(vec![GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: serde_json::to_vec(&json!([
                {
                    "id": 55,
                    "user": {"login": "reviewer"},
                    "body": "please rename this",
                    "created_at": "2026-05-31T00:04:00Z",
                    "updated_at": "2026-05-31T00:05:00Z",
                    "html_url": "https://github.com/openclaw/openclaw/pull/81834#discussion_r55",
                    "author_association": "CONTRIBUTOR",
                    "reactions": {"eyes": 2},
                    "path": "src/lib.rs",
                    "line": 42,
                    "pull_request_review_id": 44
                }
            ]))
            .unwrap(),
        }]);

        let comments = fetch_public_rest_review_comments_with(&transport, &id)
            .await
            .expect("public review comments");

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].kind, ActivityKind::ReviewComment);
        assert_eq!(comments[0].author, "reviewer");
        assert_eq!(comments[0].body, "please rename this");
        assert_eq!(comments[0].path.as_deref(), Some("src/lib.rs"));
        assert_eq!(comments[0].line, Some(42));
        assert_eq!(
            comments[0].thread_id.as_deref(),
            Some("public-rest-review-44")
        );
        assert_eq!(comments[0].reactions.eyes, 2);
        assert!(comments[0].includes_created_edit);
        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url,
            "https://api.github.com/repos/openclaw/openclaw/pulls/81834/comments?per_page=100&page=1"
        );
        assert_eq!(requests[0].token, None);
    }

    #[tokio::test]
    async fn public_rest_timeline_events_without_auth() {
        let id = ResourceId::from_owner_repo_number("openclaw/openclaw", "88499").unwrap();
        let transport = FakeGithubHttpTransport::from_responses(vec![GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: serde_json::to_vec(&json!([
                {
                    "id": 71,
                    "event": "labeled",
                    "actor": {"login": "maintainer"},
                    "created_at": "2026-05-31T00:01:00Z",
                    "label": {"name": "bug"},
                    "url": "https://api.github.com/repos/openclaw/openclaw/issues/events/71"
                },
                {
                    "id": 72,
                    "event": "renamed",
                    "actor": {"login": "maintainer"},
                    "created_at": "2026-05-31T00:02:00Z",
                    "rename": {"from": "old title", "to": "new title"}
                },
                {
                    "id": 73,
                    "event": "cross-referenced",
                    "actor": {"login": "alice"},
                    "created_at": "2026-05-31T00:03:00Z",
                    "source": {
                        "issue": {
                            "title": "related issue",
                            "html_url": "https://github.com/openclaw/openclaw/issues/88500"
                        }
                    }
                },
                {
                    "id": 74,
                    "event": "commented",
                    "actor": {"login": "alice"},
                    "created_at": "2026-05-31T00:04:00Z"
                }
            ]))
            .unwrap(),
        }]);

        let events = fetch_public_rest_timeline_events_with(&transport, &id)
            .await
            .expect("public timeline events");

        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, ActivityKind::Timeline);
        assert_eq!(events[0].author, "maintainer");
        assert_eq!(events[0].body, "added label bug");
        assert_eq!(events[0].updated_at, "2026-05-31T00:01:00Z");
        assert_eq!(
            events[0].url.as_deref(),
            Some("https://api.github.com/repos/openclaw/openclaw/issues/events/71")
        );
        assert_eq!(events[1].body, "renamed title from old title to new title");
        assert_eq!(
            events[2].body,
            "cross-referenced https://github.com/openclaw/openclaw/issues/88500"
        );
        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url,
            "https://api.github.com/repos/openclaw/openclaw/issues/88499/timeline?per_page=100&page=1"
        );
        assert_eq!(requests[0].token, None);
    }

    fn rest_comment_json(id: u64) -> Value {
        json!({
            "id": id,
            "user": {"login": "alice"},
            "body": format!("comment {id}"),
            "created_at": "2026-05-31T00:00:00Z",
            "updated_at": "2026-05-31T00:00:00Z"
        })
    }

    #[test]
    fn issue_fallback_renders_core_monitoring_surfaces() {
        let id = ResourceId::from_owner_repo_number("openclaw/openclaw", "88499").unwrap();
        let issue = RestIssueDto {
            number: 88499,
            title: "Public issue".into(),
            html_url: "https://github.com/openclaw/openclaw/issues/88499".into(),
            state: "open".into(),
            user: Some(RestUserDto {
                login: Some("alice".into()),
            }),
            created_at: "2026-05-30T00:00:00Z".into(),
            updated_at: "2026-05-31T00:00:00Z".into(),
            labels: vec![RestLabelDto { name: "bug".into() }],
            assignees: vec![RestUserDto {
                login: Some("bob".into()),
            }],
            reactions: RestReactionsDto {
                thumbs_up: 2,
                eyes: 1,
                ..RestReactionsDto::default()
            },
            body: Some("Issue body".into()),
            closed_at: None,
            state_reason: Some("REOPENED".into()),
            locked: true,
            active_lock_reason: Some("TOO_HEATED".into()),
            milestone: Some(json!({"title": "v1"})),
        };
        let activity = vec![rest_comment_activity((
            0,
            RestCommentDto {
                id: 1,
                user: Some(RestUserDto {
                    login: Some("carol".into()),
                }),
                body: Some("Public comment".into()),
                created_at: "2026-05-31T00:00:00Z".into(),
                updated_at: "2026-05-31T00:00:00Z".into(),
                html_url: Some(
                    "https://github.com/openclaw/openclaw/issues/88499#issuecomment-1".into(),
                ),
                author_association: Some("MEMBER".into()),
                reactions: RestReactionsDto::default(),
            },
        ))];

        let resource = rest_issue_resource(issue, &id, activity);

        assert_eq!(resource.kind(), ResourceKind::Issue);
        assert_eq!(resource.state, "OPEN");
        assert_eq!(resource.author, "alice");
        assert_eq!(resource.labels, ["bug"]);
        assert_eq!(resource.assignees, ["bob"]);
        assert_eq!(resource.reactions.total(), 3);
        assert_eq!(resource.activity[0].body, "Public comment");
        assert!(resource
            .metadata
            .iter()
            .any(|item| item.label == "Milestone" && item.value == "v1"));
        assert!(resource
            .metadata
            .iter()
            .any(|item| item.label == "Locked" && item.value == "yes"));
        assert!(resource
            .metadata
            .iter()
            .any(|item| item.label == "Lock reason" && item.value == "TOO_HEATED"));
    }

    #[test]
    fn pr_fallback_renders_core_monitoring_surfaces() {
        let pull = RestPullDto {
            title: "Public PR".into(),
            html_url: "https://github.com/openclaw/openclaw/pull/81834".into(),
            state: "open".into(),
            user: Some(RestUserDto {
                login: Some("alice".into()),
            }),
            created_at: "2026-05-30T00:00:00Z".into(),
            updated_at: "2026-05-31T00:00:00Z".into(),
            base: Some(RestRefDto {
                reference: "main".into(),
                sha: None,
            }),
            head: Some(RestRefDto {
                reference: "feature".into(),
                sha: Some("abcdef123456".into()),
            }),
            requested_reviewers: vec![RestUserDto {
                login: Some("reviewer".into()),
            }],
            mergeable: Some(true),
            additions: Some(10),
            deletions: Some(2),
            changed_files: Some(1),
            draft: false,
            merged_at: None,
            merge_commit_sha: Some("abc123".into()),
        };
        let commits = vec![rest_commit(RestCommitDto {
            sha: "abcdef123456".into(),
            commit: RestCommitInnerDto {
                message: "feat: public fallback\n\nbody".into(),
                author: Some(RestCommitPersonDto {
                    name: Some("Fallback Author".into()),
                    date: Some("2026-05-30T00:00:00Z".into()),
                }),
                committer: None,
            },
            author: None,
        })];
        let files = vec![rest_file(RestFileDto {
            filename: "src/lib.rs".into(),
            additions: 10,
            deletions: 2,
            status: "modified".into(),
            patch: Some("@@ -1 +1 @@\n-old\n+new".into()),
        })];

        let pr = rest_pull_request(&pull, commits, Vec::new(), files);

        assert_eq!(pr.base_ref, "main");
        assert_eq!(pr.head_ref, "feature");
        assert_eq!(pr.requested_reviewers, ["reviewer"]);
        assert_eq!(pr.commits[0].message, "feat: public fallback");
        assert_eq!(pr.commits[0].body, "body");
        assert_eq!(pr.files[0].path, "src/lib.rs");
        assert_eq!(pr.files[0].change_type, "MODIFIED");
        assert!(pr
            .metadata
            .iter()
            .any(|item| item.label == "Merge commit" && item.value == "abc123"));
    }

    #[test]
    fn pr_fallback_normalizer_tolerates_missing_optional_fields() {
        let pull = RestPullDto {
            title: "Minimal public PR".into(),
            html_url: "https://github.com/openclaw/openclaw/pull/81834".into(),
            state: "open".into(),
            user: None,
            created_at: "2026-05-30T00:00:00Z".into(),
            updated_at: "2026-05-31T00:00:00Z".into(),
            base: None,
            head: None,
            requested_reviewers: vec![RestUserDto { login: None }],
            mergeable: None,
            additions: None,
            deletions: None,
            changed_files: None,
            draft: false,
            merged_at: None,
            merge_commit_sha: None,
        };

        let pr = rest_pull_request(&pull, Vec::new(), Vec::new(), Vec::new());

        assert_eq!(pr.base_ref, "");
        assert_eq!(pr.head_ref, "");
        assert!(pr.requested_reviewers.is_empty());
        assert_eq!(pr.review_decision, None);
        assert_eq!(pr.merge_state, None);
        assert_eq!(pr.additions, 0);
        assert_eq!(pr.deletions, 0);
        assert!(pr.commits.is_empty());
        assert!(pr.checks.is_empty());
        assert!(pr.files.is_empty());
        assert!(pr
            .metadata
            .iter()
            .any(|item| item.label == "Draft" && item.value == "no"));
        assert!(!pr.metadata.iter().any(|item| item.label == "Changed files"));
    }
}
