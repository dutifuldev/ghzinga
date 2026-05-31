use std::{io, process::Stdio};

use tokio::process::Command;

pub(crate) async fn github_token() -> anyhow::Result<String> {
    if let Some(token) = std::env::var("GH_TOKEN")
        .ok()
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
    {
        return Ok(token);
    }

    let output = Command::new("gh")
        .args(["auth", "token"])
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| anyhow::anyhow!(gh_execute_error("gh auth token", &error)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{}", gh_failure_message("gh auth token", &stderr));
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        anyhow::bail!("`gh auth token` returned an empty token");
    }
    Ok(token)
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
}
