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
# Mutation testing on code changed relative to main. Listing forms cannot
# fail on a surviving mutant, so only an executing run counts as evidence.
if [ -f "$(git rev-parse --git-dir)/shallow" ]; then
  git fetch --no-tags --unshallow origin "+main:refs/remotes/origin/main"
else
  git fetch --no-tags origin "+main:refs/remotes/origin/main" \
    || git rev-parse --verify --quiet origin/main >/dev/null
fi
mutation_base="$(git merge-base origin/main HEAD)"
if [ "$mutation_base" = "$(git rev-parse HEAD)" ]; then
  echo "HEAD is at the main merge base; no changed code to mutate"
else
  mutation_diff="$(mktemp)"
  git -c diff.mnemonicPrefix=false diff "$mutation_base"...HEAD > "$mutation_diff"
  if [ -s "$mutation_diff" ]; then
    cargo mutants --timeout 120 --in-diff "$mutation_diff"
  else
    echo "no changed code to mutate"
  fi
  rm -f "$mutation_diff"
fi
slophammer-rs dry . --format json
slophammer-rs check . --format json

scripts/verify-install.sh
sh -n scripts/live-smoke.sh
GZG_LIVE_SELF_TEST=1 scripts/live-smoke.sh
sh -n scripts/herdr-plugin-live-smoke.sh
HERDR_PLUGIN_LIVE_SELF_TEST=1 scripts/herdr-plugin-live-smoke.sh
for script in plugins/herdr/test/*.sh; do
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
