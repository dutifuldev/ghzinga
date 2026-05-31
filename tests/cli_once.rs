use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn once_renders_pr_fixture_through_binary() {
    let mut cmd = Command::cargo_bin("gzg").unwrap();

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
    .stdout(contains("Conversation"))
    .stdout(contains("checks PASS"));
}

#[test]
fn once_renders_issue_fixture_through_binary() {
    let mut cmd = Command::cargo_bin("gzg").unwrap();

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
    let mut cmd = Command::cargo_bin("gzg").unwrap();

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
    .stdout(contains("Passing (1)"))
    .stdout(contains("[+ more]"));
}

#[test]
fn once_can_render_pr_files_tab() {
    let mut cmd = Command::cargo_bin("gzg").unwrap();

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
    .stdout(contains("Files changed (5)"))
    .stdout(contains("[+ more]"));
}

#[test]
fn once_can_render_emoji_symbols_when_requested() {
    let mut cmd = Command::cargo_bin("gzg").unwrap();

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
