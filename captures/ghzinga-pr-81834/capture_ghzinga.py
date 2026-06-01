#!/usr/bin/env python3
import argparse
import json
import shlex
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
BIN = REPO / "target" / "debug" / "gzg"
TARGET = "openclaw/openclaw#81834"
TITLE = "feat(senseaudio): add SenseAudio TTS provider"
LOAD_NEEDLE = TITLE
MODE = "pr"
OFFLINE_FIXTURE = None
OFFLINE_RESOURCE_FIXTURES = []
APP_FRESHNESS_PATHS = [
    "Cargo.lock",
    "Cargo.toml",
    "src",
    "fixtures",
]

SIZES = [
    ("narrow", 80, 24),
    ("medium", 120, 36),
    ("large", 160, 50),
]

def run(cmd, check=True):
    return subprocess.run(cmd, text=True, capture_output=True, check=check)


def git_commit() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO,
        text=True,
        capture_output=True,
        check=False,
    )
    return result.stdout.strip() or "unknown"


def app_freshness_paths() -> list[str]:
    return [path for path in APP_FRESHNESS_PATHS if (REPO / path).exists()]


def git_revision_exists(revision: str) -> bool:
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"{revision}^{{commit}}"],
        cwd=REPO,
        text=True,
        capture_output=True,
        check=False,
    )
    return result.returncode == 0


def app_tree_freshness_error(captured_commit: str | None, current_commit: str) -> str | None:
    if not captured_commit or captured_commit == "unknown":
        return "manifest is missing a usable git_commit"
    if captured_commit == current_commit:
        return None
    if current_commit == "unknown":
        return "current git revision could not be resolved"
    if not git_revision_exists(captured_commit):
        return f"captured revision {captured_commit!r} is not available locally"
    if not git_revision_exists(current_commit):
        return f"current revision {current_commit!r} is not available locally"

    paths = app_freshness_paths()
    if not paths:
        return "no app/rendering freshness paths are available"

    committed_diff = subprocess.run(
        ["git", "diff", "--quiet", f"{captured_commit}..{current_commit}", "--", *paths],
        cwd=REPO,
        text=True,
        capture_output=True,
        check=False,
    )
    if committed_diff.returncode == 1:
        return (
            "app/rendering paths changed since captured revision "
            f"{captured_commit[:12]}"
        )
    if committed_diff.returncode != 0:
        detail = committed_diff.stderr.strip() or committed_diff.stdout.strip()
        return f"could not compare app/rendering paths: {detail}"

    worktree_status = subprocess.run(
        ["git", "status", "--porcelain", "--", *paths],
        cwd=REPO,
        text=True,
        capture_output=True,
        check=False,
    )
    if worktree_status.returncode != 0:
        detail = worktree_status.stderr.strip() or worktree_status.stdout.strip()
        return f"could not inspect app/rendering worktree status: {detail}"
    if worktree_status.stdout.strip():
        changed = ", ".join(line.strip() for line in worktree_status.stdout.splitlines()[:3])
        return f"app/rendering paths have uncommitted changes: {changed}"

    return None


def validate_manifest_revision(
    errors: list[str],
    manifest_path: Path,
    manifest: dict,
    expected_git_commit: str,
    allow_stale_revision: bool,
):
    if allow_stale_revision:
        return
    reason = app_tree_freshness_error(manifest.get("git_commit"), expected_git_commit)
    if reason:
        errors.append(
            f"{manifest_path} git_commit {manifest.get('git_commit')!r} does not match "
            f"the current app/rendering tree ({expected_git_commit!r}): {reason}; "
            "pass --allow-stale-revision only for historical capture inspection"
        )


def tmux(*args, check=True):
    return run(["tmux", *args], check=check)


def tmux_size(session: str) -> str:
    return tmux(
        "display-message",
        "-p",
        "-t",
        session,
        "#{window_width}x#{window_height}",
    ).stdout.strip()


def wait_for_loaded(session: str, timeout: float = 45.0):
    deadline = time.time() + timeout
    last = ""
    while time.time() < deadline:
        last = capture_plain(session)
        if LOAD_NEEDLE in last and "Overview" in last:
            return
        time.sleep(0.5)
    raise RuntimeError(f"{session} did not load ghzinga. Last screen:\n{last}")


