use std::{future::Future, pin::Pin};

use anyhow::Context;
use serde_json::{json, Value};

use super::auth::github_token;

pub(crate) const GITHUB_GRAPHQL_URL: &str = "https://api.github.com/graphql";
const GITHUB_REST_URL: &str = "https://api.github.com";
pub(crate) const GITHUB_JSON_ACCEPT: &str = "application/vnd.github+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GithubHttpMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GithubHttpRequest {
    pub(crate) method: GithubHttpMethod,
    pub(crate) url: String,
    pub(crate) accept: String,
    pub(crate) token: Option<String>,
    pub(crate) body: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GithubHttpResponse {
    pub(crate) status: reqwest::StatusCode,
    pub(crate) body: Vec<u8>,
}

pub(crate) type GithubHttpFuture<'a> =
    Pin<Box<dyn Future<Output = anyhow::Result<GithubHttpResponse>> + Send + 'a>>;

pub(crate) trait GithubHttpTransport {
    fn execute<'a>(&'a self, request: GithubHttpRequest) -> GithubHttpFuture<'a>;
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ReqwestGithubHttpTransport;

impl GithubHttpTransport for ReqwestGithubHttpTransport {
    fn execute<'a>(&'a self, request: GithubHttpRequest) -> GithubHttpFuture<'a> {
        Box::pin(async move {
            let client = reqwest::Client::new();
            let builder = match request.method {
                GithubHttpMethod::Get => client.get(&request.url),
                GithubHttpMethod::Post => client.post(&request.url),
            }
            .header(reqwest::header::USER_AGENT, "ghzinga")
            .header(reqwest::header::ACCEPT, request.accept);
            let builder = if let Some(token) = request.token {
                builder.bearer_auth(token)
            } else {
                builder
            };
            let builder = if let Some(body) = request.body {
                builder.json(&body)
            } else {
                builder
            };
            let response = builder
                .send()
                .await
                .with_context(|| format!("failed to send GitHub request to {}", request.url))?;
            let status = response.status();
            let body = response.bytes().await.with_context(|| {
                format!("failed to read GitHub response body from {}", request.url)
            })?;
            Ok(GithubHttpResponse {
                status,
                body: body.to_vec(),
            })
        })
    }
}

pub(crate) async fn run_graphql_query(query: &str, variables: Value) -> anyhow::Result<Vec<u8>> {
    let token = github_token().await?;
    run_graphql_query_with(&ReqwestGithubHttpTransport, Some(&token), query, variables).await
}

pub(crate) async fn run_graphql_query_with(
    transport: &impl GithubHttpTransport,
    token: Option<&str>,
    query: &str,
    variables: Value,
) -> anyhow::Result<Vec<u8>> {
    let response = transport
        .execute(GithubHttpRequest {
            method: GithubHttpMethod::Post,
            url: GITHUB_GRAPHQL_URL.to_string(),
            accept: GITHUB_JSON_ACCEPT.to_string(),
            token: token.map(str::to_string),
            body: Some(json!({
                "query": query,
                "variables": variables,
            })),
        })
        .await?;
    let status = response.status;
    let body = response.body;
    if !status.is_success() {
        anyhow::bail!(
            "GitHub GraphQL request failed with HTTP {status}: {}",
            String::from_utf8_lossy(&body)
        );
    }
    if let Ok(value) = serde_json::from_slice::<Value>(&body) {
        if let Some(errors) = value.get("errors").filter(|errors| !errors.is_null()) {
            anyhow::bail!(
                "GitHub GraphQL request returned errors: {}",
                summarize_graphql_errors(errors)
            );
        }
    }
    Ok(body.to_vec())
}

fn summarize_graphql_errors(errors: &Value) -> String {
    let Some(errors) = errors.as_array() else {
        return compact_whitespace(&errors.to_string());
    };
    let mut summaries = Vec::new();
    for error in errors.iter().take(3) {
        let error_type = error
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("GraphQL error");
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .map(compact_whitespace)
            .unwrap_or_else(|| compact_whitespace(&error.to_string()));
        if error_type == "INSUFFICIENT_SCOPES" && message.contains("read:project") {
            summaries.push(
                "INSUFFICIENT_SCOPES: token lacks read:project for one or more GitHub GraphQL fields; update token scopes at https://github.com/settings/tokens".to_string(),
            );
        } else {
            summaries.push(format!("{error_type}: {message}"));
        }
    }
    if errors.len() > summaries.len() {
        summaries.push(format!("{} more error(s)", errors.len() - summaries.len()));
    }
    summaries.join("; ")
}

