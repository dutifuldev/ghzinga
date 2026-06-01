use std::{env::VarError, error::Error, fmt, io, process::Stdio};

use tokio::process::Command;

pub(crate) async fn github_token() -> anyhow::Result<String> {
    if let Some(token) = github_token_from_env(|name| std::env::var(name)) {
        return Ok(token);
    }

    gh_auth_token().await
}

fn github_token_from_env(
    mut get_env: impl FnMut(&str) -> Result<String, VarError>,
) -> Option<String> {
    ["GH_TOKEN", "GITHUB_TOKEN"]
        .into_iter()
        .filter_map(|name| get_env(name).ok())
        .map(|token| token.trim().to_string())
        .find(|token| !token.is_empty())
}

async fn gh_auth_token() -> anyhow::Result<String> {
    let output = Command::new("gh")
        .args(["auth", "token"])
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| GithubAuthError::Unavailable(gh_execute_error("gh auth token", &error)))?;

    github_token_from_gh_output(GhAuthTokenOutput {
        success: output.status.success(),
        stdout: &output.stdout,
        stderr: &output.stderr,
    })
}

struct GhAuthTokenOutput<'a> {
    success: bool,
    stdout: &'a [u8],
    stderr: &'a [u8],
}

fn github_token_from_gh_output(output: GhAuthTokenOutput<'_>) -> anyhow::Result<String> {
    if !output.success {
        let stderr = String::from_utf8_lossy(output.stderr);
        return Err(
            GithubAuthError::Unavailable(gh_failure_message("gh auth token", &stderr)).into(),
        );
    }
    let token = String::from_utf8_lossy(output.stdout).trim().to_string();
    if token.is_empty() {
        return Err(
            GithubAuthError::Unavailable("`gh auth token` returned an empty token".into()).into(),
        );
    }
    Ok(token)
}

#[derive(Debug)]
pub(crate) enum GithubAuthError {
    Unavailable(String),
}

impl fmt::Display for GithubAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(message) => f.write_str(message),
        }
    }
}

impl Error for GithubAuthError {}

pub(crate) fn is_auth_unavailable(error: &anyhow::Error) -> bool {
    error.downcast_ref::<GithubAuthError>().is_some()
}

pub(crate) fn should_try_public_rest_fallback(error: &anyhow::Error) -> bool {
    is_auth_unavailable(error) || is_token_rejected_by_github(error)
}

fn is_token_rejected_by_github(error: &anyhow::Error) -> bool {
    let message = error
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    message.contains("http 401")
        || message.contains("bad credentials")
        || message.contains("requires authentication")
        || message.contains("resource not accessible by personal access token")
        || message.contains("although you appear to have the correct authorization credentials")
}

pub(crate) fn gh_execute_error(command: &str, error: &io::Error) -> String {
    if error.kind() == io::ErrorKind::NotFound {
        return format!(
            "GitHub CLI executable `gh` was not found while running `{command}`. Install GitHub CLI and run `gh auth status`."
        );
    }
    format!("failed to execute `{command}`: {error}")
}

pub(crate) fn gh_failure_message(command: &str, stderr: &str) -> String {
    let stderr = stderr.trim();
    if looks_like_auth_failure(stderr) {
        return format!(
            "GitHub CLI is not authenticated for `{command}`. Run `gh auth status` and `gh auth login` if needed. Details: {stderr}"
        );
    }
    if stderr.is_empty() {
        format!("`{command}` failed without an error message")
    } else {
        format!("`{command}` failed: {stderr}")
    }
}

