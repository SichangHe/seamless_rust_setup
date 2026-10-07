#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/hotlib"
export RUSTFLAGS='-C prefer-dynamic'
timeout 180s cargo build --locked
sysroot=$(rustc --print sysroot)
host=$(rustc -vV | sed -n 's/^host: //p')
export LD_LIBRARY_PATH="$PWD/target/debug/deps:$sysroot/lib/rustlib/$host/lib:${LD_LIBRARY_PATH:-}"
work=$(mktemp -d)
cp lib/src/lib.rs "$work/original.rs"
target/debug/runner >"$work/runner.log" 2>&1 &
runner_pid=$!
cleanup() {
    kill "$runner_pid" 2>/dev/null || true
    cp "$work/original.rs" lib/src/lib.rs
    timeout 180s cargo build --locked -p reloadable >/dev/null 2>&1
    rm -rf "$work"
}
trap cleanup EXIT
sleep 1
if ! grep -q 'v1: count=' "$work/runner.log"; then
    cat "$work/runner.log" >&2
    exit 1
fi
sed 's/v1: count=/v2: count=/' "$work/original.rs" > lib/src/lib.rs
start_ms=$(date +%s%3N)
timeout 180s cargo build --locked -p reloadable
for _ in {1..100}; do
    if grep -q 'v2: count=' "$work/runner.log"; then
        end_ms=$(date +%s%3N)
        echo "build_and_observe_ms=$((end_ms - start_ms))"
        cat "$work/runner.log"
        awk '/v1: count=/{split($0,a,"="); before=a[2]} /v2: count=/{split($0,a,"="); if(a[2] > before) ok=1} END{exit !ok}' "$work/runner.log"
        exit 0
    fi
    sleep .05
done
cat "$work/runner.log" >&2
echo 'new code did not appear in running process' >&2
exit 1
