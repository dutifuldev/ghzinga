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

gzg_log="${work_dir}/gzg.log"
env \
  GHZINGA_TARGET="dutifuldev/ghzinga#29" \
  GHZINGA_SESSION="herdr-ghzinga-w1_p1" \
  GHZINGA_BIN="${script_dir}/fake-gzg.sh" \
  GZG_FAKE_LOG="$gzg_log" \
  sh "${plugin_dir}/viewer.sh" >/dev/null
assert_contains "$gzg_log" "--session herdr-ghzinga-w1_p1 dutifuldev/ghzinga#29"

fallback_bin="${work_dir}/fallback-bin"
mkdir -p "$fallback_bin"
ln -s "${script_dir}/fake-gzg.sh" "${fallback_bin}/ghzinga"
fallback_log="${work_dir}/fallback-gzg.log"
env \
  PATH="$fallback_bin:/usr/bin:/bin" \
  GHZINGA_TARGET="dutifuldev/ghzinga#30" \
  GHZINGA_SESSION="herdr-ghzinga-fallback" \
  GZG_FAKE_LOG="$fallback_log" \
  sh "${plugin_dir}/viewer.sh" >/dev/null
assert_contains "$fallback_log" "--session herdr-ghzinga-fallback dutifuldev/ghzinga#30"

missing_err="${work_dir}/missing.err"
if env \
  GHZINGA_BIN="${script_dir}/fake-gzg.sh" \
  GZG_FAKE_LOG="${work_dir}/missing-gzg.log" \
  sh "${plugin_dir}/viewer.sh" 2>"$missing_err"; then
  printf 'expected missing target to fail\n' >&2
  exit 1
fi
assert_contains "$missing_err" "GHZINGA_TARGET is not set"

printf 'OK: herdr plugin viewer tests passed.\n'
