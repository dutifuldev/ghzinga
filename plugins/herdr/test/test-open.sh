#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
plugin_dir=$(CDPATH= cd -- "${script_dir}/.." && pwd)
repo_root=$(CDPATH= cd -- "${plugin_dir}/../.." && pwd)
gzg_bin=${GHZINGA_TEST_BIN:-${repo_root}/target/debug/gzg}
work_dir=$(mktemp -d)

cleanup() {
  rm -rf "$work_dir"
}
trap cleanup EXIT INT TERM

herdr_socket="${work_dir}/herdr-a.sock"

cargo build --manifest-path "${repo_root}/Cargo.toml" --bin gzg >/dev/null

assert_contains() {
  file=$1
  expected=$2
  if ! grep -Fq -- "$expected" "$file"; then
    printf 'expected to find: %s\n' "$expected" >&2
    printf '%s\n' '--- file ---' >&2
    cat "$file" >&2
    exit 1
  fi
}

assert_not_contains() {
  file=$1
  unexpected=$2
  [ -f "$file" ] || return 0
  if grep -Fq -- "$unexpected" "$file"; then
    printf 'expected not to find: %s\n' "$unexpected" >&2
    printf '%s\n' '--- file ---' >&2
    cat "$file" >&2
    exit 1
  fi
}

first_state_file() {
  find "$1" -name '*.pane' -type f -print | sort | sed -n '1p'
}

session_from_herdr_log() {
  sed -n 's/.*--env GHZINGA_SESSION=\([^ ]*\).*/\1/p' "$1" | tail -n 1
}

assert_different() {
  left=$1
  right=$2
  if [ "$left" = "$right" ]; then
    printf 'expected values to differ: %s\n' "$left" >&2
    exit 1
  fi
}

run_open() {
  url=$1
  state=$2
  herdr_log=$3
  gzg_log=$4
  shift 4
  env \
    HERDR_PLUGIN_CLICKED_URL="$url" \
    HERDR_PANE_ID="w1:p1" \
    HERDR_PLUGIN_ID="dutifuldev.ghzinga" \
    HERDR_PLUGIN_STATE_DIR="$state" \
    HERDR_SOCKET_PATH="$herdr_socket" \
    HERDR_BIN_PATH="${script_dir}/fake-herdr.sh" \
    HERDR_FAKE_LOG="$herdr_log" \
    GZG_FAKE_LOG="$gzg_log" \
    GHZINGA_BIN="${script_dir}/fake-gzg.sh" \
    "$@" \
    "$gzg_bin" herdr-plugin open >/dev/null
}

first_state="${work_dir}/first-state"
mkdir -p "$first_state"
first_herdr="${work_dir}/first-herdr.log"
first_gzg="${work_dir}/first-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/pull/29" "$first_state" "$first_herdr" "$first_gzg"
herdr_session=$(session_from_herdr_log "$first_herdr")
assert_contains "$first_herdr" "plugin pane open --plugin dutifuldev.ghzinga --entrypoint viewer --placement split --target-pane w1:p1 --direction right"
assert_contains "$first_herdr" "--env GHZINGA_TARGET=https://github.com/dutifuldev/ghzinga/pull/29"
assert_contains "$first_herdr" "--env GHZINGA_SESSION=$herdr_session"
assert_contains "$first_herdr" "--env GHZINGA_BIN=${script_dir}/fake-gzg.sh"
assert_contains "$(first_state_file "$first_state")" "w1:p9"

second_socket="${work_dir}/herdr-b.sock"
shared_state="${work_dir}/shared-state"
mkdir -p "$shared_state"
shared_first_herdr="${work_dir}/shared-first-herdr.log"
shared_first_gzg="${work_dir}/shared-first-gzg.log"
shared_second_herdr="${work_dir}/shared-second-herdr.log"
shared_second_gzg="${work_dir}/shared-second-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/pull/30" "$shared_state" "$shared_first_herdr" "$shared_first_gzg" \
  HERDR_FAKE_OPENED_PANE=w1:p9
