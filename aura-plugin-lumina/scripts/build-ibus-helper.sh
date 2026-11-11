#!/usr/bin/env bash
# Build the *real* libibus client into a user-selected output binary.
# No Rust FFI, no new Cargo.lock dependencies, no packaged third-party font.
set -euo pipefail
test "$#" -eq 1 || { echo 'usage: build-ibus-helper.sh OUTPUT_PATH' >&2; exit 2; }
command -v pkg-config >/dev/null || exit 2
pkg-config --exists ibus-1.0 || { echo 'install libibus-1.0-dev' >&2; exit 3; }
mkdir -p "$(dirname "$1")"
cc -D_GNU_SOURCE -std=c11 -O2 -Wall -Wextra -Werror -o "$1" \
  "$(dirname "$0")/../native/ibus_input_helper.c" \
  $(pkg-config --cflags --libs ibus-1.0)
