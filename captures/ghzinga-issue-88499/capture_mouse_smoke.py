#!/usr/bin/env python3
import argparse
import json
import shlex
import sys
import time
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent / "mouse-smoke"
REPO = ROOT.parents[2]
sys.path.append(str(REPO / "captures" / "ghzinga-pr-81834"))

from capture_ghzinga import (  # noqa: E402
    app_tree_freshness_error,
    capture_ansi,
    capture_plain,
    git_commit,
    portable_command,
    repo_relative_path,
    resolve_repo_path,
    tmux,
    tmux_size,
)

BIN = REPO / "target" / "debug" / "gzg"
TARGET = "https://github.com/openclaw/openclaw/issues/88499"
TARGET_LABEL = "openclaw/openclaw#88499"
TITLE = "openai-responses provider: 404 on previous_response_id when store=false (default)"
FIXTURE = REPO / "fixtures" / "issue-88499.json"
NAVIGATION_FIXTURE = ROOT / "navigation-fixture.json"
NAVIGATION_TARGET_FIXTURE = REPO / "fixtures" / "issue-66943.json"
NAVIGATION_TARGET = "openclaw/openclaw#66943"
NAVIGATION_TARGET_TITLE = "feat: add SenseAudio audio transcription provider"
SESSION = "ghzinga-issue-mouse-smoke"
COLS = 120
ROWS = 36
CURRENT_RESOURCE_URL = "https://github.com/openclaw/openclaw/issues/88499"
DETAIL_URL = "https://github.com/openclaw/openclaw/issues/88499#issuecomment-1"
EXPANDED_BODY_MARKER = "Related regressions were discussed"
WIDE_SYMBOLS = set("✅❌⏳⚠➕➖🔄📋🌐⚙❔⏻⬇🏠💬🧱📄🔗👤👍🎯🧵📝")


def terminal_width(text: str) -> int:
    width = 0
    for char in text:
        if unicodedata.combining(char) or char in ("\u200d", "\ufe0f"):
            continue
        codepoint = ord(char)
        if (
            char in WIDE_SYMBOLS
            or 0x1F000 <= codepoint <= 0x1FAFF
            or unicodedata.east_asian_width(char) in ("F", "W")
        ):
            width += 2
        else:
            width += 1
    return width


def wait_for_text(session: str, needle: str, timeout: float = 10.0):
    deadline = time.time() + timeout
    last = ""
    while time.time() < deadline:
        last = capture_plain(session)
        if needle in last:
            return
        time.sleep(0.25)
    raise RuntimeError(f"{session} did not render {needle!r}. Last screen:\n{last}")


def find_marker_position(
    session: str,
    marker: str,
    line_contains: str | None = None,
    timeout: float = 5.0,
) -> tuple[int, int]:
    deadline = time.time() + timeout
    last = ""
    while time.time() < deadline:
        last = capture_plain(session)
        for row, line in enumerate(last.splitlines(), start=1):
            if line_contains and line_contains not in line:
                continue
            column = line.find(marker)
            if column >= 0:
                return terminal_width(line[:column]) + 1, row
        time.sleep(0.25)
    detail = f" on a line containing {line_contains!r}" if line_contains else ""
    raise RuntimeError(f"could not find marker {marker!r}{detail}:\n{last}")


def send_mouse_click(session: str, column: int, row: int):
    sequence = f"\x1b[<0;{column};{row}M\x1b[<0;{column};{row}m"
    tmux("send-keys", "-t", session, "-l", sequence)
    time.sleep(0.5)


def send_key(session: str, key: str):
    tmux("send-keys", "-t", session, key)
    time.sleep(0.5)


