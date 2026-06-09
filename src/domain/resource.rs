use std::{collections::HashMap, fmt, sync::LazyLock};

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FULL_DEPTH_WARNING_HINT: &str = "set --api-depth full or GZG_API_DEPTH=full";

static GITHUB_RESOURCE_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^https://github\.com/([^/\s]+)/([^/\s]+)/(pull|issues)/([0-9]+)(?:[/?#].*)?$")
        .expect("valid GitHub URL regex")
});
static OWNER_REPO_HASH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^/\s#]+)/([^/\s#]+)#([0-9]+)$").expect("valid owner repo hash regex")
});

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResourceIdError {
    #[error("expected a GitHub PR/issue URL, owner/repo#number, owner/repo number, or a number from inside a GitHub repo")]
    Invalid,
    #[error("resource number must be positive")]
    InvalidNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    PullRequest,
    Issue,
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PullRequest => f.write_str("PR"),
            Self::Issue => f.write_str("Issue"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId {
    pub owner: String,
    pub repo: String,
    pub number: u64,
    pub kind_hint: Option<ResourceKind>,
}

impl ResourceId {
    pub fn parse(input: &str) -> Result<Self, ResourceIdError> {
        Self::parse_with_repo_context(input, None)
    }

    pub fn parse_with_repo_context(
        input: &str,
        repo_name_with_owner: Option<&str>,
    ) -> Result<Self, ResourceIdError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ResourceIdError::Invalid);
        }

        if let Some(parsed) = Self::parse_url(trimmed)? {
            return Ok(parsed);
        }

        if let Some(parsed) = Self::parse_owner_repo_hash(trimmed)? {
            return Ok(parsed);
        }

        if let Some(repo_name_with_owner) = repo_name_with_owner {
            let number = trimmed.strip_prefix('#').unwrap_or(trimmed);
            if !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit()) {
                return Self::from_owner_repo_number(repo_name_with_owner, number);
            }
        }

        Err(ResourceIdError::Invalid)
    }

    pub fn from_owner_repo_number(owner_repo: &str, number: &str) -> Result<Self, ResourceIdError> {
        let (owner, repo) = owner_repo.split_once('/').ok_or(ResourceIdError::Invalid)?;
        let number = parse_number(number)?;
        if owner.is_empty() || repo.is_empty() {
            return Err(ResourceIdError::Invalid);
        }
        Ok(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
            number,
            kind_hint: None,
        })
    }

    pub fn repo_name_with_owner(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    pub fn canonical_name(&self) -> String {
        format!("{}/{}#{}", self.owner, self.repo, self.number)
    }

    pub fn web_url(&self) -> String {
        self.web_url_for_kind(self.kind_hint.unwrap_or(ResourceKind::Issue))
    }

    pub fn web_url_for_kind(&self, kind: ResourceKind) -> String {
        let segment = match kind {
            ResourceKind::PullRequest => "pull",
            ResourceKind::Issue => "issues",
        };
        format!(
            "https://github.com/{}/{}/{}/{}",
            self.owner, self.repo, segment, self.number
        )
    }

    pub fn relative_to_repo(
        owner: &str,
        repo: &str,
        number: &str,
    ) -> Result<Self, ResourceIdError> {
        Ok(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
            number: parse_number(number)?,
            kind_hint: None,
        })
    }

    fn parse_url(input: &str) -> Result<Option<Self>, ResourceIdError> {
        let Some(caps) = GITHUB_RESOURCE_URL_RE.captures(input) else {
            return Ok(None);
        };
        let kind_hint = match &caps[3] {
            "pull" => Some(ResourceKind::PullRequest),
            "issues" => Some(ResourceKind::Issue),
            _ => None,
        };
        Ok(Some(Self {
            owner: caps[1].to_string(),
            repo: caps[2].to_string(),
            number: parse_number(&caps[4])?,
            kind_hint,
        }))
    }

    fn parse_owner_repo_hash(input: &str) -> Result<Option<Self>, ResourceIdError> {
        let Some(caps) = OWNER_REPO_HASH_RE.captures(input) else {
            return Ok(None);
        };
        Ok(Some(Self {
            owner: caps[1].to_string(),
            repo: caps[2].to_string(),
            number: parse_number(&caps[3])?,
            kind_hint: None,
        }))
    }
}

