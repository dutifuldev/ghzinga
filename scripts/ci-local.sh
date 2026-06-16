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
sh -n scripts/herdr-plugin-live-smoke.sh
HERDR_PLUGIN_LIVE_SELF_TEST=1 scripts/herdr-plugin-live-smoke.sh
for script in plugins/herdr/open.sh plugins/herdr/viewer.sh plugins/herdr/test/*.sh; do
  sh -n "$script"
done
plugins/herdr/test/test-open.sh
plugins/herdr/test/test-viewer.sh
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
