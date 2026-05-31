#!/usr/bin/env python3
import json
import re
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parent
CONFIG = ROOT / "config.yml"
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"


SIZES = [
    ("narrow", 80, 24),
    ("medium", 120, 36),
    ("large", 160, 50),
]

CSI_RE = re.compile(r"\x1b\[([0-9;?]*)([A-Za-z])")
OSC_RE = re.compile(r"\x1b\].*?(\x07|\x1b\\)")


FG_16 = {
    30: (0, 0, 0),
    31: (205, 49, 49),
    32: (13, 188, 121),
    33: (229, 229, 16),
    34: (36, 114, 200),
    35: (188, 63, 188),
    36: (17, 168, 205),
    37: (229, 229, 229),
    90: (102, 102, 102),
    91: (241, 76, 76),
    92: (35, 209, 139),
    93: (245, 245, 67),
    94: (59, 142, 234),
    95: (214, 112, 214),
    96: (41, 184, 219),
    97: (255, 255, 255),
}
BG_16 = {code + 10: color for code, color in FG_16.items() if code < 40}
BG_16.update({code + 10: color for code, color in FG_16.items() if code >= 90})


@dataclass
class Style:
    fg: tuple[int, int, int] = (229, 229, 229)
    bg: tuple[int, int, int] = (12, 12, 12)


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
    render_ansi_png(ansi, out_dir / f"{name}.png")
    meta["captures"].append(
        {
            "name": name,
            "txt": f"{name}.txt",
            "ansi": f"{name}.ansi",
            "png": f"{name}.png",
        }
    )
    return plain


def apply_sgr(style: Style, params: list[int]) -> Style:
    if not params:
        params = [0]
    style = Style(style.fg, style.bg)
    i = 0
    while i < len(params):
        p = params[i]
        if p == 0:
            style = Style()
        elif p == 39:
            style.fg = Style().fg
        elif p == 49:
            style.bg = Style().bg
        elif p in FG_16:
            style.fg = FG_16[p]
        elif p in BG_16:
            style.bg = BG_16[p]
        elif p in (38, 48) and i + 4 < len(params) and params[i + 1] == 2:
            color = (params[i + 2], params[i + 3], params[i + 4])
            if p == 38:
                style.fg = color
            else:
                style.bg = color
            i += 4
        i += 1
    return style


def parse_ansi_line(line: str):
    line = OSC_RE.sub("", line)
    cells = []
    style = Style()
    i = 0
    while i < len(line):
        if line[i] == "\x1b":
            match = CSI_RE.match(line, i)
            if match:
                params_raw, command = match.groups()
                if command == "m":
                    params = [int(p) for p in params_raw.split(";") if p and p != "?"]
                    style = apply_sgr(style, params)
                i = match.end()
                continue
        cells.append((line[i], style.fg, style.bg))
        i += 1
    return cells


def render_ansi_png(ansi: str, path: Path):
    lines = ansi.splitlines() or [""]
    font = ImageFont.truetype(FONT, 15)
    bbox = font.getbbox("M")
    cell_w = bbox[2] - bbox[0] + 1
    cell_h = bbox[3] - bbox[1] + 6
    width = max(len(strip_ansi(line)) for line in lines)
    height = len(lines)
    image = Image.new("RGB", (max(1, width) * cell_w, max(1, height) * cell_h), Style().bg)
    draw = ImageDraw.Draw(image)
    for y, line in enumerate(lines):
        for x, (char, fg, bg) in enumerate(parse_ansi_line(line)):
            draw.rectangle(
                (x * cell_w, y * cell_h, (x + 1) * cell_w, (y + 1) * cell_h),
                fill=bg,
            )
            draw.text((x * cell_w, y * cell_h), char, font=font, fill=fg)
    image.save(path)


def strip_ansi(text: str) -> str:
    text = OSC_RE.sub("", text)
    text = CSI_RE.sub("", text)
    return text


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
    render_ansi_png(history_ansi.rstrip("\n"), out_dir / "tmux_history.png")
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
