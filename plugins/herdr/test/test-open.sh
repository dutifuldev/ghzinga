#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
plugin_dir=$(CDPATH= cd -- "${script_dir}/.." && pwd)
work_dir=$(mktemp -d)

cleanup() {
  rm -rf "$work_dir"
}
trap cleanup EXIT INT TERM

herdr_socket="${work_dir}/herdr-a.sock"
herdr_scope_key="socket_$(printf '%s\n' "$herdr_socket" | cksum | sed 's/[[:space:]].*//')"
herdr_session="herdr-ghzinga-${herdr_scope_key}_w1_p1"

state_file_for() {
  printf '%s/%s_w1_p1.pane\n' "$1" "$herdr_scope_key"
}

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
    HERDR_SOCKET_PATH="$herdr_socket" \
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
assert_contains "$first_herdr" "--env GHZINGA_SESSION=$herdr_session"
assert_contains "$first_herdr" "--env GHZINGA_BIN=${script_dir}/fake-gzg.sh"
assert_contains "$(state_file_for "$first_state")" "w1:p9"

second_socket="${work_dir}/herdr-b.sock"
second_scope_key="socket_$(printf '%s\n' "$second_socket" | cksum | sed 's/[[:space:]].*//')"
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
assert_contains "${shared_state}/${herdr_scope_key}_w1_p1.pane" "w1:p9"
assert_contains "${shared_state}/${second_scope_key}_w1_p1.pane" "w1:p8"
assert_contains "$shared_second_herdr" "--env GHZINGA_SESSION=herdr-ghzinga-${second_scope_key}_w1_p1"

issue_state="${work_dir}/issue-state"
mkdir -p "$issue_state"
issue_herdr="${work_dir}/issue-herdr.log"
issue_gzg="${work_dir}/issue-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/issues/32/?utm_source=test#note" "$issue_state" "$issue_herdr" "$issue_gzg"
assert_contains "$issue_herdr" "--env GHZINGA_TARGET=dutifuldev/ghzinga#32"

reuse_state="${work_dir}/reuse-state"
mkdir -p "$reuse_state"
printf 'w1:p9\n' >"$(state_file_for "$reuse_state")"
reuse_herdr="${work_dir}/reuse-herdr.log"
reuse_gzg="${work_dir}/reuse-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/pull/33" "$reuse_state" "$reuse_herdr" "$reuse_gzg" \
  HERDR_FAKE_EXISTING_PANE=w1:p9 \
  HERDR_FAKE_PLUGIN_PANE=w1:p9
assert_contains "$reuse_herdr" "pane get w1:p9"
assert_contains "$reuse_herdr" "plugin pane focus w1:p9"
assert_not_contains "$reuse_herdr" "plugin pane open"
assert_contains "$reuse_gzg" "open --session $herdr_session dutifuldev/ghzinga#33"

fallback_bin="${work_dir}/fallback-bin"
mkdir -p "$fallback_bin"
ln -s "${script_dir}/fake-gzg.sh" "${fallback_bin}/ghzinga"
fallback_state="${work_dir}/fallback-state"
mkdir -p "$fallback_state"
printf 'w1:p6\n' >"$(state_file_for "$fallback_state")"
fallback_herdr="${work_dir}/fallback-herdr.log"
fallback_gzg="${work_dir}/fallback-gzg.log"
run_open "https://github.com/dutifuldev/ghzinga/pull/36" "$fallback_state" "$fallback_herdr" "$fallback_gzg" \
  PATH="$fallback_bin:/usr/bin:/bin" \
  GHZINGA_BIN= \
  HERDR_FAKE_EXISTING_PANE=w1:p6 \
  HERDR_FAKE_PLUGIN_PANE=w1:p6
assert_contains "$fallback_gzg" "open --session $herdr_session dutifuldev/ghzinga#36"

stale_state="${work_dir}/stale-state"
mkdir -p "$stale_state"
printf 'w1:p8\n' >"$(state_file_for "$stale_state")"
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
printf 'w1:p7\n' >"$(state_file_for "$other_plugin_state")"
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
