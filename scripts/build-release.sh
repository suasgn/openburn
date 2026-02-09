#!/bin/bash
set -e

cd "$(dirname "$0")/.."

if [ -f .env ]; then
  set -a
  source .env
  set +a
fi

if [ -f "$TAURI_SIGNING_PRIVATE_KEY" ]; then
  export TAURI_SIGNING_PRIVATE_KEY="$(cat "$TAURI_SIGNING_PRIVATE_KEY")"
fi

rm -rf src-tauri/target/release/bundle

pnpm tauri build "$@"

echo ""
echo "Build complete. Output artifacts are in src-tauri/target/release/bundle"
