#!/usr/bin/env bash
set -euo pipefail

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo llvm-cov --fail-under-lines 85 --summary-only
cargo audit
cargo mutants --list
slophammer-rs dry . --format json
slophammer-rs check . --format json

scripts/verify-install.sh
sh -n scripts/live-smoke.sh
GZG_LIVE_SELF_TEST=1 scripts/live-smoke.sh
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
python3 scripts/update-capture-manifests.py --check