def wait_for_session_exit(session: str, timeout: float = 5.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        result = tmux("has-session", "-t", session, check=False)
        if result.returncode != 0:
            return
        time.sleep(0.25)
    raise RuntimeError(f"{session} did not exit after quit click")


def write_frame(out_dir: Path, name: str, frames: list[dict]):
    plain = capture_plain(SESSION)
    ansi = capture_ansi(SESSION)
    txt = out_dir / f"{name}.txt"
    ansi_path = out_dir / f"{name}.ansi"
    txt.write_text(plain + "\n")
    ansi_path.write_text(ansi + "\n")
    frames.append({"name": name, "txt": txt.name, "ansi": ansi_path.name})


def require_screen_contains(marker: str):
    text = capture_plain(SESSION)
    if marker not in text:
        raise RuntimeError(f"screen missing {marker!r}:\n{text}")


def write_navigation_fixture():
    resource = json.loads(FIXTURE.read_text())
    resource["activity"][0]["url"] = DETAIL_URL
    resource["related_resources"] = [
        *resource.get("related_resources", []),
        {
            "owner": "openclaw",
            "repo": "openclaw",
            "number": 66943,
            "kind_hint": "issue",
        },
    ]
    NAVIGATION_FIXTURE.write_text(json.dumps(resource, indent=2) + "\n")


def write_helper_scripts():
    opener = ROOT / "capture-open-url.sh"
    copier = ROOT / "capture-copy-url.sh"
    opener.write_text(f"#!/bin/sh\nprintf '%s\\n' \"$1\" > {shlex.quote(str(open_log_path()))}\n")
    copier.write_text(f"#!/bin/sh\ncat > {shlex.quote(str(copy_log_path()))}\n")
    opener.chmod(0o755)
    copier.chmod(0o755)


def remove_helper_scripts():
    for path in (
        ROOT / "capture-open-url.sh",
        ROOT / "capture-copy-url.sh",
        open_log_path(),
        copy_log_path(),
        capture_config_path(),
    ):
        path.unlink(missing_ok=True)


def capture_config_path() -> Path:
    return ROOT / "capture-empty-config.toml"


def capture_config_env() -> str:
    return f"GZG_CONFIG_PATH={shlex.quote(str(capture_config_path()))}"


def open_log_path() -> Path:
    return ROOT / "capture-open-url.txt"


def copy_log_path() -> Path:
    return ROOT / "capture-copy-url.txt"


def capture_adapter_env() -> str:
    return (
        f"BROWSER={shlex.quote(str(ROOT / 'capture-open-url.sh'))} "
        f"GZG_COPY_COMMAND={shlex.quote(str(ROOT / 'capture-copy-url.sh'))}"
    )


def require_file_contains(path: Path, expected: str):
    actual = path.read_text().strip() if path.exists() else ""
    if actual != expected:
        raise RuntimeError(f"{path} contains {actual!r}, expected {expected!r}")


def capture_mouse_smoke():
    ROOT.mkdir(parents=True, exist_ok=True)
    remove_helper_scripts()
    write_helper_scripts()
    write_navigation_fixture()
    tmux("kill-session", "-t", SESSION, check=False)
    command = (
        f"cd {REPO} && TERM=xterm-256color {capture_config_env()} "
        f"{capture_adapter_env()} {BIN} {TARGET} "
        f"--offline-fixture {NAVIGATION_FIXTURE} "
        f"--offline-resource-fixture {NAVIGATION_TARGET_FIXTURE} "
        f"--refresh-seconds 0"
    )
    frames = []
    mouse_coordinates = {}
    try:
        tmux("new-session", "-d", "-x", str(COLS), "-y", str(ROWS), "-s", SESSION, command)
        tmux("resize-window", "-t", SESSION, "-x", str(COLS), "-y", str(ROWS))
        wait_for_text(SESSION, TITLE)
        wait_for_text(SESSION, "[🔄 refresh]")
        write_frame(ROOT, "00_initial_overview", frames)

        overview_more = find_marker_position(SESSION, "[➕ more]")
        mouse_coordinates["overview_more"] = list(overview_more)
        send_mouse_click(SESSION, *overview_more)
        wait_for_text(SESSION, EXPANDED_BODY_MARKER)
        require_screen_contains("[➖ less]")
        write_frame(ROOT, "05_mouse_overview_more", frames)

        overview_less = find_marker_position(SESSION, "[➖ less]")
        mouse_coordinates["overview_less"] = list(overview_less)
        send_mouse_click(SESSION, *overview_less)
        wait_for_text(SESSION, "[➕ more]")
        text = capture_plain(SESSION)
        if EXPANDED_BODY_MARKER in text:
            raise RuntimeError(f"overview less left issue body expanded:\n{text}")
        write_frame(ROOT, "06_mouse_overview_less", frames)

        send_key(SESSION, "a")
        wait_for_text(SESSION, EXPANDED_BODY_MARKER)
        require_screen_contains("[➖ less]")
        write_frame(ROOT, "07_keyboard_expand_all", frames)

        send_key(SESSION, "a")
        wait_for_text(SESSION, "[➕ more]")
        text = capture_plain(SESSION)
        if EXPANDED_BODY_MARKER in text:
            raise RuntimeError(f"keyboard collapse all left issue body expanded:\n{text}")
        write_frame(ROOT, "08_keyboard_collapse_all", frames)

        activity_tab = find_marker_position(SESSION, "Activity", line_contains="Overview")
        mouse_coordinates["activity_tab"] = list(activity_tab)
        send_mouse_click(SESSION, *activity_tab)
        wait_for_text(SESSION, "Comment by @clawsweeper")
        wait_for_text(SESSION, "[details]")
        write_frame(ROOT, "10_mouse_activity_tab", frames)

        activity_details = find_marker_position(SESSION, "[details]")
        mouse_coordinates["activity_details"] = list(activity_details)
        send_mouse_click(SESSION, *activity_details)
        wait_for_text(SESSION, "focused linked activity")
        write_frame(ROOT, "11_mouse_activity_details_focus", frames)

        links_tab = find_marker_position(SESSION, "Links", line_contains="Activity")
        mouse_coordinates["links_tab"] = list(links_tab)
        send_mouse_click(SESSION, *links_tab)
        wait_for_text(SESSION, NAVIGATION_TARGET)
        write_frame(ROOT, "30_mouse_links_tab", frames)

        linked_issue = find_marker_position(SESSION, NAVIGATION_TARGET)
        mouse_coordinates["linked_issue"] = list(linked_issue)
        send_mouse_click(SESSION, *linked_issue)
        wait_for_text(SESSION, "Open linked resource")
        wait_for_text(SESSION, "[here]")
        write_frame(ROOT, "35_mouse_resource_link_prompt", frames)

        linked_issue_here = find_marker_position(SESSION, "[here]")
        mouse_coordinates["linked_issue_here"] = list(linked_issue_here)
        send_mouse_click(SESSION, *linked_issue_here)
        wait_for_text(SESSION, NAVIGATION_TARGET_TITLE)
        write_frame(ROOT, "40_mouse_navigation_row", frames)

        tmux("send-keys", "-t", SESSION, "Bspace")
        wait_for_text(SESSION, TITLE)
        write_frame(ROOT, "50_keyboard_back_after_navigation", frames)

        refresh_button = find_marker_position(SESSION, "[🔄 refresh]")
        mouse_coordinates["refresh"] = list(refresh_button)
        send_mouse_click(SESSION, *refresh_button)
        wait_for_text(SESSION, "offline fixture mode: refresh skipped")
        write_frame(ROOT, "60_mouse_footer_refresh", frames)

        actual_tmux_size = tmux_size(SESSION)
        quit_button = find_marker_position(SESSION, "[⏻ quit]")
        mouse_coordinates["quit"] = list(quit_button)
        send_mouse_click(SESSION, *quit_button)
        wait_for_session_exit(SESSION)

        manifest = {
            "target": TARGET,
            "fixture": str(NAVIGATION_FIXTURE.relative_to(REPO)),
            "extra_fixtures": [str(NAVIGATION_TARGET_FIXTURE.relative_to(REPO))],
            "binary": repo_relative_path(BIN),
            "git_commit": git_commit(),
            "config_path": repo_relative_path(capture_config_path()),
            "command": portable_command(command),
            "actual_tmux_size": actual_tmux_size,
            "adapter_outputs": {},
            "quit_exited": True,
            "mouse_coordinates": mouse_coordinates,
            "frames": frames,
        }
        (ROOT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    finally:
        tmux("kill-session", "-t", SESSION, check=False)
        remove_helper_scripts()
    validate_mouse_smoke()


def read_json(path: Path) -> dict:
    return json.loads(path.read_text())


def collect_manifest_frames(frame_entries: list[dict], manifest_path: Path, errors: list[str]) -> dict:
    frames = {}
    for frame in frame_entries:
        name = frame.get("name")
        if not name:
            errors.append(f"{manifest_path} contains a frame without a name")
            continue
        if name in frames:
            errors.append(f"{manifest_path} contains duplicate frame {name}")
            continue
        frames[name] = frame
    return frames


def self_test():
    errors = []
    frames = collect_manifest_frames(
        [
            {"name": "first", "txt": "first.txt", "ansi": "first.ansi"},
            {"name": "first", "txt": "duplicate.txt", "ansi": "duplicate.ansi"},
            {"txt": "unnamed.txt", "ansi": "unnamed.ansi"},
        ],
        Path("manifest.json"),
        errors,
    )
    if frames != {"first": {"name": "first", "txt": "first.txt", "ansi": "first.ansi"}}:
        raise SystemExit(f"self-test frame collection produced unexpected frames: {frames!r}")
    expected_errors = {
        "manifest.json contains duplicate frame first",
        "manifest.json contains a frame without a name",
    }
    if set(errors) != expected_errors:
        raise SystemExit(f"self-test frame collection produced unexpected errors: {errors!r}")
    print("OK: issue mouse-smoke validator self-test passed.")


def validate_mouse_smoke(allow_stale_revision: bool = False):
    errors = []
    manifest_path = ROOT / "manifest.json"
    if not manifest_path.exists():
        raise SystemExit(f"missing {manifest_path}")
    manifest = read_json(manifest_path)
    for extra_fixture in manifest.get("extra_fixtures", []):
        if not (REPO / extra_fixture).exists():
            errors.append(f"manifest extra fixture {extra_fixture} is missing")
    fixture_path = REPO / manifest.get("fixture", "")
    if not fixture_path.exists():
        errors.append(f"manifest fixture {fixture_path} is missing")
    else:
        fixture = read_json(fixture_path)
        if not any(
            f"{item.get('owner')}/{item.get('repo')}#{item.get('number')}" == NAVIGATION_TARGET
            for item in fixture.get("related_resources", [])
        ):
            errors.append(f"manifest fixture {fixture_path} does not include {NAVIGATION_TARGET}")
        if not any(item.get("url") == DETAIL_URL for item in fixture.get("activity", [])):
            errors.append(f"manifest fixture {fixture_path} does not include {DETAIL_URL}")
    if not allow_stale_revision:
        reason = app_tree_freshness_error(manifest.get("git_commit"), git_commit())
        if reason:
            errors.append(
                f"{manifest_path} git_commit {manifest.get('git_commit')!r} is stale: {reason}"
            )
    if manifest.get("actual_tmux_size") != f"{COLS}x{ROWS}":
        errors.append(
            f"actual_tmux_size is {manifest.get('actual_tmux_size')!r}, expected {COLS}x{ROWS}"
        )
    expected_config_path = capture_config_path().resolve()
    expected_config_value = repo_relative_path(expected_config_path)
    expected_config_env = f"GZG_CONFIG_PATH=./{expected_config_value}"
    if resolve_repo_path(manifest.get("config_path")).resolve() != expected_config_path:
        errors.append(
            f"config_path is {manifest.get('config_path')!r}, expected {expected_config_value!r}"
        )
    command = portable_command(manifest.get("command", ""))
    if expected_config_env not in command:
        errors.append("manifest command does not isolate config with GZG_CONFIG_PATH")
    for variable in ("BROWSER=", "GZG_COPY_COMMAND="):
        if variable not in manifest.get("command", ""):
            errors.append(f"manifest command does not isolate adapter with {variable.rstrip('=')}")
    expected_adapter_outputs = {}
    if manifest.get("adapter_outputs") != expected_adapter_outputs:
        errors.append(
            f"adapter_outputs is {manifest.get('adapter_outputs')!r}, "
            f"expected {expected_adapter_outputs!r}"
        )
    if manifest.get("quit_exited") is not True:
        errors.append("manifest does not record successful quit exit")
    coordinates = manifest.get("mouse_coordinates", {})
    for target in (
        "overview_more",
        "overview_less",
        "activity_tab",
        "activity_details",
        "links_tab",
        "linked_issue",
        "linked_issue_here",
        "refresh",
        "quit",
    ):
        if target not in coordinates:
            errors.append(f"manifest missing {target} mouse coordinate")

    expected = {
        "00_initial_overview": [
            "[🏠 Overview]",
            TITLE,
            "Bug Description",
            "[➕ more]",
        ],
        "05_mouse_overview_more": [
            "[🏠 Overview]",
            EXPANDED_BODY_MARKER,
            "[➖ less]",
        ],
        "06_mouse_overview_less": ["[🏠 Overview]", "Bug Description", "[➕ more]"],
        "07_keyboard_expand_all": ["[🏠 Overview]", EXPANDED_BODY_MARKER, "[➖ less]"],
        "08_keyboard_collapse_all": ["[🏠 Overview]", "Bug Description", "[➕ more]"],
        "10_mouse_activity_tab": ["[💬 Activity]", "Comment by @clawsweeper", "[details]"],
        "11_mouse_activity_details_focus": [
            "[💬 Activity]",
            "focused linked activity",
            "[details]",
        ],
        "30_mouse_links_tab": ["[🔗 Links]", NAVIGATION_TARGET],
        "35_mouse_resource_link_prompt": [
            "[🔗 Links]",
            "Open linked resource",
            NAVIGATION_TARGET,
            "[here]",
            "[new tab]",
        ],
        "40_mouse_navigation_row": [
            "[🏠 Overview]",
            NAVIGATION_TARGET_TITLE,
        ],
        "50_keyboard_back_after_navigation": [
            "[🏠 Overview]",
            TITLE,
        ],
        "60_mouse_footer_refresh": [
            "[🏠 Overview]",
            TITLE,
            "offline fixture mode: refresh skipped",
        ],
    }
    frames = collect_manifest_frames(manifest.get("frames", []), manifest_path, errors)
    for name, markers in expected.items():
        frame = frames.get(name)
        if not frame:
            errors.append(f"manifest missing frame {name}")
            continue
        for key in ("txt", "ansi"):
            path = ROOT / frame.get(key, "")
            if not path.exists():
                errors.append(f"missing {path}")
        txt_path = ROOT / frame.get("txt", "")
        text = txt_path.read_text() if txt_path.exists() else ""
        for marker in markers:
            if marker not in text:
                errors.append(f"{txt_path} missing marker {marker!r}")
        if name == "06_mouse_overview_less" and EXPANDED_BODY_MARKER in text:
            errors.append(f"{txt_path} still shows expanded issue body after collapse")
        if name == "08_keyboard_collapse_all" and EXPANDED_BODY_MARKER in text:
            errors.append(f"{txt_path} still shows expanded issue body after keyboard collapse")

    if errors:
        raise SystemExit("Issue mouse smoke validation failed:\n- " + "\n- ".join(errors))
    print(f"OK: {ROOT} issue mouse-smoke captures match expected click behavior.")


def main():
    parser = argparse.ArgumentParser(description="Capture ghzinga issue mouse smoke in tmux")
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument("--allow-stale-revision", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    elif args.validate_only:
        validate_mouse_smoke(args.allow_stale_revision)
    else:
        capture_mouse_smoke()


if __name__ == "__main__":
    main()
