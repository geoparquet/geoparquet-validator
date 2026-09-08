#!/usr/bin/env sh
# Build the npm package in npm/: the WebAssembly module for bundlers (browsers) and for Node.
# Needs the wasm32-unknown-unknown target and the wasm-bindgen-cli version in Cargo.lock.
set -e
cd "$(dirname "$0")/.."
cargo build --lib --release --target wasm32-unknown-unknown --no-default-features --features wasm
WASM=target/wasm32-unknown-unknown/release/geoparquet_validator.wasm
rm -rf npm/bundler npm/node
wasm-bindgen --target bundler --out-dir npm/bundler --out-name geoparquet_validator "$WASM"
wasm-bindgen --target nodejs  --out-dir npm/node    --out-name geoparquet_validator "$WASM"
# wasm-bindgen's Node output is CommonJS; the package itself is "type": "module"
printf '{ "type": "commonjs" }\n' > npm/node/package.json
if command -v wasm-opt >/dev/null; then
  for f in npm/bundler/geoparquet_validator_bg.wasm npm/node/geoparquet_validator_bg.wasm; do
    wasm-opt -O2 --enable-bulk-memory --enable-nontrapping-float-to-int "$f" -o "$f" 2>/dev/null || true
  done
fi
VERSION=$(grep '^version' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
sed -i.bak "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" npm/package.json && rm -f npm/package.json.bak
cp LICENSE npm/
ls -la npm/bundler npm/node | grep -E "wasm|\.js"