fn looks_like_auth_failure(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("gh auth login")
        || lower.contains("not logged")
        || lower.contains("not authenticated")
        || lower.contains("authentication required")
        || lower.contains("must authenticate")
        || lower.contains("bad credentials")
        || lower.contains("http 401")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_token_prefers_gh_token_and_trims_whitespace() {
        let token = github_token_from_env(|name| match name {
            "GH_TOKEN" => Ok("  gh-token\n".into()),
            "GITHUB_TOKEN" => Ok("github-token".into()),
            _ => Err(VarError::NotPresent),
        });

        assert_eq!(token.as_deref(), Some("gh-token"));
    }

    #[test]
    fn env_token_uses_github_token_when_gh_token_is_blank() {
        let token = github_token_from_env(|name| match name {
            "GH_TOKEN" => Ok("   ".into()),
            "GITHUB_TOKEN" => Ok("\tgithub-token\n".into()),
            _ => Err(VarError::NotPresent),
        });

        assert_eq!(token.as_deref(), Some("github-token"));
    }

    #[test]
    fn env_token_returns_none_when_no_nonempty_env_token_exists() {
        let token = github_token_from_env(|name| match name {
            "GH_TOKEN" => Ok(" ".into()),
            "GITHUB_TOKEN" => Err(VarError::NotPresent),
            _ => Err(VarError::NotPresent),
        });

        assert!(token.is_none());
    }

    #[test]
    fn gh_auth_token_output_returns_trimmed_token_when_authenticated() {
        let token = github_token_from_gh_output(GhAuthTokenOutput {
            success: true,
            stdout: b"  gh-authed-token\n",
            stderr: b"",
        })
        .expect("token from gh output");

        assert_eq!(token, "gh-authed-token");
    }

    #[test]
    fn gh_auth_token_output_rejects_empty_success_output() {
        let error = github_token_from_gh_output(GhAuthTokenOutput {
            success: true,
            stdout: b"\n",
            stderr: b"",
        })
        .expect_err("empty token should fail");

        assert!(error.to_string().contains("returned an empty token"));
        assert!(is_auth_unavailable(&error));
    }

    #[test]
    fn gh_auth_token_output_reports_auth_failure_as_unavailable() {
        let error = github_token_from_gh_output(GhAuthTokenOutput {
            success: false,
            stdout: b"",
            stderr: b"To get started with GitHub CLI, please run: gh auth login",
        })
        .expect_err("auth failure should fail");

        assert!(error.to_string().contains("not authenticated"));
        assert!(is_auth_unavailable(&error));
    }

    #[test]
    fn missing_gh_error_mentions_install_and_auth_status() {
        let message = gh_execute_error(
            "gh auth token",
            &io::Error::new(io::ErrorKind::NotFound, "no gh in path"),
        );

        assert!(message.contains("`gh` was not found"));
        assert!(message.contains("gh auth status"));
    }

    #[test]
    fn auth_failure_mentions_auth_status_and_login() {
        let message = gh_failure_message(
            "gh auth token",
            "To get started with GitHub CLI, please run: gh auth login",
        );

        assert!(message.contains("not authenticated"));
        assert!(message.contains("gh auth status"));
        assert!(message.contains("gh auth login"));
    }

    #[test]
    fn non_auth_failure_keeps_command_and_stderr() {
        let message = gh_failure_message("gh auth token", "token retrieval failed");

        assert_eq!(message, "`gh auth token` failed: token retrieval failed");
    }

    #[test]
    fn public_rest_fallback_includes_unavailable_auth() {
        let error = anyhow::Error::new(GithubAuthError::Unavailable(
            "`gh auth token` returned an empty token".into(),
        ));

        assert!(should_try_public_rest_fallback(&error));
    }

    #[test]
    fn public_rest_fallback_includes_rejected_token_errors() {
        for message in [
            "GitHub GraphQL request failed with HTTP 401 Unauthorized: {\"message\":\"Bad credentials\"}",
            "GitHub GraphQL request returned errors: FORBIDDEN: Resource not accessible by personal access token",
            "GitHub GraphQL request failed: Although you appear to have the correct authorization credentials, the organization requires SAML SSO.",
        ] {
            let error = anyhow::anyhow!(message);

            assert!(should_try_public_rest_fallback(&error), "{message}");
        }
    }

    #[test]
    fn public_rest_fallback_does_not_include_unrelated_api_errors() {
        let error = anyhow::anyhow!("GitHub GraphQL request failed with HTTP 500: server error");

        assert!(!should_try_public_rest_fallback(&error));
    }
}
