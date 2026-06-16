use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command as StdCommand,
};

use anyhow::Context;
use serde_json::Value;

use crate::domain::ResourceId;

const DEFAULT_PLUGIN_ID: &str = "dutifuldev.ghzinga";
const VIEWER_ENTRYPOINT: &str = "viewer";

pub fn run_command(args: &[String]) -> anyhow::Result<i32> {
    let Some(command) = args.first().map(String::as_str) else {
        print_usage_to_stderr();
        return Ok(2);
    };

    match command {
        "open" => run_open_entrypoint(),
        "viewer" => run_viewer_entrypoint(),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(0)
        }
        _ => {
            print_usage_to_stderr();
            Ok(2)
        }
    }
}

fn run_open_entrypoint() -> anyhow::Result<i32> {
    let clicked_url = required_env("HERDR_PLUGIN_CLICKED_URL")?;
    let source_pane = required_env("HERDR_PANE_ID")?;
    let target = normalize_github_issue_or_pr_url(&clicked_url)?;
    let herdr = env::var_os("HERDR_BIN_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("herdr"));
    let plugin_id = env::var("HERDR_PLUGIN_ID").unwrap_or_else(|_| DEFAULT_PLUGIN_ID.into());
    let state_dir = plugin_state_dir();
    fs::create_dir_all(&state_dir)
        .with_context(|| format!("failed to create {}", state_dir.display()))?;

    if let Some(session) = read_stored_line(&viewer_session_file(&state_dir, &source_pane))? {
        if focus_is_our_viewer(&herdr, &source_pane, &plugin_id)? {
            return run_ghzinga_open(&session, &target);
        }
    } else if focus_is_our_viewer(&herdr, &source_pane, &plugin_id)? {
        return run_ghzinga_open_for_current_context(&target);
    }

    let source_key = herdr_source_key(&source_pane);
    let session = format!("herdr-ghzinga-{source_key}");
    let state_file = state_dir.join(format!("{source_key}.pane"));

    if let Some(stored_pane) = read_stored_line(&state_file)? {
        if herdr_command_succeeds(&herdr, ["pane", "get", stored_pane.as_str()])
            && focus_is_our_viewer(&herdr, &stored_pane, &plugin_id)?
        {
            return run_ghzinga_open(&session, &target);
        }
    }

    let viewer_bin = ghzinga_viewer_bin();
    let response = run_herdr_capture(
        &herdr,
        vec![
            "plugin".into(),
            "pane".into(),
            "open".into(),
            "--plugin".into(),
            plugin_id,
            "--entrypoint".into(),
            VIEWER_ENTRYPOINT.into(),
            "--placement".into(),
            "split".into(),
            "--target-pane".into(),
            source_pane,
            "--direction".into(),
            "right".into(),
            "--env".into(),
            format!("GHZINGA_TARGET={target}"),
            "--env".into(),
            format!("GHZINGA_SESSION={session}"),
            "--env".into(),
            format!("GHZINGA_BIN={}", viewer_bin.display()),
            "--focus".into(),
        ],
    )?;
    print!("{response}");
    if !response.ends_with('\n') {
        println!();
    }

    if let Some(opened_pane) = plugin_pane_id_from_response(&response) {
        fs::write(&state_file, format!("{opened_pane}\n"))
            .with_context(|| format!("failed to write {}", state_file.display()))?;
        let session_file = viewer_session_file(&state_dir, &opened_pane);
        fs::write(&session_file, format!("{session}\n"))
            .with_context(|| format!("failed to write {}", session_file.display()))?;
    } else {
        eprintln!("ghzinga-herdr: warning: could not find opened pane id in Herdr response");
    }

    Ok(0)
}

fn run_viewer_entrypoint() -> anyhow::Result<i32> {
    let target = required_env("GHZINGA_TARGET")?;
    let session = env::var("GHZINGA_SESSION").unwrap_or_else(|_| "herdr-ghzinga".into());
    let bin = ghzinga_viewer_bin();
    let mut command = StdCommand::new(&bin);
    command.arg("--session").arg(&session).arg(&target);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        Err(error).with_context(|| format!("failed to exec {}", bin.display()))
    }

    #[cfg(not(unix))]
    {
        let status = command
            .status()
            .with_context(|| format!("failed to run {}", bin.display()))?;
        Ok(status.code().unwrap_or(1))
    }
}

