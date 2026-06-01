#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "${script_dir}/.." && pwd)
work_dir=$(mktemp -d)
no_gh_path="${work_dir}/no-gh-path"

cleanup() {
  rm -rf "$work_dir"
}
trap cleanup EXIT INT TERM

pr_target="${GZG_LIVE_PR_TARGET:-openclaw/openclaw#81834}"
issue_target="${GZG_LIVE_ISSUE_TARGET:-https://github.com/openclaw/openclaw/issues/88499}"
binary="${repo_root}/target/debug/gzg"

if [ ! -x "$binary" ]; then
  cargo build --manifest-path "${repo_root}/Cargo.toml" --bin gzg
fi

mkdir -p "$no_gh_path"

run_case() {
  name="$1"
  target="$2"
  tab="$3"
  shift 3
  output="${work_dir}/${name}.txt"
  GZG_CONFIG_PATH="${work_dir}/config.toml" \
    "$binary" "$target" --tab "$tab" --once --refresh-seconds 0 >"$output"

  for marker in "$@"; do
    if ! grep -Fq "$marker" "$output"; then
      printf 'Live smoke case %s did not render marker: %s\n' "$name" "$marker" >&2
      printf '--- output ---\n' >&2
      cat "$output" >&2
      exit 1
    fi
  done

  printf 'OK: %s rendered %s %s\n' "$name" "$target" "$tab"
}

run_public_case() {
  name="$1"
  target="$2"
  tab="$3"
  shift 3
  output="${work_dir}/${name}.txt"
  GH_TOKEN= GITHUB_TOKEN= PATH="$no_gh_path" GZG_CONFIG_PATH="${work_dir}/${name}.config.toml" \
    "$binary" "$target" --tab "$tab" --once --refresh-seconds 0 >"$output"

  for marker in "$@"; do
    if ! grep -Fq "$marker" "$output"; then
      printf 'Live public smoke case %s did not render marker: %s\n' "$name" "$marker" >&2
      printf '--- output ---\n' >&2
      cat "$output" >&2
      exit 1
    fi
  done

  printf 'OK: %s rendered public fallback %s %s\n' "$name" "$target" "$tab"
}

run_case pr_overview "$pr_target" overview \
  "[Overview]" "Activity  Commits  Checks  Files  Links" "[refresh]" "[expand all]"
run_case pr_activity "$pr_target" activity \
  "[Activity]" "Comment by" "[details]"
run_case pr_commits "$pr_target" commits \
  "[Commits]" "[+ more]"
run_case pr_checks "$pr_target" checks \
  "[Checks]" "Summary:" "[+ more]"
run_case pr_files "$pr_target" files \
  "[Files]" "files" "[+ more]"
run_case pr_links "$pr_target" links \
  "[Links]" "openclaw/openclaw#"
run_case issue_overview "$issue_target" overview \
  "[Overview]" "Activity  Links" "Bug Description" "[expand all]"
run_case issue_activity "$issue_target" activity \
  "[Activity]" "Comment by" "[details]"
run_case issue_links "$issue_target" links \
  "[Links]" "openclaw/openclaw#"
run_public_case public_pr_overview "$pr_target" overview \
  "[Overview]" "Activity  Commits" "[refresh]"
run_public_case public_issue_overview "$issue_target" overview \
  "[Overview]" "Activity  Links" "[refresh]"

printf 'OK: live GitHub smoke checks passed. Override targets with GZG_LIVE_PR_TARGET and GZG_LIVE_ISSUE_TARGET.\n'
