#!/usr/bin/env sh
set -eu

log=${GZG_FAKE_LOG:?GZG_FAKE_LOG is required}
printf '%s\n' "$*" >>"$log"
printf 'fake gzg: %s\n' "$*"
