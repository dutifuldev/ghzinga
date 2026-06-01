use anyhow::Context;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    domain::{
        ActivityEntry, ActivityKind, ChangedFile, CheckStatus, Commit, MetadataItem, PullRequest,
        ReactionCounts, Resource, ResourceId, ResourceKind,
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
    let pr_metadata = rest_pr_metadata(&pull);

    let mut resource = rest_issue_resource(issue, id, comments);
    resource.id.kind_hint = Some(ResourceKind::PullRequest);
    resource.url = pull.html_url;
    resource.title = pull.title;
    resource.state = pull.state.to_ascii_uppercase();
    resource.author = display_rest_author(pull.user);
    resource.created_at = pull.created_at;
    resource.updated_at = pull.updated_at;
    resource.pull_request = Some(PullRequest {
        base_ref: pull
            .base
            .map(|reference| reference.reference)
            .unwrap_or_default(),
        head_ref: pull
            .head
            .map(|reference| reference.reference)
            .unwrap_or_default(),
        requested_reviewers: pull
            .requested_reviewers
            .into_iter()
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
        checks: Vec::new(),
        files,
        metadata: pr_metadata,
    });
    resource.warnings.extend(warnings);
    resource.warnings.push(
        "public REST fallback omits GraphQL-only enrichment such as reviews, review threads, rich timeline events, projects, participants, relationship links, and check suites".into(),
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
    let mut resource = rest_issue_resource(issue, id, comments);
    resource.warnings.extend(warnings);
    resource.warnings.push(
        "public REST fallback omits GraphQL-only enrichment such as rich timeline events, projects, participants, issue relationships, duplicate issue targets, linked branches, relationship links, and review data".into(),
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

pub(super) async fn fetch_public_rest_pages_with<T: serde::de::DeserializeOwned>(
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

pub(super) fn public_rest_page_path(base_path: &str, page: u64) -> String {
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

fn rest_pull_path(id: &ResourceId) -> String {
    format!("/repos/{}/{}/pulls/{}", id.owner, id.repo, id.number)
}

fn rest_issue_path(id: &ResourceId) -> String {
    format!("/repos/{}/{}/issues/{}", id.owner, id.repo, id.number)
}

#[derive(Debug, Deserialize)]
pub(super) struct RestUserDto {
    pub(super) login: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RestLabelDto {
    pub(super) name: String,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct RestReactionsDto {
    #[serde(rename = "+1", default)]
    pub(super) thumbs_up: u64,
    #[serde(rename = "-1", default)]
    pub(super) thumbs_down: u64,
    #[serde(default)]
    pub(super) laugh: u64,
    #[serde(default)]
    pub(super) hooray: u64,
    #[serde(default)]
    pub(super) confused: u64,
    #[serde(default)]
    pub(super) heart: u64,
    #[serde(default)]
    pub(super) rocket: u64,
    #[serde(default)]
    pub(super) eyes: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct RestIssueDto {
    pub(super) number: u64,
    pub(super) title: String,
    pub(super) html_url: String,
    pub(super) state: String,
    pub(super) user: Option<RestUserDto>,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    #[serde(default)]
    pub(super) labels: Vec<RestLabelDto>,
    #[serde(default)]
    pub(super) assignees: Vec<RestUserDto>,
    #[serde(default)]
    pub(super) reactions: RestReactionsDto,
    pub(super) body: Option<String>,
    pub(super) closed_at: Option<String>,
    pub(super) state_reason: Option<String>,
    #[serde(default)]
    pub(super) locked: bool,
    pub(super) active_lock_reason: Option<String>,
    pub(super) milestone: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RestPullDto {
    pub(super) title: String,
    pub(super) html_url: String,
    pub(super) state: String,
    pub(super) user: Option<RestUserDto>,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    pub(super) base: Option<RestRefDto>,
    pub(super) head: Option<RestRefDto>,
    #[serde(default)]
    pub(super) requested_reviewers: Vec<RestUserDto>,
    pub(super) mergeable: Option<bool>,
    pub(super) additions: Option<u64>,
    pub(super) deletions: Option<u64>,
    pub(super) changed_files: Option<u64>,
    #[serde(default)]
    pub(super) draft: bool,
    pub(super) merged_at: Option<String>,
    pub(super) merge_commit_sha: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RestRefDto {
    #[serde(rename = "ref")]
    pub(super) reference: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RestCommentDto {
    pub(super) id: u64,
    pub(super) user: Option<RestUserDto>,
    pub(super) body: Option<String>,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    pub(super) html_url: Option<String>,
    pub(super) author_association: Option<String>,
    #[serde(default)]
    pub(super) reactions: RestReactionsDto,
}

#[derive(Debug, Deserialize)]
pub(super) struct RestCommitDto {
    pub(super) sha: String,
    pub(super) commit: RestCommitInnerDto,
    pub(super) author: Option<RestUserDto>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RestCommitInnerDto {
    pub(super) message: String,
    pub(super) author: Option<RestCommitPersonDto>,
    pub(super) committer: Option<RestCommitPersonDto>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RestCommitPersonDto {
    pub(super) name: Option<String>,
    pub(super) date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RestFileDto {
    pub(super) filename: String,
    pub(super) additions: u64,
    pub(super) deletions: u64,
    pub(super) status: String,
    pub(super) patch: Option<String>,
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

pub(super) fn rest_issue_resource(
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

pub(super) fn rest_pr_metadata(pr: &RestPullDto) -> Vec<MetadataItem> {
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

pub(super) fn rest_comment_activity((index, comment): (usize, RestCommentDto)) -> ActivityEntry {
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

pub(super) fn rest_commit(commit: RestCommitDto) -> Commit {
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

pub(super) fn rest_file(file: RestFileDto) -> ChangedFile {
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
