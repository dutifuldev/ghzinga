#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "${script_dir}/.." && pwd)
work_dir=$(mktemp -d)

cleanup() {
  rm -rf "$work_dir"
}
trap cleanup EXIT INT TERM

assert_output_contains() {
  output_file="$1"
  needle="$2"
  if ! grep -Fq "$needle" "$output_file"; then
    printf 'Expected %s to contain: %s\n' "$output_file" "$needle" >&2
    printf '%s\n' '--- output ---' >&2
    cat "$output_file" >&2
    exit 1
  fi
}

run_once() {
  binary="$1"
  output_file="$2"
  GZG_CONFIG_PATH="${work_dir}/config.toml" \
    "$binary" openclaw/openclaw#81834 \
      --offline-fixture "${repo_root}/fixtures/pr-81834.json" \
      --once >"$output_file"
  assert_output_contains "$output_file" "https://github.com/openclaw/openclaw/pull/81834"
  assert_output_contains "$output_file" "checks PASS"
}

plain_root="${work_dir}/plain"
cargo install --path "$repo_root" --root "$plain_root" --debug --force
test -x "${plain_root}/bin/gzg"
test -x "${plain_root}/bin/ghzinga"
run_once "${plain_root}/bin/gzg" "${work_dir}/gzg-once.txt"
run_once "${plain_root}/bin/ghzinga" "${work_dir}/ghzinga-once.txt"

linked_root="${work_dir}/linked"
"${repo_root}/scripts/install.sh" --root "$linked_root" --debug
test -x "${linked_root}/bin/gzg"
test -L "${linked_root}/bin/ghzinga"
test "$(readlink "${linked_root}/bin/ghzinga")" = "gzg"
run_once "${linked_root}/bin/ghzinga" "${work_dir}/linked-ghzinga-once.txt"

printf 'OK: cargo install exposes gzg and ghzinga; install.sh links ghzinga -> gzg\n'
