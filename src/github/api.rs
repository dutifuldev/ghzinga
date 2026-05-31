use std::{
    collections::{HashMap, HashSet},
    future::Future,
};

use anyhow::Context;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::domain::{
    ActivityEntry, ActivityKind, ChangedFile, CheckRun, CheckStatus, Commit, Deployment,
    MetadataItem, PullRequest, ReactionCounts, Resource, ResourceId, ResourceKind,
};
use crate::github::queries::{
    base_issue_query, base_pr_query, changed_files_query, check_suites_query, comments_query,
    commit_deployments_query, commits_query, project_items_query, review_thread_comments_query,
    review_threads_query, status_rollup_query, timeline_query,
};
use crate::github::transport::{run_graphql_query, run_rest_get, GITHUB_GRAPHQL_URL};
#[cfg(test)]
use crate::github::transport::{
    run_graphql_query_with, run_rest_get_with, GithubHttpFuture, GithubHttpMethod,
    GithubHttpRequest, GithubHttpResponse, GithubHttpTransport, GITHUB_JSON_ACCEPT,
};

pub trait GithubGateway {
    fn fetch_resource(
        &self,
        id: &ResourceId,
    ) -> impl Future<Output = anyhow::Result<Resource>> + Send;
}

#[derive(Debug, Clone, Default)]
pub struct GithubApiGateway;

impl GithubGateway for GithubApiGateway {
    async fn fetch_resource(&self, id: &ResourceId) -> anyhow::Result<Resource> {
        match id.kind_hint {
            Some(ResourceKind::PullRequest) => fetch_pr(id).await,
            Some(ResourceKind::Issue) => fetch_issue(id).await,
            None => match fetch_pr(id).await {
                Ok(resource) => Ok(resource),
                Err(pr_error) => fetch_issue(id)
                    .await
                    .with_context(|| format!("failed as PR first: {pr_error}")),
            },
        }
    }
}

pub fn command_preview_for_pr(id: &ResourceId) -> Vec<String> {
    graphql_preview("pullRequest", id)
}

pub fn command_preview_for_issue(id: &ResourceId) -> Vec<String> {
    graphql_preview("issue", id)
}

async fn fetch_pr(id: &ResourceId) -> anyhow::Result<Resource> {
    let output = run_graphql_base_pr(id).await?;
    let dto = pr_view_from_graphql(&output)?;
    let mut resource = dto.into_resource(id);
    enrich_project_metadata(&mut resource, id, ResourceKind::PullRequest).await;
    match fetch_comment_activity(id, ResourceKind::PullRequest).await {
        Ok(comments) => replace_comment_activity(&mut resource, comments),
        Err(error) => push_enrichment_warning(&mut resource, "comments unavailable", &error),
    }
    match fetch_review_thread_activity(id).await {
        Ok(review_comments) => resource.activity.extend(review_comments),
        Err(error) => push_enrichment_warning(&mut resource, "review threads unavailable", &error),
    }
    match fetch_timeline_activity(id, ResourceKind::PullRequest).await {
        Ok(timeline) => resource.activity.extend(timeline),
        Err(error) => push_enrichment_warning(&mut resource, "timeline unavailable", &error),
    }
    sort_activity(&mut resource.activity);
    let mut warnings = Vec::new();
    if let Some(pr) = resource.pull_request.as_mut() {
        match fetch_changed_files(id).await {
            Ok(files) => pr.files = files,
            Err(error) => warnings.push(format!("full changed file list unavailable: {error}")),
        }
        match fetch_file_patches(id).await {
            Ok(patches) => apply_file_patches(&mut pr.files, patches),
            Err(error) => warnings.push(format!("file patch context unavailable: {error}")),
        }
        match fetch_commits(id).await {
            Ok(commits) => replace_pr_commits(pr, commits),
            Err(error) => warnings.push(format!("full commit list unavailable: {error}")),
        }
        match fetch_commit_deployments(id).await {
            Ok(deployments) => apply_commit_deployments(&mut pr.commits, deployments),
            Err(error) => warnings.push(format!("commit deployments unavailable: {error}")),
        }
        match fetch_status_rollup_checks(id).await {
            Ok(checks) => pr.checks = checks,
            Err(error) => warnings.push(format!("full status rollup unavailable: {error}")),
        }
        match fetch_check_suites(id).await {
            Ok(suites) => apply_check_suites(&mut pr.checks, suites),
            Err(error) => warnings.push(format!("check suites unavailable: {error}")),
        }
    }
    resource.warnings.extend(warnings);
    Ok(resource)
}

async fn fetch_issue(id: &ResourceId) -> anyhow::Result<Resource> {
    let output = run_graphql_base_issue(id).await?;
    let dto = issue_view_from_graphql(&output)?;
    let mut resource = dto.into_resource(id);
    enrich_project_metadata(&mut resource, id, ResourceKind::Issue).await;
    match fetch_comment_activity(id, ResourceKind::Issue).await {
        Ok(comments) => replace_comment_activity(&mut resource, comments),
        Err(error) => push_enrichment_warning(&mut resource, "comments unavailable", &error),
    }
    match fetch_timeline_activity(id, ResourceKind::Issue).await {
        Ok(timeline) => resource.activity.extend(timeline),
        Err(error) => push_enrichment_warning(&mut resource, "timeline unavailable", &error),
    }
    sort_activity(&mut resource.activity);
    Ok(resource)
}

fn push_enrichment_warning(resource: &mut Resource, label: &str, error: &anyhow::Error) {
    resource.warnings.push(format!("{label}: {error}"));
}

fn replace_comment_activity(resource: &mut Resource, comments: Vec<ActivityEntry>) {
    resource
        .activity
        .retain(|entry| entry.kind != ActivityKind::Comment);
    resource.activity.extend(comments);
}

async fn enrich_project_metadata(resource: &mut Resource, id: &ResourceId, kind: ResourceKind) {
    match fetch_project_names(id, kind).await {
        Ok(projects) => apply_project_metadata(&mut resource.metadata, projects),
        Err(error) if is_project_scope_error(&error) => {}
        Err(error) => push_enrichment_warning(resource, "projects unavailable", &error),
    }
}

fn owner_repo_number_variables(id: &ResourceId, after: Option<&str>) -> Value {
    json!({
        "owner": id.owner,
        "name": id.repo,
        "number": id.number,
        "after": after,
    })
}

async fn run_graphql_base_pr(id: &ResourceId) -> anyhow::Result<Vec<u8>> {
    run_graphql_query(base_pr_query(), owner_repo_number_variables(id, None)).await
}

async fn run_graphql_base_issue(id: &ResourceId) -> anyhow::Result<Vec<u8>> {
    run_graphql_query(base_issue_query(), owner_repo_number_variables(id, None)).await
}

async fn run_graphql_project_items(
    id: &ResourceId,
    kind: ResourceKind,
    after: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    let query = project_items_query(kind);
    run_graphql_query(&query, owner_repo_number_variables(id, after)).await
}

fn graphql_preview(selector: &str, id: &ResourceId) -> Vec<String> {
    vec![
        "POST".into(),
        GITHUB_GRAPHQL_URL.into(),
        format!("repository={}", id.repo_name_with_owner()),
        format!("number={}", id.number),
        format!("selector={selector}"),
    ]
}

async fn run_graphql_comments(
    id: &ResourceId,
    kind: ResourceKind,
    after: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    let query = comments_query(kind);
    run_graphql_query(&query, owner_repo_number_variables(id, after)).await
}

async fn run_graphql_review_threads(
    id: &ResourceId,
    after: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    let query = review_threads_query();
    run_graphql_query(query, owner_repo_number_variables(id, after)).await
}

async fn run_graphql_review_thread_comments(
    thread_id: &str,
    after: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    let query = review_thread_comments_query();
    run_graphql_query(
        query,
        json!({
            "threadId": thread_id,
            "after": after,
        }),
    )
    .await
}

async fn run_graphql_changed_files(
    id: &ResourceId,
    after: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    run_graphql_query(
        changed_files_query(),
        owner_repo_number_variables(id, after),
    )
    .await
}

async fn run_graphql_commits(id: &ResourceId, after: Option<&str>) -> anyhow::Result<Vec<u8>> {
    run_graphql_query(commits_query(), owner_repo_number_variables(id, after)).await
}

async fn run_graphql_timeline(
    id: &ResourceId,
    kind: ResourceKind,
    after: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    let query = timeline_query(kind);
    run_graphql_query(&query, owner_repo_number_variables(id, after)).await
}

async fn run_graphql_commit_deployments(id: &ResourceId) -> anyhow::Result<Vec<u8>> {
    run_graphql_query(
        commit_deployments_query(),
        json!({
            "owner": id.owner,
            "name": id.repo,
            "number": id.number,
        }),
    )
    .await
}

async fn run_graphql_check_suites(id: &ResourceId, after: Option<&str>) -> anyhow::Result<Vec<u8>> {
    let query = check_suites_query();
    run_graphql_query(query, owner_repo_number_variables(id, after)).await
}

async fn run_graphql_status_rollup(
    id: &ResourceId,
    after: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    run_graphql_query(
        status_rollup_query(),
        owner_repo_number_variables(id, after),
    )
    .await
}

async fn run_pr_diff(id: &ResourceId) -> anyhow::Result<Vec<u8>> {
    run_rest_get(&pr_diff_rest_path(id), "application/vnd.github.v3.diff").await
}

fn pr_diff_rest_path(id: &ResourceId) -> String {
    format!("/repos/{}/{}/pulls/{}", id.owner, id.repo, id.number)
}

#[derive(Debug, Deserialize)]
struct UserDto {
    login: Option<String>,
    name: Option<String>,
}

