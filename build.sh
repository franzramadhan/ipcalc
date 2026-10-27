#!/usr/bin/env bash
# Build ipcalc.wasm, a wasi:http component the runwasi wasmtime shim can run.
# Why wasip1 + the wasmtime 36 adapter instead of wasm32-wasip2: see README
# ("WASI version"). Needs rustup and wasm-tools on PATH.
set -euo pipefail
cd "$(dirname "$0")"

ADAPTER_VERSION=v36.0.10
ADAPTER_SHA256=7646182f7b40db5f6beb1516729423e6c88c7f59a1397c19ef496921f2d43a7e
adapter=target/wasi_snapshot_preview1.proxy-$ADAPTER_VERSION.wasm

if [ ! -f "$adapter" ]; then
  mkdir -p target
  curl -fsSL -o "$adapter.tmp" \
    "https://github.com/bytecodealliance/wasmtime/releases/download/$ADAPTER_VERSION/wasi_snapshot_preview1.proxy.wasm"
  echo "$ADAPTER_SHA256  $adapter.tmp" | shasum -a 256 -c - >/dev/null
  mv "$adapter.tmp" "$adapter"
fi

cargo build --release --locked --target wasm32-wasip1
wasm-tools component new target/wasm32-wasip1/release/ipcalc.wasm \
  --adapt "wasi_snapshot_preview1=$adapter" -o ipcalc.wasm

# The shim serves WASI 0.2.6; fail on anything newer.
if wasm-tools component wit ipcalc.wasm | grep -E '@0\.2\.([7-9]|[1-9][0-9])'; then
  echo "ipcalc.wasm imports a WASI version newer than 0.2.6" >&2
  exit 1
fi
ls -l ipcalc.wasm
