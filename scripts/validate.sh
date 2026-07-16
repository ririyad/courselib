#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "==> Running Rust tests"
(
  cd src-tauri
  cargo test
)

echo "==> Building frontend"
npm run build

echo "==> Building Tauri debug app"
npm run tauri -- build --debug

echo "==> Validation complete"