def capture_plain(session: str) -> str:
    return tmux("capture-pane", "-t", session, "-N", "-p").stdout.rstrip("\n")


def capture_ansi(session: str) -> str:
    return tmux("capture-pane", "-t", session, "-N", "-e", "-p").stdout.rstrip("\n")


def send(session: str, *keys: str):
    tmux("send-keys", "-t", session, *keys)
    time.sleep(1.0)


def write_capture(session: str, out_dir: Path, name: str, meta: dict, frame_meta: dict):
    plain = capture_plain(session)
    ansi = capture_ansi(session)
    (out_dir / f"{name}.txt").write_text(plain + "\n")
    (out_dir / f"{name}.ansi").write_text(ansi + "\n")
    record = {
        "name": name,
        "txt": f"{name}.txt",
        "ansi": f"{name}.ansi",
        "history_txt": f"{name}.history.txt",
        "history_ansi": f"{name}.history.ansi",
    }
    record.update(frame_meta)
    meta["captures"].append(record)


def capture_frame(
    label: str,
    cols: int,
    rows: int,
    name: str,
    meta: dict,
    tab: str | None = None,
    keys: list[str] | None = None,
):
    session = f"ghzinga-{ROOT.name}-{label}-{name}"
    out_dir = ROOT / label
    out_dir.mkdir(parents=True, exist_ok=True)
    tmux("kill-session", "-t", session, check=False)
    tab_arg = f" --tab {tab}" if tab else ""
    fixture_args = ""
    if OFFLINE_FIXTURE:
        fixture_args += f" --offline-fixture {shlex.quote(str(OFFLINE_FIXTURE))}"
    for fixture in OFFLINE_RESOURCE_FIXTURES:
        fixture_args += f" --offline-resource-fixture {shlex.quote(str(fixture))}"
    command = f"TERM=xterm-256color {BIN} {TARGET}{tab_arg}{fixture_args} --refresh-seconds 0"
    print(f"capturing {MODE} {label}/{name} ({cols}x{rows}, tab={tab or 'overview'})", flush=True)
    try:
        tmux("new-session", "-d", "-x", str(cols), "-y", str(rows), "-s", session, command)
        tmux("resize-window", "-t", session, "-x", str(cols), "-y", str(rows))
        actual_size = tmux_size(session)
        meta.setdefault("actual_tmux_size", actual_size)
        wait_for_loaded(session)
        # Let the first full-frame draw settle after the load marker appears.
        time.sleep(2.0)
        for key in keys or []:
            send(session, key)
        write_capture(
            session,
            out_dir,
            name,
            meta,
            {
                "command": command,
                "tab": tab or "overview",
                "keys": keys or [],
                "actual_tmux_size": actual_size,
            },
        )
        history_plain = tmux("capture-pane", "-t", session, "-S", "-", "-E", "-", "-N", "-p").stdout
        history_ansi = tmux("capture-pane", "-t", session, "-S", "-", "-E", "-", "-N", "-e", "-p").stdout
        (out_dir / f"{name}.history.txt").write_text(history_plain)
        (out_dir / f"{name}.history.ansi").write_text(history_ansi)
    finally:
        tmux("kill-session", "-t", session, check=False)


