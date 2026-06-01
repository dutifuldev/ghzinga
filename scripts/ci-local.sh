#!/usr/bin/env bash
set -euo pipefail

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings

scripts/verify-install.sh
npx -y @simpledoc/simpledoc check
scripts/verify-no-png-captures.sh

python3 captures/ghzinga-pr-81834/capture_ghzinga.py --self-test
python3 captures/ghzinga-pr-81834/capture_ghzinga.py --validate-only
python3 captures/ghzinga-pr-81834/capture_ghzinga.py \
  --root captures/ghzinga-issue-88499 \
  --mode issue \
  --validate-only
python3 captures/ghzinga-pr-81834/capture_mouse_smoke.py --self-test
python3 captures/ghzinga-pr-81834/capture_mouse_smoke.py --validate-only
python3 captures/ghzinga-issue-88499/capture_mouse_smoke.py --self-test
python3 captures/ghzinga-issue-88499/capture_mouse_smoke.py --validate-only
