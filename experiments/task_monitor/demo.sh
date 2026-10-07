#!/usr/bin/env bash
# Run the demo program, then `top` and `kill` its tasks from outside.
# Prints the transcript; exits non-zero if a check fails.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --quiet --example demo --bin taskmon
socket_dir=$(mktemp -d "${TMPDIR:-/tmp}/taskmon.XXXXXX")
socket="$socket_dir/monitor.sock"
target/release/examples/demo "$socket" &
demo_pid=$!
trap 'kill -9 "$demo_pid" 2>/dev/null || true; wait "$demo_pid" 2>/dev/null || true; rm -rf "$socket_dir"' EXIT
mon() {
    echo "\$ taskmon SOCKET $*" >&2
    local reply
    reply=$(target/release/taskmon "$socket" "$@")
    printf '%s\n' "$reply" >&2
    printf '%s\n' "$reply"
}
id_of() { awk -v name="$1" '$7 == name { print $1 }'; }
for _ in {1..100}; do
    [[ -S "$socket" ]] && break
    sleep 0.02
done
[[ -S "$socket" ]] || { echo "FAIL: monitor did not start" >&2; exit 1; }
sleep 2
before=$(mon top 1000)
mon kill "$(id_of leaked <<<"$before")" >/dev/null
mon kill "$(id_of counter-actor <<<"$before")" >/dev/null
mon kill "$(id_of never-yields <<<"$before")" >/dev/null
mon kill 999 >/dev/null
after=$(mon top 1000)
fail() { echo "FAIL: $*" >&2; exit 1; }
[[ -z $(id_of leaked <<<"$after") ]] || fail "leaked survived kill"
[[ -z $(id_of counter-actor <<<"$after") ]] || fail "counter-actor survived kill"
[[ -n $(id_of never-yields <<<"$after") ]] || fail "never-yields died, expected it to survive"
