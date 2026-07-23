use serde_json::{json, Value};

use crate::domain::{ResourceAction, ResourceKind};
use crate::github::auth::github_token;
use crate::github::transport::{
    run_graphql_query_with, GithubHttpTransport, ReqwestGithubHttpTransport,
};

/// Submit a write action against the issue or pull request with the given
/// GraphQL node id. Mutations always require a token; there is no public
/// fallback for writes.
pub async fn submit_resource_action(
    node_id: &str,
    kind: ResourceKind,
    action: &ResourceAction,
) -> anyhow::Result<()> {
    let token = github_token().await?;
    submit_resource_action_with(
        &ReqwestGithubHttpTransport,
        Some(&token),
        node_id,
        kind,
        action,
    )
    .await
}

pub(crate) async fn submit_resource_action_with(
    transport: &impl GithubHttpTransport,
    token: Option<&str>,
    node_id: &str,
    kind: ResourceKind,
    action: &ResourceAction,
) -> anyhow::Result<()> {
    let (mutation, variables) = action_mutation(node_id, kind, action);
    run_graphql_query_with(transport, token, mutation, variables).await?;
    Ok(())
}

fn action_mutation(
    node_id: &str,
    kind: ResourceKind,
    action: &ResourceAction,
) -> (&'static str, Value) {
    match action {
        ResourceAction::Comment { body } => (
            "mutation($id: ID!, $body: String!) { addComment(input: {subjectId: $id, body: $body}) { clientMutationId } }",
            json!({ "id": node_id, "body": body }),
        ),
        ResourceAction::Close => (state_mutation(kind, StateChange::Close), id_variables(node_id)),
        ResourceAction::Reopen => (
            state_mutation(kind, StateChange::Reopen),
            id_variables(node_id),
        ),
        ResourceAction::Merge { method } => (
            "mutation($id: ID!, $method: PullRequestMergeMethod!) { mergePullRequest(input: {pullRequestId: $id, mergeMethod: $method}) { clientMutationId } }",
            json!({ "id": node_id, "method": method.graphql_value() }),
        ),
    }
}

#[derive(Clone, Copy)]
enum StateChange {
    Close,
    Reopen,
}

fn state_mutation(kind: ResourceKind, change: StateChange) -> &'static str {
    match (kind, change) {
        (ResourceKind::Issue, StateChange::Close) => {
            "mutation($id: ID!) { closeIssue(input: {issueId: $id}) { clientMutationId } }"
        }
        (ResourceKind::Issue, StateChange::Reopen) => {
            "mutation($id: ID!) { reopenIssue(input: {issueId: $id}) { clientMutationId } }"
        }
        (ResourceKind::PullRequest, StateChange::Close) => {
            "mutation($id: ID!) { closePullRequest(input: {pullRequestId: $id}) { clientMutationId } }"
        }
        (ResourceKind::PullRequest, StateChange::Reopen) => {
            "mutation($id: ID!) { reopenPullRequest(input: {pullRequestId: $id}) { clientMutationId } }"
        }
    }
}

fn id_variables(node_id: &str) -> Value {
    json!({ "id": node_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::MergeMethod;
    use crate::github::transport::{GithubHttpFuture, GithubHttpRequest, GithubHttpResponse};
    use std::sync::Mutex;

    #[test]
    fn comment_mutation_targets_subject_with_body() {
        let (mutation, variables) = action_mutation(
            "PR_node",
            ResourceKind::PullRequest,
            &ResourceAction::Comment {
                body: "hello\nworld".into(),
            },
        );
        assert!(mutation.contains("addComment"));
        assert_eq!(variables["id"], "PR_node");
        assert_eq!(variables["body"], "hello\nworld");
    }

    #[test]
    fn close_and_reopen_pick_the_kind_specific_mutation() {
        let cases = [
            (ResourceKind::Issue, ResourceAction::Close, "closeIssue"),
            (ResourceKind::Issue, ResourceAction::Reopen, "reopenIssue"),
            (
                ResourceKind::PullRequest,
                ResourceAction::Close,
                "closePullRequest",
            ),
            (
                ResourceKind::PullRequest,
                ResourceAction::Reopen,
                "reopenPullRequest",
            ),
        ];
        for (kind, action, expected) in cases {
            let (mutation, variables) = action_mutation("node", kind, &action);
            assert!(
                mutation.contains(expected),
                "expected {expected} for {kind:?} {action:?}"
            );
            assert_eq!(variables["id"], "node");
        }
    }

    #[test]
    fn merge_mutation_carries_uppercase_method() {
        let (mutation, variables) = action_mutation(
            "PR_node",
            ResourceKind::PullRequest,
            &ResourceAction::Merge {
                method: MergeMethod::Squash,
            },
        );
        assert!(mutation.contains("mergePullRequest"));
        assert!(mutation.contains("PullRequestMergeMethod"));
        assert_eq!(variables["method"], "SQUASH");
    }

    struct OneShotTransport {
        request: Mutex<Option<GithubHttpRequest>>,
        response: Mutex<Option<anyhow::Result<GithubHttpResponse>>>,
    }

    impl OneShotTransport {
        fn replying(body: &str) -> Self {
            Self {
                request: Mutex::new(None),
                response: Mutex::new(Some(Ok(GithubHttpResponse {
                    status: reqwest::StatusCode::OK,
                    body: body.as_bytes().to_vec(),
                }))),
            }
        }

        fn sent_request(&self) -> GithubHttpRequest {
            self.request
                .lock()
                .expect("request lock")
                .clone()
                .expect("one request sent")
        }
    }

    impl GithubHttpTransport for OneShotTransport {
        fn execute<'a>(&'a self, request: GithubHttpRequest) -> GithubHttpFuture<'a> {
            *self.request.lock().expect("request lock") = Some(request);
            let response = self
                .response
                .lock()
                .expect("response lock")
                .take()
                .expect("single response available");
            Box::pin(async move { response })
        }
    }

    #[tokio::test]
    async fn submit_sends_authenticated_graphql_post() {
        let transport = OneShotTransport::replying(r#"{"data":{"closeIssue":{}}}"#);
        submit_resource_action_with(
            &transport,
            Some("token-123"),
            "I_node",
            ResourceKind::Issue,
            &ResourceAction::Close,
        )
        .await
        .expect("mutation succeeds");
        let request = transport.sent_request();
        assert_eq!(request.token.as_deref(), Some("token-123"));
        let body = request.body.expect("graphql body");
        assert!(body["query"]
            .as_str()
            .expect("query string")
            .contains("closeIssue"));
        assert_eq!(body["variables"]["id"], "I_node");
    }

    #[tokio::test]
    async fn submit_surfaces_graphql_errors() {
        let transport = OneShotTransport::replying(
            r#"{"errors":[{"message":"Pull request is not mergeable"}]}"#,
        );
        let error = submit_resource_action_with(
            &transport,
            Some("token-123"),
            "PR_node",
            ResourceKind::PullRequest,
            &ResourceAction::Merge {
                method: MergeMethod::Squash,
            },
        )
        .await
        .expect_err("mutation fails");
        assert!(error.to_string().contains("not mergeable"));
    }
}
