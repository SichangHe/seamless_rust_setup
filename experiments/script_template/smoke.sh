#!/bin/sh
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
export CARGO_BUILD_BUILD_DIR="$work/build"
export CARGO_TARGET_DIR="$work/target"
cd "$work"
printf 'one\ntwo' > 'input file'
test "$(printf 'one\ntwo\n' | "$script_dir/run")" = 2
test "$("$script_dir/run" < /dev/null)" = 0
test "$("$script_dir/run" 'input file')" = "$(printf '2\tinput file')"
if "$script_dir/run" missing > /dev/null 2>&1; then
    echo 'missing input unexpectedly succeeded' >&2
    exit 1
fi
