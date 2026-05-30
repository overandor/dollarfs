#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$PROJECT_DIR/target/release"
APP_NAME="lfv.app"
APP_BUNDLE="$PROJECT_DIR/$APP_NAME"
DMG_NAME="lfv-0.4.0-macos.dmg"
VOL_NAME="lfv Installer"
TMP_MOUNT="/tmp/lfv-dmg-$$"

echo "=== lfv macOS DMG Builder v0.4.0 ==="

# Clean old artifacts
echo "[0/7] Cleaning old artifacts..."
rm -rf "$APP_BUNDLE"
rm -f "$PROJECT_DIR/lfv-"*.dmg

# 1. Build release binary
echo "[1/7] Building release binary..."
cd "$PROJECT_DIR"
cargo build --release --bin lfv

# 2. Compile native ARM64 launcher stub
echo "[2/7] Compiling ARM64 Mach-O launcher stub..."
LAUNCHER_BIN="$SCRIPT_DIR/lfv-launcher-bin"
clang -arch arm64 -O2 -o "$LAUNCHER_BIN" "$SCRIPT_DIR/launcher.c"

# 3. Create .app bundle
echo "[3/7] Creating app bundle..."
rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"

cp "$BUILD_DIR/lfv" "$APP_BUNDLE/Contents/MacOS/"
cp "$LAUNCHER_BIN" "$APP_BUNDLE/Contents/MacOS/lfv-launcher"
chmod +x "$APP_BUNDLE/Contents/MacOS/lfv-launcher"
cp "$SCRIPT_DIR/lfv-launcher.sh" "$APP_BUNDLE/Contents/MacOS/lfv-launcher.sh"
chmod +x "$APP_BUNDLE/Contents/MacOS/lfv-launcher.sh"
cp "$SCRIPT_DIR/Info.plist" "$APP_BUNDLE/Contents/"

# Optional: copy icon if present
if [[ -f "$SCRIPT_DIR/icon.icns" ]]; then
    cp "$SCRIPT_DIR/icon.icns" "$APP_BUNDLE/Contents/Resources/"
fi

# 4. Ad-hoc code sign (suppresses some Gatekeeper warnings)
echo "[4/7] Ad-hoc code signing..."
codesign --force --deep --sign - "$APP_BUNDLE" 2>/dev/null || true

# 5. Create staging directory for DMG
echo "[5/7] Staging DMG contents..."
mkdir -p "$TMP_MOUNT"
cp -a "$APP_BUNDLE" "$TMP_MOUNT/"
ln -s /Applications "$TMP_MOUNT/Applications"

# Optional: copy README into DMG
cp "$PROJECT_DIR/README.md" "$TMP_MOUNT/README.md" 2>/dev/null || true

# 6. Create DMG
echo "[6/7] Creating DMG..."
hdiutil create \
    -srcfolder "$TMP_MOUNT" \
    -volname "$VOL_NAME" \
    -fs HFS+ \
    -fsargs "-c c=64,a=16,e=16" \
    -format UDBZ \
    -size 20m \
    "$PROJECT_DIR/$DMG_NAME" \
    2>/dev/null || true

# Retry without -fsargs if first attempt fails
if [[ ! -f "$PROJECT_DIR/$DMG_NAME" ]]; then
    hdiutil create \
        -srcfolder "$TMP_MOUNT" \
        -volname "$VOL_NAME" \
        -fs HFS+ \
        -format UDBZ \
        -size 20m \
        "$PROJECT_DIR/$DMG_NAME"
fi

# 7. Clean up
echo "[7/7] Cleaning up..."
rm -rf "$TMP_MOUNT"
rm -f "$LAUNCHER_BIN"

echo ""
echo "Done: $PROJECT_DIR/$DMG_NAME"
echo ""
echo "Install: double-click the DMG, drag lfv.app to Applications."
echo "Run: open /Applications/lfv.app"
echo ""
echo "Note: If Gatekeeper blocks the app, right-click lfv.app → Open."
