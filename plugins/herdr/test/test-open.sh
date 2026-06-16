#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
plugin_dir=$(CDPATH= cd -- "${script_dir}/.." && pwd)
work_dir=$(mktemp -d)

cleanup() {
  rm -rf "$work_dir"
}
trap cleanup EXIT INT TERM

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
    HERDR_BIN_PATH="${script_dir}/fake-herdr.sh" \
    HERDR_FAKE_LOG="$herdr_log" \
    GZG_FAKE_LOG="$gzg_log" \
    GHZINGA_BIN="${script_dir}/fake-gzg.sh" \
    "$@" \
    sh "${plugin_dir}/open.sh" >/dev/null
}

first_state="${work_dir}/first-state"
mkdir -p "$first_state"
first_herdr="${work_dir}/first-herdr.log"
first_gzg="${work_dir}/first-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/pull/29" "$first_state" "$first_herdr" "$first_gzg"
assert_contains "$first_herdr" "plugin pane open --plugin dutifuldev.ghzinga --entrypoint viewer --placement split --target-pane w1:p1 --direction right"
assert_contains "$first_herdr" "--env GHZINGA_TARGET=dutifuldev/ghzinga#29"
assert_contains "$first_herdr" "--env GHZINGA_SESSION=herdr-ghzinga-w1_p1"
assert_contains "$first_herdr" "--env GHZINGA_BIN=${script_dir}/fake-gzg.sh"
assert_contains "${first_state}/w1_p1.pane" "w1:p9"

issue_state="${work_dir}/issue-state"
mkdir -p "$issue_state"
issue_herdr="${work_dir}/issue-herdr.log"
issue_gzg="${work_dir}/issue-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/issues/32/?utm_source=test#note" "$issue_state" "$issue_herdr" "$issue_gzg"
assert_contains "$issue_herdr" "--env GHZINGA_TARGET=dutifuldev/ghzinga#32"

reuse_state="${work_dir}/reuse-state"
mkdir -p "$reuse_state"
printf 'w1:p9\n' >"${reuse_state}/w1_p1.pane"
reuse_herdr="${work_dir}/reuse-herdr.log"
reuse_gzg="${work_dir}/reuse-gzg.log"
HERDR_FAKE_EXISTING_PANE=w1:p9 HERDR_FAKE_PLUGIN_PANE=w1:p9 run_open "https://github.com/dutifuldev/ghzinga/pull/33" "$reuse_state" "$reuse_herdr" "$reuse_gzg"
assert_contains "$reuse_herdr" "pane get w1:p9"
assert_contains "$reuse_herdr" "plugin pane focus w1:p9"
assert_not_contains "$reuse_herdr" "plugin pane open"
assert_contains "$reuse_gzg" "open --session herdr-ghzinga-w1_p1 dutifuldev/ghzinga#33"

stale_state="${work_dir}/stale-state"
mkdir -p "$stale_state"
printf 'w1:p8\n' >"${stale_state}/w1_p1.pane"
stale_herdr="${work_dir}/stale-herdr.log"
stale_gzg="${work_dir}/stale-gzg.log"
HERDR_FAKE_EXISTING_PANE=w1:p8 run_open "https://github.com/dutifuldev/ghzinga/issues/34" "$stale_state" "$stale_herdr" "$stale_gzg"
assert_contains "$stale_herdr" "pane get w1:p8"
assert_contains "$stale_herdr" "plugin pane focus w1:p8"
assert_contains "$stale_herdr" "plugin pane open --plugin dutifuldev.ghzinga"
assert_not_contains "$stale_gzg" "open --session"

other_plugin_state="${work_dir}/other-plugin-state"
mkdir -p "$other_plugin_state"
printf 'w1:p7\n' >"${other_plugin_state}/w1_p1.pane"
other_plugin_herdr="${work_dir}/other-plugin-herdr.log"
other_plugin_gzg="${work_dir}/other-plugin-gzg.log"
HERDR_FAKE_EXISTING_PANE=w1:p7 HERDR_FAKE_PLUGIN_PANE=w1:p7 HERDR_FAKE_PLUGIN_ID=example.other run_open "https://github.com/dutifuldev/ghzinga/pull/35" "$other_plugin_state" "$other_plugin_herdr" "$other_plugin_gzg"
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
  sh "${plugin_dir}/open.sh" 2>"$invalid_err"; then
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
  sh "${plugin_dir}/open.sh" 2>"$missing_err"; then
  printf 'expected missing URL to fail\n' >&2
  exit 1
fi
assert_contains "$missing_err" "HERDR_PLUGIN_CLICKED_URL is not set"

printf 'OK: herdr plugin open tests passed.\n'
