#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$PROJECT_DIR/target/release"
APP_NAME="lfv.app"
APP_BUNDLE="$PROJECT_DIR/$APP_NAME"
DMG_NAME="lfv-0.3.0-macos.dmg"
VOL_NAME="lfv Installer"
TMP_MOUNT="/tmp/lfv-dmg-$$"

echo "=== lfv macOS DMG Builder ==="

# 1. Build release binary
echo "[1/6] Building release binary..."
cd "$PROJECT_DIR"
cargo build --release --bin lfv

# 2. Create .app bundle
echo "[2/6] Creating app bundle..."
rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"

cp "$BUILD_DIR/lfv" "$APP_BUNDLE/Contents/MacOS/"
cp "$SCRIPT_DIR/lfv-launcher" "$APP_BUNDLE/Contents/MacOS/"
chmod +x "$APP_BUNDLE/Contents/MacOS/lfv-launcher"
cp "$SCRIPT_DIR/Info.plist" "$APP_BUNDLE/Contents/"

# Optional: copy icon if present
if [[ -f "$SCRIPT_DIR/icon.icns" ]]; then
    cp "$SCRIPT_DIR/icon.icns" "$APP_BUNDLE/Contents/Resources/"
fi

# 3. Ad-hoc code sign (suppresses some Gatekeeper warnings)
echo "[3/6] Ad-hoc code signing..."
codesign --force --deep --sign - "$APP_BUNDLE" 2>/dev/null || true

# 4. Create staging directory for DMG
echo "[4/6] Staging DMG contents..."
mkdir -p "$TMP_MOUNT"
cp -a "$APP_BUNDLE" "$TMP_MOUNT/"
ln -s /Applications "$TMP_MOUNT/Applications"

# Optional: copy README into DMG
cp "$PROJECT_DIR/README.md" "$TMP_MOUNT/README.md" 2>/dev/null || true

# 5. Create DMG
echo "[5/6] Creating DMG..."
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

# 6. Clean up
echo "[6/6] Cleaning up..."
rm -rf "$TMP_MOUNT"

echo ""
echo "Done: $PROJECT_DIR/$DMG_NAME"
echo ""
echo "Install: double-click the DMG, drag lfv.app to Applications."
echo "Run: open /Applications/lfv.app"
echo ""
echo "Note: If Gatekeeper blocks the app, right-click lfv.app → Open."
