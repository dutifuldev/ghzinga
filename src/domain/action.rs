use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::{Resource, ResourceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

impl MergeMethod {
    pub fn graphql_value(self) -> &'static str {
        match self {
            Self::Merge => "MERGE",
            Self::Squash => "SQUASH",
            Self::Rebase => "REBASE",
        }
    }
}

impl fmt::Display for MergeMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Merge => f.write_str("merge commit"),
            Self::Squash => f.write_str("squash"),
            Self::Rebase => f.write_str("rebase"),
        }
    }
}

/// A menu-level action choice before any payload is collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Comment,
    Merge,
    Close,
    Reopen,
}

impl fmt::Display for ActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Comment => f.write_str("Comment"),
            Self::Merge => f.write_str("Merge"),
            Self::Close => f.write_str("Close"),
            Self::Reopen => f.write_str("Reopen"),
        }
    }
}

/// A fully specified action ready to submit to GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceAction {
    Comment { body: String },
    Close,
    Reopen,
    Merge { method: MergeMethod },
}

impl ResourceAction {
    pub fn progress_label(&self) -> &'static str {
        match self {
            Self::Comment { .. } => "commenting",
            Self::Close => "closing",
            Self::Reopen => "reopening",
            Self::Merge { .. } => "merging",
        }
    }

    pub fn success_label(&self) -> &'static str {
        match self {
            Self::Comment { .. } => "comment posted",
            Self::Close => "closed",
            Self::Reopen => "reopened",
            Self::Merge { .. } => "merged",
        }
    }
}

/// Actions the viewer can take on this resource right now. Server-side
/// permission is the final authority; this only drives menu visibility.
pub fn available_actions(resource: &Resource) -> Vec<ActionKind> {
    if resource.actions.node_id.is_empty() {
        return Vec::new();
    }
    let mut actions = Vec::new();
    if !resource.actions.locked {
        actions.push(ActionKind::Comment);
    }
    if resource.actions.viewer_can_update {
        match resource.state.as_str() {
            "OPEN" => {
                if can_merge(resource) {
                    actions.push(ActionKind::Merge);
                }
                actions.push(ActionKind::Close);
            }
            "CLOSED" => actions.push(ActionKind::Reopen),
            _ => {}
        }
    }
    actions
}

fn can_merge(resource: &Resource) -> bool {
    resource.kind() == ResourceKind::PullRequest
        && resource
            .pull_request
            .as_ref()
            .is_some_and(|pr| !pr.allowed_merge_methods.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actionable_issue() -> Resource {
        let mut resource = crate::test_fixtures::issue_resource(7, "Actionable issue");
        resource.actions.node_id = "I_node".into();
        resource.actions.viewer_can_update = true;
        resource.pull_request = None;
        resource
    }

    fn actionable_pr() -> Resource {
        let mut resource = crate::test_fixtures::pr_resource_with_patch(None);
        resource.actions.node_id = "PR_node".into();
        resource.actions.viewer_can_update = true;
        resource
            .pull_request
            .as_mut()
            .expect("pr fixture has pull request data")
            .allowed_merge_methods = vec![MergeMethod::Squash, MergeMethod::Rebase];
        resource
    }

    #[test]
    fn no_actions_without_node_id() {
        let mut resource = actionable_issue();
        resource.actions.node_id = String::new();
        assert!(available_actions(&resource).is_empty());
    }

    #[test]
    fn open_issue_offers_comment_and_close() {
        let resource = actionable_issue();
        assert_eq!(
            available_actions(&resource),
            vec![ActionKind::Comment, ActionKind::Close]
        );
    }

    #[test]
    fn closed_issue_offers_comment_and_reopen() {
        let mut resource = actionable_issue();
        resource.state = "CLOSED".into();
        assert_eq!(
            available_actions(&resource),
            vec![ActionKind::Comment, ActionKind::Reopen]
        );
    }

    #[test]
    fn open_pr_offers_comment_merge_and_close() {
        let resource = actionable_pr();
        assert_eq!(
            available_actions(&resource),
            vec![ActionKind::Comment, ActionKind::Merge, ActionKind::Close]
        );
    }

    #[test]
    fn merged_pr_offers_only_comment() {
        let mut resource = actionable_pr();
        resource.state = "MERGED".into();
        assert_eq!(available_actions(&resource), vec![ActionKind::Comment]);
    }

    #[test]
    fn pr_without_allowed_methods_hides_merge() {
        let mut resource = actionable_pr();
        resource
            .pull_request
            .as_mut()
            .unwrap()
            .allowed_merge_methods = Vec::new();
        assert_eq!(
            available_actions(&resource),
            vec![ActionKind::Comment, ActionKind::Close]
        );
    }

    #[test]
    fn locked_resource_hides_comment() {
        let mut resource = actionable_issue();
        resource.actions.locked = true;
        assert_eq!(available_actions(&resource), vec![ActionKind::Close]);
    }

    #[test]
    fn without_update_permission_only_comment_remains() {
        let mut resource = actionable_pr();
        resource.actions.viewer_can_update = false;
        assert_eq!(available_actions(&resource), vec![ActionKind::Comment]);
    }

    #[test]
    fn merge_method_graphql_values_are_uppercase() {
        assert_eq!(MergeMethod::Merge.graphql_value(), "MERGE");
        assert_eq!(MergeMethod::Squash.graphql_value(), "SQUASH");
        assert_eq!(MergeMethod::Rebase.graphql_value(), "REBASE");
    }

    #[test]
    fn action_labels_cover_all_variants() {
        let merge = ResourceAction::Merge {
            method: MergeMethod::Squash,
        };
        assert_eq!(merge.progress_label(), "merging");
        assert_eq!(merge.success_label(), "merged");
        let comment = ResourceAction::Comment { body: "hi".into() };
        assert_eq!(comment.progress_label(), "commenting");
        assert_eq!(comment.success_label(), "comment posted");
    }
}