fn compact_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) async fn run_rest_get(path: &str, accept: &str) -> anyhow::Result<Vec<u8>> {
    let token = github_token().await?;
    run_rest_get_with(&ReqwestGithubHttpTransport, Some(&token), path, accept).await
}

pub(crate) async fn run_rest_get_with(
    transport: &impl GithubHttpTransport,
    token: Option<&str>,
    path: &str,
    accept: &str,
) -> anyhow::Result<Vec<u8>> {
    let url = format!("{GITHUB_REST_URL}{path}");
    let response = transport
        .execute(GithubHttpRequest {
            method: GithubHttpMethod::Get,
            url,
            accept: accept.to_string(),
            token: token.map(str::to_string),
            body: None,
        })
        .await
        .with_context(|| format!("failed to send GitHub REST request to {path}"))?;
    let status = response.status;
    let body = response.body;
    if !status.is_success() {
        anyhow::bail!(
            "GitHub REST request to {path} failed with HTTP {status}: {}",
            String::from_utf8_lossy(&body)
        );
    }
    Ok(body.to_vec())
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use anyhow::Context;

    use super::*;

    #[derive(Debug)]
    struct FakeGithubHttpTransport {
        requests: Mutex<Vec<GithubHttpRequest>>,
        responses: Mutex<VecDeque<GithubHttpResponse>>,
    }

    impl FakeGithubHttpTransport {
        fn new(response: GithubHttpResponse) -> Self {
            Self::from_responses(vec![response])
        }

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
    async fn graphql_transport_receives_post_shape_and_returns_body() {
        let transport = FakeGithubHttpTransport::new(GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: br#"{"data":{"ok":true}}"#.to_vec(),
        });

        let output = run_graphql_query_with(
            &transport,
            Some("token-1"),
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
        assert_eq!(request.token.as_deref(), Some("token-1"));
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

        let error = run_graphql_query_with(&transport, Some("token-1"), "query", json!({}))
            .await
            .expect_err("GraphQL errors should fail");

        assert!(error
            .to_string()
            .contains("GitHub GraphQL request returned errors"));
        assert!(error.to_string().contains("GraphQL error: bad query"));
    }

    #[tokio::test]
    async fn graphql_transport_summarizes_scope_errors() {
        let transport = FakeGithubHttpTransport::new(GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: br#"{"errors":[{"locations":[{"line":120,"column":44}],"message":"Your token has not been granted the required scopes to execute this query. The 'id' field requires one of the following scopes: ['read:project'], but your token has only been granted the: ['repo'] scopes. Please modify your token's scopes at: https://github.com/settings/tokens.","type":"INSUFFICIENT_SCOPES"},{"message":"same scope issue","type":"INSUFFICIENT_SCOPES"}]}"#.to_vec(),
        });

        let error = run_graphql_query_with(&transport, Some("token-1"), "query", json!({}))
            .await
            .expect_err("GraphQL scope errors should fail");
        let message = error.to_string();

        assert!(message.contains("INSUFFICIENT_SCOPES: token lacks read:project"));
        assert!(message.contains("https://github.com/settings/tokens"));
        assert!(!message.contains("\"locations\""));
    }

    #[tokio::test]
    async fn rest_transport_receives_get_shape_and_returns_body() {
        let transport = FakeGithubHttpTransport::new(GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: b"diff --git a/file b/file".to_vec(),
        });

        let output = run_rest_get_with(
            &transport,
            Some("token-1"),
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
        assert_eq!(request.token.as_deref(), Some("token-1"));
        assert_eq!(request.body, None);
    }

    #[tokio::test]
    async fn rest_transport_can_omit_authorization_for_public_requests() {
        let transport = FakeGithubHttpTransport::new(GithubHttpResponse {
            status: reqwest::StatusCode::OK,
            body: br#"{"ok":true}"#.to_vec(),
        });

        let output = run_rest_get_with(
            &transport,
            None,
            "/repos/openclaw/openclaw/issues/88499",
            GITHUB_JSON_ACCEPT,
        )
        .await
        .expect("REST response");

        assert_eq!(output, br#"{"ok":true}"#);
        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].token, None);
    }

    #[tokio::test]
    async fn rest_transport_includes_status_and_body_on_http_failure() {
        let transport = FakeGithubHttpTransport::new(GithubHttpResponse {
            status: reqwest::StatusCode::NOT_FOUND,
            body: br#"{"message":"Not Found"}"#.to_vec(),
        });

        let error = run_rest_get_with(&transport, Some("token-1"), "/missing", GITHUB_JSON_ACCEPT)
            .await
            .expect_err("HTTP failure should fail");

        let message = error.to_string();
        assert!(message.contains("GitHub REST request to /missing failed with HTTP 404"));
        assert!(message.contains("Not Found"));
    }
}
