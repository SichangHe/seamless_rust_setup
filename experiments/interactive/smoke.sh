#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
export RUSTFLAGS='-C prefer-dynamic'
timeout 180s cargo build --locked
sysroot=$(rustc --print sysroot)
host=$(rustc -vV | sed -n 's/^host: //p')
export LD_LIBRARY_PATH="$PWD/target/debug/deps:$sysroot/lib/rustlib/$host/lib:${LD_LIBRARY_PATH:-}"
session=$(mktemp -d)
target/debug/app "$session" >"$session/host.log" 2>&1 &
host_pid=$!
trap 'kill "$host_pid" 2>/dev/null || true; rm -rf "$session"' EXIT
for _ in {1..100}; do
    test -d "$session/req" && break
    sleep .05
done
eval_snippet() { PRY_TIMING=1 timeout 35s bash bin/pry-eval "$session" "$1"; }
eval_snippet snippets/inspect.rs | tee "$session/inspect.out"
grep -q '3 orders, revenue 6249' "$session/inspect.out"
eval_snippet snippets/mutate.rs | tee "$session/mutate.out"
grep -q 'now 4 orders, revenue 6250' "$session/mutate.out"
eval_snippet snippets/inspect.rs | tee "$session/inspect2.out"
grep -q '4 orders, revenue 6250' "$session/inspect2.out"
submit_reused_name() {
    rm -f "$session/res/reused.status"
    cp "$1" "$session/req/reused.tmp"
    mv "$session/req/reused.tmp" "$session/req/reused.rs"
    for _ in {1..200}; do
        if test -f "$session/res/reused.status"; then
            grep -q '^ok$' "$session/res/reused.status"
            return
        fi
        sleep .05
    done
    echo 'reused filename request timed out' >&2
    return 1
}
submit_reused_name snippets/inspect.rs
grep -q '4 orders, revenue 6250' "$session/res/reused.out"
submit_reused_name snippets/mutate.rs
grep -q 'now 5 orders, revenue 6251' "$session/res/reused.out"
if eval_snippet snippets/wrong_type.rs >"$session/wrong.out" 2>&1; then
    echo 'wrong context type unexpectedly succeeded' >&2; exit 1
fi
grep -q 'type_mismatch' "$session/wrong.out"
if eval_snippet snippets/compiler_error.txt >"$session/compile.out" 2>&1; then
    echo 'invalid snippet unexpectedly succeeded' >&2; exit 1
fi
grep -q 'compile_error' "$session/compile.out"
if eval_snippet snippets/panic.rs >"$session/panic.out" 2>&1; then
    echo 'panic snippet unexpectedly succeeded' >&2; exit 1
fi
grep -q '# pry: panic' "$session/panic.out"
mkdir "$session/absent-host"
timeout_status=0
PRY_TIMEOUT_SECONDS=1 bash bin/pry-eval "$session/absent-host" snippets/inspect.rs >"$session/timeout.out" 2>&1 || timeout_status=$?
test "$timeout_status" -eq 124
grep -q 'request may still execute' "$session/timeout.out"
eval_snippet snippets/inspect.rs >/dev/null
bash bin/pry-eval "$session" --continue
for _ in {1..100}; do
    grep -q 'step 1: revenue 6251 cents' "$session/host.log" && exit 0
    sleep .05
done
echo 'host did not resume with modified state' >&2
exit 1
