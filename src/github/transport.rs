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
    pub(crate) token: String,
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
struct ReqwestGithubHttpTransport;

impl GithubHttpTransport for ReqwestGithubHttpTransport {
    fn execute<'a>(&'a self, request: GithubHttpRequest) -> GithubHttpFuture<'a> {
        Box::pin(async move {
            let client = reqwest::Client::new();
            let builder = match request.method {
                GithubHttpMethod::Get => client.get(&request.url),
                GithubHttpMethod::Post => client.post(&request.url),
            }
            .bearer_auth(request.token)
            .header(reqwest::header::USER_AGENT, "ghzinga")
            .header(reqwest::header::ACCEPT, request.accept);
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
    run_graphql_query_with(&ReqwestGithubHttpTransport, &token, query, variables).await
}

pub(crate) async fn run_graphql_query_with(
    transport: &impl GithubHttpTransport,
    token: &str,
    query: &str,
    variables: Value,
) -> anyhow::Result<Vec<u8>> {
    let response = transport
        .execute(GithubHttpRequest {
            method: GithubHttpMethod::Post,
            url: GITHUB_GRAPHQL_URL.to_string(),
            accept: GITHUB_JSON_ACCEPT.to_string(),
            token: token.to_string(),
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
    run_rest_get_with(&ReqwestGithubHttpTransport, &token, path, accept).await
}

pub(crate) async fn run_rest_get_with(
    transport: &impl GithubHttpTransport,
    token: &str,
    path: &str,
    accept: &str,
) -> anyhow::Result<Vec<u8>> {
    let url = format!("{GITHUB_REST_URL}{path}");
    let response = transport
        .execute(GithubHttpRequest {
            method: GithubHttpMethod::Get,
            url,
            accept: accept.to_string(),
            token: token.to_string(),
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
