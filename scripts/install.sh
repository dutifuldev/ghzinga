#!/usr/bin/env sh
set -eu

usage() {
  printf 'Usage: %s [--root PATH] [--debug]\n' "$0" >&2
}

install_root="${CARGO_INSTALL_ROOT:-${HOME}/.cargo}"
debug=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --root)
      if [ "$#" -lt 2 ]; then
        usage
        exit 2
      fi
      install_root="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --debug)
      debug=1
      shift
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "${script_dir}/.." && pwd)
bin_dir="${install_root}/bin"

if [ "$debug" -eq 1 ]; then
  cargo install --path "$repo_root" --bin gzg --root "$install_root" --force --debug
else
  cargo install --path "$repo_root" --bin gzg --root "$install_root" --force
fi

mkdir -p "$bin_dir"
rm -f "${bin_dir}/ghzinga"
ln -s gzg "${bin_dir}/ghzinga"

printf 'Installed %s/gzg\n' "$bin_dir"
printf 'Linked %s/ghzinga -> gzg\n' "$bin_dir"
