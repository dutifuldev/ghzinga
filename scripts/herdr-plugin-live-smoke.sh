#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
session=${HERDR_PLUGIN_LIVE_SESSION:-ghzinga-plugin-test}
target_url=${HERDR_PLUGIN_LIVE_URL:-https://github.com/openclaw/openclaw/pull/81834}

if [ "${HERDR_PLUGIN_LIVE_SELF_TEST:-0}" = "1" ]; then
  printf 'OK: herdr plugin live smoke self-test passed.\n'
  exit 0
fi

command -v herdr >/dev/null 2>&1 || {
  printf 'herdr is required for the live Herdr smoke test.\n' >&2
  exit 1
}
command -v python3 >/dev/null 2>&1 || {
  printf 'python3 is required for the live Herdr smoke test.\n' >&2
  exit 1
}

cargo build --manifest-path "${repo_root}/Cargo.toml" --bin gzg >/dev/null

HERDR_PLUGIN_LIVE_REPO_ROOT=$repo_root \
HERDR_PLUGIN_LIVE_SESSION_NAME=$session \
HERDR_PLUGIN_LIVE_TARGET_URL=$target_url \
python3 - <<'PY'
import json
import os
import pty
import select
import shutil
import signal
import subprocess
import tempfile
import time
from pathlib import Path

import errno
import fcntl
import struct
import termios


REPO = Path(os.environ["HERDR_PLUGIN_LIVE_REPO_ROOT"])
SESSION = os.environ["HERDR_PLUGIN_LIVE_SESSION_NAME"]
TARGET_URL = os.environ["HERDR_PLUGIN_LIVE_TARGET_URL"]
ROWS = 40
COLS = 140


def clean_env(extra=None):
    env = os.environ.copy()
    for key in (
        "HERDR_ENV",
        "HERDR_SOCKET_PATH",
        "HERDR_PANE_ID",
        "HERDR_TAB_ID",
        "HERDR_WORKSPACE_ID",
        "HERDR_SESSION",
    ):
        env.pop(key, None)
    env.setdefault("TERM", "xterm-256color")
    if extra:
        env.update(extra)
    return env


CLEAN_ENV = clean_env()


def run_herdr(args, *, check=True, timeout=20):
    result = subprocess.run(
        ["herdr", "--session", SESSION, *args],
        cwd=REPO,
        env=CLEAN_ENV,
        text=True,
        capture_output=True,
        timeout=timeout,
    )
    if check and result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise RuntimeError(f"herdr {' '.join(args)} failed: {detail}")
    return result


def stop_session():
    subprocess.run(
        ["herdr", "session", "stop", SESSION, "--json"],
        cwd=REPO,
        env=CLEAN_ENV,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    subprocess.run(
        ["herdr", "session", "delete", SESSION, "--json"],
        cwd=REPO,
        env=CLEAN_ENV,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )


def drain_pty(fd):
    while True:
        ready, _, _ = select.select([fd], [], [], 0)
        if not ready:
            return
        try:
            data = os.read(fd, 8192)
        except OSError as exc:
            if exc.errno in (errno.EAGAIN, errno.EIO):
                return
            raise
        if not data:
            return


def parse_json(stdout):
    return json.loads(stdout)["result"]


def wait_for_panes(fd, *, count=1, timeout=15):
    deadline = time.time() + timeout
    last = ""
    while time.time() < deadline:
        drain_pty(fd)
        result = run_herdr(["pane", "list"], check=False)
        last = result.stderr or result.stdout
        if result.returncode == 0:
            panes = parse_json(result.stdout)["panes"]
            if len(panes) >= count:
                return panes
        time.sleep(0.25)
    raise RuntimeError(f"expected at least {count} Herdr panes. Last output:\n{last}")


def wait_for_visible(pane_id, needles, *, timeout=45):
    deadline = time.time() + timeout
    last = ""
    while time.time() < deadline:
        result = run_herdr(
            ["pane", "read", pane_id, "--source", "visible", "--lines", "80"],
            check=False,
        )
        if result.returncode == 0:
            last = result.stdout
            if all(needle in last for needle in needles):
                return last
        time.sleep(0.5)
    raise RuntimeError(
        f"pane {pane_id} did not render {needles!r}. Last visible output:\n{last}"
    )


def write_executable(path, contents):
    path.write_text(contents)
    path.chmod(0o755)


def main():
    tmp = Path(tempfile.mkdtemp(prefix="ghzinga-herdr-plugin-"))
    child_pid = None
    try:
        stop_session()

        herdr_wrapper = tmp / "herdr-session.sh"
        gzg_wrapper = tmp / "gzg-fixture.sh"
        state_dir = tmp / "state"
        state_dir.mkdir()

        write_executable(
            herdr_wrapper,
            "#!/bin/sh\n"
            "exec herdr --session \"$HERDR_PLUGIN_LIVE_SESSION_NAME\" \"$@\"\n",
        )
        write_executable(
            gzg_wrapper,
            "#!/bin/sh\n"
            f"exec {str(REPO / 'target' / 'debug' / 'gzg')!r} \"$@\" "
            f"--offline-fixture {str(REPO / 'fixtures' / 'pr-81834.json')!r} "
            "--no-restore --refresh-seconds 0\n",
        )

        child_pid, fd = pty.fork()
        if child_pid == 0:
            os.chdir(REPO)
            env = clean_env({"TERM": "xterm-256color"})
            os.execvpe("herdr", ["herdr", "--session", SESSION], env)

        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        fcntl.fcntl(fd, fcntl.F_SETFL, os.O_NONBLOCK)

        panes = wait_for_panes(fd, count=1, timeout=20)
        source_pane = panes[0]["pane_id"]

        run_herdr(["plugin", "unlink", "dutifuldev.ghzinga"], check=False)
        link = parse_json(run_herdr(["plugin", "link", str(REPO / "plugins" / "herdr")]).stdout)
        handlers = link["plugin"].get("link_handlers", [])
        if not any(handler.get("id") == "github-issue-pr" for handler in handlers):
            raise RuntimeError(f"linked plugin did not expose github-issue-pr handler: {handlers}")

        action_env = clean_env(
            {
                "HERDR_PLUGIN_LIVE_SESSION_NAME": SESSION,
                "HERDR_PLUGIN_CLICKED_URL": TARGET_URL,
                "HERDR_PANE_ID": source_pane,
                "HERDR_PLUGIN_ID": "dutifuldev.ghzinga",
                "HERDR_PLUGIN_STATE_DIR": str(state_dir),
                "HERDR_BIN_PATH": str(herdr_wrapper),
                "GHZINGA_BIN": str(gzg_wrapper),
            }
        )
        opened = subprocess.run(
            ["sh", str(REPO / "plugins" / "herdr" / "open.sh")],
            cwd=REPO,
            env=action_env,
            text=True,
            capture_output=True,
            timeout=30,
        )
        if opened.returncode != 0:
            detail = opened.stderr.strip() or opened.stdout.strip()
            raise RuntimeError(f"plugin open entrypoint failed: {detail}")

        panes = wait_for_panes(fd, count=2, timeout=20)
        neighbor = run_herdr(
            ["pane", "neighbor", "--direction", "right", "--pane", source_pane],
            check=True,
        )
        neighbor_result = parse_json(neighbor.stdout)["neighbor"]
        neighbor_pane = neighbor_result["neighbor_pane_id"]
        if neighbor_pane == source_pane:
            raise RuntimeError("right neighbor resolved to the source pane")

        visible = wait_for_visible(neighbor_pane, ["Overview", "Activity", "Files"])
        if "openclaw" not in visible.lower():
            raise RuntimeError(f"ghzinga pane did not show the expected fixture:\n{visible}")

        print(f"OK: linked Herdr plugin {link['plugin']['plugin_id']}")
        print(f"OK: source pane {source_pane} opened right-side ghzinga pane {neighbor_pane}")
        print(f"OK: ghzinga rendered fixture content for {TARGET_URL}")
    finally:
        if child_pid is not None:
            try:
                os.kill(child_pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        stop_session()
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
PY
