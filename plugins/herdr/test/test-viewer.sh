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

gzg_log="${work_dir}/gzg.log"
env \
  GHZINGA_TARGET="https://github.com/dutifuldev/ghzinga/pull/29" \
  GHZINGA_SESSION="herdr-ghzinga-w1_p1" \
  GHZINGA_BIN="${script_dir}/fake-gzg.sh" \
  GZG_FAKE_LOG="$gzg_log" \
  "$gzg_bin" herdr-plugin viewer >/dev/null
assert_contains "$gzg_log" "--session herdr-ghzinga-w1_p1 https://github.com/dutifuldev/ghzinga/pull/29"

missing_err="${work_dir}/missing.err"
if env \
  GHZINGA_BIN="${script_dir}/fake-gzg.sh" \
  GZG_FAKE_LOG="${work_dir}/missing-gzg.log" \
  "$gzg_bin" herdr-plugin viewer 2>"$missing_err"; then
  printf 'expected missing target to fail\n' >&2
  exit 1
fi
assert_contains "$missing_err" "GHZINGA_TARGET is not set"

printf 'OK: herdr plugin viewer tests passed.\n'
