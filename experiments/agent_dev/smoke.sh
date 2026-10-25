#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
export RUSTFLAGS='-C prefer-dynamic'
timeout 180s cargo build --locked
sysroot=$(rustc --print sysroot)
host=$(rustc -vV | sed -n 's/^host: //p')
export LD_LIBRARY_PATH="$PWD/target/debug/deps:$sysroot/lib/rustlib/$host/lib:${LD_LIBRARY_PATH:-}"
session=$(mktemp -d)
target/debug/agent_dev host "$session" >"$session/host.log" 2>&1 &
host_pid=$!
trap 'kill "$host_pid" 2>/dev/null || true; wait "$host_pid" 2>/dev/null || true; rm -rf "$session"' EXIT
if ! timeout 45s target/debug/agent_dev "${1:-smoke}" "$session"; then
    cat "$session/host.log" >&2
    exit 1
fi
