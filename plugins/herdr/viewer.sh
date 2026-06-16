#!/usr/bin/env sh
set -eu

ghzinga_bin() {
  if [ -n "${GHZINGA_BIN:-}" ]; then
    printf '%s\n' "$GHZINGA_BIN"
    return
  fi
  if command -v gzg >/dev/null 2>&1; then
    printf '%s\n' 'gzg'
    return
  fi
  if command -v ghzinga >/dev/null 2>&1; then
    printf '%s\n' 'ghzinga'
    return
  fi
  printf '%s\n' 'gzg'
}

target=${GHZINGA_TARGET:-}
[ -n "$target" ] || {
  printf 'ghzinga-herdr: GHZINGA_TARGET is not set\n' >&2
  exit 1
}

session=${GHZINGA_SESSION:-herdr-ghzinga}
gzg=$(ghzinga_bin)

exec "$gzg" --session "$session" "$target"
