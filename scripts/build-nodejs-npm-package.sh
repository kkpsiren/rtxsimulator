#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PKG_ARG="${1:-crates/wasm/pkg}"

if [[ "$PKG_ARG" = /* ]]; then
  PKG_DIR="$PKG_ARG"
else
  PKG_DIR="$ROOT_DIR/$PKG_ARG"
fi

cd "$ROOT_DIR"

rm -rf "$PKG_DIR"
wasm-pack build crates/wasm --target nodejs --out-dir "$PKG_DIR" --release
node scripts/prepare-nodejs-npm-package.mjs "$PKG_DIR"
node scripts/check-nodejs-npm-package.mjs "$PKG_DIR"
