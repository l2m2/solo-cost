#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

echo "=== solo-cost 打包 ==="
VER=$(node -p "require('./package.json').version")
echo "版本: $VER"

echo ""
echo "--- 清理旧产物 ---"
rm -rf src-tauri/target/release/bundle

echo ""
echo "--- Tauri build (release) ---"
pnpm tauri build

echo ""
echo "--- 产物 ---"
BUNDLE_DIR="src-tauri/target/release/bundle"
if [ -d "$BUNDLE_DIR/macos" ]; then
  echo "macOS:"
  ls -lh "$BUNDLE_DIR/macos"/
fi
if [ -d "$BUNDLE_DIR/dmg" ]; then
  echo "DMG:"
  ls -lh "$BUNDLE_DIR/dmg"/
fi

ARCH=$(uname -m)
if [ "$ARCH" = "x86_64" ]; then
  TAG="x86_64"
elif [ "$ARCH" = "arm64" ]; then
  TAG="arm64"
else
  TAG="$ARCH"
fi

# Rename DMG to include full arch tag
for dmg in "$BUNDLE_DIR/dmg"/*.dmg; do
  if [ -f "$dmg" ]; then
    NEW="${dmg%_x64.dmg}_${TAG}.dmg"
    NEW="${NEW%_arm64.dmg}_${TAG}.dmg"
    if [ "$dmg" != "$NEW" ]; then
      mv "$dmg" "$NEW" 2>/dev/null || true
      echo "DMG: $(ls -lh "$NEW" | awk '{print $5, $NF}')"
    fi
  fi
done

echo ""
echo "打包完成: v$VER ($TAG)"
