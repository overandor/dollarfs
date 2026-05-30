#!/bin/bash
# lfv-launcher — macOS app bundle entry point for lfv
# Opens Terminal.app and runs `lfv tui` using the bundled binary.

set -e

SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_ROOT="$(cd "$SELF_DIR/.." && pwd)"
BINARY="$SELF_DIR/lfv"

# Fallback: if lfv binary is not bundled, try PATH
if [[ ! -x "$BINARY" ]]; then
    if command -v lfv >/dev/null 2>&1; then
        BINARY="lfv"
    else
        osascript -e 'display alert "lfv not found" message "The lfv binary is missing from the app bundle and not in PATH." buttons {"OK"} default button "OK"' >/dev/null 2>&1 || true
        exit 1
    fi
fi

# Ensure config directory exists so init isn't interactive in a weird way
LFV_CONFIG="${LFV_CONFIG:-$HOME/.local_file_value}"
mkdir -p "$LFV_CONFIG"

# Open Terminal and exec lfv tui so the shell is replaced
osascript <<EOF
tell application "Terminal"
    if not (exists window 1) then
        reopen
    end if
    activate
    do script "exec \"$BINARY\" tui"
end tell
EOF
