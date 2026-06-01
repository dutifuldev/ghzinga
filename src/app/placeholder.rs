use crate::domain::{
    MetadataItem, PullRequest, ReactionCounts, Resource, ResourceId, ResourceKind,
};

pub(crate) fn loading_resource_placeholder(id: ResourceId) -> Resource {
    let display_kind = id.kind_hint.unwrap_or(ResourceKind::PullRequest);
    let url_kind = id.kind_hint.unwrap_or(ResourceKind::Issue);
    let canonical = id.canonical_name();
    let repo = id.repo_name_with_owner();
    let pull_request = if display_kind == ResourceKind::PullRequest {
        Some(PullRequest {
            base_ref: "base".into(),
            head_ref: "head".into(),
            requested_reviewers: vec![],
            review_decision: None,
            merge_state: None,
            additions: 0,
            deletions: 0,
            commits: vec![],
            checks: vec![],
            files: vec![],
            metadata: vec![],
        })
    } else {
        None
    };

    Resource {
        id: id.clone(),
        title: format!("Loading {canonical}"),
        url: id.web_url_for_kind(url_kind),
        state: "LOADING".into(),
        author: "GitHub".into(),
        created_at: "now".into(),
        updated_at: "now".into(),
        labels: vec![],
        assignees: vec![],
        reactions: ReactionCounts::default(),
        body: format!(
            "Loading {canonical} from GitHub.\n\nThe TUI is ready while the API request runs. Data for {repo} will replace this placeholder as soon as GitHub responds."
        ),
        activity: vec![],
        related_resources: vec![],
        metadata: vec![MetadataItem {
            label: "startup".into(),
            value: "loading GitHub data".into(),
        }],
        warnings: vec![],
        pull_request,
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{ResourceId, ResourceKind};

    use super::loading_resource_placeholder;

    #[test]
    fn loading_placeholder_for_ambiguous_resource_shows_pr_shell() {
        let resource = loading_resource_placeholder(ResourceId {
            owner: "openclaw".into(),
            repo: "openclaw".into(),
            number: 81834,
            kind_hint: None,
        });

        assert_eq!(resource.title, "Loading openclaw/openclaw#81834");
        assert_eq!(resource.state, "LOADING");
        assert!(resource.is_pull_request());
        assert_eq!(
            resource.web_url(),
            "https://github.com/openclaw/openclaw/issues/81834"
        );
        assert!(resource.body.contains("The TUI is ready"));
    }

    #[test]
    fn loading_placeholder_for_issue_url_keeps_issue_shell() {
        let resource = loading_resource_placeholder(ResourceId {
            owner: "openclaw".into(),
            repo: "openclaw".into(),
            number: 88499,
            kind_hint: Some(ResourceKind::Issue),
        });

        assert!(!resource.is_pull_request());
        assert_eq!(
            resource.web_url(),
            "https://github.com/openclaw/openclaw/issues/88499"
        );
    }
}