run_open "https://github.com/dutifuldev/ghzinga/pull/31" "$shared_state" "$shared_second_herdr" "$shared_second_gzg" \
  HERDR_SOCKET_PATH="$second_socket" \
  HERDR_FAKE_OPENED_PANE=w1:p8
shared_first_session=$(session_from_herdr_log "$shared_first_herdr")
shared_second_session=$(session_from_herdr_log "$shared_second_herdr")
assert_different "$shared_first_session" "$shared_second_session"
assert_contains "$shared_second_herdr" "--env GHZINGA_SESSION=$shared_second_session"
if [ "$(find "$shared_state" -name '*.pane' -type f | wc -l | tr -d ' ')" != "2" ]; then
  printf 'expected two scoped state files\n' >&2
  find "$shared_state" -name '*.pane' -type f -print >&2
  exit 1
fi
grep -R -Fq 'w1:p9' "$shared_state"
grep -R -Fq 'w1:p8' "$shared_state"

issue_state="${work_dir}/issue-state"
mkdir -p "$issue_state"
issue_herdr="${work_dir}/issue-herdr.log"
issue_gzg="${work_dir}/issue-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/issues/32/?utm_source=test#note" "$issue_state" "$issue_herdr" "$issue_gzg"
assert_contains "$issue_herdr" "--env GHZINGA_TARGET=https://github.com/dutifuldev/ghzinga/issues/32"

reuse_state="${work_dir}/reuse-state"
mkdir -p "$reuse_state"
reuse_initial_herdr="${work_dir}/reuse-initial-herdr.log"
reuse_initial_gzg="${work_dir}/reuse-initial-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/pull/32" "$reuse_state" "$reuse_initial_herdr" "$reuse_initial_gzg" \
  HERDR_FAKE_OPENED_PANE=w1:p9
reuse_session=$(session_from_herdr_log "$reuse_initial_herdr")
self_herdr="${work_dir}/self-herdr.log"
self_gzg="${work_dir}/self-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/issues/37" "$reuse_state" "$self_herdr" "$self_gzg" \
  HERDR_PANE_ID=w1:p9 \
  HERDR_FAKE_PLUGIN_PANE=w1:p9
assert_contains "$self_herdr" "plugin pane focus w1:p9"
assert_not_contains "$self_herdr" "plugin pane open"
assert_contains "$self_gzg" "open --session $reuse_session https://github.com/dutifuldev/ghzinga/issues/37"
reuse_herdr="${work_dir}/reuse-herdr.log"
reuse_gzg="${work_dir}/reuse-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/pull/33" "$reuse_state" "$reuse_herdr" "$reuse_gzg" \
  HERDR_FAKE_EXISTING_PANE=w1:p9 \
  HERDR_FAKE_PLUGIN_PANE=w1:p9
assert_contains "$reuse_herdr" "pane get w1:p9"
assert_contains "$reuse_herdr" "plugin pane focus w1:p9"
assert_not_contains "$reuse_herdr" "plugin pane open"
assert_contains "$reuse_gzg" "open --session $reuse_session https://github.com/dutifuldev/ghzinga/pull/33"

fallback_bin="${work_dir}/fallback-bin"
mkdir -p "$fallback_bin"
ln -s "${script_dir}/fake-gzg.sh" "${fallback_bin}/ghzinga"
fallback_state="${work_dir}/fallback-state"
mkdir -p "$fallback_state"
fallback_initial_herdr="${work_dir}/fallback-initial-herdr.log"
fallback_initial_gzg="${work_dir}/fallback-initial-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/pull/36" "$fallback_state" "$fallback_initial_herdr" "$fallback_initial_gzg" \
  HERDR_FAKE_OPENED_PANE=w1:p6
fallback_session=$(session_from_herdr_log "$fallback_initial_herdr")
fallback_herdr="${work_dir}/fallback-herdr.log"
fallback_gzg="${work_dir}/fallback-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/pull/36" "$fallback_state" "$fallback_herdr" "$fallback_gzg" \
  PATH="$fallback_bin:/usr/bin:/bin" \
  GHZINGA_BIN= \
  HERDR_FAKE_EXISTING_PANE=w1:p6 \
  HERDR_FAKE_PLUGIN_PANE=w1:p6