fn run_ghzinga_open(session: &str, target: &str) -> anyhow::Result<i32> {
    let bin = ghzinga_control_bin();
    let status = StdCommand::new(&bin)
        .arg("open")
        .arg("--session")
        .arg(session)
        .arg(target)
        .status()
        .with_context(|| format!("failed to run {}", bin.display()))?;
    Ok(status.code().unwrap_or(1))
}

fn run_ghzinga_open_for_current_context(target: &str) -> anyhow::Result<i32> {
    let bin = ghzinga_control_bin();
    let status = StdCommand::new(&bin)
        .arg("open")
        .arg(target)
        .status()
        .with_context(|| format!("failed to run {}", bin.display()))?;
    Ok(status.code().unwrap_or(1))
}

fn required_env(name: &str) -> anyhow::Result<String> {
    let value = env::var(name).with_context(|| format!("{name} is not set"))?;
    if value.is_empty() {
        anyhow::bail!("{name} is not set");
    }
    Ok(value)
}

fn normalize_github_issue_or_pr_url(input: &str) -> anyhow::Result<String> {
    let trimmed = input.trim();
    if !trimmed.starts_with("https://github.com/") {
        anyhow::bail!("unsupported GitHub issue/PR URL: {input}");
    }
    let id = ResourceId::parse(trimmed)
        .map_err(|_| anyhow::anyhow!("unsupported GitHub issue/PR URL: {input}"))?;
    let kind = id
        .kind_hint
        .ok_or_else(|| anyhow::anyhow!("unsupported GitHub issue/PR URL: {input}"))?;
    Ok(id.web_url_for_kind(kind))
}

