#!/usr/bin/env bash
set -euo pipefail
if [[ $# -gt 1 || ( $# -eq 1 && $1 != --build-only ) ]]; then
    echo "usage: bash $0 [--build-only]" >&2
    exit 2
fi
cd "$(dirname "$0")/hotlib"
export RUSTFLAGS='-C prefer-dynamic'
export CARGO_TARGET_DIR="$PWD/target"
# 🧑 "code hot reloading ... loading and running dylib or something at run time to dynamically run code"
if [[ ${1:-} == --build-only ]]; then
    timeout 180s cargo build --locked -p reloadable
    exit
fi
timeout 180s cargo build --locked
sysroot=$(rustc --print sysroot)
host=$(rustc -vV | sed -n 's/^host: //p')
export LD_LIBRARY_PATH="$PWD/target/debug/deps:$sysroot/lib/rustlib/$host/lib:${LD_LIBRARY_PATH:-}"
exec target/debug/runner