def capture_size(label: str, cols: int, rows: int):
    out_dir = ROOT / label
    out_dir.mkdir(parents=True, exist_ok=True)
    meta = {
        "label": label,
        "target": TARGET,
        "title": TITLE,
        "mode": MODE,
        "binary": str(BIN),
        "git_commit": git_commit(),
        "requested_columns": cols,
        "requested_rows": rows,
        "offline_fixture": str(OFFLINE_FIXTURE) if OFFLINE_FIXTURE else None,
        "offline_resource_fixtures": [str(fixture) for fixture in OFFLINE_RESOURCE_FIXTURES],
        "captures": [],
    }
    if MODE == "issue":
        frames = [
            ("00_overview_top", None, []),
            ("01_overview_expanded", None, ["e"]),
            ("02_overview_pagedown", None, ["e", "PageDown"]),
            ("10_activity_top", "activity", []),
            ("11_activity_pagedown", "activity", ["PageDown"]),
            ("20_links_top", "links", []),
            ("30_help", "links", ["?"]),
        ]
    else:
        frames = [
            ("00_overview_top", None, []),
            ("01_overview_expanded", None, ["e"]),
            ("02_overview_pagedown", None, ["e", "PageDown"]),
            ("10_activity_top", "activity", []),
            ("11_activity_pagedown", "activity", ["PageDown"]),
            ("20_commits_top", "commits", []),
            ("30_checks_top", "checks", []),
            ("31_checks_pagedown", "checks", ["PageDown"]),
            ("40_files_top", "files", []),
            ("41_files_pagedown", "files", ["PageDown"]),
            ("50_links_top", "links", []),
            ("60_help", "links", ["?"]),
        ]
    for name, tab, keys in frames:
        capture_frame(label, cols, rows, name, meta, tab, keys)
    (out_dir / "manifest.json").write_text(json.dumps(meta, indent=2) + "\n")


def expected_frames(mode: str) -> list[str]:
    if mode == "issue":
        return [
            "00_overview_top",
            "01_overview_expanded",
            "02_overview_pagedown",
            "10_activity_top",
            "11_activity_pagedown",
            "20_links_top",
            "30_help",
        ]
    return [
        "00_overview_top",
        "01_overview_expanded",
        "02_overview_pagedown",
        "10_activity_top",
        "11_activity_pagedown",
        "20_commits_top",
        "30_checks_top",
        "31_checks_pagedown",
        "40_files_top",
        "41_files_pagedown",
        "50_links_top",
        "60_help",
    ]


def expected_markers(mode: str) -> list[str]:
    if mode == "issue":
        return ["[Overview]", "[Activity]", "[Links]", "Help"]
    return ["[Activity]", "[Commits]", "[Checks]", "[Files]", "[Links]", "Help"]


def expected_content_markers(mode: str, target: str | None) -> dict[str, list[str]]:
    if mode == "pr" and target == "openclaw/openclaw#81834":
        return {
            "00_overview_top": [
                "Problem: senseaudio bundled plugin only has ASR; no TTS.",
            ],
            "10_activity_top": [
                "Comment by @github-actions",
                "Dependency Changes Detected",
            ],
            "11_activity_pagedown": [
                "Comment by @KLilyZ",
                "Updated the provider docs",
            ],
            "20_commits_top": [
                "feat(senseaudio): add SenseAudio TTS provider",
            ],
            "30_checks_top": [
                "Summary: PASS",
                "Passing (1)",
                "All checks",
            ],
            "40_files_top": [
                "extensions/senseaudio/index.ts",
            ],
            "41_files_pagedown": [
                "docs/providers/senseaudio.md",
            ],
            "50_links_top": [
                "openclaw/openclaw#66943",
            ],
        }
    if mode == "issue" and target == "https://github.com/openclaw/openclaw/issues/88499":
        return {
            "00_overview_top": [
                "Bug Description",
                "previous_response_id",
            ],
            "10_activity_top": [
                "Comment by @clawsweeper",
            ],
            "11_activity_pagedown": [
                "Comment by @tianxiaochannel-oss88",
                "Adding a fresh macOS/Slack data point",
            ],
            "20_links_top": [
                "openclaw/openclaw#84904",
                "openclaw/openclaw#87310",
            ],
        }
    return {}


def scrollbar_evidence_frames(mode: str) -> list[str]:
    if mode == "issue":
        return ["02_overview_pagedown", "11_activity_pagedown"]
    return ["02_overview_pagedown", "11_activity_pagedown", "31_checks_pagedown", "41_files_pagedown"]


def read_json(path: Path) -> dict:
    return json.loads(path.read_text())