fn plugin_state_dir() -> PathBuf {
    env::var_os("HERDR_PLUGIN_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("ghzinga-herdr-plugin"))
}

fn viewer_session_file(state_dir: &Path, pane_id: &str) -> PathBuf {
    state_dir.join(format!("{}.session", herdr_source_key(pane_id)))
}

fn herdr_source_key(source_pane: &str) -> String {
    format!("{}_{}", herdr_scope_key(), state_key_for_pane(source_pane))
}

fn herdr_scope_key() -> String {
    if let Ok(socket) = env::var("HERDR_SOCKET_PATH") {
        if !socket.is_empty() {
            return format!("socket_{}", stable_key_for_text(&socket));
        }
    }
    if let Ok(session) = env::var("HERDR_SESSION") {
        if !session.is_empty() {
            return format!("session_{}", stable_key_for_text(&session));
        }
    }
    "default".into()
}

fn state_key_for_pane(pane_id: &str) -> String {
    pane_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn stable_key_for_text(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn read_stored_line(path: &Path) -> anyhow::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(raw) => Ok(raw
            .lines()
            .next()
            .map(str::trim)
            .filter(|pane| !pane.is_empty())
            .map(str::to_string)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn focus_is_our_viewer(herdr: &Path, pane_id: &str, plugin_id: &str) -> anyhow::Result<bool> {
    let output = StdCommand::new(herdr)
        .arg("plugin")
        .arg("pane")
        .arg("focus")
        .arg(pane_id)
        .output()
        .with_context(|| format!("failed to run {}", herdr.display()))?;
    if !output.status.success() {
        return Ok(false);
    }
    let response = String::from_utf8_lossy(&output.stdout);
    Ok(plugin_focuses_viewer(&response, plugin_id))
}

fn herdr_command_succeeds<'a>(herdr: &Path, args: impl IntoIterator<Item = &'a str>) -> bool {
    StdCommand::new(herdr)
        .args(args)
        .status()
        .is_ok_and(|status| status.success())
}

fn run_herdr_capture(herdr: &Path, args: Vec<String>) -> anyhow::Result<String> {
    let output = StdCommand::new(herdr)
        .args(&args)
        .output()
        .with_context(|| format!("failed to run {}", herdr.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        stderr.trim().to_string()
    };
    anyhow::bail!(
        "herdr {} failed: {}",
        args.join(" "),
        if detail.is_empty() {
            "no output"
        } else {
            &detail
        }
    );
}

fn plugin_focuses_viewer(response: &str, plugin_id: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(response) else {
        return false;
    };
    value
        .pointer("/result/plugin_pane/plugin_id")
        .and_then(Value::as_str)
        == Some(plugin_id)
        && value
            .pointer("/result/plugin_pane/entrypoint")
            .and_then(Value::as_str)
            == Some(VIEWER_ENTRYPOINT)
}

fn plugin_pane_id_from_response(response: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(response).ok()?;
    value
        .pointer("/result/plugin_pane/pane/pane_id")
        .or_else(|| value.pointer("/result/plugin_pane/pane_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn ghzinga_viewer_bin() -> PathBuf {
    env_path("GHZINGA_BIN")
        .or_else(|| env::current_exe().ok())
        .or_else(|| command_on_path("gzg"))
        .or_else(|| command_on_path("ghzinga"))
        .unwrap_or_else(|| PathBuf::from("gzg"))
}

fn ghzinga_control_bin() -> PathBuf {
    env_path("GHZINGA_BIN")
        .or_else(|| command_on_path("gzg"))
        .or_else(|| command_on_path("ghzinga"))
        .or_else(|| env::current_exe().ok())
        .unwrap_or_else(|| PathBuf::from("gzg"))
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn command_on_path(command: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|path| path.join(command))
        .find(|candidate| candidate.is_file())
}

fn print_usage() {
    println!("usage: gzg herdr-plugin <open|viewer>");
}

fn print_usage_to_stderr() {
    eprintln!("usage: gzg herdr-plugin <open|viewer>");
}

#[cfg(test)]
mod tests {
    use std::{env, ffi::OsString, fs, path::PathBuf, sync::Mutex};

    use super::{
        herdr_source_key, normalize_github_issue_or_pr_url, plugin_focuses_viewer,
        plugin_pane_id_from_response, run_open_entrypoint, stable_key_for_text, state_key_for_pane,
    };

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const PLUGIN_ENV_KEYS: &[&str] = &[
        "GZG_FAKE_LOG",
        "GHZINGA_BIN",
        "HERDR_BIN_PATH",
        "HERDR_FAKE_EXISTING_PANE",
        "HERDR_FAKE_LOG",
        "HERDR_FAKE_OPENED_PANE",
        "HERDR_FAKE_PLUGIN_ENTRYPOINT",
        "HERDR_FAKE_PLUGIN_ID",
        "HERDR_FAKE_PLUGIN_PANE",
        "HERDR_PANE_ID",
        "HERDR_PLUGIN_CLICKED_URL",
        "HERDR_PLUGIN_ID",
        "HERDR_PLUGIN_STATE_DIR",
        "HERDR_SESSION",
        "HERDR_SOCKET_PATH",
    ];

    fn first_file_with_extension(dir: &std::path::Path, extension: &str) -> PathBuf {
        let mut files = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some(extension))
            .collect::<Vec<_>>();
        files.sort();
        files.into_iter().next().unwrap()
    }

    struct EnvRestore {
        values: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvRestore {
        fn clear() -> Self {
            let values = PLUGIN_ENV_KEYS
                .iter()
                .map(|key| {
                    let value = env::var_os(key);
                    env::remove_var(key);
                    (*key, value)
                })
                .collect();
            Self { values }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                if let Some(value) = value {
                    env::set_var(key, value);
                } else {
                    env::remove_var(key);
                }
            }
        }
    }

    #[test]
    fn normalize_github_issue_or_pr_url_strips_suffixes_and_preserves_kind() {
        assert_eq!(
            normalize_github_issue_or_pr_url(
                "https://github.com/dutifuldev/ghzinga/pull/29/files#diff"
            )
            .unwrap(),
            "https://github.com/dutifuldev/ghzinga/pull/29"
        );
        assert_eq!(
            normalize_github_issue_or_pr_url(
                "https://github.com/dutifuldev/ghzinga/issues/32/?utm_source=test#note"
            )
            .unwrap(),
            "https://github.com/dutifuldev/ghzinga/issues/32"
        );
    }

    #[test]
    fn normalize_github_issue_or_pr_url_rejects_other_github_paths() {
        let error =
            normalize_github_issue_or_pr_url("https://github.com/dutifuldev/ghzinga/tree/main")
                .unwrap_err();
        assert!(error
            .to_string()
            .contains("unsupported GitHub issue/PR URL"));
    }

    #[test]
    fn pane_and_scope_keys_are_stable_for_state_files() {
        assert_eq!(state_key_for_pane("w1:p1"), "w1_p1");
        assert_eq!(state_key_for_pane("tab/pane:3"), "tab_pane_3");
        assert_eq!(
            stable_key_for_text("/tmp/herdr-a.sock"),
            stable_key_for_text("/tmp/herdr-a.sock")
        );
        assert_ne!(
            stable_key_for_text("/tmp/herdr-a.sock"),
            stable_key_for_text("/tmp/herdr-b.sock")
        );
    }

    #[test]
    fn plugin_focus_validation_requires_matching_plugin_and_viewer() {
        let response = r#"{"result":{"plugin_pane":{"plugin_id":"dutifuldev.ghzinga","entrypoint":"viewer","pane":{"pane_id":"w1:p9"}}}}"#;
        assert!(plugin_focuses_viewer(response, "dutifuldev.ghzinga"));
        assert!(!plugin_focuses_viewer(response, "example.other"));

        let wrong_entrypoint = r#"{"result":{"plugin_pane":{"plugin_id":"dutifuldev.ghzinga","entrypoint":"other","pane":{"pane_id":"w1:p9"}}}}"#;
        assert!(!plugin_focuses_viewer(
            wrong_entrypoint,
            "dutifuldev.ghzinga"
        ));
    }

    #[test]
    fn plugin_pane_id_is_read_from_herdr_response() {
        let response = r#"{"result":{"plugin_pane":{"pane":{"pane_id":"w1:p9"}}}}"#;
        assert_eq!(
            plugin_pane_id_from_response(response).as_deref(),
            Some("w1:p9")
        );
    }

    #[test]
    fn open_entrypoint_opens_new_plugin_pane_and_records_state() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvRestore::clear();
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("state");
        let herdr_log = temp.path().join("herdr.log");
        let gzg_log = temp.path().join("gzg.log");
        let socket = temp.path().join("herdr.sock");

        env::set_var(
            "HERDR_PLUGIN_CLICKED_URL",
            "https://github.com/dutifuldev/ghzinga/pull/29/files#diff",
        );
        env::set_var("HERDR_PANE_ID", "w1:p1");
        env::set_var("HERDR_PLUGIN_STATE_DIR", &state_dir);
        env::set_var("HERDR_SOCKET_PATH", &socket);
        env::set_var(
            "HERDR_BIN_PATH",
            repo_file("plugins/herdr/test/fake-herdr.sh"),
        );
        env::set_var("HERDR_FAKE_LOG", &herdr_log);
        env::set_var("GHZINGA_BIN", repo_file("plugins/herdr/test/fake-gzg.sh"));
        env::set_var("GZG_FAKE_LOG", &gzg_log);

        assert_eq!(run_open_entrypoint().unwrap(), 0);

        let herdr_log = fs::read_to_string(herdr_log).unwrap();
        assert!(herdr_log.contains(
            "plugin pane open --plugin dutifuldev.ghzinga --entrypoint viewer --placement split"
        ));
        assert!(herdr_log
            .contains("--env GHZINGA_TARGET=https://github.com/dutifuldev/ghzinga/pull/29"));
        assert!(herdr_log.contains("--env GHZINGA_SESSION=herdr-ghzinga-socket_"));
        assert!(herdr_log.contains("_w1_p1"));

        assert_eq!(
            fs::read_to_string(first_file_with_extension(&state_dir, "pane")).unwrap(),
            "w1:p9\n"
        );
        assert!(
            fs::read_to_string(first_file_with_extension(&state_dir, "session"))
                .unwrap()
                .starts_with("herdr-ghzinga-socket_")
        );
        assert!(!gzg_log.exists());
    }

    #[test]
    fn open_entrypoint_reuses_existing_matching_viewer_pane() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvRestore::clear();
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("state");
        fs::create_dir_all(&state_dir).unwrap();
        let herdr_log = temp.path().join("herdr.log");
        let gzg_log = temp.path().join("gzg.log");
        let socket = temp.path().join("herdr.sock");

        env::set_var(
            "HERDR_PLUGIN_CLICKED_URL",
            "https://github.com/dutifuldev/ghzinga/issues/32/?utm_source=test#note",
        );
        env::set_var("HERDR_PANE_ID", "w1:p1");
        env::set_var("HERDR_PLUGIN_STATE_DIR", &state_dir);
        env::set_var("HERDR_SOCKET_PATH", &socket);
        env::set_var(
            "HERDR_BIN_PATH",
            repo_file("plugins/herdr/test/fake-herdr.sh"),
        );
        env::set_var("HERDR_FAKE_LOG", &herdr_log);
        env::set_var("HERDR_FAKE_EXISTING_PANE", "w1:p9");
        env::set_var("HERDR_FAKE_PLUGIN_PANE", "w1:p9");
        env::set_var("GHZINGA_BIN", repo_file("plugins/herdr/test/fake-gzg.sh"));
        env::set_var("GZG_FAKE_LOG", &gzg_log);

        let source_key = herdr_source_key("w1:p1");
        fs::write(state_dir.join(format!("{source_key}.pane")), "w1:p9\n").unwrap();

        assert_eq!(run_open_entrypoint().unwrap(), 0);

        let herdr_log = fs::read_to_string(herdr_log).unwrap();
        assert!(herdr_log.contains("pane get w1:p9"));
        assert!(herdr_log.contains("plugin pane focus w1:p9"));
        assert!(!herdr_log.contains("plugin pane open"));

        let gzg_log = fs::read_to_string(gzg_log).unwrap();
        assert!(gzg_log.contains(&format!(
            "open --session herdr-ghzinga-{source_key} https://github.com/dutifuldev/ghzinga/issues/32"
        )));
    }

    #[test]
    fn open_entrypoint_reuses_current_viewer_when_link_originates_inside_ghzinga() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvRestore::clear();
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("state");
        let initial_herdr_log = temp.path().join("initial-herdr.log");
        let initial_gzg_log = temp.path().join("initial-gzg.log");
        let self_herdr_log = temp.path().join("self-herdr.log");
        let self_gzg_log = temp.path().join("self-gzg.log");
        let socket = temp.path().join("herdr.sock");

        env::set_var(
            "HERDR_PLUGIN_CLICKED_URL",
            "https://github.com/dutifuldev/ghzinga/pull/29",
        );
        env::set_var("HERDR_PANE_ID", "w1:p1");
        env::set_var("HERDR_PLUGIN_STATE_DIR", &state_dir);
        env::set_var("HERDR_SOCKET_PATH", &socket);
        env::set_var(
            "HERDR_BIN_PATH",
            repo_file("plugins/herdr/test/fake-herdr.sh"),
        );
        env::set_var("HERDR_FAKE_LOG", &initial_herdr_log);
        env::set_var("HERDR_FAKE_OPENED_PANE", "w1:p9");
        env::set_var("GHZINGA_BIN", repo_file("plugins/herdr/test/fake-gzg.sh"));
        env::set_var("GZG_FAKE_LOG", &initial_gzg_log);

        assert_eq!(run_open_entrypoint().unwrap(), 0);

        env::set_var(
            "HERDR_PLUGIN_CLICKED_URL",
            "https://github.com/dutifuldev/ghzinga/issues/32",
        );
        env::set_var("HERDR_PANE_ID", "w1:p9");
        env::set_var("HERDR_FAKE_LOG", &self_herdr_log);
        env::set_var("HERDR_FAKE_PLUGIN_PANE", "w1:p9");
        env::set_var("GZG_FAKE_LOG", &self_gzg_log);

        assert_eq!(run_open_entrypoint().unwrap(), 0);

        let source_key = herdr_source_key("w1:p1");
        let self_herdr_log = fs::read_to_string(self_herdr_log).unwrap();
        assert!(self_herdr_log.contains("plugin pane focus w1:p9"));
        assert!(!self_herdr_log.contains("plugin pane open"));

        let self_gzg_log = fs::read_to_string(self_gzg_log).unwrap();
        assert!(self_gzg_log.contains(&format!(
            "open --session herdr-ghzinga-{source_key} https://github.com/dutifuldev/ghzinga/issues/32"
        )));
    }

    #[test]
    fn open_entrypoint_uses_current_viewer_context_when_reverse_state_is_missing() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvRestore::clear();
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("state");
        let herdr_log = temp.path().join("herdr.log");
        let gzg_log = temp.path().join("gzg.log");
        let socket = temp.path().join("herdr.sock");

        env::set_var(
            "HERDR_PLUGIN_CLICKED_URL",
            "https://github.com/dutifuldev/ghzinga/pull/38",
        );
        env::set_var("HERDR_PANE_ID", "w1:p9");
        env::set_var("HERDR_PLUGIN_STATE_DIR", &state_dir);
        env::set_var("HERDR_SOCKET_PATH", &socket);
        env::set_var(
            "HERDR_BIN_PATH",
            repo_file("plugins/herdr/test/fake-herdr.sh"),
        );
        env::set_var("HERDR_FAKE_LOG", &herdr_log);
        env::set_var("HERDR_FAKE_PLUGIN_PANE", "w1:p9");
        env::set_var("GHZINGA_BIN", repo_file("plugins/herdr/test/fake-gzg.sh"));
        env::set_var("GZG_FAKE_LOG", &gzg_log);

        assert_eq!(run_open_entrypoint().unwrap(), 0);

        let herdr_log = fs::read_to_string(herdr_log).unwrap();
        assert!(herdr_log.contains("plugin pane focus w1:p9"));
        assert!(!herdr_log.contains("plugin pane open"));

        let gzg_log = fs::read_to_string(gzg_log).unwrap();
        assert!(gzg_log.contains("open https://github.com/dutifuldev/ghzinga/pull/38"));
        assert!(!gzg_log.contains("--session"));
    }

    #[test]
    fn open_entrypoint_reopens_when_stored_pane_is_not_our_viewer() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvRestore::clear();
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("state");
        fs::create_dir_all(&state_dir).unwrap();
        let herdr_log = temp.path().join("herdr.log");
        let gzg_log = temp.path().join("gzg.log");
        let socket = temp.path().join("herdr.sock");

        env::set_var(
            "HERDR_PLUGIN_CLICKED_URL",
            "https://github.com/dutifuldev/ghzinga/pull/35",
        );
        env::set_var("HERDR_PANE_ID", "w1:p1");
        env::set_var("HERDR_PLUGIN_STATE_DIR", &state_dir);
        env::set_var("HERDR_SOCKET_PATH", &socket);
        env::set_var(
            "HERDR_BIN_PATH",
            repo_file("plugins/herdr/test/fake-herdr.sh"),
        );
        env::set_var("HERDR_FAKE_LOG", &herdr_log);
        env::set_var("HERDR_FAKE_EXISTING_PANE", "w1:p7");
        env::set_var("HERDR_FAKE_PLUGIN_PANE", "w1:p7");
        env::set_var("HERDR_FAKE_PLUGIN_ID", "example.other");
        env::set_var("GHZINGA_BIN", repo_file("plugins/herdr/test/fake-gzg.sh"));
        env::set_var("GZG_FAKE_LOG", &gzg_log);

        let source_key = herdr_source_key("w1:p1");
        fs::write(state_dir.join(format!("{source_key}.pane")), "w1:p7\n").unwrap();

        assert_eq!(run_open_entrypoint().unwrap(), 0);

        let herdr_log = fs::read_to_string(herdr_log).unwrap();
        assert!(herdr_log.contains("pane get w1:p7"));
        assert!(herdr_log.contains("plugin pane focus w1:p7"));
        assert!(herdr_log.contains("plugin pane open --plugin dutifuldev.ghzinga"));
        assert!(!gzg_log.exists());
    }

    fn repo_file(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
    }
}
