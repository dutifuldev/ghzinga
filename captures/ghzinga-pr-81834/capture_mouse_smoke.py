#!/usr/bin/env python3
import argparse
import json
import subprocess
import time
from pathlib import Path

from capture_ghzinga import (
    app_tree_freshness_error,
    capture_ansi,
    capture_plain,
    git_commit,
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
SESSION = "ghzinga-mouse-smoke"
COLS = 120
ROWS = 36


def wait_for_text(session: str, needle: str, timeout: float = 10.0):
    deadline = time.time() + timeout
    last = ""
    while time.time() < deadline:
        last = capture_plain(session)
        if needle in last:
            return
        time.sleep(0.25)
    raise RuntimeError(f"{session} did not render {needle!r}. Last screen:\n{last}")


def find_marker_position(session: str, marker: str, line_contains: str | None = None) -> tuple[int, int]:
    text = capture_plain(session)
    for row, line in enumerate(text.splitlines(), start=1):
        if line_contains and line_contains not in line:
            continue
        column = line.find(marker)
        if column >= 0:
            # xterm SGR mouse coordinates are 1-based; click inside the marker.
            return column + 2, row
    detail = f" on a line containing {line_contains!r}" if line_contains else ""
    raise RuntimeError(f"could not find marker {marker!r}{detail}:\n{text}")


def send_mouse_click(session: str, column: int, row: int):
    sequence = f"\x1b[<0;{column};{row}M\x1b[<0;{column};{row}m"
    tmux("send-keys", "-t", session, "-l", sequence)
    time.sleep(0.5)


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
    resource["related_resources"] = [
        {
            "owner": "openclaw",
            "repo": "openclaw",
            "number": 66943,
            "kind_hint": "issue",
        }
    ]
    NAVIGATION_FIXTURE.write_text(json.dumps(resource, indent=2) + "\n")


def capture_mouse_smoke():
    ROOT.mkdir(parents=True, exist_ok=True)
    write_navigation_fixture()
    tmux("kill-session", "-t", SESSION, check=False)
    command = (
        f"cd {REPO} && TERM=xterm-256color {BIN} {TARGET} "
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
        write_frame(ROOT, "00_initial_overview", frames)

        overview_more = find_marker_position(SESSION, "[+ more]", line_contains="commit fb948c9")
        mouse_coordinates["overview_more"] = list(overview_more)
        send_mouse_click(SESSION, *overview_more)
        wait_for_text(SESSION, "committed: 1mo ago")
        require_screen_contains("[- less]")
        write_frame(ROOT, "05_mouse_overview_more", frames)

        overview_less = find_marker_position(SESSION, "[- less]", line_contains="commit fb948c9")
        mouse_coordinates["overview_less"] = list(overview_less)
        send_mouse_click(SESSION, *overview_less)
        wait_for_text(SESSION, "[+ more]")
        text = capture_plain(SESSION)
        if "committed: 1mo ago" in text:
            raise RuntimeError(f"overview less left commit detail expanded:\n{text}")
        write_frame(ROOT, "06_mouse_overview_less", frames)

        files_tab = find_marker_position(SESSION, "Files", line_contains="Overview")
        mouse_coordinates["files_tab"] = list(files_tab)
        send_mouse_click(SESSION, *files_tab)
        wait_for_text(SESSION, "docs/plugins/plugin-inventory.md")
        require_screen_contains("[Files]")
        write_frame(ROOT, "10_mouse_files_tab", frames)

        expand_all = find_marker_position(SESSION, "[expand all]")
        mouse_coordinates["expand_all"] = list(expand_all)
        send_mouse_click(SESSION, *expand_all)
        require_screen_contains("path: docs/plugins/plugin-inventory.md")
        tmux("send-keys", "-t", SESSION, "End")
        time.sleep(0.5)
        wait_for_text(SESSION, "[collapse all]")
        write_frame(ROOT, "20_mouse_expand_all", frames)

        collapse_all = find_marker_position(SESSION, "[collapse all]")
        mouse_coordinates["collapse_all"] = list(collapse_all)
        send_mouse_click(SESSION, *collapse_all)
        wait_for_text(SESSION, "[expand all]")
        text = capture_plain(SESSION)
        if "path: docs/plugins/plugin-inventory.md" in text:
            raise RuntimeError(f"collapse all left first file expanded:\n{text}")
        write_frame(ROOT, "30_mouse_collapse_all", frames)

        links_tab = find_marker_position(SESSION, "Links", line_contains="[Files]")
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
        wait_for_text(SESSION, f"returned to {TARGET}")
        write_frame(ROOT, "60_keyboard_back_after_navigation", frames)

        manifest = {
            "target": TARGET,
            "fixture": str(NAVIGATION_FIXTURE.relative_to(REPO)),
            "extra_fixtures": [str(NAVIGATION_TARGET_FIXTURE.relative_to(REPO))],
            "binary": str(BIN),
            "git_commit": git_commit(),
            "command": command,
            "actual_tmux_size": tmux_size(SESSION),
            "mouse_coordinates": mouse_coordinates,
            "frames": frames,
        }
        (ROOT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    finally:
        tmux("kill-session", "-t", SESSION, check=False)
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

    expected = {
        "00_initial_overview": [
            "[Overview]",
            "Problem: senseaudio bundled plugin only has ASR; no TTS.",
        ],
        "05_mouse_overview_more": [
            "[Overview]",
            "* commit fb948c9",
            "[- less]",
            "committed: 1mo ago",
        ],
        "06_mouse_overview_less": ["[Overview]", "* commit fb948c9", "[+ more]"],
        "10_mouse_files_tab": ["[Files]", "docs/plugins/plugin-inventory.md", "[expand all]"],
        "20_mouse_expand_all": [
            "[Files]",
            "[collapse all]",
            "path: docs/plugins/reference.md",
        ],
        "30_mouse_collapse_all": ["[Files]", "[expand all]"],
        "40_mouse_links_tab": ["[Links]", NAVIGATION_TARGET],
        "50_mouse_navigation_row": [
            "[Overview]",
            NAVIGATION_TARGET_TITLE,
            "opened openclaw/openclaw#66943",
        ],
        "60_keyboard_back_after_navigation": [
            "[Overview]",
            "Problem: senseaudio bundled plugin only has ASR; no TTS.",
            f"returned to {TARGET}",
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
