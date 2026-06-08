use crate::domain::{ChangedFile, PullRequest, ReactionCounts, Resource, ResourceId, ResourceKind};

pub(crate) fn issue_resource(number: u64, title: &str) -> Resource {
    Resource {
        id: ResourceId {
            owner: "owner".into(),
            repo: "repo".into(),
            number,
            kind_hint: Some(ResourceKind::Issue),
        },
        title: title.into(),
        url: format!("https://github.com/owner/repo/issues/{number}"),
        state: "OPEN".into(),
        author: "alice".into(),
        created_at: "now".into(),
        updated_at: "now".into(),
        labels: vec![],
        assignees: vec![],
        reactions: ReactionCounts::default(),
        body: "Body".into(),
        activity: vec![],
        related_resources: vec![],
        metadata: vec![],
        warnings: vec![],
        pull_request: None,
    }
}

pub(crate) fn pr_resource_with_patch(patch: Option<&str>) -> Resource {
    let mut resource = issue_resource(1, "Pull request");
    resource.id.kind_hint = Some(ResourceKind::PullRequest);
    resource.url = "https://github.com/owner/repo/pull/1".into();
    resource.pull_request = Some(PullRequest {
        base_ref: "main".into(),
        head_ref: "feature".into(),
        requested_reviewers: vec![],
        review_decision: None,
        merge_state: None,
        additions: 1,
        deletions: 0,
        commits: vec![],
        checks: vec![],
        files: vec![ChangedFile {
            path: "src/lib.rs".into(),
            additions: 1,
            deletions: 0,
            change_type: "MODIFIED".into(),
            patch: patch.map(str::to_string),
        }],
        metadata: vec![],
    });
    resource
}
