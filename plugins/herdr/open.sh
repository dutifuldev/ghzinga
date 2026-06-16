#!/usr/bin/env sh
set -eu
set -f

die() {
  printf 'ghzinga-herdr: %s\n' "$*" >&2
  exit 1
}

normalize_github_url() {
  url="$1"
  case "$url" in
    https://github.com/*) path=${url#https://github.com/} ;;
    *) return 1 ;;
  esac

  path=${path%%\?*}
  path=${path%%\#*}
  while [ "${path%/}" != "$path" ]; do
    path=${path%/}
  done

  old_ifs=$IFS
  IFS=/
  set -- $path
  IFS=$old_ifs

  owner=${1:-}
  repo=${2:-}
  kind=${3:-}
  number=${4:-}

  [ -n "$owner" ] || return 1
  [ -n "$repo" ] || return 1
  case "$kind" in
    issues | pull) ;;
    *) return 1 ;;
  esac
  case "$number" in
    '' | *[!0-9]*) return 1 ;;
  esac

  printf '%s/%s#%s\n' "$owner" "$repo" "$number"
}

state_key_for_pane() {
  printf '%s\n' "$1" | sed 's/[^A-Za-z0-9_-]/_/g'
}

json_pane_id() {
  sed -n 's/.*"pane_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | tail -n 1
}

plugin_focuses_viewer() {
  response=$1
  printf '%s\n' "$response" | grep -F "\"plugin_id\":\"$plugin_id\"" >/dev/null 2>&1 || return 1
  printf '%s\n' "$response" | grep -F '"entrypoint":"viewer"' >/dev/null 2>&1
}

clicked_url=${HERDR_PLUGIN_CLICKED_URL:-}
[ -n "$clicked_url" ] || die 'HERDR_PLUGIN_CLICKED_URL is not set'

source_pane=${HERDR_PANE_ID:-}
[ -n "$source_pane" ] || die 'HERDR_PANE_ID is not set'

target=$(normalize_github_url "$clicked_url") || die "unsupported GitHub issue/PR URL: $clicked_url"

herdr=${HERDR_BIN_PATH:-herdr}
gzg=${GHZINGA_BIN:-gzg}
plugin_id=${HERDR_PLUGIN_ID:-dutifuldev.ghzinga}
state_dir=${HERDR_PLUGIN_STATE_DIR:-${TMPDIR:-/tmp}/ghzinga-herdr-plugin}
mkdir -p "$state_dir"

source_key=$(state_key_for_pane "$source_pane")
session="herdr-ghzinga-${source_key}"
state_file="${state_dir}/${source_key}.pane"

stored_pane=
if [ -f "$state_file" ]; then
  stored_pane=$(sed -n '1p' "$state_file")
fi

if [ -n "$stored_pane" ] && "$herdr" pane get "$stored_pane" >/dev/null 2>&1; then
  if focus_response=$("$herdr" plugin pane focus "$stored_pane" 2>/dev/null) &&
    plugin_focuses_viewer "$focus_response"; then
    "$gzg" open --session "$session" "$target"
    exit 0
  fi
fi

set -- "$herdr" plugin pane open \
  --plugin "$plugin_id" \
  --entrypoint viewer \
  --placement split \
  --target-pane "$source_pane" \
  --direction right \
  --env "GHZINGA_TARGET=$target" \
  --env "GHZINGA_SESSION=$session" \
  --focus

if [ -n "${GHZINGA_BIN:-}" ]; then
  set -- "$@" --env "GHZINGA_BIN=$GHZINGA_BIN"
fi

response=$("$@")
printf '%s\n' "$response"

opened_pane=$(printf '%s\n' "$response" | json_pane_id)
if [ -n "$opened_pane" ]; then
  printf '%s\n' "$opened_pane" >"$state_file"
else
  printf 'ghzinga-herdr: warning: could not find opened pane id in Herdr response\n' >&2
fi
