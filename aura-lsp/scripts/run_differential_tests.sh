#!/usr/bin/env bash
# Debugger compatibility smoke tests used by CI.
#
# This script does NOT claim semantic equivalence between Aura backends.
# It compiles the checked-in C debugger fixtures and requires the selected
# debugger(s) to launch each fixture, break at main, and inspect the frame
# without returning an error.

set -Eeuo pipefail

BACKEND="${1:-both}"
TIMEOUT_SECONDS="${2:-60}"

if ! [[ "$TIMEOUT_SECONDS" =~ ^[0-9]+$ ]] || [ "$TIMEOUT_SECONDS" -lt 1 ]; then
    echo "Invalid timeout: $TIMEOUT_SECONDS" >&2
    exit 2
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
LSP_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
PROGRAM_DIR="$LSP_ROOT/tests/programs"
RESULT_DIR="$LSP_ROOT/results/debugger"
BUILD_DIR="$(mktemp -d "${TMPDIR:-/tmp}/aura-debugger-smoke.XXXXXX")"

cleanup() {
    rm -rf "$BUILD_DIR"
}
trap cleanup EXIT

mkdir -p "$RESULT_DIR"

require_command() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "Missing required command: $1" >&2
        exit 3
    }
}

require_command timeout

mapfile -t SOURCES < <(find "$PROGRAM_DIR" -maxdepth 1 -type f -name '*.c' -print | sort)

if [ "${#SOURCES[@]}" -eq 0 ]; then
    echo "No debugger fixtures found in $PROGRAM_DIR" >&2
    exit 4
fi

run_gdb() {
    require_command gcc
    require_command gdb

    local source name binary log
    mkdir -p "$RESULT_DIR/gdb"

    for source in "${SOURCES[@]}"; do
        name="$(basename "$source" .c)"
        binary="$BUILD_DIR/${name}-gdb"
        log="$RESULT_DIR/gdb/${name}.log"

        echo "[GDB] compile $name"
        gcc -g -O0 -Wall -Wextra -o "$binary" "$source"

        echo "[GDB] inspect $name"
        if ! timeout "${TIMEOUT_SECONDS}s" \
            gdb -q -batch \
                -ex "set pagination off" \
                -ex "break main" \
                -ex "run" \
                -ex "info locals" \
                -ex "quit" \
                --args "$binary" >"$log" 2>&1; then
            cat "$log" >&2 || true
            echo "GDB smoke failed for $name" >&2
            return 1
        fi

        grep -q "Breakpoint" "$log" || {
            cat "$log" >&2 || true
            echo "GDB did not reach a breakpoint for $name" >&2
            return 1
        }

        echo "[GDB] PASS $name"
    done
}

run_lldb() {
    require_command clang
    require_command lldb

    local source name binary log
    mkdir -p "$RESULT_DIR/lldb"

    for source in "${SOURCES[@]}"; do
        name="$(basename "$source" .c)"
        binary="$BUILD_DIR/${name}-lldb"
        log="$RESULT_DIR/lldb/${name}.log"

        echo "[LLDB] compile $name"
        clang -g -O0 -Wall -Wextra -o "$binary" "$source"

        echo "[LLDB] inspect $name"
        if ! timeout "${TIMEOUT_SECONDS}s" \
            lldb --batch \
                -o "breakpoint set --name main" \
                -o "run" \
                -o "frame variable" \
                -o "quit" \
                -- "$binary" >"$log" 2>&1; then
            cat "$log" >&2 || true
            echo "LLDB smoke failed for $name" >&2
            return 1
        fi

        if ! grep -Eq 'stop reason = breakpoint|stopped.*breakpoint|Breakpoint' "$log"; then
            cat "$log" >&2 || true
            echo "LLDB did not reach a breakpoint for $name" >&2
            return 1
        fi

        echo "[LLDB] PASS $name"
    done
}

case "$BACKEND" in
    gdb)
        run_gdb
        ;;
    lldb)
        run_lldb
        ;;
    both)
        run_gdb
        run_lldb
        ;;
    *)
        echo "Usage: $0 [gdb|lldb|both] [timeout-seconds]" >&2
        exit 2
        ;;
esac

echo "Debugger compatibility smoke passed: backend=$BACKEND fixtures=${#SOURCES[@]}"