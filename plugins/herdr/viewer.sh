#!/usr/bin/env sh
set -eu

target=${GHZINGA_TARGET:-}
[ -n "$target" ] || {
  printf 'ghzinga-herdr: GHZINGA_TARGET is not set\n' >&2
  exit 1
}

session=${GHZINGA_SESSION:-herdr-ghzinga}
gzg=${GHZINGA_BIN:-gzg}

exec "$gzg" --session "$session" "$target"
