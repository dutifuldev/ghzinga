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

/// What kind of GitHub object an edit rewrites; picks the update mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditKind {
    IssueBody,
    PullRequestBody,
    IssueComment,
    Review,
    ReviewComment,
}

/// An editable GitHub object the viewer is allowed to update, per the
/// server's own `viewerCanUpdate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditTarget {
    pub node_id: String,
    pub kind: EditKind,
    /// The object's current raw markdown, used to prefill the composer.
    pub current_body: String,
}

/// A menu-level action choice before any payload is collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Comment,
    Edit,
    Merge,
    Close,
    Reopen,
}

impl fmt::Display for ActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Comment => f.write_str("Comment"),
            Self::Edit => f.write_str("Edit"),
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
    Edit { target: EditTarget, body: String },
    Close,
    Reopen,
    Merge { method: MergeMethod },
}

impl ResourceAction {
    pub fn progress_label(&self) -> &'static str {
        match self {
            Self::Comment { .. } => "commenting",
            Self::Edit { .. } => "saving edit",
            Self::Close => "closing",
            Self::Reopen => "reopening",
            Self::Merge { .. } => "merging",
        }
    }

    pub fn success_label(&self) -> &'static str {
        match self {
            Self::Comment { .. } => "comment posted",
            Self::Edit { .. } => "edit saved",
            Self::Close => "closed",
            Self::Reopen => "reopened",
            Self::Merge { .. } => "merged",
        }
    }

    /// The draft text a composer submitted, when this action carries one.
    pub fn draft_body(&self) -> Option<&str> {
        match self {
            Self::Comment { body } | Self::Edit { body, .. } => Some(body),
            _ => None,
        }
    }
}

/// The resource description as an edit target, when the viewer may edit it.
pub fn body_edit_target(resource: &Resource) -> Option<EditTarget> {
    if resource.actions.node_id.is_empty() || !resource.actions.viewer_can_update {
        return None;
    }
    let kind = match resource.kind() {
        ResourceKind::Issue => EditKind::IssueBody,
        ResourceKind::PullRequest => EditKind::PullRequestBody,
    };
    Some(EditTarget {
        node_id: resource.actions.node_id.clone(),
        kind,
        current_body: resource.body.clone(),
    })
}

/// Everything the viewer may edit right now: the description first, then
/// editable activity entries in display order.
pub fn editable_targets(resource: &Resource) -> Vec<EditTarget> {
    let mut targets: Vec<EditTarget> = body_edit_target(resource).into_iter().collect();
    targets.extend(
        resource
            .activity
            .iter()
            .filter_map(|entry| entry.edit.clone()),
    );
    targets
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
    if !editable_targets(resource).is_empty() {
        actions.push(ActionKind::Edit);
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
    fn open_issue_offers_comment_edit_and_close() {
        let resource = actionable_issue();
        assert_eq!(
            available_actions(&resource),
            vec![ActionKind::Comment, ActionKind::Edit, ActionKind::Close]
        );
    }

    #[test]
    fn closed_issue_offers_comment_and_reopen() {
        let mut resource = actionable_issue();
        resource.state = "CLOSED".into();
        assert_eq!(
            available_actions(&resource),
            vec![ActionKind::Comment, ActionKind::Edit, ActionKind::Reopen]
        );
    }

    #[test]
    fn open_pr_offers_comment_edit_merge_and_close() {
        let resource = actionable_pr();
        assert_eq!(
            available_actions(&resource),
            vec![
                ActionKind::Comment,
                ActionKind::Edit,
                ActionKind::Merge,
                ActionKind::Close
            ]
        );
    }

    #[test]
    fn merged_pr_offers_only_comment() {
        let mut resource = actionable_pr();
        resource.state = "MERGED".into();
        assert_eq!(
            available_actions(&resource),
            vec![ActionKind::Comment, ActionKind::Edit]
        );
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
            vec![ActionKind::Comment, ActionKind::Edit, ActionKind::Close]
        );
    }

    #[test]
    fn locked_resource_hides_comment() {
        let mut resource = actionable_issue();
        resource.actions.locked = true;
        assert_eq!(
            available_actions(&resource),
            vec![ActionKind::Edit, ActionKind::Close]
        );
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

    #[test]
    fn body_edit_target_requires_permission() {
        let mut resource = actionable_issue();
        let target = body_edit_target(&resource).expect("editable body");
        assert_eq!(target.kind, EditKind::IssueBody);
        assert_eq!(target.node_id, "I_node");
        assert_eq!(target.current_body, resource.body);
        resource.actions.viewer_can_update = false;
        assert!(body_edit_target(&resource).is_none());
    }

    #[test]
    fn editable_targets_list_description_then_entries() {
        let mut resource = actionable_issue();
        let comment_target = EditTarget {
            node_id: "IC_1".into(),
            kind: EditKind::IssueComment,
            current_body: "a comment".into(),
        };
        resource.activity.push(crate::domain::ActivityEntry {
            id: "IC_1".into(),
            edit: Some(comment_target.clone()),
            kind: crate::domain::ActivityKind::Comment,
            author: "bob".into(),
            body: "a comment".into(),
            updated_at: "now".into(),
            path: None,
            line: None,
            url: None,
            author_association: None,
            reactions: Default::default(),
            includes_created_edit: false,
            is_minimized: false,
            minimized_reason: None,
            thread_id: None,
            thread_resolved: None,
            thread_outdated: None,
        });
        let targets = editable_targets(&resource);
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].kind, EditKind::IssueBody);
        assert_eq!(targets[1], comment_target);
    }

    #[test]
    fn draft_body_exists_only_for_text_actions() {
        assert_eq!(
            ResourceAction::Comment { body: "x".into() }.draft_body(),
            Some("x")
        );
        let edit = ResourceAction::Edit {
            target: EditTarget {
                node_id: "n".into(),
                kind: EditKind::IssueComment,
                current_body: "old".into(),
            },
            body: "new".into(),
        };
        assert_eq!(edit.draft_body(), Some("new"));
        assert_eq!(edit.progress_label(), "saving edit");
        assert_eq!(edit.success_label(), "edit saved");
        assert_eq!(ResourceAction::Close.draft_body(), None);
    }
}
