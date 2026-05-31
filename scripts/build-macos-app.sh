#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="$ROOT/target/aarch64-apple-darwin/release"
APP_DIR="$TARGET_DIR/ClaudeMeter.app"
CONTENTS="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"

cargo build --release --target aarch64-apple-darwin

rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR" "$RESOURCES"

cp "$TARGET_DIR/claudemeter" "$RESOURCES/claudemeter-agent"
chmod +x "$RESOURCES/claudemeter-agent"

swiftc \
  -O \
  -framework AppKit \
  -framework Foundation \
  -framework UniformTypeIdentifiers \
  "$ROOT/macos/ClaudeMeterApp.swift" \
  -o "$MACOS_DIR/ClaudeMeter"

cat > "$CONTENTS/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>ClaudeMeter</string>
  <key>CFBundleIdentifier</key>
  <string>com.klivak.claudemeter</string>
  <key>CFBundleName</key>
  <string>ClaudeMeter</string>
  <key>CFBundleDisplayName</key>
  <string>ClaudeMeter</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>4.0.0</string>
  <key>CFBundleVersion</key>
  <string>4.0.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

if command -v codesign >/dev/null 2>&1; then
  IDENTITY="${MACOS_CODESIGN_IDENTITY:--}"
  codesign --force --deep --sign "$IDENTITY" "$APP_DIR"
fi

ditto -c -k --keepParent "$APP_DIR" "$TARGET_DIR/ClaudeMeter-macos-arm64.app.zip"
cp "$TARGET_DIR/claudemeter" "$TARGET_DIR/claudemeter-macos-arm64"
