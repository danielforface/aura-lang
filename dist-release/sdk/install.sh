#!/usr/bin/env sh
set -eu

info() { printf '%s\n' "[AuraSDK] $*"; }

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
AURA_HOME=${AURA_HOME:-"$SCRIPT_DIR"}

BIN_DIR="$AURA_HOME/bin"

if [ ! -d "$BIN_DIR" ]; then
  echo "bin/ folder not found at: $BIN_DIR" >&2
  exit 1
fi

# Pick a profile file to edit.
PROFILE=""
if [ -n "${SHELL:-}" ] && echo "$SHELL" | grep -q "zsh"; then
  PROFILE="$HOME/.zshrc"
elif [ -f "$HOME/.bashrc" ]; then
  PROFILE="$HOME/.bashrc"
else
  PROFILE="$HOME/.profile"
fi

touch "$PROFILE"

ensure_line() {
  line="$1"
  file="$2"
  if ! grep -Fqs "$line" "$file"; then
    printf '\n%s\n' "$line" >> "$file"
  fi
}

ensure_line "export AURA_HOME=\"$AURA_HOME\"" "$PROFILE"
ensure_line "export PATH=\"$BIN_DIR:$PATH\"" "$PROFILE"

info "Wrote AURA_HOME + PATH updates to $PROFILE"

# Smoke test (best-effort)
if [ -x "$BIN_DIR/aura" ]; then
  "$BIN_DIR/aura" --version >/dev/null 2>&1 || true
  "$BIN_DIR/aura" --help >/dev/null 2>&1 || true
  info "Smoke test attempted: aura --version / --help"
else
  info "Smoke test skipped: $BIN_DIR/aura not found or not executable"
fi

info "Done. Restart your terminal and run: aura init"
