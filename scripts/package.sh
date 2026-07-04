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

echo ""
echo "打包完成: v$VER"