assert_contains "$fallback_gzg" "open --session $fallback_session https://github.com/dutifuldev/ghzinga/pull/36"

stale_state="${work_dir}/stale-state"
mkdir -p "$stale_state"
stale_initial_herdr="${work_dir}/stale-initial-herdr.log"
stale_initial_gzg="${work_dir}/stale-initial-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/issues/34" "$stale_state" "$stale_initial_herdr" "$stale_initial_gzg" \
  HERDR_FAKE_OPENED_PANE=w1:p8
stale_herdr="${work_dir}/stale-herdr.log"
stale_gzg="${work_dir}/stale-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/issues/34" "$stale_state" "$stale_herdr" "$stale_gzg" \
  HERDR_FAKE_EXISTING_PANE=w1:p8
assert_contains "$stale_herdr" "pane get w1:p8"
assert_contains "$stale_herdr" "plugin pane focus w1:p8"
assert_contains "$stale_herdr" "plugin pane open --plugin dutifuldev.ghzinga"
assert_not_contains "$stale_gzg" "open --session"

other_plugin_state="${work_dir}/other-plugin-state"
mkdir -p "$other_plugin_state"
other_plugin_initial_herdr="${work_dir}/other-plugin-initial-herdr.log"
other_plugin_initial_gzg="${work_dir}/other-plugin-initial-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/pull/35" "$other_plugin_state" "$other_plugin_initial_herdr" "$other_plugin_initial_gzg" \
  HERDR_FAKE_OPENED_PANE=w1:p7
other_plugin_herdr="${work_dir}/other-plugin-herdr.log"
other_plugin_gzg="${work_dir}/other-plugin-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/pull/35" "$other_plugin_state" "$other_plugin_herdr" "$other_plugin_gzg" \
  HERDR_FAKE_EXISTING_PANE=w1:p7 \
  HERDR_FAKE_PLUGIN_PANE=w1:p7 \
  HERDR_FAKE_PLUGIN_ID=example.other
assert_contains "$other_plugin_herdr" "pane get w1:p7"
assert_contains "$other_plugin_herdr" "plugin pane focus w1:p7"
assert_contains "$other_plugin_herdr" "plugin pane open --plugin dutifuldev.ghzinga"
assert_not_contains "$other_plugin_gzg" "open --session"

invalid_err="${work_dir}/invalid.err"
if env \
  HERDR_PLUGIN_CLICKED_URL="https://github.com/dutifuldev/ghzinga/tree/main" \
  HERDR_PANE_ID="w1:p1" \
  HERDR_PLUGIN_STATE_DIR="${work_dir}/invalid-state" \
  HERDR_BIN_PATH="${script_dir}/fake-herdr.sh" \
  HERDR_FAKE_LOG="${work_dir}/invalid-herdr.log" \
  GZG_FAKE_LOG="${work_dir}/invalid-gzg.log" \
  "$gzg_bin" herdr-plugin open 2>"$invalid_err"; then
  printf 'expected invalid URL to fail\n' >&2
  exit 1
fi
assert_contains "$invalid_err" "unsupported GitHub issue/PR URL"

missing_err="${work_dir}/missing.err"
if env \
  HERDR_PANE_ID="w1:p1" \
  HERDR_PLUGIN_STATE_DIR="${work_dir}/missing-state" \
  HERDR_BIN_PATH="${script_dir}/fake-herdr.sh" \
  HERDR_FAKE_LOG="${work_dir}/missing-herdr.log" \
  GZG_FAKE_LOG="${work_dir}/missing-gzg.log" \
  "$gzg_bin" herdr-plugin open 2>"$missing_err"; then
  printf 'expected missing URL to fail\n' >&2
  exit 1
fi
assert_contains "$missing_err" "HERDR_PLUGIN_CLICKED_URL is not set"

printf 'OK: herdr plugin open tests passed.\n'
