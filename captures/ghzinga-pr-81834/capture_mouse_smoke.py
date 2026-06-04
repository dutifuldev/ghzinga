#!/usr/bin/env python3
import argparse
import json
import shlex
import subprocess
import time
import unicodedata
from pathlib import Path

from capture_ghzinga import (
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

ROOT = Path(__file__).resolve().parent / "mouse-smoke"
REPO = ROOT.parents[2]
BIN = REPO / "target" / "debug" / "gzg"
TARGET = "openclaw/openclaw#81834"
FIXTURE = REPO / "fixtures" / "pr-81834.json"
NAVIGATION_FIXTURE = ROOT / "navigation-fixture.json"
NAVIGATION_TARGET_FIXTURE = REPO / "fixtures" / "issue-66943.json"
NAVIGATION_TARGET = "openclaw/openclaw#66943"
NAVIGATION_TARGET_TITLE = "feat: add SenseAudio audio transcription provider"
LOAD_FULL_FIXTURE = ROOT / "load-full-fixture.json"
LOAD_FULL_WARNING = (
    "normal API depth shows the first 100 only for comments; "
    "set --api-depth full or GZG_API_DEPTH=full for exhaustive pagination"
)
SESSION = "ghzinga-mouse-smoke"
COLS = 120
ROWS = 36
CURRENT_RESOURCE_URL = "https://github.com/openclaw/openclaw/pull/81834"
DETAIL_URL = "https://github.com/openclaw/openclaw/pull/81834#issuecomment-smoke-1"
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
                # xterm SGR mouse coordinates are 1-based; click inside the marker.
                return terminal_width(line[:column]) + 1, row
        time.sleep(0.25)
    detail = f" on a line containing {line_contains!r}" if line_contains else ""
    raise RuntimeError(f"could not find marker {marker!r}{detail}:\n{last}")


def send_mouse_click(session: str, column: int, row: int):
    sequence = f"\x1b[<0;{column};{row}M\x1b[<0;{column};{row}m"
    tmux("send-keys", "-t", session, "-l", sequence)
    time.sleep(0.5)


def click_marker_until_text(
    session: str,
    marker: str,
    expected: str,
    timeout: float = 10.0,
) -> tuple[int, int]:
    deadline = time.time() + timeout
    last_position = find_marker_position(session, marker)
    while time.time() < deadline:
        last_position = find_marker_position(session, marker)
        send_mouse_click(session, *last_position)
        if expected in capture_plain(session):
            return last_position
        time.sleep(0.25)
    raise RuntimeError(
        f"{session} did not render {expected!r} after clicking {marker!r}. "
        f"Last screen:\n{capture_plain(session)}"
    )


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
    frames.append(
        {
            "name": name,
            "txt": txt.name,
            "ansi": ansi_path.name,
        }
    )


def require_screen_contains(marker: str):
    text = capture_plain(SESSION)
    if marker not in text:
        raise RuntimeError(f"screen missing {marker!r}:\n{text}")


def write_navigation_fixture():
    resource = json.loads(FIXTURE.read_text())
    resource["activity"][0]["url"] = DETAIL_URL
    resource["related_resources"] = [
        {
            "owner": "openclaw",
            "repo": "openclaw",
            "number": 66943,
            "kind_hint": "issue",
        }
    ]
    NAVIGATION_FIXTURE.write_text(json.dumps(resource, indent=2) + "\n")


def write_load_full_fixture():
    resource = json.loads(FIXTURE.read_text())
    warnings = resource.setdefault("warnings", [])
    if LOAD_FULL_WARNING not in warnings:
        warnings.append(LOAD_FULL_WARNING)
    LOAD_FULL_FIXTURE.write_text(json.dumps(resource, indent=2) + "\n")


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


def read_saved_config() -> str:
    if not capture_config_path().exists():
        raise RuntimeError(f"{capture_config_path()} was not created")
    contents = capture_config_path().read_text()
    for expected in (
        'theme = "default"',
        'symbols = "emoji"',
        'spacing = "compact"',
        'width_mode = "fixed"',
        "fixed_width = 118",
        'scrollbar = "on-scroll"',
    ):
        if expected not in contents:
            raise RuntimeError(f"{capture_config_path()} missing {expected!r}:\n{contents}")
    return contents


def capture_mouse_smoke():
    ROOT.mkdir(parents=True, exist_ok=True)
    remove_helper_scripts()
    write_helper_scripts()
    write_navigation_fixture()
    write_load_full_fixture()
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
        wait_for_text(SESSION, "Problem: senseaudio bundled plugin only has ASR; no TTS.")
        wait_for_text(SESSION, "[🔄 refresh]")
        write_frame(ROOT, "00_initial_overview", frames)

        overview_more = find_marker_position(SESSION, "[➕ more]", line_contains="commit fb948c9")
        mouse_coordinates["overview_more"] = list(overview_more)
        send_mouse_click(SESSION, *overview_more)
        wait_for_text(SESSION, "committed: 1mo ago")
        require_screen_contains("[➖ less]")
        write_frame(ROOT, "05_mouse_overview_more", frames)

        overview_less = find_marker_position(SESSION, "[➖ less]", line_contains="commit fb948c9")
        mouse_coordinates["overview_less"] = list(overview_less)
        send_mouse_click(SESSION, *overview_less)
        wait_for_text(SESSION, "[➕ more]")
        text = capture_plain(SESSION)
        if "committed: 1mo ago" in text:
            raise RuntimeError(f"overview less left commit detail expanded:\n{text}")
        write_frame(ROOT, "06_mouse_overview_less", frames)

        files_tab = find_marker_position(SESSION, "Files", line_contains="Overview")
        mouse_coordinates["files_tab"] = list(files_tab)
        send_mouse_click(SESSION, *files_tab)
        wait_for_text(SESSION, "docs/plugins/plugin-inventory.md")
        require_screen_contains("[📄 Files]")
        write_frame(ROOT, "10_mouse_files_tab", frames)

        file_row = find_marker_position(
            SESSION,
            "[➕ more]",
            line_contains="docs/plugins/reference.md",
        )
        mouse_coordinates["file_row_more"] = list(file_row)
        send_mouse_click(SESSION, *file_row)
        wait_for_text(SESSION, "path: docs/plugins/reference.md")
        wait_for_text(SESSION, "change: MODIFIED, additions: 1, deletions: 1")
        write_frame(ROOT, "11_mouse_file_row_more", frames)

        file_row_less = find_marker_position(
            SESSION,
            "[➖ less]",
            line_contains="docs/plugins/reference.md",
        )
        mouse_coordinates["file_row_less"] = list(file_row_less)
        send_mouse_click(SESSION, *file_row_less)
        wait_for_text(SESSION, "[➕ more]")
        text = capture_plain(SESSION)
        if "path: docs/plugins/reference.md" in text:
            raise RuntimeError(f"file row less left file detail expanded:\n{text}")
        write_frame(ROOT, "12_mouse_file_row_less", frames)

        send_key(SESSION, "a")
        wait_for_text(SESSION, "path: docs/plugins/plugin-inventory.md")
        wait_for_text(SESSION, "[➖ collapse]")
        write_frame(ROOT, "13_keyboard_expand_all", frames)

        send_key(SESSION, "a")
        wait_for_text(SESSION, "[➕ expand ]")
        text = capture_plain(SESSION)
        if "path: docs/plugins/plugin-inventory.md" in text:
            raise RuntimeError(f"keyboard collapse all left first file expanded:\n{text}")
        write_frame(ROOT, "14_keyboard_collapse_all", frames)

        expand_all = find_marker_position(SESSION, "[➕ expand ]")
        mouse_coordinates["expand_all"] = list(expand_all)
        send_mouse_click(SESSION, *expand_all)
        require_screen_contains("path: docs/plugins/plugin-inventory.md")
        tmux("send-keys", "-t", SESSION, "End")
        time.sleep(0.5)
        wait_for_text(SESSION, "[➖ collapse]")
        write_frame(ROOT, "20_mouse_expand_all", frames)

        collapse_all = find_marker_position(SESSION, "[➖ collapse]")
        mouse_coordinates["collapse_all"] = list(collapse_all)
        send_mouse_click(SESSION, *collapse_all)
        wait_for_text(SESSION, "[➕ expand ]")
        text = capture_plain(SESSION)
        if "path: docs/plugins/plugin-inventory.md" in text:
            raise RuntimeError(f"collapse all left first file expanded:\n{text}")
        write_frame(ROOT, "30_mouse_collapse_all", frames)

        checks_tab = find_marker_position(SESSION, "Checks", line_contains="Files")
        mouse_coordinates["checks_tab"] = list(checks_tab)
        send_mouse_click(SESSION, *checks_tab)
        wait_for_text(SESSION, "Summary: PASS")
        wait_for_text(SESSION, "[✅ PASS] suite/CI [➕ more]")
        write_frame(ROOT, "35_mouse_checks_tab", frames)

        check_row = find_marker_position(SESSION, "[➕ more]", line_contains="suite/CI")
        mouse_coordinates["check_row_more"] = list(check_row)
        send_mouse_click(SESSION, *check_row)
        wait_for_text(SESSION, "summary: 38 skipped, 2 neutral, 86 successful")
        write_frame(ROOT, "36_mouse_check_row_more", frames)

        check_row_less = find_marker_position(SESSION, "[➖ less]", line_contains="suite/CI")
        mouse_coordinates["check_row_less"] = list(check_row_less)
        send_mouse_click(SESSION, *check_row_less)
        wait_for_text(SESSION, "[✅ PASS] suite/CI [➕ more]")
        text = capture_plain(SESSION)
        if "summary: 38 skipped, 2 neutral, 86 successful" in text:
            raise RuntimeError(f"check row less left check detail expanded:\n{text}")
        write_frame(ROOT, "37_mouse_check_row_less", frames)

        links_tab = find_marker_position(SESSION, "Links", line_contains="Checks")
        mouse_coordinates["links_tab"] = list(links_tab)
        send_mouse_click(SESSION, *links_tab)
        wait_for_text(SESSION, NAVIGATION_TARGET)
        require_screen_contains(f"  {NAVIGATION_TARGET}")
        write_frame(ROOT, "40_mouse_links_tab", frames)

        linked_issue = find_marker_position(SESSION, NAVIGATION_TARGET)
        mouse_coordinates["linked_issue"] = list(linked_issue)
        send_mouse_click(SESSION, *linked_issue)
        wait_for_text(SESSION, NAVIGATION_TARGET_TITLE)
        write_frame(ROOT, "50_mouse_navigation_row", frames)

        tmux("send-keys", "-t", SESSION, "Bspace")
        wait_for_text(SESSION, "Problem: senseaudio bundled plugin only has ASR; no TTS.")
        write_frame(ROOT, "60_keyboard_back_after_navigation", frames)

        activity_tab = find_marker_position(SESSION, "Activity", line_contains="Overview")
        mouse_coordinates["activity_tab_for_detail"] = list(activity_tab)
        send_mouse_click(SESSION, *activity_tab)
        wait_for_text(SESSION, "Comment by @github-actions")
        wait_for_text(SESSION, "[details]")
        write_frame(ROOT, "62_mouse_activity_tab_for_detail", frames)

        activity_details = find_marker_position(SESSION, "[details]")
        mouse_coordinates["activity_details"] = list(activity_details)
        send_mouse_click(SESSION, *activity_details)
        wait_for_text(SESSION, f"opened {DETAIL_URL}")
        require_file_contains(open_log_path(), DETAIL_URL)
        write_frame(ROOT, "63_mouse_activity_details_open", frames)

        overview_tab = find_marker_position(SESSION, "Overview", line_contains="Activity")
        mouse_coordinates["overview_tab_after_detail"] = list(overview_tab)
        send_mouse_click(SESSION, *overview_tab)
        wait_for_text(SESSION, "Problem: senseaudio bundled plugin only has ASR; no TTS.")
        write_frame(ROOT, "64_mouse_back_to_overview_after_detail", frames)

        refresh_button = find_marker_position(SESSION, "[🔄 refresh]")
        mouse_coordinates["refresh"] = list(refresh_button)
        send_mouse_click(SESSION, *refresh_button)
        wait_for_text(SESSION, "offline fixture mode: refresh skipped")
        write_frame(ROOT, "65_mouse_footer_refresh", frames)

        help_button = find_marker_position(SESSION, "[❔ help]")
        mouse_coordinates["help"] = list(help_button)
        send_mouse_click(SESSION, *help_button)
        wait_for_text(SESSION, "Keyboard")
        wait_for_text(SESSION, "Mouse")
        write_frame(ROOT, "70_mouse_footer_help", frames)

        settings_button = find_marker_position(SESSION, "[⚙ settings]")
        mouse_coordinates["settings"] = list(settings_button)
        send_mouse_click(SESSION, *settings_button)
        wait_for_text(SESSION, "Settings")
        wait_for_text(SESSION, "Width")
        wait_for_text(SESSION, "Spacing")
        wait_for_text(SESSION, "Scrollbar")
        write_frame(ROOT, "80_mouse_footer_settings", frames)

        compact_setting = find_marker_position(SESSION, "[ ] compact")
        mouse_coordinates["settings_compact"] = list(compact_setting)
        send_mouse_click(SESSION, *compact_setting)
        wait_for_text(SESSION, "[x] compact")
        wait_for_text(SESSION, "saved settings to")
        saved_config = read_saved_config()
        write_frame(ROOT, "81_mouse_settings_compact", frames)

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
            "adapter_outputs": {
                "detail_url": DETAIL_URL,
            },
            "saved_config": saved_config,
            "quit_exited": True,
            "mouse_coordinates": mouse_coordinates,
            "frames": frames,
        }

        load_full_command = (
            f"cd {REPO} && TERM=xterm-256color {capture_config_env()} "
            f"{BIN} {TARGET} "
            f"--offline-fixture {LOAD_FULL_FIXTURE} "
            f"--refresh-seconds 0"
        )
        tmux("new-session", "-d", "-x", str(COLS), "-y", str(ROWS), "-s", SESSION, load_full_command)
        tmux("resize-window", "-t", SESSION, "-x", str(COLS), "-y", str(ROWS))
        wait_for_text(SESSION, "[⬇ full]")
        load_full_button = click_marker_until_text(
            SESSION,
            "[⬇ full]",
            "offline fixture mode: full-depth load skipped",
        )
        mouse_coordinates["load_full"] = list(load_full_button)
        write_frame(ROOT, "90_mouse_footer_load_full", frames)
        manifest["load_full_fixture"] = str(LOAD_FULL_FIXTURE.relative_to(REPO))
        manifest["load_full_command"] = portable_command(load_full_command)
        manifest["mouse_coordinates"] = mouse_coordinates
        manifest["frames"] = frames
        tmux("kill-session", "-t", SESSION, check=False)

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
    print("OK: mouse-smoke validator self-test passed.")


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
    load_full_fixture_path = REPO / manifest.get("load_full_fixture", "")
    if not load_full_fixture_path.exists():
        errors.append(f"manifest load_full_fixture {load_full_fixture_path} is missing")
    else:
        load_full_fixture = read_json(load_full_fixture_path)
        if LOAD_FULL_WARNING not in load_full_fixture.get("warnings", []):
            errors.append(
                f"manifest load_full_fixture {load_full_fixture_path} does not include "
                "the partial-depth warning"
            )
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
    load_full_command = portable_command(manifest.get("load_full_command", ""))
    if expected_config_env not in command:
        errors.append("manifest command does not isolate config with GZG_CONFIG_PATH")
    if expected_config_env not in load_full_command:
        errors.append("manifest load_full_command does not isolate config with GZG_CONFIG_PATH")
    for variable in ("BROWSER=", "GZG_COPY_COMMAND="):
        if variable not in manifest.get("command", ""):
            errors.append(f"manifest command does not isolate adapter with {variable.rstrip('=')}")
    expected_adapter_outputs = {"detail_url": DETAIL_URL}
    if manifest.get("adapter_outputs") != expected_adapter_outputs:
        errors.append(
            f"adapter_outputs is {manifest.get('adapter_outputs')!r}, "
            f"expected {expected_adapter_outputs!r}"
        )
    if manifest.get("quit_exited") is not True:
        errors.append("manifest does not record successful quit exit")
    coordinates = manifest.get("mouse_coordinates", {})
    for target in (
        "file_row_more",
        "file_row_less",
        "check_row_more",
        "check_row_less",
        "activity_tab_for_detail",
        "activity_details",
        "overview_tab_after_detail",
        "settings_compact",
        "load_full",
        "quit",
    ):
        if target not in coordinates:
            errors.append(f"manifest missing {target} mouse coordinate")
    saved_config = manifest.get("saved_config", "")
    for expected_config_line in (
        'theme = "default"',
        'symbols = "emoji"',
        'spacing = "compact"',
        'width_mode = "fixed"',
        "fixed_width = 118",
        'scrollbar = "on-scroll"',
    ):
        if expected_config_line not in saved_config:
            errors.append(f"manifest saved_config missing {expected_config_line!r}")

    expected = {
        "00_initial_overview": [
            "[🏠 Overview]",
            "Problem: senseaudio bundled plugin only has ASR; no TTS.",
        ],
        "05_mouse_overview_more": [
            "[🏠 Overview]",
            "* commit fb948c9",
            "[➖ less]",
            "committed: 1mo ago",
        ],
        "06_mouse_overview_less": ["[🏠 Overview]", "* commit fb948c9", "[➕ more]"],
        "10_mouse_files_tab": ["[📄 Files]", "docs/plugins/plugin-inventory.md", "[➕ expand ]"],
        "11_mouse_file_row_more": [
            "[📄 Files]",
            "docs/plugins/reference.md [➖ less]",
            "path: docs/plugins/reference.md",
            "change: MODIFIED, additions: 1, deletions: 1",
        ],
        "12_mouse_file_row_less": [
            "[📄 Files]",
            "docs/plugins/reference.md [➕ more]",
        ],
        "13_keyboard_expand_all": [
            "[📄 Files]",
            "[➖ collapse]",
            "path: docs/plugins/plugin-inventory.md",
        ],
        "14_keyboard_collapse_all": ["[📄 Files]", "[➕ expand ]"],
        "20_mouse_expand_all": [
            "[📄 Files]",
            "[➖ collapse]",
            "path: extensions/senseaudio/speech-provider.ts",
        ],
        "30_mouse_collapse_all": ["[📄 Files]", "[➕ expand ]"],
        "35_mouse_checks_tab": [
            "[✅ Checks]",
            "Summary: PASS",
            "[✅ PASS] suite/CI [➕ more]",
        ],
        "36_mouse_check_row_more": [
            "[✅ Checks]",
            "[✅ PASS] suite/CI [➖ less]",
            "summary: 38 skipped, 2 neutral, 86 successful",
        ],
        "37_mouse_check_row_less": [
            "[✅ Checks]",
            "[✅ PASS] suite/CI [➕ more]",
        ],
        "40_mouse_links_tab": ["[🔗 Links]", NAVIGATION_TARGET],
        "50_mouse_navigation_row": [
            "[🏠 Overview]",
            NAVIGATION_TARGET_TITLE,
        ],
        "60_keyboard_back_after_navigation": [
            "[🏠 Overview]",
            "Problem: senseaudio bundled plugin only has ASR; no TTS.",
        ],
        "62_mouse_activity_tab_for_detail": [
            "[💬 Activity]",
            "Comment by @github-actions",
            "[details]",
        ],
        "63_mouse_activity_details_open": [
            "[💬 Activity]",
            f"opened {DETAIL_URL}",
            "[details]",
        ],
        "64_mouse_back_to_overview_after_detail": [
            "[🏠 Overview]",
            "Problem: senseaudio bundled plugin only has ASR; no TTS.",
        ],
        "65_mouse_footer_refresh": [
            "[🏠 Overview]",
            "Problem: senseaudio bundled plugin only has ASR; no TTS.",
            "offline fixture mode: refresh skipped",
        ],
        "70_mouse_footer_help": ["Help", "Keyboard", "Mouse", "[❔ help]"],
        "80_mouse_footer_settings": ["Settings", "Width", "Spacing", "Scrollbar", "[⚙ settings]"],
        "81_mouse_settings_compact": ["Settings", "[x] compact", "saved settings to"],
        "90_mouse_footer_load_full": [
            "[🏠 Overview]",
            "[⬇ full]",
            "offline fixture mode: full-depth load skipped",
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
        if name == "30_mouse_collapse_all" and "path: docs/plugins/plugin-inventory.md" in text:
            errors.append(f"{txt_path} still shows expanded file detail after collapse")
        if name == "12_mouse_file_row_less" and "path: docs/plugins/reference.md" in text:
            errors.append(f"{txt_path} still shows expanded file detail after row collapse")
        if name == "14_keyboard_collapse_all" and "path: docs/plugins/plugin-inventory.md" in text:
            errors.append(f"{txt_path} still shows expanded file detail after keyboard collapse")
        if (
            name == "37_mouse_check_row_less"
            and "summary: 38 skipped, 2 neutral, 86 successful" in text
        ):
            errors.append(f"{txt_path} still shows expanded check detail after row collapse")
        if name == "06_mouse_overview_less" and "committed: 1mo ago" in text:
            errors.append(f"{txt_path} still shows expanded commit detail after collapse")

    if errors:
        raise SystemExit("Mouse smoke validation failed:\n- " + "\n- ".join(errors))
    print(f"OK: {ROOT} mouse-smoke captures match expected click behavior.")


def main():
    parser = argparse.ArgumentParser(description="Capture ghzinga mouse smoke in tmux")
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