def collect_manifest_entries(
    entries: list[dict],
    manifest_path: Path,
    key: str,
    item_name: str,
    errors: list[str],
) -> dict:
    collected = {}
    for entry in entries:
        value = entry.get(key)
        if not value:
            errors.append(f"{manifest_path} contains a {item_name} without {key}")
            continue
        if value in collected:
            errors.append(f"{manifest_path} contains duplicate {item_name} {value}")
            continue
        collected[value] = entry
    return collected


def self_test():
    errors = []
    captures = collect_manifest_entries(
        [
            {"name": "first", "txt": "first.txt"},
            {"name": "first", "txt": "duplicate.txt"},
            {"txt": "unnamed.txt"},
        ],
        Path("manifest.json"),
        "name",
        "capture frame",
        errors,
    )
    if captures != {"first": {"name": "first", "txt": "first.txt"}}:
        raise SystemExit(f"self-test capture collection produced unexpected frames: {captures!r}")
    expected_capture_errors = {
        "manifest.json contains duplicate capture frame first",
        "manifest.json contains a capture frame without name",
    }
    if set(errors) != expected_capture_errors:
        raise SystemExit(
            f"self-test capture collection produced unexpected errors: {errors!r}"
        )

    errors = []
    sizes = collect_manifest_entries(
        [
            {"label": "narrow", "columns": 80, "rows": 24},
            {"label": "narrow", "columns": 80, "rows": 24},
            {"columns": 120, "rows": 36},
        ],
        Path("manifest.json"),
        "label",
        "size entry",
        errors,
    )
    if sizes != {"narrow": {"label": "narrow", "columns": 80, "rows": 24}}:
        raise SystemExit(f"self-test size collection produced unexpected sizes: {sizes!r}")
    expected_size_errors = {
        "manifest.json contains duplicate size entry narrow",
        "manifest.json contains a size entry without label",
    }
    if set(errors) != expected_size_errors:
        raise SystemExit(f"self-test size collection produced unexpected errors: {errors!r}")
    print("OK: capture validator self-test passed.")


def validate_capture_root(root: Path, mode: str, allow_stale_revision: bool = False):
    errors = []
    expected_git_commit = git_commit()
    target = None
    root_manifest = root / "manifest.json"
    if not root_manifest.exists():
        errors.append(f"missing {root_manifest}")
    else:
        manifest = read_json(root_manifest)
        target = manifest.get("target")
        if manifest.get("mode") != mode:
            errors.append(f"{root_manifest} mode is {manifest.get('mode')!r}, expected {mode!r}")
        validate_manifest_revision(
            errors,
            root_manifest,
            manifest,
            expected_git_commit,
            allow_stale_revision,
        )
        sizes = collect_manifest_entries(
            manifest.get("sizes", []),
            root_manifest,
            "label",
            "size entry",
            errors,
        )
        for label, cols, rows in SIZES:
            size = sizes.get(label)
            if not size:
                errors.append(f"{root_manifest} missing size entry {label}")
                continue
            if size.get("columns") != cols or size.get("rows") != rows:
                errors.append(
                    f"{root_manifest} size {label} is "
                    f"{size.get('columns')}x{size.get('rows')}, expected {cols}x{rows}"
                )

    frames = expected_frames(mode)
    markers = expected_markers(mode) + [
        "[refresh]",
        "[copy]",
        "[open]",
        "[settings]",
        "[help]",
        "[quit]",
    ]
    content_markers = expected_content_markers(mode, target)
    saw_scrollbar_thumb = False
    for label, cols, rows in SIZES:
        size_dir = root / label
        manifest_path = size_dir / "manifest.json"
        if not manifest_path.exists():
            errors.append(f"missing {manifest_path}")
            continue
        manifest = read_json(manifest_path)
        validate_manifest_revision(
            errors,
            manifest_path,
            manifest,
            expected_git_commit,
            allow_stale_revision,
        )
        expected_size = f"{cols}x{rows}"
        if manifest.get("actual_tmux_size") != expected_size:
            errors.append(
                f"{manifest_path} actual_tmux_size is {manifest.get('actual_tmux_size')!r}, expected {expected_size!r}"
            )
        captures = collect_manifest_entries(
            manifest.get("captures", []),
            manifest_path,
            "name",
            "capture frame",
            errors,
        )
        missing_frames = [frame for frame in frames if frame not in captures]
        if missing_frames:
            errors.append(f"{manifest_path} missing frames: {', '.join(missing_frames)}")

        combined_text = []
        frame_text = {}
        for frame in frames:
            capture = captures.get(frame, {})
            for key in ("txt", "ansi", "history_txt", "history_ansi"):
                value = capture.get(key)
                if not value:
                    errors.append(f"{manifest_path} frame {frame} missing {key}")
                    continue
                path = size_dir / value
                if not path.exists():
                    errors.append(f"missing {path}")
            txt_path = size_dir / f"{frame}.txt"
            if txt_path.exists():
                text = txt_path.read_text()
                frame_text[frame] = text
                combined_text.append(text)
            history_path = size_dir / f"{frame}.history.txt"
            if history_path.exists():
                frame_text[frame] = "\n".join(
                    [frame_text.get(frame, ""), history_path.read_text()]
                )
        combined = "\n".join(combined_text)
        for marker in markers:
            if marker not in combined:
                errors.append(f"{size_dir} missing marker {marker!r}")
        for frame, frame_markers in content_markers.items():
            text = frame_text.get(frame, "")
            for marker in frame_markers:
                if marker not in text:
                    errors.append(f"{size_dir}/{frame}.txt missing content marker {marker!r}")
        scroll_frames_with_thumb = [
            frame
            for frame in scrollbar_evidence_frames(mode)
            if "█" in frame_text.get(frame, "")
        ]
        saw_scrollbar_thumb = saw_scrollbar_thumb or bool(scroll_frames_with_thumb)

    if not saw_scrollbar_thumb:
        frames_label = ", ".join(scrollbar_evidence_frames(mode))
        errors.append(
            f"{root} missing transient scrollbar thumb in PageDown frames: {frames_label}"
        )
    if errors:
        raise SystemExit("Capture validation failed:\n- " + "\n- ".join(errors))
    print(f"OK: {root} captures match expected {mode} frames, markers, and content.")