fn parse_number(input: &str) -> Result<u64, ResourceIdError> {
    let number = input.parse::<u64>().map_err(|_| ResourceIdError::Invalid)?;
    if number == 0 {
        return Err(ResourceIdError::InvalidNumber);
    }
    Ok(number)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReactionCounts {
    pub thumbs_up: u64,
    pub thumbs_down: u64,
    pub laugh: u64,
    pub hooray: u64,
    pub confused: u64,
    pub heart: u64,
    pub rocket: u64,
    pub eyes: u64,
}

impl ReactionCounts {
    pub fn total(&self) -> u64 {
        self.thumbs_up
            + self.thumbs_down
            + self.laugh
            + self.hooray
            + self.confused
            + self.heart
            + self.rocket
            + self.eyes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resource {
    pub id: ResourceId,
    pub title: String,
    pub url: String,
    pub state: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    pub labels: Vec<String>,
    #[serde(default)]
    pub assignees: Vec<String>,
    pub reactions: ReactionCounts,
    pub body: String,
    pub activity: Vec<ActivityEntry>,
    #[serde(default)]
    pub related_resources: Vec<ResourceId>,
    #[serde(default)]
    pub metadata: Vec<MetadataItem>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub pull_request: Option<PullRequest>,
}

impl Resource {
    pub fn web_url(&self) -> String {
        let url = self.url.trim();
        if is_github_web_url(url) {
            url.to_string()
        } else {
            self.id.web_url_for_kind(self.kind())
        }
    }

    pub fn kind(&self) -> ResourceKind {
        if self.pull_request.is_some() {
            ResourceKind::PullRequest
        } else {
            ResourceKind::Issue
        }
    }

    pub fn is_pull_request(&self) -> bool {
        self.pull_request.is_some()
    }

    pub fn has_partial_depth_warning(&self) -> bool {
        self.warnings
            .iter()
            .any(|warning| warning.contains(FULL_DEPTH_WARNING_HINT))
    }

    pub fn uses_public_rest_fallback(&self) -> bool {
        self.warnings
            .iter()
            .any(|warning| warning.starts_with(PUBLIC_REST_FALLBACK_WARNING_PREFIX))
    }

    pub fn merge_file_patch_context_from(&mut self, patch_resource: &Resource) -> ResourceMerge {
        let mut merge = ResourceMerge::default();

        for warning in &patch_resource.warnings {
            if !self.warnings.iter().any(|item| item == warning) {
                self.warnings.push(warning.clone());
                merge.warnings_changed = true;
            }
        }

        let patch_by_path = patch_resource
            .pull_request
            .as_ref()
            .map(file_patch_by_path)
            .unwrap_or_default();

        if let Some(pr) = &mut self.pull_request {
            for file in &mut pr.files {
                let Some(patch) = patch_by_path.get(&file.path) else {
                    continue;
                };
                if file.patch.as_ref() != Some(*patch) {
                    file.patch = Some((*patch).clone());
                    merge.files_changed = true;
                }
            }
        }

        merge
    }

    pub fn preserve_loaded_file_patch_context_from(&mut self, existing: &Resource) {
        for warning in &existing.warnings {
            if warning.starts_with(FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING)
                && !self.warnings.iter().any(|item| item == warning)
            {
                self.warnings.push(warning.clone());
            }
        }

        let patch_by_path = existing
            .pull_request
            .as_ref()
            .map(file_patch_by_path)
            .unwrap_or_default();

        if let Some(pr) = &mut self.pull_request {
            for file in &mut pr.files {
                if file.patch.is_some() {
                    continue;
                }
                if let Some(patch) = patch_by_path.get(&file.path) {
                    file.patch = Some((*patch).clone());
                }
            }
        }
    }

    pub fn fingerprint(&self) -> String {
        let pr_fingerprint = self.pull_request.as_ref().map_or_else(String::new, |pr| {
            format!(
                "{}:{}:{}:{}:{}:{}:{}",
                pr.additions,
                pr.deletions,
                pr.commits
                    .iter()
                    .map(|commit| {
                        format!(
                            "{}:{}:{}:{}:{}:{:?}:{}:{}",
                            commit.oid,
                            commit.message,
                            commit.body,
                            commit.author,
                            commit.authors.join(","),
                            commit.authored_at,
                            commit.committed_at,
                            commit
                                .deployments
                                .iter()
                                .map(|deployment| {
                                    format!(
                                        "{}:{}:{:?}:{:?}:{:?}:{:?}:{}",
                                        deployment.environment,
                                        deployment.state,
                                        deployment.description,
                                        deployment.environment_url,
                                        deployment.log_url,
                                        deployment.created_at,
                                        deployment.updated_at
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("|"),
                pr.checks
                    .iter()
                    .map(|check| {
                        format!(
                            "{}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}",
                            check.name,
                            check.status,
                            check.summary,
                            check.details_url,
                            check.started_at,
                            check.completed_at,
                            check.raw_conclusion
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("|"),
                pr.files.len(),
                pr.requested_reviewers.join("|"),
                metadata_fingerprint(&pr.metadata)
            )
        });
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}",
            self.id.canonical_name(),
            self.title,
            self.state,
            self.updated_at,
            self.activity_fingerprint(),
            self.assignees.join("|"),
            self.related_resources
                .iter()
                .map(ResourceId::canonical_name)
                .collect::<Vec<_>>()
                .join("|"),
            metadata_fingerprint(&self.metadata),
            pr_fingerprint
        )
    }

    pub fn changed_sections(&self, other: &Self) -> Vec<String> {
        let mut sections = Vec::new();
        push_changed(&mut sections, "identity", self.id != other.id);
        push_changed(
            &mut sections,
            "summary",
            self.title != other.title || self.state != other.state || self.body != other.body,
        );
        push_changed(
            &mut sections,
            "metadata",
            self.labels != other.labels
                || self.assignees != other.assignees
                || self.reactions != other.reactions
                || self.related_resources != other.related_resources
                || self.metadata != other.metadata,
        );
        push_changed(&mut sections, "warnings", self.warnings != other.warnings);
        push_changed(&mut sections, "activity", self.activity != other.activity);
        match (&self.pull_request, &other.pull_request) {
            (Some(left), Some(right)) => {
                push_changed(
                    &mut sections,
                    "review",
                    left.requested_reviewers != right.requested_reviewers
                        || left.review_decision != right.review_decision,
                );
                push_changed(
                    &mut sections,
                    "merge",
                    left.base_ref != right.base_ref
                        || left.head_ref != right.head_ref
                        || left.merge_state != right.merge_state,
                );
                push_changed(
                    &mut sections,
                    "diff",
                    left.additions != right.additions || left.deletions != right.deletions,
                );
                push_changed(&mut sections, "commits", left.commits != right.commits);
                push_changed(&mut sections, "checks", left.checks != right.checks);
                push_changed(&mut sections, "files", left.files != right.files);
                push_changed(
                    &mut sections,
                    "pr metadata",
                    left.metadata != right.metadata,
                );
            }
            (None, None) => {}
            _ => sections.push("kind".to_string()),
        }
        sections
    }

    fn activity_fingerprint(&self) -> String {
        self.activity
            .iter()
            .map(|entry| {
                format!(
                    "{}:{:?}:{}:{}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{}:{}:{}:{:?}",
                    entry.id,
                    entry.kind,
                    entry.updated_at,
                    entry.body,
                    entry.path,
                    entry.line,
                    entry.thread_resolved,
                    entry.thread_outdated,
                    entry.url,
                    entry.author_association,
                    entry.reactions.total(),
                    entry.includes_created_edit,
                    entry.is_minimized,
                    entry.minimized_reason
                )
            })
            .collect::<Vec<_>>()
            .join("|")
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ResourceMerge {
    pub warnings_changed: bool,
    pub files_changed: bool,
}

impl ResourceMerge {
    pub fn changed(self) -> bool {
        self.warnings_changed || self.files_changed
    }
}

fn file_patch_by_path(pr: &PullRequest) -> HashMap<&String, &String> {
    pr.files
        .iter()
        .filter_map(|file| file.patch.as_ref().map(|patch| (&file.path, patch)))
        .collect()
}

fn push_changed(sections: &mut Vec<String>, label: &str, changed: bool) {
    if changed {
        sections.push(label.to_string());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataItem {
    pub label: String,
    pub value: String,
}

fn is_github_web_url(url: &str) -> bool {
    url.starts_with("https://github.com/")
}

fn metadata_fingerprint(items: &[MetadataItem]) -> String {
    items
        .iter()
        .map(|item| format!("{}={}", item.label, item.value))
        .collect::<Vec<_>>()
        .join("|")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequest {
    pub base_ref: String,
    pub head_ref: String,
    #[serde(default)]
    pub requested_reviewers: Vec<String>,
    pub review_decision: Option<String>,
    pub merge_state: Option<String>,
    pub additions: u64,
    pub deletions: u64,
    pub commits: Vec<Commit>,
    pub checks: Vec<CheckRun>,
    pub files: Vec<ChangedFile>,
    #[serde(default)]
    pub metadata: Vec<MetadataItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub id: String,
    #[serde(default)]
    pub kind: ActivityKind,
    pub author: String,
    pub body: String,
    pub updated_at: String,
    pub path: Option<String>,
    pub line: Option<u64>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub author_association: Option<String>,
    #[serde(default)]
    pub reactions: ReactionCounts,
    #[serde(default)]
    pub includes_created_edit: bool,
    #[serde(default)]
    pub is_minimized: bool,
    #[serde(default)]
    pub minimized_reason: Option<String>,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub thread_resolved: Option<bool>,
    #[serde(default)]
    pub thread_outdated: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    #[default]
    Comment,
    Review,
    ReviewComment,
    CommitComment,
    Timeline,
}

impl ActivityKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Comment => "Comment",
            Self::Review => "Review",
            Self::ReviewComment => "Review comment",
            Self::CommitComment => "Commit comment",
            Self::Timeline => "Timeline",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commit {
    pub oid: String,
    pub message: String,
    #[serde(default)]
    pub body: String,
    pub author: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub authored_at: Option<String>,
    pub committed_at: String,
    pub status: CheckStatus,
    #[serde(default)]
    pub deployments: Vec<Deployment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
    pub environment: String,
    pub state: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub environment_url: Option<String>,
    #[serde(default)]
    pub log_url: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Success,
    Failure,
    Pending,
    Skipped,
    Neutral,
    Unknown,
}

impl CheckStatus {
    pub fn from_github(status: Option<&str>, conclusion: Option<&str>) -> Self {
        let status = status.map(|value| value.to_ascii_uppercase());
        let conclusion = conclusion.map(|value| value.to_ascii_uppercase());
        match (status.as_deref(), conclusion.as_deref()) {
            (Some("COMPLETED"), Some("SUCCESS")) | (Some("SUCCESS"), _) => Self::Success,
            (Some("COMPLETED"), Some("FAILURE" | "TIMED_OUT" | "STARTUP_FAILURE")) => Self::Failure,
            (Some("COMPLETED"), Some("ACTION_REQUIRED")) => Self::Pending,
            (Some("COMPLETED"), Some("SKIPPED" | "CANCELLED" | "STALE")) => Self::Skipped,
            (Some("COMPLETED"), Some("NEUTRAL")) => Self::Neutral,
            (Some("COMPLETED"), _) => Self::Unknown,
            (Some("ERROR" | "FAILURE"), _) => Self::Failure,
            (Some("EXPECTED"), _) => Self::Pending,
            (Some("QUEUED" | "IN_PROGRESS" | "PENDING" | "WAITING" | "REQUESTED"), _) => {
                Self::Pending
            }
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Success => "PASS",
            Self::Failure => "FAIL",
            Self::Pending => "PENDING",
            Self::Skipped => "SKIP",
            Self::Neutral => "NEUTRAL",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRun {
    pub name: String,
    pub status: CheckStatus,
    pub summary: Option<String>,
    #[serde(default)]
    pub details_url: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub raw_status: Option<String>,
    #[serde(default)]
    pub raw_conclusion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CheckCounts {
    pub success: usize,
    pub failure: usize,
    pub pending: usize,
    pub skipped: usize,
    pub neutral: usize,
    pub unknown: usize,
}

impl CheckCounts {
    pub fn total(self) -> usize {
        self.success + self.failure + self.pending + self.skipped + self.neutral + self.unknown
    }
}

impl PullRequest {
    pub fn check_counts(&self) -> CheckCounts {
        let mut counts = CheckCounts::default();
        for check in &self.checks {
            match check.status {
                CheckStatus::Success => counts.success += 1,
                CheckStatus::Failure => counts.failure += 1,
                CheckStatus::Pending => counts.pending += 1,
                CheckStatus::Skipped => counts.skipped += 1,
                CheckStatus::Neutral => counts.neutral += 1,
                CheckStatus::Unknown => counts.unknown += 1,
            }
        }
        counts
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub additions: u64,
    pub deletions: u64,
    pub change_type: String,
    #[serde(default)]
    pub patch: Option<String>,
}

pub const FILE_PATCH_CONTEXT_UNAVAILABLE_WARNING: &str = "file patch context unavailable";
pub const PUBLIC_REST_FALLBACK_WARNING_PREFIX: &str = "using public REST fallback ";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pull_request_url() {
        let id = ResourceId::parse("https://github.com/openclaw/openclaw/pull/81834").unwrap();

        assert_eq!(id.owner, "openclaw");
        assert_eq!(id.repo, "openclaw");
        assert_eq!(id.number, 81834);
        assert_eq!(id.kind_hint, Some(ResourceKind::PullRequest));
    }

    #[test]
    fn parses_issue_url_with_query() {
        let id =
            ResourceId::parse("https://github.com/openclaw/openclaw/issues/42#comment").unwrap();

        assert_eq!(id.canonical_name(), "openclaw/openclaw#42");
        assert_eq!(id.kind_hint, Some(ResourceKind::Issue));
    }

    #[test]
    fn parses_owner_repo_hash() {
        let id = ResourceId::parse("openclaw/openclaw#81834").unwrap();

        assert_eq!(id.repo_name_with_owner(), "openclaw/openclaw");
        assert_eq!(id.number, 81834);
        assert_eq!(id.kind_hint, None);
    }

    #[test]
    fn parses_owner_repo_number_args() {
        let id = ResourceId::from_owner_repo_number("openclaw/openclaw", "81834").unwrap();

        assert_eq!(id.canonical_name(), "openclaw/openclaw#81834");
    }

    #[test]
    fn parses_relative_numbers_with_repo_context() {
        let id = ResourceId::parse_with_repo_context("81834", Some("openclaw/openclaw")).unwrap();

        assert_eq!(id.canonical_name(), "openclaw/openclaw#81834");
        assert_eq!(id.kind_hint, None);

        let hash_id =
            ResourceId::parse_with_repo_context("#88499", Some("openclaw/openclaw")).unwrap();

        assert_eq!(hash_id.canonical_name(), "openclaw/openclaw#88499");
    }

    #[test]
    fn rejects_relative_numbers_without_repo_context() {
        assert_eq!(
            ResourceId::parse_with_repo_context("81834", None).unwrap_err(),
            ResourceIdError::Invalid
        );
    }

    #[test]
    fn rejects_zero_number() {
        assert_eq!(
            ResourceId::parse("openclaw/openclaw#0").unwrap_err(),
            ResourceIdError::InvalidNumber
        );
    }

    #[test]
    fn builds_relative_resource_ids() {
        let id = ResourceId::relative_to_repo("openclaw", "openclaw", "66943").unwrap();

        assert_eq!(id.canonical_name(), "openclaw/openclaw#66943");
    }

    #[test]
    fn builds_kind_aware_web_urls() {
        let pr = ResourceId {
            owner: "openclaw".into(),
            repo: "openclaw".into(),
            number: 81834,
            kind_hint: Some(ResourceKind::PullRequest),
        };
        let issue = ResourceId {
            owner: "openclaw".into(),
            repo: "openclaw".into(),
            number: 88499,
            kind_hint: Some(ResourceKind::Issue),
        };
        let unknown = ResourceId {
            owner: "openclaw".into(),
            repo: "openclaw".into(),
            number: 88499,
            kind_hint: None,
        };

        assert_eq!(
            pr.web_url(),
            "https://github.com/openclaw/openclaw/pull/81834"
        );
        assert_eq!(
            issue.web_url(),
            "https://github.com/openclaw/openclaw/issues/88499"
        );
        assert_eq!(
            unknown.web_url(),
            "https://github.com/openclaw/openclaw/issues/88499"
        );
    }

    #[test]
    fn resource_web_url_ignores_non_github_url_values() {
        let mut resource = Resource {
            id: ResourceId {
                owner: "huggingface".into(),
                repo: "huggingface.js".into(),
                number: 2185,
                kind_hint: Some(ResourceKind::PullRequest),
            },
            title: "title".into(),
            url: "http://huggingface/huggingface.js#2185".into(),
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
            pull_request: Some(PullRequest {
                base_ref: "main".into(),
                head_ref: "topic".into(),
                requested_reviewers: Vec::new(),
                review_decision: None,
                merge_state: None,
                additions: 0,
                deletions: 0,
                commits: Vec::new(),
                checks: Vec::new(),
                files: Vec::new(),
                metadata: Vec::new(),
            }),
        };

        assert_eq!(
            resource.web_url(),
            "https://github.com/huggingface/huggingface.js/pull/2185"
        );

        resource.url = " https://github.com/huggingface/huggingface.js/pull/2185 ".into();
        assert_eq!(
            resource.web_url(),
            "https://github.com/huggingface/huggingface.js/pull/2185"
        );
    }

    #[test]
    fn fingerprint_changes_when_activity_content_or_metadata_changes() {
        let id = ResourceId::from_owner_repo_number("openclaw/openclaw", "1").unwrap();
        let mut resource = Resource {
            id,
            title: "title".into(),
            url: "https://github.com/openclaw/openclaw/issues/1".into(),
            state: "OPEN".into(),
            author: "alice".into(),
            created_at: "created".into(),
            updated_at: "updated".into(),
            labels: Vec::new(),
            assignees: Vec::new(),
            reactions: ReactionCounts::default(),
            body: "body".into(),
            activity: vec![ActivityEntry {
                id: "comment".into(),
                kind: ActivityKind::Comment,
                author: "bob".into(),
                body: "before".into(),
                updated_at: "time".into(),
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
            related_resources: Vec::new(),
            metadata: Vec::new(),
            warnings: Vec::new(),
            pull_request: None,
        };
        let before = resource.fingerprint();

        resource.activity[0].body = "after".into();
        let after_body = resource.fingerprint();

        assert_ne!(before, after_body);

        resource.activity[0].body = "before".into();
        resource.activity[0].reactions.eyes = 1;

        assert_ne!(before, resource.fingerprint());
    }

    #[test]
    fn detects_partial_depth_warning_marker() {
        let id = ResourceId::from_owner_repo_number("openclaw/openclaw", "1").unwrap();
        let mut resource = Resource {
            id,
            title: "title".into(),
            url: "https://github.com/openclaw/openclaw/issues/1".into(),
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

        assert!(!resource.has_partial_depth_warning());

        resource.warnings.push(format!(
            "normal API depth shows the first 100 only for comments; {FULL_DEPTH_WARNING_HINT} for exhaustive pagination"
        ));

        assert!(resource.has_partial_depth_warning());
    }

    #[test]
    fn maps_github_check_states_and_conclusions() {
        assert_eq!(
            CheckStatus::from_github(Some("completed"), Some("success")),
            CheckStatus::Success
        );
        assert_eq!(
            CheckStatus::from_github(Some("COMPLETED"), Some("CANCELLED")),
            CheckStatus::Skipped
        );
        assert_eq!(
            CheckStatus::from_github(Some("COMPLETED"), Some("ACTION_REQUIRED")),
            CheckStatus::Pending
        );
        assert_eq!(
            CheckStatus::from_github(Some("failure"), None),
            CheckStatus::Failure
        );
    }
}
