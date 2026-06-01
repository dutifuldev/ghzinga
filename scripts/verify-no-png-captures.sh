#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "${script_dir}/.." && pwd)

cd "$repo_root"

tracked_pngs=$(git ls-files 'captures/**/*.png')
working_pngs=""

if [ -d captures ]; then
  working_pngs=$(find captures -name '*.png' -print)
fi

if [ -n "$tracked_pngs" ] || [ -n "$working_pngs" ]; then
  printf 'PNG capture artifacts are not allowed in this repository.\n' >&2
  if [ -n "$tracked_pngs" ]; then
    printf '\nTracked PNG files:\n%s\n' "$tracked_pngs" >&2
  fi
  if [ -n "$working_pngs" ]; then
    printf '\nPNG files present under captures/:\n%s\n' "$working_pngs" >&2
  fi
  exit 1
fi

printf 'OK: no PNG capture artifacts are tracked or present under captures/\n'
