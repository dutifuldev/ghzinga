use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;

fn gzg_command() -> Command {
    let config_path = std::env::temp_dir().join("ghzinga-cli-once-empty-config.toml");
    let _ = std::fs::remove_file(&config_path);
    let mut cmd = Command::cargo_bin("gzg").unwrap();
    cmd.env("GZG_CONFIG_PATH", config_path);
    cmd
}

fn ghzinga_command() -> Command {
    let config_path = std::env::temp_dir().join("ghzinga-cli-once-alias-empty-config.toml");
    let _ = std::fs::remove_file(&config_path);
    let mut cmd = Command::cargo_bin("ghzinga").unwrap();
    cmd.env("GZG_CONFIG_PATH", config_path);
    cmd
}

#[test]
fn once_renders_pr_fixture_through_binary() {
    let mut cmd = gzg_command();

    cmd.args([
        "openclaw/openclaw#81834",
        "--offline-fixture",
        "fixtures/pr-81834.json",
        "--once",
    ])
    .assert()
    .success()
    .stdout(contains("openclaw/openclaw#81834"))
    .stdout(contains(
        "[Overview] Activity  Commits  Checks  Files  Links",
    ))
    .stdout(contains("* @KLilyZ opened"))
    .stdout(contains("checks PASS"));
}

#[test]
fn once_renders_pr_fixture_through_long_binary_name() {
    let mut cmd = ghzinga_command();

    cmd.args([
        "openclaw/openclaw#81834",
        "--offline-fixture",
        "fixtures/pr-81834.json",
        "--once",
    ])
    .assert()
    .success()
    .stdout(contains("openclaw/openclaw#81834"))
    .stdout(contains(
        "[Overview] Activity  Commits  Checks  Files  Links",
    ))
    .stdout(contains("checks PASS"));
}

#[test]
fn once_renders_issue_fixture_through_binary() {
    let mut cmd = gzg_command();

    cmd.args([
        "openclaw/openclaw#66943",
        "--offline-fixture",
        "fixtures/issue-66943.json",
        "--once",
    ])
    .assert()
    .success()
    .stdout(contains("openclaw/openclaw#66943"))
    .stdout(contains("[Overview] Activity  Links"))
    .stdout(contains("Related PR"));
}

#[test]
fn once_can_render_pr_checks_tab() {
    let mut cmd = gzg_command();

    cmd.args([
        "openclaw/openclaw#81834",
        "--offline-fixture",
        "fixtures/pr-81834.json",
        "--tab",
        "checks",
        "--once",
    ])
    .assert()
    .success()
    .stdout(contains("[Checks]"))
    .stdout(contains("Passing (5)"))
    .stdout(contains("suite/CI"))
    .stdout(contains("[+ more]"));
}

#[test]
fn once_can_render_pr_files_tab() {
    let mut cmd = gzg_command();

    cmd.args([
        "openclaw/openclaw#81834",
        "--offline-fixture",
        "fixtures/pr-81834.json",
        "--tab",
        "files",
        "--once",
    ])
    .assert()
    .success()
    .stdout(contains("[Files]"))
    .stdout(contains("extensions/senseaudio/index.ts"))
    .stdout(contains("[+ more]"));
}

#[test]
fn once_can_render_emoji_symbols_when_requested() {
    let mut cmd = gzg_command();

    cmd.args([
        "openclaw/openclaw#81834",
        "--offline-fixture",
        "fixtures/pr-81834.json",
        "--symbols",
        "emoji",
        "--tab",
        "checks",
        "--once",
    ])
    .assert()
    .success()
    .stdout(contains("[➕ more]"));
}

#[test]
fn once_uses_config_symbols_when_cli_does_not_override() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "[ui]\ntheme = \"solarized-dark\"\nsymbols = \"emoji\"\nspacing = \"compact\"\n",
    )
    .unwrap();
    let mut cmd = Command::cargo_bin("gzg").unwrap();

    cmd.env("GZG_CONFIG_PATH", config_path)
        .args([
            "openclaw/openclaw#81834",
            "--offline-fixture",
            "fixtures/pr-81834.json",
            "--tab",
            "checks",
            "--once",
        ])
        .assert()
        .success()
        .stdout(contains("[➕ more]"))
        .stdout(contains("bg: Rgb(0, 43, 54)"))
        .stdout(contains("\"Summary: PASS"));
}

#[test]
fn once_cli_ui_flags_override_saved_config() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "[ui]\ntheme = \"solarized-dark\"\nsymbols = \"emoji\"\nspacing = \"compact\"\n",
    )
    .unwrap();
    let mut cmd = Command::cargo_bin("gzg").unwrap();

    cmd.env("GZG_CONFIG_PATH", config_path)
        .args([
            "openclaw/openclaw#81834",
            "--offline-fixture",
            "fixtures/pr-81834.json",
            "--tab",
            "checks",
            "--theme",
            "default",
            "--symbols",
            "ascii",
            "--spacing",
            "comfortable",
            "--once",
        ])
        .assert()
        .success()
        .stdout(contains("[OK PASS] All checks [+ more]"))
        .stdout(contains("[➕ more]").not())
        .stdout(contains("bg: Rgb(26, 27, 38)"))
        .stdout(contains("bg: Rgb(0, 43, 54)").not())
        .stdout(contains("\"  Summary: PASS"));
}
