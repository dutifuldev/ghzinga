#!/usr/bin/env sh
set -eu

log=${HERDR_FAKE_LOG:?HERDR_FAKE_LOG is required}
printf '%s\n' "$*" >>"$log"

if [ "$#" -ge 3 ] && [ "$1" = "pane" ] && [ "$2" = "get" ]; then
  if [ "${HERDR_FAKE_EXISTING_PANE:-}" = "$3" ]; then
    printf '{"id":"fake","result":{"type":"pane_info","pane":{"pane_id":"%s"}}}\n' "$3"
    exit 0
  fi
  printf 'pane not found\n' >&2
  exit 1
fi

if [ "$#" -ge 4 ] && [ "$1" = "plugin" ] && [ "$2" = "pane" ] && [ "$3" = "focus" ]; then
  printf '{"id":"fake","result":{"type":"plugin_pane_focused","plugin_pane":{"plugin_id":"dutifuldev.ghzinga","entrypoint":"viewer","pane":{"pane_id":"%s"}}}}\n' "$4"
  exit 0
fi

if [ "$#" -ge 4 ] && [ "$1" = "plugin" ] && [ "$2" = "pane" ] && [ "$3" = "open" ]; then
  pane=${HERDR_FAKE_OPENED_PANE:-w1:p9}
  printf '{"id":"fake","result":{"type":"plugin_pane_opened","plugin_pane":{"plugin_id":"dutifuldev.ghzinga","entrypoint":"viewer","pane":{"pane_id":"%s"}}}}\n' "$pane"
  exit 0
fi

printf '{"id":"fake","result":{"type":"ok"}}\n'