def main():
    global ROOT, TARGET, TITLE, LOAD_NEEDLE, MODE, OFFLINE_FIXTURE, OFFLINE_RESOURCE_FIXTURES
    parser = argparse.ArgumentParser(description="Capture ghzinga in tmux")
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--target", default=TARGET)
    parser.add_argument("--title", default=TITLE)
    parser.add_argument("--load-needle", default=None)
    parser.add_argument("--mode", choices=["pr", "issue"], default=MODE)
    parser.add_argument("--offline-fixture", type=Path)
    parser.add_argument("--offline-resource-fixture", type=Path, action="append", default=[])
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument(
        "--allow-stale-revision",
        action="store_true",
        help="allow manifests captured from a different git revision",
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    ROOT = args.root.resolve()
    TARGET = args.target
    TITLE = args.title
    LOAD_NEEDLE = args.load_needle or TITLE
    MODE = args.mode
    OFFLINE_FIXTURE = args.offline_fixture.resolve() if args.offline_fixture else None
    OFFLINE_RESOURCE_FIXTURES = [fixture.resolve() for fixture in args.offline_resource_fixture]
    ROOT.mkdir(parents=True, exist_ok=True)
    if args.validate_only:
        validate_capture_root(ROOT, MODE, args.allow_stale_revision)
        return

    overall = {
        "target": TARGET,
        "title": TITLE,
        "mode": MODE,
        "binary": str(BIN),
        "git_commit": git_commit(),
        "offline_fixture": str(OFFLINE_FIXTURE) if OFFLINE_FIXTURE else None,
        "offline_resource_fixtures": [str(fixture) for fixture in OFFLINE_RESOURCE_FIXTURES],
        "sizes": [],
    }
    for label, cols, rows in SIZES:
        capture_size(label, cols, rows)
        overall["sizes"].append({"label": label, "columns": cols, "rows": rows})
    (ROOT / "manifest.json").write_text(json.dumps(overall, indent=2) + "\n")
    validate_capture_root(ROOT, MODE, args.allow_stale_revision)


if __name__ == "__main__":
    main()