impl UserDto {
    fn display_name(&self) -> String {
        self.login
            .clone()
            .or_else(|| self.name.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[derive(Debug, Deserialize)]
struct LabelDto {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReactionGroupDto {
    content: String,
    users: TotalCountDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TotalCountDto {
    total_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentDto {
    id: Option<String>,
    author: Option<UserDto>,
    author_association: Option<String>,
    body: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    url: Option<String>,
    includes_created_edit: Option<bool>,
    is_minimized: Option<bool>,
    minimized_reason: Option<String>,
    #[serde(default)]
    reaction_groups: Vec<ReactionGroupDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentsResponse {
    data: CommentsData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentsData {
    repository: Option<CommentsRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentsRepository {
    issue: Option<CommentsResource>,
    pull_request: Option<CommentsResource>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentsResource {
    comments: CommentsConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentsConnection {
    page_info: PageInfoDto,
    nodes: Vec<CommentDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewDto {
    id: Option<String>,
    author: Option<UserDto>,
    author_association: Option<String>,
    body: Option<String>,
    state: Option<String>,
    submitted_at: Option<String>,
    updated_at: Option<String>,
    url: Option<String>,
    #[serde(default)]
    reaction_groups: Vec<ReactionGroupDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadsResponse {
    data: ReviewThreadsData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadsData {
    repository: Option<ReviewThreadsRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadsRepository {
    pull_request: Option<ReviewThreadsPullRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadsPullRequest {
    review_threads: ReviewThreadsConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadsConnection {
    page_info: PageInfoDto,
    nodes: Vec<ReviewThreadDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadDto {
    id: Option<String>,
    is_resolved: Option<bool>,
    is_outdated: Option<bool>,
    path: Option<String>,
    line: Option<u64>,
    comments: ReviewThreadCommentsConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadCommentsConnection {
    page_info: PageInfoDto,
    nodes: Vec<ReviewThreadCommentDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadCommentsResponse {
    data: ReviewThreadCommentsData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadCommentsData {
    node: Option<ReviewThreadCommentsNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadCommentsNode {
    comments: ReviewThreadCommentsConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadCommentDto {
    id: Option<String>,
    author: Option<UserDto>,
    author_association: Option<String>,
    body: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    url: Option<String>,
    includes_created_edit: Option<bool>,
    is_minimized: Option<bool>,
    minimized_reason: Option<String>,
    #[serde(default)]
    reaction_groups: Vec<ReactionGroupDto>,
    path: Option<String>,
    line: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangedFilesResponse {
    data: ChangedFilesData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangedFilesData {
    repository: Option<ChangedFilesRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangedFilesRepository {
    pull_request: Option<ChangedFilesPullRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangedFilesPullRequest {
    files: ChangedFilesConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangedFilesConnection {
    page_info: PageInfoDto,
    nodes: Vec<FileDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimelineResponse {
    data: TimelineData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimelineData {
    repository: Option<TimelineRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimelineRepository {
    issue: Option<TimelineResource>,
    pull_request: Option<TimelineResource>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimelineResource {
    timeline_items: TimelineItemsConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimelineItemsConnection {
    page_info: PageInfoDto,
    nodes: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectItemsResponse {
    data: ProjectItemsData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectItemsData {
    repository: Option<ProjectItemsRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectItemsRepository {
    issue: Option<ProjectItemsResource>,
    pull_request: Option<ProjectItemsResource>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectItemsResource {
    project_items: ProjectItemsConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectItemsConnection {
    page_info: PageInfoDto,
    nodes: Vec<ProjectItemDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectItemDto {
    project: Option<ProjectDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectDto {
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfoDto {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDto {
    oid: String,
    message_headline: String,
    message_body: Option<String>,
    committed_date: Option<String>,
    authored_date: Option<String>,
    authors: Option<Vec<UserDto>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitsResponse {
    data: CommitsData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitsData {
    repository: Option<CommitsRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitsRepository {
    pull_request: Option<CommitsPullRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitsPullRequest {
    commits: CommitsConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitsConnection {
    page_info: PageInfoDto,
    nodes: Vec<CommitNodeDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitNodeDto {
    commit: CommitWithAuthorsConnectionDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitWithAuthorsConnectionDto {
    oid: String,
    message_headline: String,
    message_body: Option<String>,
    committed_date: Option<String>,
    authored_date: Option<String>,
    authors: Option<CommitAuthorsConnection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitAuthorsConnection {
    nodes: Vec<CommitAuthorDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitAuthorDto {
    name: Option<String>,
    user: Option<UserDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDeploymentsResponse {
    data: CommitDeploymentsData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDeploymentsData {
    repository: Option<CommitDeploymentsRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDeploymentsRepository {
    pull_request: Option<CommitDeploymentsPullRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDeploymentsPullRequest {
    commits: CommitDeploymentsConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDeploymentsConnection {
    nodes: Vec<CommitDeploymentNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDeploymentNode {
    commit: CommitDeploymentCommit,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDeploymentCommit {
    oid: String,
    deployments: DeploymentConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeploymentConnection {
    nodes: Vec<DeploymentDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeploymentDto {
    environment: Option<String>,
    task: Option<String>,
    description: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    latest_status: Option<DeploymentStatusDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeploymentStatusDto {
    state: Option<String>,
    description: Option<String>,
    environment_url: Option<String>,
    log_url: Option<String>,
    created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuitesResponse {
    data: CheckSuitesData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuitesData {
    repository: Option<CheckSuitesRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuitesRepository {
    pull_request: Option<CheckSuitesPullRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuitesPullRequest {
    commits: CheckSuitesCommitConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuitesCommitConnection {
    nodes: Vec<CheckSuitesCommitNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuitesCommitNode {
    commit: CheckSuitesCommit,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuitesCommit {
    check_suites: CheckSuiteConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuiteConnection {
    page_info: PageInfoDto,
    nodes: Vec<CheckSuiteDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuiteDto {
    status: Option<String>,
    conclusion: Option<String>,
    url: Option<String>,
    app: Option<CheckSuiteAppDto>,
    workflow_run: Option<CheckSuiteWorkflowRunDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuiteAppDto {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuiteWorkflowRunDto {
    url: Option<String>,
    workflow: Option<CheckSuiteWorkflowDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckSuiteWorkflowDto {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusRollupResponse {
    data: StatusRollupData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusRollupData {
    repository: Option<StatusRollupRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusRollupRepository {
    pull_request: Option<StatusRollupPullRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusRollupPullRequest {
    status_check_rollup: Option<StatusCheckRollupDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusCheckRollupDto {
    contexts: StatusContextConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusContextConnection {
    page_info: PageInfoDto,
    nodes: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckDto {
    name: Option<String>,
    context: Option<String>,
    workflow_name: Option<String>,
    status: Option<String>,
    state: Option<String>,
    conclusion: Option<String>,
    details_url: Option<String>,
    target_url: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileDto {
    path: String,
    additions: u64,
    deletions: u64,
    change_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RelatedResourceDto {
    number: Option<u64>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrView {
    number: u64,
    title: String,
    url: String,
    state: String,
    author: Option<UserDto>,
    created_at: String,
    updated_at: String,
    labels: Vec<LabelDto>,
    #[serde(default)]
    assignees: Vec<UserDto>,
    reaction_groups: Vec<ReactionGroupDto>,
    body: String,
    base_ref_name: String,
    head_ref_name: String,
    base_ref_oid: Option<String>,
    head_ref_oid: Option<String>,
    head_repository: Option<Value>,
    head_repository_owner: Option<Value>,
    review_decision: Option<String>,
    #[serde(default)]
    review_requests: Vec<Value>,
    #[serde(default)]
    closing_issues_references: Vec<RelatedResourceDto>,
    merge_state_status: Option<String>,
    mergeable: Option<String>,
    #[serde(default)]
    is_draft: bool,
    #[serde(default)]
    is_cross_repository: bool,
    #[serde(default)]
    maintainer_can_modify: bool,
    changed_files: Option<u64>,
    #[serde(default)]
    closed: bool,
    closed_at: Option<String>,
    merged_at: Option<String>,
    merged_by: Option<UserDto>,
    milestone: Option<Value>,
    #[serde(default)]
    project_items: Vec<Value>,
    auto_merge_request: Option<Value>,
    merge_commit: Option<Value>,
    potential_merge_commit: Option<Value>,
    additions: u64,
    deletions: u64,
    commits: Vec<CommitDto>,
    status_check_rollup: Vec<CheckDto>,
    files: Vec<FileDto>,
    comments: Vec<CommentDto>,
    reviews: Vec<ReviewDto>,
}

impl PrView {
    fn into_resource(self, requested: &ResourceId) -> Resource {
        let resource_metadata = pr_resource_metadata(&self);
        let pull_request_metadata = pr_metadata(&self);
        let id = ResourceId {
            owner: requested.owner.clone(),
            repo: requested.repo.clone(),
            number: self.number,
            kind_hint: Some(ResourceKind::PullRequest),
        };
        Resource {
            id,
            title: self.title,
            url: self.url,
            state: self.state,
            author: display_author(self.author),
            created_at: self.created_at,
            updated_at: self.updated_at,
            labels: self.labels.into_iter().map(|label| label.name).collect(),
            assignees: names_from_users(self.assignees),
            reactions: reaction_counts(self.reaction_groups),
            body: self.body,
            activity: pr_activity(self.comments, self.reviews),
            related_resources: related_resource_ids(
                self.closing_issues_references,
                ResourceKind::Issue,
                requested,
            ),
            metadata: resource_metadata,
            warnings: Vec::new(),
            pull_request: Some(PullRequest {
                base_ref: self.base_ref_name,
                head_ref: self.head_ref_name,
                requested_reviewers: review_request_names(self.review_requests),
                review_decision: self.review_decision.filter(|value| !value.is_empty()),
                merge_state: self.merge_state_status.filter(|value| !value.is_empty()),
                additions: self.additions,
                deletions: self.deletions,
                commits: self.commits.into_iter().map(commit_from_dto).collect(),
                checks: self
                    .status_check_rollup
                    .into_iter()
                    .map(check_from_dto)
                    .collect(),
                files: self.files.into_iter().map(file_from_dto).collect(),
                metadata: pull_request_metadata,
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueView {
    number: u64,
    title: String,
    url: String,
    state: String,
    author: Option<UserDto>,
    created_at: String,
    updated_at: String,
    labels: Vec<LabelDto>,
    #[serde(default)]
    assignees: Vec<UserDto>,
    reaction_groups: Vec<ReactionGroupDto>,
    body: String,
    #[serde(default)]
    closed: bool,
    #[serde(default)]
    is_pinned: bool,
    state_reason: Option<String>,
    closed_at: Option<String>,
    milestone: Option<Value>,
    #[serde(default)]
    project_items: Vec<Value>,
    #[serde(default)]
    closed_by_pull_requests_references: Vec<RelatedResourceDto>,
    comments: Vec<CommentDto>,
}

impl IssueView {
    fn into_resource(self, requested: &ResourceId) -> Resource {
        let metadata = issue_metadata(&self);
        Resource {
            id: ResourceId {
                owner: requested.owner.clone(),
                repo: requested.repo.clone(),
                number: self.number,
                kind_hint: Some(ResourceKind::Issue),
            },
            title: self.title,
            url: self.url,
            state: self.state,
            author: display_author(self.author),
            created_at: self.created_at,
            updated_at: self.updated_at,
            labels: self.labels.into_iter().map(|label| label.name).collect(),
            assignees: names_from_users(self.assignees),
            reactions: reaction_counts(self.reaction_groups),
            body: self.body,
            activity: comments_to_activity(self.comments),
            related_resources: related_resource_ids(
                self.closed_by_pull_requests_references,
                ResourceKind::PullRequest,
                requested,
            ),
            metadata,
            warnings: Vec::new(),
            pull_request: None,
        }
    }
}

fn pr_view_from_graphql(raw: &[u8]) -> anyhow::Result<PrView> {
    let mut value: Value =
        serde_json::from_slice(raw).context("failed to parse base PR GraphQL JSON")?;
    let mut pr = value
        .pointer_mut("/data/repository/pullRequest")
        .and_then(Value::take_non_null)
        .context("base PR GraphQL response did not include a pull request")?;
    normalize_base_pr_value(&mut pr);
    serde_json::from_value(pr).context("failed to normalize base PR GraphQL JSON")
}

fn issue_view_from_graphql(raw: &[u8]) -> anyhow::Result<IssueView> {
    let mut value: Value =
        serde_json::from_slice(raw).context("failed to parse base issue GraphQL JSON")?;
    let mut issue = value
        .pointer_mut("/data/repository/issue")
        .and_then(Value::take_non_null)
        .context("base issue GraphQL response did not include an issue")?;
    normalize_base_issue_value(&mut issue);
    serde_json::from_value(issue).context("failed to normalize base issue GraphQL JSON")
}

trait TakeNonNull {
    fn take_non_null(&mut self) -> Option<Value>;
}

impl TakeNonNull for Value {
    fn take_non_null(&mut self) -> Option<Value> {
        if self.is_null() {
            return None;
        }
        Some(self.take())
    }
}

fn normalize_base_pr_value(pr: &mut Value) {
    for key in [
        "labels",
        "assignees",
        "reviewRequests",
        "closingIssuesReferences",
        "projectItems",
        "files",
        "comments",
        "reviews",
    ] {
        replace_connection_with_nodes(pr, key);
    }
    normalize_commits_connection(pr);
    normalize_status_check_rollup(pr);
}

fn normalize_base_issue_value(issue: &mut Value) {
    for key in [
        "labels",
        "assignees",
        "projectItems",
        "closedByPullRequestsReferences",
        "comments",
    ] {
        replace_connection_with_nodes(issue, key);
    }
}

fn replace_connection_with_nodes(value: &mut Value, key: &str) {
    let nodes = value
        .get_mut(key)
        .and_then(|connection| connection.get_mut("nodes"))
        .map(Value::take)
        .unwrap_or_else(|| Value::Array(Vec::new()));
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), nodes);
    }
}

fn normalize_commits_connection(pr: &mut Value) {
    let commits = pr
        .get_mut("commits")
        .and_then(|connection| connection.get_mut("nodes"))
        .and_then(Value::as_array_mut)
        .map(|nodes| {
            nodes
                .iter_mut()
                .filter_map(|node| {
                    let mut commit = node.get_mut("commit")?.take();
                    normalize_commit_authors(&mut commit);
                    Some(commit)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(object) = pr.as_object_mut() {
        object.insert("commits".to_string(), Value::Array(commits));
    }
}

fn normalize_commit_authors(commit: &mut Value) {
    let authors = commit
        .get_mut("authors")
        .and_then(|connection| connection.get_mut("nodes"))
        .and_then(Value::as_array_mut)
        .map(|nodes| {
            nodes
                .iter_mut()
                .filter_map(|author| {
                    if let Some(user) = author.get_mut("user").filter(|user| !user.is_null()) {
                        return Some(user.take());
                    }
                    let name = author.get("name").and_then(Value::as_str)?;
                    Some(json!({ "login": name, "name": name }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(object) = commit.as_object_mut() {
        object.insert("authors".to_string(), Value::Array(authors));
    }
}

fn normalize_status_check_rollup(pr: &mut Value) {
    let checks = pr
        .get_mut("statusCheckRollup")
        .and_then(|rollup| rollup.get_mut("contexts"))
        .and_then(|contexts| contexts.get_mut("nodes"))
        .and_then(Value::as_array_mut)
        .map(|nodes| {
            nodes
                .iter_mut()
                .map(|node| {
                    let check = node.take();
                    normalize_status_check_value(check)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(object) = pr.as_object_mut() {
        object.insert("statusCheckRollup".to_string(), Value::Array(checks));
    }
}

fn normalize_status_check_value(mut check: Value) -> Value {
    if let Some(workflow_name) = check
        .pointer("/checkSuite/workflowRun/workflow/name")
        .cloned()
    {
        if let Some(object) = check.as_object_mut() {
            object.insert("workflowName".to_string(), workflow_name);
        }
    }
    check
}

fn issue_metadata(issue: &IssueView) -> Vec<MetadataItem> {
    let mut items = Vec::new();
    push_bool_metadata(&mut items, "Closed", issue.closed);
    push_bool_metadata(&mut items, "Pinned", issue.is_pinned);
    push_nonempty_metadata(&mut items, "State reason", issue.state_reason.as_deref());
    push_nonempty_metadata(&mut items, "Closed at", issue.closed_at.as_deref());
    push_nonempty_metadata(
        &mut items,
        "Milestone",
        value_title(issue.milestone.as_ref()).as_deref(),
    );
    push_vec_metadata(&mut items, "Projects", value_titles(&issue.project_items));
    items
}

fn pr_resource_metadata(pr: &PrView) -> Vec<MetadataItem> {
    let mut items = Vec::new();
    push_bool_metadata(&mut items, "Closed", pr.closed);
    push_bool_metadata(&mut items, "Draft", pr.is_draft);
    push_bool_metadata(&mut items, "Cross repository", pr.is_cross_repository);
    push_bool_metadata(
        &mut items,
        "Maintainer can modify",
        pr.maintainer_can_modify,
    );
    push_nonempty_metadata(&mut items, "Mergeable", pr.mergeable.as_deref());
    push_nonempty_metadata(
        &mut items,
        "Changed files",
        pr.changed_files.map(|count| count.to_string()).as_deref(),
    );
    push_nonempty_metadata(
        &mut items,
        "Milestone",
        value_title(pr.milestone.as_ref()).as_deref(),
    );
    push_vec_metadata(&mut items, "Projects", value_titles(&pr.project_items));
    items
}

fn pr_metadata(pr: &PrView) -> Vec<MetadataItem> {
    let mut items = Vec::new();
    push_nonempty_metadata(&mut items, "Base ref OID", pr.base_ref_oid.as_deref());
    push_nonempty_metadata(&mut items, "Head ref OID", pr.head_ref_oid.as_deref());
    push_nonempty_metadata(
        &mut items,
        "Head repository",
        value_title(pr.head_repository.as_ref()).as_deref(),
    );
    push_nonempty_metadata(
        &mut items,
        "Head repository owner",
        value_title(pr.head_repository_owner.as_ref()).as_deref(),
    );
    push_nonempty_metadata(&mut items, "Closed at", pr.closed_at.as_deref());
    push_nonempty_metadata(&mut items, "Merged at", pr.merged_at.as_deref());
    push_nonempty_metadata(
        &mut items,
        "Merged by",
        pr.merged_by.as_ref().map(UserDto::display_name).as_deref(),
    );
    push_nonempty_metadata(
        &mut items,
        "Auto-merge",
        pr.auto_merge_request
            .as_ref()
            .map(|_| "enabled".to_string())
            .as_deref(),
    );
    push_nonempty_metadata(
        &mut items,
        "Merge commit",
        value_oid(pr.merge_commit.as_ref()).as_deref(),
    );
    push_nonempty_metadata(
        &mut items,
        "Potential merge commit",
        value_oid(pr.potential_merge_commit.as_ref()).as_deref(),
    );
    items
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

fn push_vec_metadata(items: &mut Vec<MetadataItem>, label: &str, values: Vec<String>) {
    if values.is_empty() {
        return;
    }
    items.push(MetadataItem {
        label: label.to_string(),
        value: values.join(", "),
    });
}

fn apply_project_metadata(items: &mut Vec<MetadataItem>, projects: Vec<String>) {
    let projects = deduped_nonempty_strings(projects);
    if projects.is_empty() {
        return;
    }
    if let Some(item) = items.iter_mut().find(|item| item.label == "Projects") {
        item.value = projects.join(", ");
        return;
    }
    push_vec_metadata(items, "Projects", projects);
}

fn deduped_nonempty_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn is_project_scope_error(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("read:project") || message.contains("INSUFFICIENT_SCOPES")
}

fn value_titles(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| value_title(Some(value)))
        .collect()
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

fn value_oid(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|value| value.get("oid").and_then(Value::as_str))
        .map(str::to_string)
}

fn display_author(author: Option<UserDto>) -> String {
    author
        .map(|author| author.display_name())
        .unwrap_or_else(|| "unknown".to_string())
}

fn names_from_users(users: Vec<UserDto>) -> Vec<String> {
    users
        .into_iter()
        .map(|user| user.display_name())
        .filter(|name| name != "unknown")
        .collect()
}

fn review_request_names(requests: Vec<Value>) -> Vec<String> {
    requests
        .into_iter()
        .filter_map(|request| display_review_request(&request))
        .collect()
}

fn related_resource_ids(
    references: Vec<RelatedResourceDto>,
    fallback_kind: ResourceKind,
    requested: &ResourceId,
) -> Vec<ResourceId> {
    references
        .into_iter()
        .filter_map(|reference| {
            reference
                .url
                .as_deref()
                .and_then(|url| ResourceId::parse(url).ok())
                .or_else(|| {
                    reference.number.map(|number| ResourceId {
                        owner: requested.owner.clone(),
                        repo: requested.repo.clone(),
                        number,
                        kind_hint: Some(fallback_kind),
                    })
                })
        })
        .collect()
}

fn display_review_request(request: &Value) -> Option<String> {
    if let Some(name) = ["login", "name", "slug"]
        .into_iter()
        .find_map(|key| request.get(key).and_then(Value::as_str))
    {
        return Some(name.to_string());
    }
    request
        .get("requestedReviewer")
        .and_then(display_review_request)
}

fn reaction_counts(groups: Vec<ReactionGroupDto>) -> ReactionCounts {
    let mut counts = ReactionCounts::default();
    for group in groups {
        let count = group.users.total_count;
        match group.content.as_str() {
            "THUMBS_UP" => counts.thumbs_up = count,
            "THUMBS_DOWN" => counts.thumbs_down = count,
            "LAUGH" => counts.laugh = count,
            "HOORAY" => counts.hooray = count,
            "CONFUSED" => counts.confused = count,
            "HEART" => counts.heart = count,
            "ROCKET" => counts.rocket = count,
            "EYES" => counts.eyes = count,
            _ => {}
        }
    }
    counts
}

fn comments_to_activity(comments: Vec<CommentDto>) -> Vec<ActivityEntry> {
    comments
        .into_iter()
        .enumerate()
        .map(|(index, comment)| ActivityEntry {
            id: comment.id.unwrap_or_else(|| format!("comment-{index}")),
            kind: ActivityKind::Comment,
            author: display_author(comment.author),
            body: comment.body,
            updated_at: comment
                .updated_at
                .or(comment.created_at)
                .unwrap_or_else(|| "unknown".to_string()),
            path: None,
            line: None,
            url: comment.url,
            author_association: comment.author_association,
            reactions: reaction_counts(comment.reaction_groups),
            includes_created_edit: comment.includes_created_edit.unwrap_or(false),
            is_minimized: comment.is_minimized.unwrap_or(false),
            minimized_reason: comment.minimized_reason.filter(|value| !value.is_empty()),
            thread_id: None,
            thread_resolved: None,
            thread_outdated: None,
        })
        .collect()
}

async fn fetch_comment_activity(
    id: &ResourceId,
    kind: ResourceKind,
) -> anyhow::Result<Vec<ActivityEntry>> {
    let mut activity = Vec::new();
    let mut after = None;
    loop {
        let output = run_graphql_comments(id, kind, after.as_deref()).await?;
        let response: CommentsResponse =
            serde_json::from_slice(&output).context("failed to parse comments GraphQL JSON")?;
        let Some(page) = comment_activity_page(response) else {
            return Ok(activity);
        };
        activity.extend(page.activity);
        if !page.has_next_page {
            return Ok(activity);
        }
        let Some(cursor) = page.end_cursor else {
            anyhow::bail!("comments page reported next page without an end cursor");
        };
        after = Some(cursor);
    }
}

struct CommentActivityPage {
    activity: Vec<ActivityEntry>,
    has_next_page: bool,
    end_cursor: Option<String>,
}

fn comment_activity_page(response: CommentsResponse) -> Option<CommentActivityPage> {
    let repository = response.data.repository?;
    let resource = repository.pull_request.or(repository.issue)?;
    let page_info = resource.comments.page_info;
    Some(CommentActivityPage {
        activity: comments_to_activity(resource.comments.nodes),
        has_next_page: page_info.has_next_page,
        end_cursor: page_info.end_cursor,
    })
}

async fn fetch_review_thread_activity(id: &ResourceId) -> anyhow::Result<Vec<ActivityEntry>> {
    let mut activity = Vec::new();
    let mut after = None;
    loop {
        let output = run_graphql_review_threads(id, after.as_deref()).await?;
        let response: ReviewThreadsResponse = serde_json::from_slice(&output)
            .context("failed to parse reviewThreads GraphQL JSON")?;
        let Some(page) = review_thread_page(response) else {
            return Ok(activity);
        };
        for mut thread in page.threads {
            fetch_remaining_review_thread_comments(&mut thread).await?;
            activity.extend(review_thread_to_activity(thread));
        }
        if !page.has_next_page {
            return Ok(activity);
        }
        let Some(cursor) = page.end_cursor else {
            anyhow::bail!("review threads page reported next page without an end cursor");
        };
        after = Some(cursor);
    }
}

async fn fetch_remaining_review_thread_comments(
    thread: &mut ReviewThreadDto,
) -> anyhow::Result<()> {
    while thread.comments.page_info.has_next_page {
        let thread_id = thread
            .id
            .as_deref()
            .context("review thread comment page reported next page without a thread id")?;
        let cursor = thread
            .comments
            .page_info
            .end_cursor
            .as_deref()
            .context("review thread comment page reported next page without an end cursor")?;
        let output = run_graphql_review_thread_comments(thread_id, Some(cursor)).await?;
        let response: ReviewThreadCommentsResponse = serde_json::from_slice(&output)
            .context("failed to parse review thread comments GraphQL JSON")?;
        let Some(page) = review_thread_comments_page(response) else {
            return Ok(());
        };
        thread.comments.nodes.extend(page.nodes);
        thread.comments.page_info = PageInfoDto {
            has_next_page: page.has_next_page,
            end_cursor: page.end_cursor,
        };
    }
    Ok(())
}

async fn fetch_timeline_activity(
    id: &ResourceId,
    kind: ResourceKind,
) -> anyhow::Result<Vec<ActivityEntry>> {
    let mut activity = Vec::new();
    let mut after = None;
    loop {
        let output = run_graphql_timeline(id, kind, after.as_deref()).await?;
        let response: TimelineResponse =
            serde_json::from_slice(&output).context("failed to parse timeline GraphQL JSON")?;
        let Some(page) = timeline_activity_page(response) else {
            return Ok(activity);
        };
        activity.extend(page.activity);
        if !page.has_next_page {
            return Ok(activity);
        }
        let Some(cursor) = page.end_cursor else {
            anyhow::bail!("timeline page reported next page without an end cursor");
        };
        after = Some(cursor);
    }
}

async fn fetch_project_names(id: &ResourceId, kind: ResourceKind) -> anyhow::Result<Vec<String>> {
    let mut projects = Vec::new();
    let mut after = None;
    loop {
        let output = run_graphql_project_items(id, kind, after.as_deref()).await?;
        let response: ProjectItemsResponse = serde_json::from_slice(&output)
            .context("failed to parse project items GraphQL JSON")?;
        let Some(page) = project_items_page(response) else {
            return Ok(projects);
        };
        projects.extend(page.projects);
        if !page.has_next_page {
            return Ok(deduped_nonempty_strings(projects));
        }
        let Some(cursor) = page.end_cursor else {
            anyhow::bail!("project items page reported next page without an end cursor");
        };
        after = Some(cursor);
    }
}

struct ProjectItemsPage {
    projects: Vec<String>,
    has_next_page: bool,
    end_cursor: Option<String>,
}

fn project_items_page(response: ProjectItemsResponse) -> Option<ProjectItemsPage> {
    let repository = response.data.repository?;
    let resource = repository.pull_request.or(repository.issue)?;
    let page_info = resource.project_items.page_info;
    Some(ProjectItemsPage {
        projects: resource
            .project_items
            .nodes
            .into_iter()
            .filter_map(|item| item.project?.title)
            .filter(|title| !title.trim().is_empty())
            .collect(),
        has_next_page: page_info.has_next_page,
        end_cursor: page_info.end_cursor,
    })
}

async fn fetch_changed_files(id: &ResourceId) -> anyhow::Result<Vec<ChangedFile>> {
    let mut files = Vec::new();
    let mut after = None;
    loop {
        let output = run_graphql_changed_files(id, after.as_deref()).await?;
        let response: ChangedFilesResponse = serde_json::from_slice(&output)
            .context("failed to parse changed files GraphQL JSON")?;
        let Some(repository) = response.data.repository else {
            return Ok(files);
        };
        let Some(pull_request) = repository.pull_request else {
            return Ok(files);
        };
        let page_info = pull_request.files.page_info;
        files.extend(pull_request.files.nodes.into_iter().map(file_from_dto));
        if !page_info.has_next_page {
            return Ok(files);
        }
        let Some(cursor) = page_info.end_cursor else {
            anyhow::bail!("changed files page reported next page without an end cursor");
        };
        after = Some(cursor);
    }
}

async fn fetch_commits(id: &ResourceId) -> anyhow::Result<Vec<Commit>> {
    let mut commits = Vec::new();
    let mut after = None;
    loop {
        let output = run_graphql_commits(id, after.as_deref()).await?;
        let response: CommitsResponse =
            serde_json::from_slice(&output).context("failed to parse commits GraphQL JSON")?;
        let Some(page) = commits_page(response) else {
            return Ok(commits);
        };
        commits.extend(page.commits);
        if !page.has_next_page {
            return Ok(commits);
        }
        let Some(cursor) = page.end_cursor else {
            anyhow::bail!("commits page reported next page without an end cursor");
        };
        after = Some(cursor);
    }
}

async fn fetch_commit_deployments(
    id: &ResourceId,
) -> anyhow::Result<HashMap<String, Vec<Deployment>>> {
    let output = run_graphql_commit_deployments(id).await?;
    let response: CommitDeploymentsResponse = serde_json::from_slice(&output)
        .context("failed to parse commit deployments GraphQL JSON")?;
    Ok(commit_deployments_from_response(response))
}

async fn fetch_check_suites(id: &ResourceId) -> anyhow::Result<Vec<CheckRun>> {
    let mut checks = Vec::new();
    let mut after = None;
    loop {
        let output = run_graphql_check_suites(id, after.as_deref()).await?;
        let response: CheckSuitesResponse =
            serde_json::from_slice(&output).context("failed to parse check suites GraphQL JSON")?;
        let Some(page) = check_suites_page(response) else {
            return Ok(deduped_check_suites(checks));
        };
        checks.extend(page.checks);
        if !page.has_next_page {
            return Ok(deduped_check_suites(checks));
        }
        let Some(cursor) = page.end_cursor else {
            anyhow::bail!("check suites page reported next page without an end cursor");
        };
        after = Some(cursor);
    }
}

async fn fetch_status_rollup_checks(id: &ResourceId) -> anyhow::Result<Vec<CheckRun>> {
    let mut checks = Vec::new();
    let mut after = None;
    loop {
        let output = run_graphql_status_rollup(id, after.as_deref()).await?;
        let response: StatusRollupResponse = serde_json::from_slice(&output)
            .context("failed to parse status rollup GraphQL JSON")?;
        let Some(page) = status_rollup_page(response)? else {
            return Ok(checks);
        };
        checks.extend(page.checks);
        if !page.has_next_page {
            return Ok(checks);
        }
        let Some(cursor) = page.end_cursor else {
            anyhow::bail!("status rollup page reported next page without an end cursor");
        };
        after = Some(cursor);
    }
}

async fn fetch_file_patches(id: &ResourceId) -> anyhow::Result<HashMap<String, String>> {
    let output = run_pr_diff(id).await?;
    let diff = String::from_utf8_lossy(&output);
    Ok(parse_unified_diff_patches(&diff))
}

fn apply_commit_deployments(
    commits: &mut [Commit],
    deployments_by_commit: HashMap<String, Vec<Deployment>>,
) {
    for commit in commits {
        if let Some(deployments) = deployments_by_commit.get(&commit.oid) {
            commit.deployments = deployments.clone();
        }
    }
}

fn apply_check_suites(checks: &mut Vec<CheckRun>, suites: Vec<CheckRun>) {
    let mut names = checks
        .iter()
        .map(|check| check.name.clone())
        .collect::<HashSet<_>>();
    let additions = suites
        .into_iter()
        .filter(|suite| names.insert(suite.name.clone()))
        .collect::<Vec<_>>();
    if additions.is_empty() {
        return;
    }
    let mut merged = additions;
    merged.append(checks);
    *checks = merged;
}

fn apply_file_patches(files: &mut [ChangedFile], patches: HashMap<String, String>) {
    for file in files {
        if let Some(patch) = patches.get(&file.path) {
            file.patch = Some(patch.clone());
        }
    }
}

fn replace_pr_commits(pr: &mut PullRequest, commits: Vec<Commit>) {
    if !commits.is_empty() {
        pr.commits = commits;
    }
}

struct CommitsPage {
    commits: Vec<Commit>,
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[cfg(test)]
fn commits_from_response(response: CommitsResponse) -> Vec<Commit> {
    commits_page(response)
        .map(|page| page.commits)
        .unwrap_or_default()
}

fn commits_page(response: CommitsResponse) -> Option<CommitsPage> {
    let repository = response.data.repository?;
    let pull_request = repository.pull_request?;
    let page_info = pull_request.commits.page_info;
    let commits = pull_request
        .commits
        .nodes
        .into_iter()
        .map(|node| commit_from_dto(commit_dto_from_connection(node.commit)))
        .collect();
    Some(CommitsPage {
        commits,
        has_next_page: page_info.has_next_page,
        end_cursor: page_info.end_cursor,
    })
}

fn commit_dto_from_connection(commit: CommitWithAuthorsConnectionDto) -> CommitDto {
    CommitDto {
        oid: commit.oid,
        message_headline: commit.message_headline,
        message_body: commit.message_body,
        committed_date: commit.committed_date,
        authored_date: commit.authored_date,
        authors: commit.authors.map(|authors| {
            authors
                .nodes
                .into_iter()
                .map(author_from_commit_author)
                .collect()
        }),
    }
}

fn author_from_commit_author(author: CommitAuthorDto) -> UserDto {
    if let Some(user) = author.user {
        return user;
    }
    let name = author.name.unwrap_or_else(|| "unknown".to_string());
    UserDto {
        login: Some(name.clone()),
        name: Some(name),
    }
}

struct CheckSuitesPage {
    checks: Vec<CheckRun>,
    has_next_page: bool,
    end_cursor: Option<String>,
}

struct StatusRollupPage {
    checks: Vec<CheckRun>,
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[cfg(test)]
fn check_suites_from_response(response: CheckSuitesResponse) -> Vec<CheckRun> {
    check_suites_page(response)
        .map(|page| deduped_check_suites(page.checks))
        .unwrap_or_default()
}

#[cfg(test)]
fn status_rollup_checks_from_response(response: StatusRollupResponse) -> Vec<CheckRun> {
    status_rollup_page(response)
        .ok()
        .flatten()
        .map(|page| page.checks)
        .unwrap_or_default()
}

fn status_rollup_page(response: StatusRollupResponse) -> anyhow::Result<Option<StatusRollupPage>> {
    let Some(repository) = response.data.repository else {
        return Ok(None);
    };
    let Some(pull_request) = repository.pull_request else {
        return Ok(None);
    };
    let Some(rollup) = pull_request.status_check_rollup else {
        return Ok(Some(StatusRollupPage {
            checks: Vec::new(),
            has_next_page: false,
            end_cursor: None,
        }));
    };
    let page_info = rollup.contexts.page_info;
    let checks = rollup
        .contexts
        .nodes
        .into_iter()
        .map(status_check_from_value)
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(Some(StatusRollupPage {
        checks,
        has_next_page: page_info.has_next_page,
        end_cursor: page_info.end_cursor,
    }))
}

fn status_check_from_value(value: Value) -> anyhow::Result<CheckRun> {
    serde_json::from_value(normalize_status_check_value(value))
        .context("failed to parse status rollup context")
        .map(check_from_dto)
}

fn check_suites_page(response: CheckSuitesResponse) -> Option<CheckSuitesPage> {
    let repository = response.data.repository?;
    let pull_request = repository.pull_request?;
    let node = pull_request.commits.nodes.into_iter().next()?;
    let page_info = node.commit.check_suites.page_info;
    let checks = node
        .commit
        .check_suites
        .nodes
        .into_iter()
        .map(check_suite_from_dto)
        .collect();
    Some(CheckSuitesPage {
        checks,
        has_next_page: page_info.has_next_page,
        end_cursor: page_info.end_cursor,
    })
}

fn deduped_check_suites(checks: Vec<CheckRun>) -> Vec<CheckRun> {
    let mut by_name = HashMap::new();
    for check in checks {
        by_name.insert(check.name.clone(), check);
    }
    let mut checks = by_name.into_values().collect::<Vec<_>>();
    checks.sort_by(|left, right| left.name.cmp(&right.name));
    checks
}

fn check_suite_from_dto(suite: CheckSuiteDto) -> CheckRun {
    let raw_status = suite.status.filter(|value| !value.is_empty());
    let raw_conclusion = suite.conclusion.filter(|value| !value.is_empty());
    let workflow_name = suite
        .workflow_run
        .as_ref()
        .and_then(|run| run.workflow.as_ref())
        .and_then(|workflow| workflow.name.as_ref())
        .filter(|value| !value.is_empty())
        .cloned();
    let app_name = suite
        .app
        .as_ref()
        .and_then(|app| app.name.as_ref())
        .filter(|value| !value.is_empty())
        .cloned();
    let name = workflow_name
        .as_ref()
        .or(app_name.as_ref())
        .map(|name| format!("suite/{name}"))
        .unwrap_or_else(|| "suite/check suite".to_string());
    let details_url = suite
        .workflow_run
        .and_then(|run| run.url)
        .or(suite.url)
        .filter(|value| !value.is_empty());
    let summary = app_name.map(|app| format!("check suite from {app}"));

    CheckRun {
        name,
        status: classify_check(raw_status.as_deref(), raw_conclusion.as_deref()),
        summary,
        details_url,
        started_at: None,
        completed_at: None,
        raw_status,
        raw_conclusion,
    }
}

fn commit_deployments_from_response(
    response: CommitDeploymentsResponse,
) -> HashMap<String, Vec<Deployment>> {
    response
        .data
        .repository
        .and_then(|repository| repository.pull_request)
        .map(|pull_request| {
            pull_request
                .commits
                .nodes
                .into_iter()
                .map(|node| {
                    (
                        node.commit.oid,
                        node.commit
                            .deployments
                            .nodes
                            .into_iter()
                            .map(deployment_from_dto)
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default()
}

fn deployment_from_dto(deployment: DeploymentDto) -> Deployment {
    let status = deployment.latest_status;
    let state = status
        .as_ref()
        .and_then(|status| status.state.clone())
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let description = status
        .as_ref()
        .and_then(|status| status.description.clone())
        .or(deployment.description);
    let created_at = deployment
        .created_at
        .or_else(|| status.as_ref().and_then(|status| status.created_at.clone()));
    Deployment {
        environment: deployment
            .environment
            .filter(|value| !value.is_empty())
            .or(deployment.task)
            .unwrap_or_else(|| "deployment".to_string()),
        state,
        description,
        environment_url: status
            .as_ref()
            .and_then(|status| status.environment_url.clone()),
        log_url: status.as_ref().and_then(|status| status.log_url.clone()),
        created_at,
        updated_at: deployment
            .updated_at
            .or_else(|| status.and_then(|status| status.created_at))
            .unwrap_or_else(|| "unknown".to_string()),
    }
}

#[cfg(test)]
fn changed_files_from_response(response: ChangedFilesResponse) -> Vec<ChangedFile> {
    response
        .data
        .repository
        .and_then(|repository| repository.pull_request)
        .map(|pull_request| {
            pull_request
                .files
                .nodes
                .into_iter()
                .map(file_from_dto)
                .collect()
        })
        .unwrap_or_default()
}

struct ReviewThreadPage {
    threads: Vec<ReviewThreadDto>,
    has_next_page: bool,
    end_cursor: Option<String>,
}

struct ReviewThreadCommentsPage {
    nodes: Vec<ReviewThreadCommentDto>,
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[cfg(test)]
struct ReviewThreadActivityPage {
    activity: Vec<ActivityEntry>,
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[cfg(test)]
fn review_thread_activity(response: ReviewThreadsResponse) -> Vec<ActivityEntry> {
    review_thread_activity_page(response)
        .map(|page| page.activity)
        .unwrap_or_default()
}

#[cfg(test)]
fn review_thread_activity_page(
    response: ReviewThreadsResponse,
) -> Option<ReviewThreadActivityPage> {
    let page = review_thread_page(response)?;
    let activity = page
        .threads
        .into_iter()
        .flat_map(review_thread_to_activity)
        .collect();
    Some(ReviewThreadActivityPage {
        activity,
        has_next_page: page.has_next_page,
        end_cursor: page.end_cursor,
    })
}

fn review_thread_page(response: ReviewThreadsResponse) -> Option<ReviewThreadPage> {
    let repository = response.data.repository?;
    let pull_request = repository.pull_request?;
    let page_info = pull_request.review_threads.page_info;
    Some(ReviewThreadPage {
        threads: pull_request.review_threads.nodes,
        has_next_page: page_info.has_next_page,
        end_cursor: page_info.end_cursor,
    })
}

fn review_thread_comments_page(
    response: ReviewThreadCommentsResponse,
) -> Option<ReviewThreadCommentsPage> {
    let node = response.data.node?;
    let page_info = node.comments.page_info;
    Some(ReviewThreadCommentsPage {
        nodes: node.comments.nodes,
        has_next_page: page_info.has_next_page,
        end_cursor: page_info.end_cursor,
    })
}

fn review_thread_to_activity(thread: ReviewThreadDto) -> Vec<ActivityEntry> {
    let mut entries = Vec::new();
    for comment in thread.comments.nodes {
        entries.push(ActivityEntry {
            id: comment.id.unwrap_or_else(|| {
                format!(
                    "review-comment-{}-{}",
                    thread.path.as_deref().unwrap_or("unknown"),
                    entries.len()
                )
            }),
            kind: ActivityKind::ReviewComment,
            author: display_author(comment.author),
            body: comment.body,
            updated_at: comment
                .updated_at
                .or(comment.created_at)
                .unwrap_or_else(|| "unknown".to_string()),
            path: comment.path.or_else(|| thread.path.clone()),
            line: comment.line.or(thread.line),
            url: comment.url,
            author_association: comment.author_association,
            reactions: reaction_counts(comment.reaction_groups),
            includes_created_edit: comment.includes_created_edit.unwrap_or(false),
            is_minimized: comment.is_minimized.unwrap_or(false),
            minimized_reason: comment.minimized_reason.filter(|value| !value.is_empty()),
            thread_id: thread.id.clone(),
            thread_resolved: thread.is_resolved,
            thread_outdated: thread.is_outdated,
        });
    }
    entries
}

struct TimelineActivityPage {
    activity: Vec<ActivityEntry>,
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[cfg(test)]
fn timeline_activity(response: TimelineResponse) -> Vec<ActivityEntry> {
    timeline_activity_page(response)
        .map(|page| page.activity)
        .unwrap_or_default()
}

fn timeline_activity_page(response: TimelineResponse) -> Option<TimelineActivityPage> {
    let repository = response.data.repository?;
    let resource = repository.pull_request.or(repository.issue)?;
    let page_info = resource.timeline_items.page_info;
    let activity = resource
        .timeline_items
        .nodes
        .into_iter()
        .enumerate()
        .map(|(index, node)| timeline_node_to_activity(index, &node))
        .collect();
    Some(TimelineActivityPage {
        activity,
        has_next_page: page_info.has_next_page,
        end_cursor: page_info.end_cursor,
    })
}

fn timeline_node_to_activity(index: usize, node: &Value) -> ActivityEntry {
    let typename = string_field(node, "__typename").unwrap_or("TimelineEvent");
    let body = timeline_body(typename, node);
    ActivityEntry {
        id: string_field(node, "id")
            .map(str::to_string)
            .unwrap_or_else(|| format!("timeline-{index}")),
        kind: ActivityKind::Timeline,
        author: actor_login(node).unwrap_or_else(|| "github".to_string()),
        body,
        updated_at: string_field(node, "createdAt")
            .map(str::to_string)
            .unwrap_or_else(|| "unknown".to_string()),
        path: None,
        line: None,
        url: cross_reference_url(node),
        author_association: None,
        reactions: ReactionCounts::default(),
        includes_created_edit: false,
        is_minimized: false,
        minimized_reason: None,
        thread_id: None,
        thread_resolved: None,
        thread_outdated: None,
    }
}

fn timeline_body(typename: &str, node: &Value) -> String {
    match typename {
        "ClosedEvent" => match string_field_at(node, &["closer", "__typename"]) {
            Some(closer) => format!("closed by {closer}"),
            None => "closed".to_string(),
        },
        "ReopenedEvent" => "reopened".to_string(),
        "LabeledEvent" => format!(
            "added label {}",
            string_field_at(node, &["label", "name"]).unwrap_or("unknown")
        ),
        "UnlabeledEvent" => format!(
            "removed label {}",
            string_field_at(node, &["label", "name"]).unwrap_or("unknown")
        ),
        "AssignedEvent" => format!("assigned {}", assignee_name(node)),
        "UnassignedEvent" => format!("unassigned {}", assignee_name(node)),
        "PinnedEvent" => "pinned".to_string(),
        "UnpinnedEvent" => "unpinned".to_string(),
        "LockedEvent" => format!(
            "locked{}",
            string_field(node, "lockReason")
                .map(|reason| format!(": {}", format_github_state(reason)))
                .unwrap_or_default()
        ),
        "UnlockedEvent" => "unlocked".to_string(),
        "SubscribedEvent" => "subscribed".to_string(),
        "UnsubscribedEvent" => "unsubscribed".to_string(),
        "MentionedEvent" => "mentioned".to_string(),
        "CommentDeletedEvent" => "deleted a comment".to_string(),
        "TransferredEvent" => format!(
            "transferred from {}",
            string_field_at(node, &["fromRepository", "nameWithOwner"]).unwrap_or("unknown")
        ),
        "MarkedAsDuplicateEvent" => format!(
            "marked duplicate of {}",
            resource_reference_label(node.get("canonical").unwrap_or(&Value::Null))
        ),
        "UnmarkedAsDuplicateEvent" => format!(
            "unmarked duplicate of {}",
            resource_reference_label(node.get("canonical").unwrap_or(&Value::Null))
        ),
        "ConnectedEvent" => format!(
            "connected {} to {}",
            resource_reference_label(node.get("source").unwrap_or(&Value::Null)),
            resource_reference_label(node.get("subject").unwrap_or(&Value::Null))
        ),
        "DisconnectedEvent" => format!(
            "disconnected {} from {}",
            resource_reference_label(node.get("source").unwrap_or(&Value::Null)),
            resource_reference_label(node.get("subject").unwrap_or(&Value::Null))
        ),
        "ReferencedEvent" => format!(
            "referenced commit {}",
            string_field_at(node, &["commit", "oid"])
                .map(|oid| oid.chars().take(12).collect::<String>())
                .unwrap_or_else(|| "unknown".to_string())
        ),
        "CrossReferencedEvent" => cross_reference_body(node),
        "RenamedTitleEvent" => format!(
            "renamed title from \"{}\" to \"{}\"",
            string_field(node, "previousTitle").unwrap_or("unknown"),
            string_field(node, "currentTitle").unwrap_or("unknown")
        ),
        "MilestonedEvent" => format!(
            "added milestone {}",
            string_field(node, "milestoneTitle").unwrap_or("unknown")
        ),
        "DemilestonedEvent" => format!(
            "removed milestone {}",
            string_field(node, "milestoneTitle").unwrap_or("unknown")
        ),
        "ConvertedToDiscussionEvent" => format!(
            "converted to discussion {}",
            string_field_at(node, &["discussion", "title"]).unwrap_or("unknown")
        ),
        "IssueCommentPinnedEvent" => "pinned a comment".to_string(),
        "IssueCommentUnpinnedEvent" => "unpinned a comment".to_string(),
        "IssueTypeAddedEvent" => {
            format!("added issue type {}", issue_type_name(node, "issueType"))
        }
        "IssueTypeRemovedEvent" => {
            format!("removed issue type {}", issue_type_name(node, "issueType"))
        }
        "IssueTypeChangedEvent" => format!(
            "changed issue type from {} to {}",
            issue_type_name(node, "prevIssueType"),
            issue_type_name(node, "issueType")
        ),
        "SubIssueAddedEvent" => format!(
            "added sub-issue {}",
            resource_reference_label(node.get("subIssue").unwrap_or(&Value::Null))
        ),
        "SubIssueRemovedEvent" => format!(
            "removed sub-issue {}",
            resource_reference_label(node.get("subIssue").unwrap_or(&Value::Null))
        ),
        "ParentIssueAddedEvent" => format!(
            "added parent issue {}",
            resource_reference_label(node.get("parent").unwrap_or(&Value::Null))
        ),
        "ParentIssueRemovedEvent" => format!(
            "removed parent issue {}",
            resource_reference_label(node.get("parent").unwrap_or(&Value::Null))
        ),
        "BlockedByAddedEvent" => format!(
            "blocked by {}",
            resource_reference_label(node.get("blockingIssue").unwrap_or(&Value::Null))
        ),
        "BlockedByRemovedEvent" => format!(
            "removed blocking issue {}",
            resource_reference_label(node.get("blockingIssue").unwrap_or(&Value::Null))
        ),
        "BlockingAddedEvent" => format!(
            "blocking {}",
            resource_reference_label(node.get("blockedIssue").unwrap_or(&Value::Null))
        ),
        "BlockingRemovedEvent" => format!(
            "removed blocked issue {}",
            resource_reference_label(node.get("blockedIssue").unwrap_or(&Value::Null))
        ),
        "MergedEvent" => {
            let commit = short_oid_at(node, &["commit", "oid"]);
            let branch = string_field(node, "mergeRefName").unwrap_or("base branch");
            format!("merged into {branch} at {commit}")
        }
        "ReviewRequestedEvent" => {
            format!("requested review from {}", requested_reviewer_name(node))
        }
        "ReviewRequestRemovedEvent" => {
            format!(
                "removed review request from {}",
                requested_reviewer_name(node)
            )
        }
        "ReadyForReviewEvent" => "marked ready for review".to_string(),
        "ConvertToDraftEvent" => "converted to draft".to_string(),
        "BaseRefChangedEvent" => format!(
            "changed base from {} to {}",
            string_field(node, "previousRefName").unwrap_or("unknown"),
            string_field(node, "currentRefName").unwrap_or("unknown")
        ),
        "BaseRefForcePushedEvent" => ref_force_push_body("force-pushed base", node),
        "BaseRefDeletedEvent" => format!(
            "deleted base branch {}",
            string_field(node, "baseRefName").unwrap_or("unknown")
        ),
        "HeadRefForcePushedEvent" => ref_force_push_body("force-pushed head", node),
        "HeadRefDeletedEvent" => format!(
            "deleted head branch {}",
            string_field(node, "headRefName").unwrap_or("unknown")
        ),
        "HeadRefRestoredEvent" => "restored head branch".to_string(),
        "ReviewDismissedEvent" => format!(
            "dismissed {} review{}",
            format_github_state(string_field(node, "previousReviewState").unwrap_or("review")),
            string_field(node, "dismissalMessage")
                .filter(|message| !message.trim().is_empty())
                .map(|message| format!(": {message}"))
                .unwrap_or_default()
        ),
        "AddedToMergeQueueEvent" => format!(
            "added to merge queue{}",
            string_field_at(node, &["enqueuer", "login"])
                .map(|login| format!(" by {login}"))
                .unwrap_or_default()
        ),
        "RemovedFromMergeQueueEvent" => format!(
            "removed from merge queue{}{}",
            string_field_at(node, &["beforeCommit", "oid"])
                .map(|_| format!(" at {}", short_oid_at(node, &["beforeCommit", "oid"])))
                .unwrap_or_default(),
            string_field(node, "reason")
                .map(|reason| format!(": {reason}"))
                .unwrap_or_default()
        ),
        "AutomaticBaseChangeFailedEvent" => {
            automatic_base_change_body("failed to change base", node)
        }
        "AutomaticBaseChangeSucceededEvent" => {
            automatic_base_change_body("automatically changed base", node)
        }
        "AutoRebaseEnabledEvent" => format!(
            "enabled auto-rebase{}",
            string_field_at(node, &["enabler", "login"])
                .map(|login| format!(" by {login}"))
                .unwrap_or_default()
        ),
        "AutoSquashEnabledEvent" => format!(
            "enabled auto-squash{}",
            string_field_at(node, &["enabler", "login"])
                .map(|login| format!(" by {login}"))
                .unwrap_or_default()
        ),
        "AutoMergeEnabledEvent" => "enabled auto-merge".to_string(),
        "AutoMergeDisabledEvent" => format!(
            "disabled auto-merge{}",
            string_field(node, "reason")
                .map(|reason| format!(": {reason}"))
                .unwrap_or_default()
        ),
        other => format!("{other} event"),
    }
}

fn cross_reference_body(node: &Value) -> String {
    let source = node.get("source").unwrap_or(&Value::Null);
    let label = resource_reference_label(source);
    let title = string_field(source, "title").unwrap_or("untitled");
    let url = string_field(source, "url");
    match url {
        Some(url) => format!("cross-referenced by {label}: {title}\n{url}"),
        None => format!("cross-referenced by {label}: {title}"),
    }
}

fn resource_reference_label(value: &Value) -> String {
    let number = value.get("number").and_then(Value::as_u64);
    let title = string_field(value, "title").unwrap_or("untitled");
    let repo = string_field_at(value, &["repository", "nameWithOwner"]);
    match (repo, number) {
        (Some(repo), Some(number)) => format!("{repo}#{number}"),
        (_, Some(number)) => format!("#{number}"),
        _ => title.to_string(),
    }
}

fn issue_type_name(node: &Value, key: &str) -> String {
    string_field_at(node, &[key, "name"])
        .unwrap_or("unknown")
        .to_string()
}

fn ref_force_push_body(prefix: &str, node: &Value) -> String {
    let ref_name = string_field_at(node, &["ref", "name"]).unwrap_or("branch");
    let before = short_oid_at(node, &["beforeCommit", "oid"]);
    let after = short_oid_at(node, &["afterCommit", "oid"]);
    format!("{prefix} {ref_name} from {before} to {after}")
}

fn automatic_base_change_body(prefix: &str, node: &Value) -> String {
    format!(
        "{prefix} from {} to {}",
        string_field(node, "oldBase").unwrap_or("unknown"),
        string_field(node, "newBase").unwrap_or("unknown")
    )
}

fn short_oid_at(node: &Value, path: &[&str]) -> String {
    string_field_at(node, path)
        .map(short_oid)
        .unwrap_or_else(|| "unknown".to_string())
}

fn short_oid(oid: &str) -> String {
    oid.chars().take(12).collect()
}

fn cross_reference_url(node: &Value) -> Option<String> {
    let paths: &[&[&str]] = &[
        &["source", "url"],
        &["canonical", "url"],
        &["duplicate", "url"],
        &["subject", "url"],
        &["subIssue", "url"],
        &["parent", "url"],
        &["blockingIssue", "url"],
        &["blockedIssue", "url"],
        &["discussion", "url"],
        &["issueComment", "url"],
        &["url"],
    ];
    paths
        .iter()
        .find_map(|path| string_field_at(node, path).map(str::to_string))
}

fn actor_login(node: &Value) -> Option<String> {
    string_field_at(node, &["actor", "login"]).map(str::to_string)
}

fn assignee_name(node: &Value) -> String {
    string_field_at(node, &["assignee", "login"])
        .or_else(|| string_field_at(node, &["assignee", "__typename"]))
        .unwrap_or("unknown")
        .to_string()
}

fn requested_reviewer_name(node: &Value) -> String {
    string_field_at(node, &["requestedReviewer", "login"])
        .or_else(|| string_field_at(node, &["requestedReviewer", "slug"]))
        .or_else(|| string_field_at(node, &["requestedReviewer", "name"]))
        .or_else(|| string_field_at(node, &["requestedReviewer", "__typename"]))
        .unwrap_or("unknown")
        .to_string()
}

fn format_github_state(value: &str) -> String {
    value
        .split('_')
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            let mut chars = lower.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn string_field_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

fn pr_activity(comments: Vec<CommentDto>, reviews: Vec<ReviewDto>) -> Vec<ActivityEntry> {
    let mut activity = comments_to_activity(comments);
    activity.extend(reviews.into_iter().enumerate().map(|(index, review)| {
        let state = review.state.unwrap_or_else(|| "REVIEW".to_string());
        let body = review.body.unwrap_or_default();
        let body = if body.trim().is_empty() {
            state.clone()
        } else {
            format!("{state}: {body}")
        };
        ActivityEntry {
            id: review.id.unwrap_or_else(|| format!("review-{index}")),
            kind: ActivityKind::Review,
            author: display_author(review.author),
            body,
            updated_at: review
                .updated_at
                .or(review.submitted_at)
                .unwrap_or_else(|| "unknown".to_string()),
            path: None,
            line: None,
            url: review.url,
            author_association: review.author_association,
            reactions: reaction_counts(review.reaction_groups),
            includes_created_edit: false,
            is_minimized: false,
            minimized_reason: None,
            thread_id: None,
            thread_resolved: None,
            thread_outdated: None,
        }
    }));
    sort_activity(&mut activity);
    activity
}

fn sort_activity(activity: &mut [ActivityEntry]) {
    activity.sort_by(|left, right| {
        left.updated_at
            .cmp(&right.updated_at)
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn commit_from_dto(commit: CommitDto) -> Commit {
    let authors = commit
        .authors
        .unwrap_or_default()
        .into_iter()
        .map(|author| author.display_name())
        .filter(|name| name != "unknown")
        .collect::<Vec<_>>();
    let author = authors
        .first()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let authored_at = commit.authored_date.filter(|value| !value.is_empty());
    Commit {
        oid: commit.oid,
        message: commit.message_headline,
        body: commit.message_body.unwrap_or_default(),
        author,
        authors,
        authored_at: authored_at.clone(),
        committed_at: commit
            .committed_date
            .or(authored_at)
            .unwrap_or_else(|| "unknown".to_string()),
        status: CheckStatus::Unknown,
        deployments: Vec::new(),
    }
}

fn check_from_dto(check: CheckDto) -> CheckRun {
    let status = classify_check(
        check.status.as_deref().or(check.state.as_deref()),
        check.conclusion.as_deref(),
    );
    let raw_status = check
        .status
        .or(check.state)
        .filter(|value| !value.is_empty());
    let raw_conclusion = check.conclusion.filter(|value| !value.is_empty());
    let name = match (check.workflow_name, check.name, check.context) {
        (Some(workflow), Some(name), _) if workflow != name => format!("{workflow}/{name}"),
        (Some(workflow), _, _) => workflow,
        (_, Some(name), _) => name,
        (_, _, Some(context)) => context,
        _ => "check".to_string(),
    };
    CheckRun {
        name,
        status,
        summary: None,
        details_url: check
            .details_url
            .or(check.target_url)
            .filter(|value| !value.is_empty()),
        started_at: check.started_at.filter(|value| !value.is_empty()),
        completed_at: check.completed_at.filter(|value| !value.is_empty()),
        raw_status,
        raw_conclusion,
    }
}

fn classify_check(status: Option<&str>, conclusion: Option<&str>) -> CheckStatus {
    match (status, conclusion) {
        (Some("COMPLETED"), Some("SUCCESS")) | (Some("SUCCESS"), _) => CheckStatus::Success,
        (Some("COMPLETED"), Some("FAILURE" | "TIMED_OUT" | "STARTUP_FAILURE")) => {
            CheckStatus::Failure
        }
        (Some("COMPLETED"), Some("ACTION_REQUIRED")) => CheckStatus::Pending,
        (Some("COMPLETED"), Some("SKIPPED" | "CANCELLED" | "STALE")) => CheckStatus::Skipped,
        (Some("COMPLETED"), Some("NEUTRAL")) => CheckStatus::Neutral,
        (Some("COMPLETED"), _) => CheckStatus::Unknown,
        (Some("ERROR" | "FAILURE"), _) => CheckStatus::Failure,
        (Some("EXPECTED"), _) => CheckStatus::Pending,
        (Some("QUEUED" | "IN_PROGRESS" | "PENDING" | "WAITING" | "REQUESTED"), _) => {
            CheckStatus::Pending
        }
        _ => CheckStatus::Unknown,
    }
}

fn file_from_dto(file: FileDto) -> ChangedFile {
    ChangedFile {
        path: file.path,
        additions: file.additions,
        deletions: file.deletions,
        change_type: file.change_type.unwrap_or_else(|| "MODIFIED".to_string()),
        patch: None,
    }
}

fn parse_unified_diff_patches(diff: &str) -> HashMap<String, String> {
    let mut patches = HashMap::new();
    let mut current_path: Option<String> = None;
    let mut current_lines = Vec::new();

    for line in diff.lines() {
        if let Some(path) = parse_diff_header_path(line) {
            if let Some(path) = current_path.take() {
                if !current_lines.is_empty() {
                    patches.insert(path, current_lines.join("\n"));
                }
            }
            current_path = Some(path);
            current_lines.clear();
            current_lines.push(line.to_string());
            continue;
        }

        if current_path.is_some() {
            current_lines.push(line.to_string());
        }
    }

    if let Some(path) = current_path {
        if !current_lines.is_empty() {
            patches.insert(path, current_lines.join("\n"));
        }
    }

    patches
}

fn parse_diff_header_path(line: &str) -> Option<String> {
    let rest = line.strip_prefix("diff --git ")?;
    let (_left, right) = rest.split_once(' ')?;
    right
        .strip_prefix("b/")
        .or_else(|| right.strip_prefix("a/"))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Debug)]
    struct FakeGithubHttpTransport {
        requests: Mutex<Vec<GithubHttpRequest>>,
        response: Mutex<Option<GithubHttpResponse>>,
    }

    impl FakeGithubHttpTransport {
        fn new(response: GithubHttpResponse) -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
                response: Mutex::new(Some(response)),
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
                self.response
                    .lock()
                    .expect("response lock")
                    .take()
                    .context("fake response already used")
            })
        }
    }

    #[test]
    fn pr_command_preview_uses_direct_graphql_shape() {
        let id = ResourceId::from_owner_repo_number("openclaw/openclaw", "81834").unwrap();

        assert_eq!(
            command_preview_for_pr(&id),
            vec![
                "POST",
                "https://api.github.com/graphql",
                "repository=openclaw/openclaw",
                "number=81834",
                "selector=pullRequest",
            ]
        );
    }

    #[tokio::test]
    async fn graphql_transport_receives_post_shape_and_returns_body() {
        let transport = FakeGithubHttpTransport::new(GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: br#"{"data":{"ok":true}}"#.to_vec(),
        });

        let output = run_graphql_query_with(
            &transport,
            "token-1",
            "query Example { viewer { login } }",
            json!({"owner": "openclaw", "name": "openclaw"}),
        )
        .await
        .expect("GraphQL response");

        assert_eq!(output, br#"{"data":{"ok":true}}"#);
        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.method, GithubHttpMethod::Post);
        assert_eq!(request.url, GITHUB_GRAPHQL_URL);
        assert_eq!(request.accept, GITHUB_JSON_ACCEPT);
        assert_eq!(request.token, "token-1");
        assert_eq!(
            request.body,
            Some(json!({
                "query": "query Example { viewer { login } }",
                "variables": {"owner": "openclaw", "name": "openclaw"},
            }))
        );
    }

    #[tokio::test]
    async fn graphql_transport_errors_on_graphql_errors_payload() {
        let transport = FakeGithubHttpTransport::new(GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: br#"{"errors":[{"message":"bad query"}]}"#.to_vec(),
        });

        let error = run_graphql_query_with(&transport, "token-1", "query", json!({}))
            .await
            .expect_err("GraphQL errors should fail");

        assert!(error
            .to_string()
            .contains("GitHub GraphQL request returned errors"));
    }

    #[tokio::test]
    async fn rest_transport_receives_get_shape_and_returns_body() {
        let transport = FakeGithubHttpTransport::new(GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: b"diff --git a/file b/file".to_vec(),
        });

        let output = run_rest_get_with(
            &transport,
            "token-1",
            "/repos/openclaw/openclaw/pulls/81834",
            "application/vnd.github.v3.diff",
        )
        .await
        .expect("REST response");

        assert_eq!(output, b"diff --git a/file b/file");
        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.method, GithubHttpMethod::Get);
        assert_eq!(
            request.url,
            "https://api.github.com/repos/openclaw/openclaw/pulls/81834"
        );
        assert_eq!(request.accept, "application/vnd.github.v3.diff");
        assert_eq!(request.token, "token-1");
        assert_eq!(request.body, None);
    }

    #[tokio::test]
    async fn rest_transport_includes_status_and_body_on_http_failure() {
        let transport = FakeGithubHttpTransport::new(GithubHttpResponse {
            status: reqwest::StatusCode::NOT_FOUND,
            body: br#"{"message":"Not Found"}"#.to_vec(),
        });

        let error = run_rest_get_with(&transport, "token-1", "/missing", GITHUB_JSON_ACCEPT)
            .await
            .expect_err("HTTP failure should fail");

        let message = error.to_string();
        assert!(message.contains("GitHub REST request to /missing failed with HTTP 404"));
        assert!(message.contains("Not Found"));
    }

    #[test]
    fn pr_diff_uses_rest_pull_diff_path() {
        let id = ResourceId::from_owner_repo_number("openclaw/openclaw", "81834").unwrap();

        assert_eq!(
            pr_diff_rest_path(&id),
            "/repos/openclaw/openclaw/pulls/81834"
        );
    }

    #[test]
    fn timeline_query_keeps_pr_only_events_out_of_issue_queries() {
        let issue_query = timeline_query(ResourceKind::Issue);
        let pr_query = timeline_query(ResourceKind::PullRequest);

        assert!(issue_query.contains("issue(number: $number)"));
        assert!(issue_query.contains("CLOSED_EVENT"));
        assert!(issue_query.contains("LOCKED_EVENT"));
        assert!(issue_query.contains("MARKED_AS_DUPLICATE_EVENT"));
        assert!(!issue_query.contains("MERGED_EVENT"));
        assert!(!issue_query.contains("ReviewRequestedEvent"));
        assert!(pr_query.contains("pullRequest(number: $number)"));
        assert!(pr_query.contains("LOCKED_EVENT"));
        assert!(pr_query.contains("MARKED_AS_DUPLICATE_EVENT"));
        assert!(pr_query.contains("MERGED_EVENT"));
        assert!(pr_query.contains("ReviewRequestedEvent"));
        assert!(pr_query.contains("BASE_REF_CHANGED_EVENT"));
        assert!(pr_query.contains("HEAD_REF_FORCE_PUSHED_EVENT"));
        assert!(pr_query.contains("REVIEW_DISMISSED_EVENT"));
        assert!(pr_query.contains("ADDED_TO_MERGE_QUEUE_EVENT"));
        assert!(issue_query.contains("ISSUE_TYPE_CHANGED_EVENT"));
        assert!(issue_query.contains("SUB_ISSUE_ADDED_EVENT"));
        assert!(issue_query.contains("BLOCKED_BY_ADDED_EVENT"));
        assert!(issue_query.contains("ConvertedToDiscussionEvent"));
    }

    #[test]
    fn comments_query_uses_selector_and_pagination_state() {
        let issue_query = comments_query(ResourceKind::Issue);
        let pr_query = comments_query(ResourceKind::PullRequest);

        assert!(issue_query.contains("issue(number: $number)"));
        assert!(pr_query.contains("pullRequest(number: $number)"));
        for query in [issue_query, pr_query] {
            assert!(query.contains("$after: String"));
            assert!(query.contains("comments(first: 100, after: $after)"));
            assert!(query.contains("pageInfo"));
            assert!(query.contains("hasNextPage"));
            assert!(query.contains("endCursor"));
            assert!(query.contains("reactionGroups"));
        }
    }

    #[test]
    fn project_items_query_uses_selector_and_pagination_state() {
        let issue_query = project_items_query(ResourceKind::Issue);
        let pr_query = project_items_query(ResourceKind::PullRequest);

        assert!(issue_query.contains("issue(number: $number)"));
        assert!(pr_query.contains("pullRequest(number: $number)"));
        for query in [issue_query, pr_query] {
            assert!(query.contains("$after: String"));
            assert!(query.contains("projectItems(first: 100, after: $after)"));
            assert!(query.contains("pageInfo"));
            assert!(query.contains("hasNextPage"));
            assert!(query.contains("endCursor"));
            assert!(query.contains("project"));
            assert!(query.contains("title"));
        }
    }

    #[test]
    fn project_items_page_preserves_pagination_state() {
        let page = project_items_page(ProjectItemsResponse {
            data: ProjectItemsData {
                repository: Some(ProjectItemsRepository {
                    issue: None,
                    pull_request: Some(ProjectItemsResource {
                        project_items: ProjectItemsConnection {
                            page_info: PageInfoDto {
                                has_next_page: true,
                                end_cursor: Some("project-cursor-2".into()),
                            },
                            nodes: vec![
                                ProjectItemDto {
                                    project: Some(ProjectDto {
                                        title: Some("Roadmap".into()),
                                    }),
                                },
                                ProjectItemDto {
                                    project: Some(ProjectDto {
                                        title: Some(" ".into()),
                                    }),
                                },
                                ProjectItemDto { project: None },
                            ],
                        },
                    }),
                }),
            },
        })
        .expect("project items page");

        assert!(page.has_next_page);
        assert_eq!(page.end_cursor.as_deref(), Some("project-cursor-2"));
        assert_eq!(page.projects, vec!["Roadmap"]);
    }

    #[test]
    fn apply_project_metadata_replaces_existing_value_and_dedupes() {
        let mut metadata = vec![
            MetadataItem {
                label: "Closed".into(),
                value: "no".into(),
            },
            MetadataItem {
                label: "Projects".into(),
                value: "Old".into(),
            },
        ];

        apply_project_metadata(
            &mut metadata,
            vec![" Roadmap ".into(), "Roadmap".into(), "Triage".into()],
        );

        assert_eq!(metadata.len(), 2);
        assert_eq!(metadata[1].label, "Projects");
        assert_eq!(metadata[1].value, "Roadmap, Triage");
    }

    #[test]
    fn project_scope_errors_are_suppressed_for_optional_metadata() {
        let error = anyhow::anyhow!(
            "GitHub GraphQL request returned errors: INSUFFICIENT_SCOPES read:project"
        );

        assert!(is_project_scope_error(&error));
    }

    #[test]
    fn check_suites_query_requests_pagination_state() {
        let query = check_suites_query();

        assert!(query.contains("$after: String"));
        assert!(query.contains("checkSuites(first: 100, after: $after)"));
        assert!(query.contains("pageInfo"));
        assert!(query.contains("hasNextPage"));
        assert!(query.contains("endCursor"));
    }

    #[test]
    fn status_rollup_query_requests_pagination_state_and_context_fields() {
        let query = status_rollup_query();

        assert!(query.contains("$after: String"));
        assert!(query.contains("statusCheckRollup"));
        assert!(query.contains("contexts(first: 100, after: $after)"));
        assert!(query.contains("pageInfo"));
        assert!(query.contains("hasNextPage"));
        assert!(query.contains("endCursor"));
        assert!(query.contains("... on CheckRun"));
        assert!(query.contains("detailsUrl"));
        assert!(query.contains("checkSuite { workflowRun { workflow { name } } }"));
        assert!(query.contains("... on StatusContext"));
        assert!(query.contains("targetUrl"));
    }

    #[test]
    fn commits_query_requests_pagination_state_and_full_commit_fields() {
        let query = commits_query();

        assert!(query.contains("$after: String"));
        assert!(query.contains("commits(first: 100, after: $after)"));
        assert!(query.contains("pageInfo"));
        assert!(query.contains("hasNextPage"));
        assert!(query.contains("endCursor"));
        assert!(query.contains("messageHeadline"));
        assert!(query.contains("messageBody"));
        assert!(query.contains("committedDate"));
        assert!(query.contains("authoredDate"));
        assert!(query.contains("authors(first: 100)"));
    }

    #[test]
    fn review_threads_query_requests_pagination_state() {
        let query = review_threads_query();

        assert!(query.contains("$after: String"));
        assert!(query.contains("reviewThreads(first: 100, after: $after)"));
        assert!(query.contains("comments(first: 100)"));
        assert!(query.contains("pageInfo"));
        assert!(query.contains("hasNextPage"));
        assert!(query.contains("endCursor"));
    }

    #[test]
    fn comment_activity_page_preserves_pagination_state() {
        let page = comment_activity_page(CommentsResponse {
            data: CommentsData {
                repository: Some(CommentsRepository {
                    issue: Some(CommentsResource {
                        comments: CommentsConnection {
                            page_info: PageInfoDto {
                                has_next_page: true,
                                end_cursor: Some("comment-cursor-2".into()),
                            },
                            nodes: vec![CommentDto {
                                id: Some("comment-1".into()),
                                author: Some(UserDto {
                                    login: Some("alice".into()),
                                    name: None,
                                }),
                                author_association: Some("MEMBER".into()),
                                body: "hello".into(),
                                created_at: Some("2026-01-01T00:00:00Z".into()),
                                updated_at: None,
                                url: Some(
                                    "https://github.com/openclaw/openclaw/issues/1#issuecomment-1"
                                        .into(),
                                ),
                                includes_created_edit: Some(true),
                                is_minimized: Some(false),
                                minimized_reason: None,
                                reaction_groups: vec![ReactionGroupDto {
                                    content: "THUMBS_UP".into(),
                                    users: TotalCountDto { total_count: 2 },
                                }],
                            }],
                        },
                    }),
                    pull_request: None,
                }),
            },
        })
        .expect("comment page");

        assert!(page.has_next_page);
        assert_eq!(page.end_cursor.as_deref(), Some("comment-cursor-2"));
        assert_eq!(page.activity.len(), 1);
        assert_eq!(page.activity[0].kind, ActivityKind::Comment);
        assert_eq!(page.activity[0].author, "alice");
        assert_eq!(
            page.activity[0].author_association.as_deref(),
            Some("MEMBER")
        );
        assert_eq!(page.activity[0].reactions.thumbs_up, 2);
        assert!(page.activity[0].includes_created_edit);
    }

    #[test]
    fn replace_comment_activity_keeps_other_activity() {
        let mut resource = Resource {
            id: ResourceId::from_owner_repo_number("openclaw/openclaw", "81834").unwrap(),
            title: "title".into(),
            url: "https://github.com/openclaw/openclaw/pull/81834".into(),
            state: "OPEN".into(),
            author: "alice".into(),
            created_at: "created".into(),
            updated_at: "updated".into(),
            labels: Vec::new(),
            assignees: Vec::new(),
            reactions: ReactionCounts::default(),
            body: "body".into(),
            activity: vec![
                ActivityEntry {
                    id: "old-comment".into(),
                    kind: ActivityKind::Comment,
                    author: "alice".into(),
                    body: "old".into(),
                    updated_at: "old".into(),
                    path: None,
                    line: None,
                    url: None,
                    author_association: None,
                    reactions: ReactionCounts::default(),
                    includes_created_edit: false,
                    is_minimized: false,
                    minimized_reason: None,
                    thread_id: None,
                    thread_resolved: None,
                    thread_outdated: None,
                },
                ActivityEntry {
                    id: "review".into(),
                    kind: ActivityKind::Review,
                    author: "bob".into(),
                    body: "approved".into(),
                    updated_at: "new".into(),
                    path: None,
                    line: None,
                    url: None,
                    author_association: None,
                    reactions: ReactionCounts::default(),
                    includes_created_edit: false,
                    is_minimized: false,
                    minimized_reason: None,
                    thread_id: None,
                    thread_resolved: None,
                    thread_outdated: None,
                },
            ],
            related_resources: Vec::new(),
            metadata: Vec::new(),
            warnings: Vec::new(),
            pull_request: None,
        };

        replace_comment_activity(
            &mut resource,
            vec![ActivityEntry {
                id: "new-comment".into(),
                kind: ActivityKind::Comment,
                author: "carol".into(),
                body: "new".into(),
                updated_at: "newer".into(),
                path: None,
                line: None,
                url: None,
                author_association: None,
                reactions: ReactionCounts::default(),
                includes_created_edit: false,
                is_minimized: false,
                minimized_reason: None,
                thread_id: None,
                thread_resolved: None,
                thread_outdated: None,
            }],
        );

        assert_eq!(resource.activity.len(), 2);
        assert!(resource.activity.iter().any(|entry| entry.id == "review"));
        assert!(resource
            .activity
            .iter()
            .any(|entry| entry.id == "new-comment"));
        assert!(!resource
            .activity
            .iter()
            .any(|entry| entry.id == "old-comment"));
    }

    #[test]
    fn review_thread_comments_query_requests_comment_pagination_state() {
        let query = review_thread_comments_query();

        assert!(query.contains("$threadId: ID!"));
        assert!(query.contains("node(id: $threadId)"));
        assert!(query.contains("comments(first: 100, after: $after)"));
        assert!(query.contains("pageInfo"));
        assert!(query.contains("hasNextPage"));
        assert!(query.contains("endCursor"));
    }

    #[test]
    fn enrichment_warning_records_label_and_error() {
        let mut resource = Resource {
            id: ResourceId::from_owner_repo_number("openclaw/openclaw", "81834").unwrap(),
            title: "title".into(),
            url: "https://github.com/openclaw/openclaw/pull/81834".into(),
            state: "OPEN".into(),
            author: "alice".into(),
            created_at: "created".into(),
            updated_at: "updated".into(),
            labels: Vec::new(),
            assignees: Vec::new(),
            reactions: ReactionCounts::default(),
            body: "body".into(),
            activity: Vec::new(),
            related_resources: Vec::new(),
            metadata: Vec::new(),
            warnings: Vec::new(),
            pull_request: None,
        };
        let error = anyhow::anyhow!("permission denied");

        push_enrichment_warning(&mut resource, "timeline unavailable", &error);

        assert_eq!(
            resource.warnings,
            vec!["timeline unavailable: permission denied"]
        );
    }

    #[test]
    fn classifies_cancelled_checks_as_skipped_not_failure() {
        assert_eq!(
            classify_check(Some("COMPLETED"), Some("CANCELLED")),
            CheckStatus::Skipped
        );
        assert_eq!(
            classify_check(Some("COMPLETED"), Some("ACTION_REQUIRED")),
            CheckStatus::Pending
        );
    }

    #[test]
    fn check_from_dto_preserves_github_metadata() {
        let check = check_from_dto(CheckDto {
            name: Some("unit".into()),
            context: None,
            workflow_name: Some("CI".into()),
            status: Some("COMPLETED".into()),
            state: None,
            conclusion: Some("FAILURE".into()),
            details_url: Some("https://github.com/openclaw/openclaw/actions/runs/1/job/2".into()),
            target_url: None,
            started_at: Some("2026-05-30T03:28:54Z".into()),
            completed_at: Some("2026-05-30T03:28:56Z".into()),
        });

        assert_eq!(check.name, "CI/unit");
        assert_eq!(check.status, CheckStatus::Failure);
        assert_eq!(check.raw_status.as_deref(), Some("COMPLETED"));
        assert_eq!(check.raw_conclusion.as_deref(), Some("FAILURE"));
        assert_eq!(
            check.details_url.as_deref(),
            Some("https://github.com/openclaw/openclaw/actions/runs/1/job/2")
        );
        assert_eq!(check.started_at.as_deref(), Some("2026-05-30T03:28:54Z"));
        assert_eq!(check.completed_at.as_deref(), Some("2026-05-30T03:28:56Z"));
    }

    #[test]
    fn check_from_dto_handles_status_context_fields() {
        let check = check_from_dto(CheckDto {
            name: None,
            context: Some("license/cla".into()),
            workflow_name: None,
            status: None,
            state: Some("SUCCESS".into()),
            conclusion: None,
            details_url: None,
            target_url: Some("https://ci.example.test/status/1".into()),
            started_at: None,
            completed_at: None,
        });

        assert_eq!(check.name, "license/cla");
        assert_eq!(check.status, CheckStatus::Success);
        assert_eq!(check.raw_status.as_deref(), Some("SUCCESS"));
        assert_eq!(
            check.details_url.as_deref(),
            Some("https://ci.example.test/status/1")
        );
    }

    #[test]
    fn commit_from_dto_preserves_body_dates_and_authors() {
        let commit = commit_from_dto(CommitDto {
            oid: "abcdef123".into(),
            message_headline: "feat: add thing".into(),
            message_body: Some("body\n\nCo-Authored-By: Friend <friend@example.com>".into()),
            committed_date: Some("2026-05-30T03:18:51Z".into()),
            authored_date: Some("2026-05-14T13:10:00Z".into()),
            authors: Some(vec![
                UserDto {
                    login: Some("alice".into()),
                    name: None,
                },
                UserDto {
                    login: Some("friend".into()),
                    name: None,
                },
            ]),
        });

        assert_eq!(commit.oid, "abcdef123");
        assert_eq!(commit.message, "feat: add thing");
        assert!(commit.body.contains("Co-Authored-By"));
        assert_eq!(commit.author, "alice");
        assert_eq!(commit.authors, vec!["alice", "friend"]);
        assert_eq!(commit.authored_at.as_deref(), Some("2026-05-14T13:10:00Z"));
        assert_eq!(commit.committed_at, "2026-05-30T03:18:51Z");
        assert!(commit.deployments.is_empty());
    }

    #[test]
    fn commits_page_preserves_pagination_state_and_author_fallbacks() {
        let page = commits_page(CommitsResponse {
            data: CommitsData {
                repository: Some(CommitsRepository {
                    pull_request: Some(CommitsPullRequest {
                        commits: CommitsConnection {
                            page_info: PageInfoDto {
                                has_next_page: true,
                                end_cursor: Some("commit-cursor-2".into()),
                            },
                            nodes: vec![CommitNodeDto {
                                commit: CommitWithAuthorsConnectionDto {
                                    oid: "abcdef123".into(),
                                    message_headline: "feat: add thing".into(),
                                    message_body: Some("body".into()),
                                    committed_date: Some("2026-05-30T03:18:51Z".into()),
                                    authored_date: Some("2026-05-14T13:10:00Z".into()),
                                    authors: Some(CommitAuthorsConnection {
                                        nodes: vec![
                                            CommitAuthorDto {
                                                name: Some("fallback-name".into()),
                                                user: None,
                                            },
                                            CommitAuthorDto {
                                                name: None,
                                                user: Some(UserDto {
                                                    login: Some("alice".into()),
                                                    name: None,
                                                }),
                                            },
                                        ],
                                    }),
                                },
                            }],
                        },
                    }),
                }),
            },
        })
        .expect("commits page");

        assert!(page.has_next_page);
        assert_eq!(page.end_cursor.as_deref(), Some("commit-cursor-2"));
        assert_eq!(page.commits.len(), 1);
        assert_eq!(page.commits[0].oid, "abcdef123");
        assert_eq!(page.commits[0].body, "body");
        assert_eq!(page.commits[0].authors, vec!["fallback-name", "alice"]);
    }

    #[test]
    fn commits_from_response_returns_empty_for_missing_pr() {
        let commits = commits_from_response(CommitsResponse {
            data: CommitsData { repository: None },
        });

        assert!(commits.is_empty());
    }

    #[test]
    fn commit_deployments_from_response_maps_environment_status_and_urls() {
        let response = CommitDeploymentsResponse {
            data: CommitDeploymentsData {
                repository: Some(CommitDeploymentsRepository {
                    pull_request: Some(CommitDeploymentsPullRequest {
                        commits: CommitDeploymentsConnection {
                            nodes: vec![CommitDeploymentNode {
                                commit: CommitDeploymentCommit {
                                    oid: "abcdef123".into(),
                                    deployments: DeploymentConnection {
                                        nodes: vec![DeploymentDto {
                                            environment: Some("preview".into()),
                                            task: Some("deploy".into()),
                                            description: None,
                                            created_at: Some("2026-05-30T03:20:00Z".into()),
                                            updated_at: Some("2026-05-30T03:21:00Z".into()),
                                            latest_status: Some(DeploymentStatusDto {
                                                state: Some("SUCCESS".into()),
                                                description: Some("Preview deployed".into()),
                                                environment_url: Some(
                                                    "https://example.test/preview".into(),
                                                ),
                                                log_url: Some("https://example.test/logs".into()),
                                                created_at: Some("2026-05-30T03:21:00Z".into()),
                                            }),
                                        }],
                                    },
                                },
                            }],
                        },
                    }),
                }),
            },
        };

        let deployments = commit_deployments_from_response(response);

        assert_eq!(deployments["abcdef123"][0].environment, "preview");
        assert_eq!(deployments["abcdef123"][0].state, "SUCCESS");
        assert_eq!(
            deployments["abcdef123"][0].description.as_deref(),
            Some("Preview deployed")
        );
        assert_eq!(
            deployments["abcdef123"][0].environment_url.as_deref(),
            Some("https://example.test/preview")
        );
        assert_eq!(
            deployments["abcdef123"][0].log_url.as_deref(),
            Some("https://example.test/logs")
        );
    }

    #[test]
    fn applies_commit_deployments_to_matching_commits() {
        let mut commits = vec![Commit {
            oid: "abcdef123".into(),
            message: "feat: add thing".into(),
            body: String::new(),
            author: "alice".into(),
            authors: vec!["alice".into()],
            authored_at: None,
            committed_at: "2026-05-30T03:18:51Z".into(),
            status: CheckStatus::Unknown,
            deployments: Vec::new(),
        }];
        let mut deployments = HashMap::new();
        deployments.insert(
            "abcdef123".into(),
            vec![Deployment {
                environment: "preview".into(),
                state: "SUCCESS".into(),
                description: None,
                environment_url: None,
                log_url: None,
                created_at: None,
                updated_at: "2026-05-30T03:21:00Z".into(),
            }],
        );

        apply_commit_deployments(&mut commits, deployments);

        assert_eq!(commits[0].deployments[0].environment, "preview");
    }

    #[test]
    fn replace_pr_commits_keeps_base_commits_when_paginated_list_is_empty() {
        let mut pr = PullRequest {
            base_ref: "main".into(),
            head_ref: "feature".into(),
            requested_reviewers: Vec::new(),
            review_decision: None,
            merge_state: None,
            additions: 0,
            deletions: 0,
            commits: vec![Commit {
                oid: "base".into(),
                message: "base commit".into(),
                body: String::new(),
                author: "alice".into(),
                authors: vec!["alice".into()],
                authored_at: None,
                committed_at: "2026-05-30T03:18:51Z".into(),
                status: CheckStatus::Unknown,
                deployments: Vec::new(),
            }],
            checks: Vec::new(),
            files: Vec::new(),
            metadata: Vec::new(),
        };

        replace_pr_commits(&mut pr, Vec::new());

        assert_eq!(pr.commits.len(), 1);
        assert_eq!(pr.commits[0].oid, "base");

        replace_pr_commits(
            &mut pr,
            vec![Commit {
                oid: "full".into(),
                message: "full commit".into(),
                body: String::new(),
                author: "bob".into(),
                authors: vec!["bob".into()],
                authored_at: None,
                committed_at: "2026-05-30T03:19:51Z".into(),
                status: CheckStatus::Unknown,
                deployments: Vec::new(),
            }],
        );

        assert_eq!(pr.commits.len(), 1);
        assert_eq!(pr.commits[0].oid, "full");
    }

    #[test]
    fn check_suite_from_dto_maps_workflow_status_and_urls() {
        let check = check_suite_from_dto(CheckSuiteDto {
            status: Some("COMPLETED".into()),
            conclusion: Some("ACTION_REQUIRED".into()),
            url: Some("https://github.com/openclaw/openclaw/commit/abc/checks".into()),
            app: Some(CheckSuiteAppDto {
                name: Some("GitHub Actions".into()),
            }),
            workflow_run: Some(CheckSuiteWorkflowRunDto {
                url: Some("https://github.com/openclaw/openclaw/actions/runs/1".into()),
                workflow: Some(CheckSuiteWorkflowDto {
                    name: Some("CI".into()),
                }),
            }),
        });

        assert_eq!(check.name, "suite/CI");
        assert_eq!(check.status, CheckStatus::Pending);
        assert_eq!(check.raw_status.as_deref(), Some("COMPLETED"));
        assert_eq!(check.raw_conclusion.as_deref(), Some("ACTION_REQUIRED"));
        assert_eq!(
            check.details_url.as_deref(),
            Some("https://github.com/openclaw/openclaw/actions/runs/1")
        );
        assert_eq!(
            check.summary.as_deref(),
            Some("check suite from GitHub Actions")
        );
    }

    #[test]
    fn check_suites_from_response_keeps_latest_suite_by_name() {
        let response = CheckSuitesResponse {
            data: CheckSuitesData {
                repository: Some(CheckSuitesRepository {
                    pull_request: Some(CheckSuitesPullRequest {
                        commits: CheckSuitesCommitConnection {
                            nodes: vec![CheckSuitesCommitNode {
                                commit: CheckSuitesCommit {
                                    check_suites: CheckSuiteConnection {
                                        page_info: PageInfoDto {
                                            has_next_page: false,
                                            end_cursor: None,
                                        },
                                        nodes: vec![
                                            CheckSuiteDto {
                                                status: Some("IN_PROGRESS".into()),
                                                conclusion: None,
                                                url: Some("https://example.test/old".into()),
                                                app: Some(CheckSuiteAppDto {
                                                    name: Some("GitHub Actions".into()),
                                                }),
                                                workflow_run: Some(CheckSuiteWorkflowRunDto {
                                                    url: Some(
                                                        "https://example.test/run-old".into(),
                                                    ),
                                                    workflow: Some(CheckSuiteWorkflowDto {
                                                        name: Some("CI".into()),
                                                    }),
                                                }),
                                            },
                                            CheckSuiteDto {
                                                status: Some("COMPLETED".into()),
                                                conclusion: Some("SUCCESS".into()),
                                                url: Some("https://example.test/new".into()),
                                                app: Some(CheckSuiteAppDto {
                                                    name: Some("GitHub Actions".into()),
                                                }),
                                                workflow_run: Some(CheckSuiteWorkflowRunDto {
                                                    url: Some(
                                                        "https://example.test/run-new".into(),
                                                    ),
                                                    workflow: Some(CheckSuiteWorkflowDto {
                                                        name: Some("CI".into()),
                                                    }),
                                                }),
                                            },
                                        ],
                                    },
                                },
                            }],
                        },
                    }),
                }),
            },
        };

        let suites = check_suites_from_response(response);

        assert_eq!(suites.len(), 1);
        assert_eq!(suites[0].name, "suite/CI");
        assert_eq!(suites[0].status, CheckStatus::Success);
        assert_eq!(
            suites[0].details_url.as_deref(),
            Some("https://example.test/run-new")
        );
    }

    #[test]
    fn check_suites_page_preserves_pagination_state() {
        let page = check_suites_page(CheckSuitesResponse {
            data: CheckSuitesData {
                repository: Some(CheckSuitesRepository {
                    pull_request: Some(CheckSuitesPullRequest {
                        commits: CheckSuitesCommitConnection {
                            nodes: vec![CheckSuitesCommitNode {
                                commit: CheckSuitesCommit {
                                    check_suites: CheckSuiteConnection {
                                        page_info: PageInfoDto {
                                            has_next_page: true,
                                            end_cursor: Some("suite-cursor-2".into()),
                                        },
                                        nodes: vec![CheckSuiteDto {
                                            status: Some("IN_PROGRESS".into()),
                                            conclusion: None,
                                            url: Some("https://example.test/suite".into()),
                                            app: Some(CheckSuiteAppDto {
                                                name: Some("GitHub Actions".into()),
                                            }),
                                            workflow_run: Some(CheckSuiteWorkflowRunDto {
                                                url: Some("https://example.test/run".into()),
                                                workflow: Some(CheckSuiteWorkflowDto {
                                                    name: Some("CI".into()),
                                                }),
                                            }),
                                        }],
                                    },
                                },
                            }],
                        },
                    }),
                }),
            },
        })
        .expect("check suites page");

        assert!(page.has_next_page);
        assert_eq!(page.end_cursor.as_deref(), Some("suite-cursor-2"));
        assert_eq!(page.checks.len(), 1);
        assert_eq!(page.checks[0].name, "suite/CI");
        assert_eq!(page.checks[0].status, CheckStatus::Pending);
    }

    #[test]
    fn status_rollup_page_preserves_pagination_state_and_context_types() {
        let page = status_rollup_page(StatusRollupResponse {
            data: StatusRollupData {
                repository: Some(StatusRollupRepository {
                    pull_request: Some(StatusRollupPullRequest {
                        status_check_rollup: Some(StatusCheckRollupDto {
                            contexts: StatusContextConnection {
                                page_info: PageInfoDto {
                                    has_next_page: true,
                                    end_cursor: Some("status-cursor-2".into()),
                                },
                                nodes: vec![
                                    json!({
                                        "__typename": "CheckRun",
                                        "name": "unit",
                                        "status": "COMPLETED",
                                        "conclusion": "SUCCESS",
                                        "detailsUrl": "https://example.test/check",
                                        "startedAt": "2026-05-30T03:18:51Z",
                                        "completedAt": "2026-05-30T03:19:51Z",
                                        "checkSuite": {
                                            "workflowRun": {
                                                "workflow": {"name": "CI"}
                                            }
                                        }
                                    }),
                                    json!({
                                        "__typename": "StatusContext",
                                        "context": "license/cla",
                                        "state": "PENDING",
                                        "targetUrl": "https://example.test/status"
                                    }),
                                ],
                            },
                        }),
                    }),
                }),
            },
        })
        .expect("status rollup page result")
        .expect("status rollup page");

        assert!(page.has_next_page);
        assert_eq!(page.end_cursor.as_deref(), Some("status-cursor-2"));
        assert_eq!(page.checks.len(), 2);
        assert_eq!(page.checks[0].name, "CI/unit");
        assert_eq!(page.checks[0].status, CheckStatus::Success);
        assert_eq!(
            page.checks[0].details_url.as_deref(),
            Some("https://example.test/check")
        );
        assert_eq!(page.checks[1].name, "license/cla");
        assert_eq!(page.checks[1].status, CheckStatus::Pending);
        assert_eq!(
            page.checks[1].details_url.as_deref(),
            Some("https://example.test/status")
        );
    }

    #[test]
    fn status_rollup_checks_from_response_handles_null_rollup() {
        let checks = status_rollup_checks_from_response(StatusRollupResponse {
            data: StatusRollupData {
                repository: Some(StatusRollupRepository {
                    pull_request: Some(StatusRollupPullRequest {
                        status_check_rollup: None,
                    }),
                }),
            },
        });

        assert!(checks.is_empty());
    }

    #[test]
    fn apply_check_suites_dedupes_existing_names() {
        let mut checks = vec![CheckRun {
            name: "suite/CI".into(),
            status: CheckStatus::Success,
            summary: None,
            details_url: None,
            started_at: None,
            completed_at: None,
            raw_status: None,
            raw_conclusion: None,
        }];
        let suites = vec![
            CheckRun {
                name: "suite/CI".into(),
                status: CheckStatus::Pending,
                summary: None,
                details_url: None,
                started_at: None,
                completed_at: None,
                raw_status: Some("IN_PROGRESS".into()),
                raw_conclusion: None,
            },
            CheckRun {
                name: "suite/CodeQL".into(),
                status: CheckStatus::Success,
                summary: None,
                details_url: None,
                started_at: None,
                completed_at: None,
                raw_status: Some("COMPLETED".into()),
                raw_conclusion: Some("SUCCESS".into()),
            },
        ];

        apply_check_suites(&mut checks, suites);

        assert_eq!(checks.len(), 2);
        assert_eq!(checks[0].name, "suite/CodeQL");
        assert_eq!(checks[1].name, "suite/CI");
        assert_eq!(checks[1].status, CheckStatus::Success);
    }

    #[test]
    fn maps_reactions_by_content_name() {
        let counts = reaction_counts(vec![
            ReactionGroupDto {
                content: "THUMBS_UP".into(),
                users: TotalCountDto { total_count: 3 },
            },
            ReactionGroupDto {
                content: "EYES".into(),
                users: TotalCountDto { total_count: 1 },
            },
        ]);

        assert_eq!(counts.thumbs_up, 3);
        assert_eq!(counts.eyes, 1);
        assert_eq!(counts.total(), 4);
    }

    #[test]
    fn maps_people_and_review_requests_to_display_names() {
        assert_eq!(
            names_from_users(vec![UserDto {
                login: Some("assignee".into()),
                name: None,
            }]),
            vec!["assignee"]
        );
        assert_eq!(
            review_request_names(vec![
                serde_json::json!({"requestedReviewer": {"login": "reviewer"}}),
                serde_json::json!({"slug": "docs-team"}),
            ]),
            vec!["reviewer", "docs-team"]
        );
    }

    #[test]
    fn pr_view_preserves_extra_github_metadata() {
        let requested = ResourceId::from_owner_repo_number("openclaw/openclaw", "81834").unwrap();
        let dto: PrView = serde_json::from_str(
            r#"{
                "number": 81834,
                "title": "metadata test",
                "url": "https://github.com/openclaw/openclaw/pull/81834",
                "state": "OPEN",
                "author": {"login": "alice"},
                "createdAt": "created",
                "updatedAt": "updated",
                "labels": [],
                "assignees": [],
                "reactionGroups": [],
                "body": "body",
                "baseRefName": "main",
                "headRefName": "branch",
                "baseRefOid": "base-sha",
                "headRefOid": "head-sha",
                "headRepository": {"name": "fork"},
                "headRepositoryOwner": {"login": "alice"},
                "reviewDecision": null,
                "reviewRequests": [],
                "closingIssuesReferences": [],
                "mergeStateStatus": "CLEAN",
                "mergeable": "MERGEABLE",
                "isDraft": false,
                "isCrossRepository": true,
                "maintainerCanModify": true,
                "changedFiles": 14,
                "closed": false,
                "closedAt": null,
                "mergedAt": null,
                "mergedBy": null,
                "milestone": {"title": "v1"},
                "projectItems": [{"project": {"title": "Roadmap"}}],
                "autoMergeRequest": null,
                "mergeCommit": null,
                "potentialMergeCommit": {"oid": "merge-sha"},
                "additions": 1,
                "deletions": 2,
                "commits": [],
                "statusCheckRollup": [],
                "files": [],
                "comments": [],
                "reviews": []
            }"#,
        )
        .unwrap();

        let resource = dto.into_resource(&requested);
        let pr = resource.pull_request.as_ref().unwrap();

        assert!(resource
            .metadata
            .iter()
            .any(|item| item.label == "Cross repository" && item.value == "yes"));
        assert!(resource
            .metadata
            .iter()
            .any(|item| item.label == "Changed files" && item.value == "14"));
        assert!(resource
            .metadata
            .iter()
            .any(|item| item.label == "Milestone" && item.value == "v1"));
        assert!(resource
            .metadata
            .iter()
            .any(|item| item.label == "Projects" && item.value == "Roadmap"));
        assert!(pr
            .metadata
            .iter()
            .any(|item| item.label == "Head repository owner" && item.value == "alice"));
        assert!(pr
            .metadata
            .iter()
            .any(|item| item.label == "Potential merge commit" && item.value == "merge-sha"));
    }

    #[test]
    fn issue_view_preserves_extra_github_metadata() {
        let requested = ResourceId::from_owner_repo_number("openclaw/openclaw", "88499").unwrap();
        let dto: IssueView = serde_json::from_str(
            r#"{
                "number": 88499,
                "title": "issue metadata test",
                "url": "https://github.com/openclaw/openclaw/issues/88499",
                "state": "CLOSED",
                "author": {"login": "alice"},
                "createdAt": "created",
                "updatedAt": "updated",
                "labels": [],
                "assignees": [],
                "reactionGroups": [],
                "body": "body",
                "closed": true,
                "isPinned": true,
                "stateReason": "COMPLETED",
                "closedAt": "closed",
                "milestone": {"title": "v2"},
                "projectItems": [{"project": {"title": "Triage"}}],
                "closedByPullRequestsReferences": [],
                "comments": []
            }"#,
        )
        .unwrap();

        let resource = dto.into_resource(&requested);

        assert!(resource
            .metadata
            .iter()
            .any(|item| item.label == "Closed" && item.value == "yes"));
        assert!(resource
            .metadata
            .iter()
            .any(|item| item.label == "Pinned" && item.value == "yes"));
        assert!(resource
            .metadata
            .iter()
            .any(|item| item.label == "State reason" && item.value == "COMPLETED"));
        assert!(resource
            .metadata
            .iter()
            .any(|item| item.label == "Projects" && item.value == "Triage"));
    }

    #[test]
    fn parses_unified_diff_patches_by_file_path() {
        let patches = parse_unified_diff_patches(
            "From abc\n\
diff --git a/src/one.rs b/src/one.rs\n\
index 111..222 100644\n\
--- a/src/one.rs\n\
+++ b/src/one.rs\n\
@@ -1 +1 @@\n\
-old\n\
+new\n\
diff --git a/docs/two.md b/docs/two.md\n\
@@ -2 +2 @@\n\
+line\n",
        );

        assert!(patches["src/one.rs"].contains("+new"));
        assert!(patches["docs/two.md"].contains("+line"));
    }

    #[test]
    fn applies_file_patches_to_matching_changed_files() {
        let mut files = vec![ChangedFile {
            path: "src/one.rs".into(),
            additions: 1,
            deletions: 1,
            change_type: "MODIFIED".into(),
            patch: None,
        }];
        let patches = HashMap::from([("src/one.rs".to_string(), "patch body".to_string())]);

        apply_file_patches(&mut files, patches);

        assert_eq!(files[0].patch.as_deref(), Some("patch body"));
    }

    #[test]
    fn related_resource_ids_parse_urls_and_number_fallbacks() {
        let requested = ResourceId::from_owner_repo_number("openclaw/openclaw", "88499").unwrap();
        let related = related_resource_ids(
            vec![
                RelatedResourceDto {
                    number: None,
                    url: Some("https://github.com/other/repo/pull/12".into()),
                },
                RelatedResourceDto {
                    number: Some(34),
                    url: None,
                },
            ],
            ResourceKind::PullRequest,
            &requested,
        );

        assert_eq!(related.len(), 2);
        assert_eq!(related[0].canonical_name(), "other/repo#12");
        assert_eq!(related[0].kind_hint, Some(ResourceKind::PullRequest));
        assert_eq!(related[1].canonical_name(), "openclaw/openclaw#34");
        assert_eq!(related[1].kind_hint, Some(ResourceKind::PullRequest));
    }

    #[test]
    fn pr_activity_includes_reviews_with_state() {
        let activity = pr_activity(
            vec![CommentDto {
                id: Some("comment".into()),
                author: Some(UserDto {
                    login: Some("alice".into()),
                    name: None,
                }),
                author_association: Some("MEMBER".into()),
                body: "plain comment".into(),
                created_at: Some("2026-01-01T00:00:00Z".into()),
                updated_at: None,
                url: Some("https://github.com/openclaw/openclaw/pull/81834#issuecomment-1".into()),
                includes_created_edit: Some(true),
                is_minimized: Some(false),
                minimized_reason: None,
                reaction_groups: vec![ReactionGroupDto {
                    content: "THUMBS_UP".into(),
                    users: TotalCountDto { total_count: 2 },
                }],
            }],
            vec![ReviewDto {
                id: Some("review".into()),
                author: Some(UserDto {
                    login: Some("bob".into()),
                    name: None,
                }),
                author_association: Some("CONTRIBUTOR".into()),
                body: Some("looks good".into()),
                state: Some("APPROVED".into()),
                submitted_at: Some("2026-01-02T00:00:00Z".into()),
                updated_at: None,
                url: Some(
                    "https://github.com/openclaw/openclaw/pull/81834#pullrequestreview-1".into(),
                ),
                reaction_groups: Vec::new(),
            }],
        );

        assert_eq!(activity.len(), 2);
        assert_eq!(activity[0].kind, ActivityKind::Comment);
        assert_eq!(activity[0].author_association.as_deref(), Some("MEMBER"));
        assert_eq!(activity[0].reactions.thumbs_up, 2);
        assert!(activity[0].includes_created_edit);
        assert_eq!(activity[1].kind, ActivityKind::Review);
        assert_eq!(activity[1].body, "APPROVED: looks good");
        assert_eq!(
            activity[1].url.as_deref(),
            Some("https://github.com/openclaw/openclaw/pull/81834#pullrequestreview-1")
        );
    }

    #[test]
    fn review_thread_activity_keeps_path_and_line() {
        let activity = review_thread_activity(ReviewThreadsResponse {
            data: ReviewThreadsData {
                repository: Some(ReviewThreadsRepository {
                    pull_request: Some(ReviewThreadsPullRequest {
                        review_threads: ReviewThreadsConnection {
                            page_info: PageInfoDto {
                                has_next_page: false,
                                end_cursor: None,
                            },
                            nodes: vec![ReviewThreadDto {
                                id: Some("thread-1".into()),
                                is_resolved: Some(false),
                                is_outdated: Some(true),
                                path: Some("src/lib.rs".into()),
                                line: Some(42),
                                comments: ReviewThreadCommentsConnection {
                                    page_info: PageInfoDto {
                                        has_next_page: false,
                                        end_cursor: None,
                                    },
                                    nodes: vec![ReviewThreadCommentDto {
                                        id: Some("review-comment".into()),
                                        author: Some(UserDto {
                                            login: Some("reviewer".into()),
                                            name: None,
                                        }),
                                        author_association: Some("MEMBER".into()),
                                        body: "Please split this branch.".into(),
                                        created_at: Some("2026-01-01T00:00:00Z".into()),
                                        updated_at: Some("2026-01-02T00:00:00Z".into()),
                                        url: Some("https://github.com/openclaw/openclaw/pull/81834#discussion_r1".into()),
                                        includes_created_edit: Some(false),
                                        is_minimized: Some(true),
                                        minimized_reason: Some("resolved".into()),
                                        reaction_groups: vec![ReactionGroupDto {
                                            content: "EYES".into(),
                                            users: TotalCountDto { total_count: 1 },
                                        }],
                                        path: None,
                                        line: None,
                                    }],
                                },
                            }],
                        },
                    }),
                }),
            },
        });

        assert_eq!(activity.len(), 1);
        assert_eq!(activity[0].kind, ActivityKind::ReviewComment);
        assert_eq!(activity[0].path.as_deref(), Some("src/lib.rs"));
        assert_eq!(activity[0].line, Some(42));
        assert_eq!(activity[0].thread_id.as_deref(), Some("thread-1"));
        assert_eq!(activity[0].thread_resolved, Some(false));
        assert_eq!(activity[0].thread_outdated, Some(true));
        assert_eq!(activity[0].author_association.as_deref(), Some("MEMBER"));
        assert_eq!(activity[0].reactions.eyes, 1);
        assert!(activity[0].is_minimized);
        assert_eq!(activity[0].minimized_reason.as_deref(), Some("resolved"));
        assert_eq!(activity[0].author, "reviewer");
    }

    #[test]
    fn review_thread_activity_page_preserves_pagination_state() {
        let page = review_thread_activity_page(ReviewThreadsResponse {
            data: ReviewThreadsData {
                repository: Some(ReviewThreadsRepository {
                    pull_request: Some(ReviewThreadsPullRequest {
                        review_threads: ReviewThreadsConnection {
                            page_info: PageInfoDto {
                                has_next_page: true,
                                end_cursor: Some("cursor-2".into()),
                            },
                            nodes: vec![ReviewThreadDto {
                                id: Some("thread-2".into()),
                                is_resolved: Some(true),
                                is_outdated: Some(false),
                                path: Some("src/main.rs".into()),
                                line: Some(7),
                                comments: ReviewThreadCommentsConnection {
                                    page_info: PageInfoDto {
                                        has_next_page: false,
                                        end_cursor: None,
                                    },
                                    nodes: vec![ReviewThreadCommentDto {
                                        id: Some("review-comment-2".into()),
                                        author: Some(UserDto {
                                            login: Some("maintainer".into()),
                                            name: None,
                                        }),
                                        author_association: Some("MEMBER".into()),
                                        body: "Follow-up.".into(),
                                        created_at: Some("2026-01-03T00:00:00Z".into()),
                                        updated_at: Some("2026-01-04T00:00:00Z".into()),
                                        url: None,
                                        includes_created_edit: Some(false),
                                        is_minimized: Some(false),
                                        minimized_reason: None,
                                        reaction_groups: Vec::new(),
                                        path: Some("src/main.rs".into()),
                                        line: Some(8),
                                    }],
                                },
                            }],
                        },
                    }),
                }),
            },
        })
        .expect("review thread page");

        assert!(page.has_next_page);
        assert_eq!(page.end_cursor.as_deref(), Some("cursor-2"));
        assert_eq!(page.activity.len(), 1);
        assert_eq!(page.activity[0].thread_id.as_deref(), Some("thread-2"));
        assert_eq!(page.activity[0].thread_resolved, Some(true));
        assert_eq!(page.activity[0].path.as_deref(), Some("src/main.rs"));
        assert_eq!(page.activity[0].line, Some(8));
    }

    #[test]
    fn review_thread_comments_page_preserves_pagination_state() {
        let page = review_thread_comments_page(ReviewThreadCommentsResponse {
            data: ReviewThreadCommentsData {
                node: Some(ReviewThreadCommentsNode {
                    comments: ReviewThreadCommentsConnection {
                        page_info: PageInfoDto {
                            has_next_page: true,
                            end_cursor: Some("comment-cursor-2".into()),
                        },
                        nodes: vec![ReviewThreadCommentDto {
                            id: Some("review-comment-3".into()),
                            author: Some(UserDto {
                                login: Some("reviewer".into()),
                                name: None,
                            }),
                            author_association: Some("MEMBER".into()),
                            body: "Another follow-up.".into(),
                            created_at: Some("2026-01-05T00:00:00Z".into()),
                            updated_at: Some("2026-01-06T00:00:00Z".into()),
                            url: None,
                            includes_created_edit: Some(false),
                            is_minimized: Some(false),
                            minimized_reason: None,
                            reaction_groups: Vec::new(),
                            path: Some("src/lib.rs".into()),
                            line: Some(43),
                        }],
                    },
                }),
            },
        })
        .expect("review thread comments page");

        assert!(page.has_next_page);
        assert_eq!(page.end_cursor.as_deref(), Some("comment-cursor-2"));
        assert_eq!(page.nodes.len(), 1);
        assert_eq!(page.nodes[0].id.as_deref(), Some("review-comment-3"));
    }

    #[test]
    fn timeline_activity_maps_github_events() {
        let activity = timeline_activity(TimelineResponse {
            data: TimelineData {
                repository: Some(TimelineRepository {
                    issue: Some(TimelineResource {
                        timeline_items: TimelineItemsConnection {
                            page_info: PageInfoDto {
                                has_next_page: false,
                                end_cursor: None,
                            },
                            nodes: vec![
                                serde_json::json!({
                                    "__typename": "LabeledEvent",
                                    "id": "label-event",
                                    "createdAt": "2026-05-31T02:28:11Z",
                                    "actor": {"login": "clawsweeper"},
                                    "label": {"name": "P2"}
                                }),
                                serde_json::json!({
                                    "__typename": "CrossReferencedEvent",
                                    "id": "cross-ref",
                                    "createdAt": "2026-05-31T07:01:12Z",
                                    "actor": {"login": "alice"},
                                    "source": {
                                        "__typename": "Issue",
                                        "number": 88538,
                                        "title": "related issue",
                                        "url": "https://github.com/openclaw/openclaw/issues/88538",
                                        "repository": {"nameWithOwner": "openclaw/openclaw"}
                                    }
                                }),
                                serde_json::json!({
                                    "__typename": "ReviewRequestedEvent",
                                    "id": "review-requested",
                                    "createdAt": "2026-05-31T07:02:12Z",
                                    "actor": {"login": "alice"},
                                    "requestedReviewer": {"__typename": "User", "login": "maintainer"}
                                }),
                                serde_json::json!({
                                    "__typename": "ReadyForReviewEvent",
                                    "id": "ready",
                                    "createdAt": "2026-05-31T07:03:12Z",
                                    "actor": {"login": "alice"}
                                }),
                                serde_json::json!({
                                    "__typename": "MergedEvent",
                                    "id": "merged",
                                    "createdAt": "2026-05-31T07:04:12Z",
                                    "actor": {"login": "alice"},
                                    "mergeRefName": "main",
                                    "commit": {"oid": "abcdef1234567890"}
                                }),
                                serde_json::json!({
                                    "__typename": "LockedEvent",
                                    "id": "locked",
                                    "createdAt": "2026-05-31T07:05:12Z",
                                    "actor": {"login": "alice"},
                                    "lockReason": "RESOLVED"
                                }),
                                serde_json::json!({
                                    "__typename": "MarkedAsDuplicateEvent",
                                    "id": "duplicate",
                                    "createdAt": "2026-05-31T07:06:12Z",
                                    "actor": {"login": "alice"},
                                    "canonical": {
                                        "__typename": "Issue",
                                        "number": 88499,
                                        "title": "canonical issue",
                                        "url": "https://github.com/openclaw/openclaw/issues/88499",
                                        "repository": {"nameWithOwner": "openclaw/openclaw"}
                                    }
                                }),
                                serde_json::json!({
                                    "__typename": "ConnectedEvent",
                                    "id": "connected",
                                    "createdAt": "2026-05-31T07:07:12Z",
                                    "actor": {"login": "alice"},
                                    "source": {
                                        "__typename": "Issue",
                                        "number": 88499,
                                        "title": "source issue",
                                        "url": "https://github.com/openclaw/openclaw/issues/88499",
                                        "repository": {"nameWithOwner": "openclaw/openclaw"}
                                    },
                                    "subject": {
                                        "__typename": "PullRequest",
                                        "number": 81834,
                                        "title": "subject pr",
                                        "url": "https://github.com/openclaw/openclaw/pull/81834",
                                        "repository": {"nameWithOwner": "openclaw/openclaw"}
                                    }
                                }),
                                serde_json::json!({
                                    "__typename": "BaseRefChangedEvent",
                                    "id": "base-changed",
                                    "createdAt": "2026-05-31T07:08:12Z",
                                    "actor": {"login": "alice"},
                                    "previousRefName": "develop",
                                    "currentRefName": "main"
                                }),
                                serde_json::json!({
                                    "__typename": "HeadRefForcePushedEvent",
                                    "id": "head-force",
                                    "createdAt": "2026-05-31T07:09:12Z",
                                    "actor": {"login": "alice"},
                                    "ref": {"name": "feature"},
                                    "beforeCommit": {"oid": "111111111111aaaa"},
                                    "afterCommit": {"oid": "222222222222bbbb"}
                                }),
                                serde_json::json!({
                                    "__typename": "ReviewDismissedEvent",
                                    "id": "review-dismissed",
                                    "createdAt": "2026-05-31T07:10:12Z",
                                    "actor": {"login": "alice"},
                                    "previousReviewState": "APPROVED",
                                    "dismissalMessage": "new commits",
                                    "url": "https://github.com/openclaw/openclaw/pull/81834#event-1"
                                }),
                                serde_json::json!({
                                    "__typename": "AddedToMergeQueueEvent",
                                    "id": "merge-queue",
                                    "createdAt": "2026-05-31T07:11:12Z",
                                    "actor": {"login": "alice"},
                                    "enqueuer": {"login": "maintainer"}
                                }),
                                serde_json::json!({
                                    "__typename": "IssueTypeChangedEvent",
                                    "id": "type-changed",
                                    "createdAt": "2026-05-31T07:12:12Z",
                                    "actor": {"login": "alice"},
                                    "prevIssueType": {"name": "Bug"},
                                    "issueType": {"name": "Task"}
                                }),
                                serde_json::json!({
                                    "__typename": "SubIssueAddedEvent",
                                    "id": "sub-issue",
                                    "createdAt": "2026-05-31T07:13:12Z",
                                    "actor": {"login": "alice"},
                                    "subIssue": {
                                        "number": 88538,
                                        "title": "child issue",
                                        "url": "https://github.com/openclaw/openclaw/issues/88538",
                                        "repository": {"nameWithOwner": "openclaw/openclaw"}
                                    }
                                }),
                                serde_json::json!({
                                    "__typename": "BlockedByAddedEvent",
                                    "id": "blocked-by",
                                    "createdAt": "2026-05-31T07:14:12Z",
                                    "actor": {"login": "alice"},
                                    "blockingIssue": {
                                        "number": 88499,
                                        "title": "blocking issue",
                                        "url": "https://github.com/openclaw/openclaw/issues/88499",
                                        "repository": {"nameWithOwner": "openclaw/openclaw"}
                                    }
                                }),
                                serde_json::json!({
                                    "__typename": "ConvertedToDiscussionEvent",
                                    "id": "converted-discussion",
                                    "createdAt": "2026-05-31T07:15:12Z",
                                    "actor": {"login": "alice"},
                                    "discussion": {
                                        "title": "design discussion",
                                        "url": "https://github.com/orgs/openclaw/discussions/12"
                                    }
                                }),
                                serde_json::json!({
                                    "__typename": "IssueCommentPinnedEvent",
                                    "id": "pinned-comment",
                                    "createdAt": "2026-05-31T07:16:12Z",
                                    "actor": {"login": "alice"},
                                    "issueComment": {
                                        "url": "https://github.com/openclaw/openclaw/issues/88499#issuecomment-2"
                                    }
                                }),
                            ],
                        },
                    }),
                    pull_request: None,
                }),
            },
        });

        assert_eq!(activity.len(), 17);
        assert_eq!(activity[0].kind, ActivityKind::Timeline);
        assert_eq!(activity[0].author, "clawsweeper");
        assert_eq!(activity[0].body, "added label P2");
        assert_eq!(activity[1].author, "alice");
        assert!(activity[1]
            .body
            .contains("cross-referenced by openclaw/openclaw#88538"));
        assert_eq!(
            activity[1].url.as_deref(),
            Some("https://github.com/openclaw/openclaw/issues/88538")
        );
        assert_eq!(activity[2].body, "requested review from maintainer");
        assert_eq!(activity[3].body, "marked ready for review");
        assert_eq!(activity[4].body, "merged into main at abcdef123456");
        assert_eq!(activity[5].body, "locked: Resolved");
        assert_eq!(
            activity[6].body,
            "marked duplicate of openclaw/openclaw#88499"
        );
        assert_eq!(
            activity[6].url.as_deref(),
            Some("https://github.com/openclaw/openclaw/issues/88499")
        );
        assert_eq!(
            activity[7].body,
            "connected openclaw/openclaw#88499 to openclaw/openclaw#81834"
        );
        assert_eq!(activity[8].body, "changed base from develop to main");
        assert_eq!(
            activity[9].body,
            "force-pushed head feature from 111111111111 to 222222222222"
        );
        assert_eq!(activity[10].body, "dismissed Approved review: new commits");
        assert_eq!(
            activity[10].url.as_deref(),
            Some("https://github.com/openclaw/openclaw/pull/81834#event-1")
        );
        assert_eq!(activity[11].body, "added to merge queue by maintainer");
        assert_eq!(activity[12].body, "changed issue type from Bug to Task");
        assert_eq!(activity[13].body, "added sub-issue openclaw/openclaw#88538");
        assert_eq!(
            activity[13].url.as_deref(),
            Some("https://github.com/openclaw/openclaw/issues/88538")
        );
        assert_eq!(activity[14].body, "blocked by openclaw/openclaw#88499");
        assert_eq!(
            activity[15].body,
            "converted to discussion design discussion"
        );
        assert_eq!(
            activity[15].url.as_deref(),
            Some("https://github.com/orgs/openclaw/discussions/12")
        );
        assert_eq!(activity[16].body, "pinned a comment");
        assert_eq!(
            activity[16].url.as_deref(),
            Some("https://github.com/openclaw/openclaw/issues/88499#issuecomment-2")
        );
    }

    #[test]
    fn timeline_activity_page_preserves_pagination_state() {
        let page = timeline_activity_page(TimelineResponse {
            data: TimelineData {
                repository: Some(TimelineRepository {
                    issue: Some(TimelineResource {
                        timeline_items: TimelineItemsConnection {
                            page_info: PageInfoDto {
                                has_next_page: true,
                                end_cursor: Some("cursor-2".into()),
                            },
                            nodes: vec![serde_json::json!({
                                "__typename": "PinnedEvent",
                                "id": "pinned",
                                "createdAt": "2026-05-31T07:08:12Z",
                                "actor": {"login": "alice"}
                            })],
                        },
                    }),
                    pull_request: None,
                }),
            },
        })
        .expect("timeline page");

        assert!(page.has_next_page);
        assert_eq!(page.end_cursor.as_deref(), Some("cursor-2"));
        assert_eq!(page.activity.len(), 1);
        assert_eq!(page.activity[0].body, "pinned");
    }

    #[test]
    fn changed_files_from_graphql_keep_change_type() {
        let files = changed_files_from_response(ChangedFilesResponse {
            data: ChangedFilesData {
                repository: Some(ChangedFilesRepository {
                    pull_request: Some(ChangedFilesPullRequest {
                        files: ChangedFilesConnection {
                            page_info: PageInfoDto {
                                has_next_page: false,
                                end_cursor: None,
                            },
                            nodes: vec![
                                FileDto {
                                    path: "src/lib.rs".into(),
                                    additions: 10,
                                    deletions: 2,
                                    change_type: Some("MODIFIED".into()),
                                },
                                FileDto {
                                    path: "src/new.rs".into(),
                                    additions: 7,
                                    deletions: 0,
                                    change_type: Some("ADDED".into()),
                                },
                            ],
                        },
                    }),
                }),
            },
        });

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].change_type, "MODIFIED");
        assert_eq!(files[1].change_type, "ADDED");
    }
}
