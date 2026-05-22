#!/bin/bash

# Build script for DollarfsDashboard macOS app

set -e

APP_NAME="DollarfsDashboard"
BUILD_DIR="build"
APP_PATH="$BUILD_DIR/$APP_NAME.app"
CONTENTS_DIR="$APP_PATH/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

echo "Building $APP_NAME..."

# Clean previous build
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

# Create app structure
mkdir -p "$MACOS_DIR"
mkdir -p "$RESOURCES_DIR"

# Copy Swift files
echo "Compiling Swift files..."
SDK_PATH=$(xcrun --show-sdk-path)
swiftc \
    DollarfsDashboardApp.swift \
    DatabaseManager.swift \
    DashboardView.swift \
    OllamaManager.swift \
    LLMView.swift \
    -o "$MACOS_DIR/$APP_NAME" \
    -framework SwiftUI \
    -framework AppKit \
    -framework Foundation \
    -framework CoreData \
    -sdk "$SDK_PATH" \
    -target arm64-apple-macos13.0

# Copy Info.plist
cp Info.plist "$CONTENTS_DIR/"

# Copy icon if exists
if [ -f "AppIcon.icns" ]; then
    cp AppIcon.icns "$RESOURCES_DIR/"
fi

echo "Build complete: $APP_PATH"
echo "To run: open $APP_PATH"
