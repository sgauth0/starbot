#!/usr/bin/env bash
set -euo pipefail

TARGET=/var/www/sites/stella/starbot.cloud
BUILD_DIR=.next
STANDALONE_DIR=$BUILD_DIR/standalone

npm install
npm run build

mkdir -p "$TARGET"
rm -rf "$TARGET"/*

rsync -a "$STANDALONE_DIR"/ "$TARGET/"
rsync -a "$BUILD_DIR/static"/ "$TARGET/.next/static/"
rsync -a public/ "$TARGET/public/"
cp -a package.json "$TARGET/package.json"
if [ -f package-lock.json ]; then
  cp package-lock.json "$TARGET/package-lock.json"
fi
