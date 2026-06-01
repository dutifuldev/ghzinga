#!/usr/bin/env python3
import json
import subprocess
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parent
CONFIG = ROOT / "config.yml"


SIZES = [
    ("narrow", 80, 24),
    ("medium", 120, 36),
    ("large", 160, 50),
]

def run(cmd, check=True):
    return subprocess.run(cmd, text=True, capture_output=True, check=check)


def tmux(*args, check=True):
    return run(["tmux", *args], check=check)


def wait_for_loaded(session: str, needle: str, timeout: float = 30.0):
    deadline = time.time() + timeout
    last = ""
    while time.time() < deadline:
        last = capture_plain(session)
        if needle in last and "Loading" not in last:
            return
        time.sleep(0.5)
    raise RuntimeError(f"{session} did not load expected content. Last screen:\n{last}")


def capture_plain(session: str) -> str:
    return tmux("capture-pane", "-t", session, "-N", "-p").stdout.rstrip("\n")


def capture_ansi(session: str) -> str:
    return tmux("capture-pane", "-t", session, "-N", "-e", "-p").stdout.rstrip("\n")


def send(session: str, *keys: str):
    tmux("send-keys", "-t", session, *keys)
    time.sleep(0.8)


def write_capture(session: str, out_dir: Path, name: str, meta: dict):
    plain = capture_plain(session)
    ansi = capture_ansi(session)
    (out_dir / f"{name}.txt").write_text(plain + "\n")
    (out_dir / f"{name}.ansi").write_text(ansi + "\n")
    meta["captures"].append(
        {
            "name": name,
            "txt": f"{name}.txt",
            "ansi": f"{name}.ansi",
        }
    )
    return plain


def capture_size(label: str, cols: int, rows: int):
    session = f"ghzinga-ghdash-{label}"
    out_dir = ROOT / label
    out_dir.mkdir(parents=True, exist_ok=True)
    tmux("kill-session", "-t", session, check=False)
    tmux(
        "new-session",
        "-d",
        "-x",
        str(cols),
        "-y",
        str(rows),
        "-s",
        session,
        f"TERM=xterm-256color gh dash --config {CONFIG}",
    )
    tmux("resize-window", "-t", session, "-x", str(cols), "-y", str(rows))
    actual_size = tmux(
        "display-message",
        "-t",
        session,
        "-p",
        "#{window_width}x#{window_height}",
    ).stdout.strip()
    meta = {
        "label": label,
        "requested_columns": cols,
        "requested_rows": rows,
        "actual_tmux_size": actual_size,
        "session": session,
        "command": f"gh dash --config {CONFIG}",
        "captures": [],
    }
    wait_for_loaded(session, "feat(senseaudio)")

    write_capture(session, out_dir, "00_initial", meta)
    send(session, "j")
    write_capture(session, out_dir, "01_after_j_down", meta)
    send(session, "k")
    write_capture(session, out_dir, "02_after_k_up", meta)
    send(session, "C-d")
    write_capture(session, out_dir, "03_after_ctrl_d_page_down", meta)
    send(session, "C-u")
    write_capture(session, out_dir, "04_after_ctrl_u_page_up", meta)
    send(session, "PageDown")
    write_capture(session, out_dir, "05_after_page_down", meta)
    send(session, "PageUp")
    write_capture(session, out_dir, "06_after_page_up", meta)
    send(session, "G")
    write_capture(session, out_dir, "07_after_G_end", meta)
    send(session, "g")
    write_capture(session, out_dir, "08_after_g_home", meta)

    history_plain = tmux("capture-pane", "-t", session, "-S", "-", "-E", "-", "-N", "-p").stdout
    history_ansi = tmux("capture-pane", "-t", session, "-S", "-", "-E", "-", "-N", "-e", "-p").stdout
    (out_dir / "tmux_history.txt").write_text(history_plain)
    (out_dir / "tmux_history.ansi").write_text(history_ansi)
    (out_dir / "manifest.json").write_text(json.dumps(meta, indent=2) + "\n")
    tmux("kill-session", "-t", session, check=False)


def main():
    overall = {
        "target": "openclaw/openclaw#81834",
        "title": "feat(senseaudio): add SenseAudio TTS provider",
        "config": str(CONFIG),
        "sizes": [],
    }
    for label, cols, rows in SIZES:
        capture_size(label, cols, rows)
        overall["sizes"].append({"label": label, "columns": cols, "rows": rows})
    (ROOT / "manifest.json").write_text(json.dumps(overall, indent=2) + "\n")


if __name__ == "__main__":
    main()
