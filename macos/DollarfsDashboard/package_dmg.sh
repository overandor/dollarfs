#!/bin/bash

# Package DollarfsDashboard as .dmg

set -e

APP_NAME="DollarfsDashboard"
BUILD_DIR="build"
APP_PATH="$BUILD_DIR/$APP_NAME.app"
DMG_NAME="DollarfsDashboard-0.2.0"
DMG_PATH="$BUILD_DIR/$DMG_NAME.dmg"

echo "Packaging $APP_NAME as .dmg..."

# Check if app exists
if [ ! -d "$APP_PATH" ]; then
    echo "Error: $APP_PATH not found. Run build.sh first."
    exit 1
fi

# Create temporary DMG
hdiutil create -volname "$APP_NAME" -srcfolder "$APP_PATH" -ov -format UDRW "$DMG_PATH.temp.dmg"

# Mount the DMG
DEVICE=$(hdiutil attach -readwrite -noverify -noautoopen "$DMG_PATH.temp.dmg" | egrep '^/dev/' | sed 1q | awk '{print $1}')

# Set up the DMG appearance
echo "Configuring DMG..."
VOLUME="/Volumes/$APP_NAME"

# Create symbolic link to Applications
ln -s /Applications "$VOLUME/Applications"

# Set window position
echo '
tell application "Finder"
    tell disk "'$APP_NAME'"
        open
        set current view of container window to icon view
        set toolbar visible of container window to false
        set statusbar visible of container window to false
        set the bounds of container window to {400, 100, 920, 440}
        set viewOptions to the icon view options of container window
        set arrangement of viewOptions to not arranged
        set icon size of viewOptions to 128
        set position of item "'$APP_NAME'" of container window to {130, 205}
        set position of item "Applications" of container window to {400, 205}
        close
        open
        update without registering applications
    end tell
end tell
' | osascript

# Unmount
hdiutil detach "$DEVICE"

# Convert to compressed DMG
echo "Creating compressed DMG..."
hdiutil convert "$DMG_PATH.temp.dmg" -format UDZO -imagekey zlib-level=9 -o "$DMG_PATH"

# Clean up
rm -f "$DMG_PATH.temp.dmg"

echo "DMG created: $DMG_PATH"
