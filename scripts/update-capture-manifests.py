#!/usr/bin/env python3
import argparse
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.append(str(REPO / "captures" / "ghzinga-pr-81834"))

from capture_ghzinga import app_tree_hash, git_commit  # noqa: E402


def ghzinga_manifests() -> list[Path]:
    return sorted((REPO / "captures").glob("ghzinga-*/**/manifest.json"))


def read_json(path: Path) -> dict:
    return json.loads(path.read_text())


def write_json(path: Path, value: dict):
    path.write_text(json.dumps(value, indent=2) + "\n")


def check_manifests(expected_tree_hash: str) -> int:
    errors = []
    for path in ghzinga_manifests():
        manifest = read_json(path)
        actual = manifest.get("app_tree_hash")
        if actual != expected_tree_hash:
            errors.append(
                f"{path.relative_to(REPO)} app_tree_hash is {actual!r}, expected {expected_tree_hash!r}"
            )
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        print(
            "Run scripts/update-capture-manifests.py after confirming capture output still matches.",
            file=sys.stderr,
        )
        return 1
    print("OK: ghzinga capture manifests match the current app/rendering tree hash.")
    return 0


def update_manifests(commit: str, tree_hash: str):
    for path in ghzinga_manifests():
        manifest = read_json(path)
        manifest["git_commit"] = commit
        manifest["app_tree_hash"] = tree_hash
        write_json(path, manifest)
    print(f"Updated {len(ghzinga_manifests())} ghzinga capture manifests to {commit}.")


def main():
    parser = argparse.ArgumentParser(
        description="Stamp ghzinga capture manifests with the current app/rendering tree identity."
    )
    parser.add_argument(
        "--commit",
        default=git_commit(),
        help="Git commit to record in manifests. Defaults to HEAD.",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify manifests already match the current app/rendering tree hash.",
    )
    args = parser.parse_args()

    tree_hash = app_tree_hash(args.commit)
    if tree_hash == "unknown":
        raise SystemExit(f"could not compute app/rendering tree hash for {args.commit!r}")

    if args.check:
        raise SystemExit(check_manifests(tree_hash))

    update_manifests(args.commit, tree_hash)


if __name__ == "__main__":
    main()
