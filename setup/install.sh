#!/bin/sh
# Install the global Rust build settings into Cargo's user config.
# Undo: ./uninstall.sh
set -eu
dir="${CARGO_HOME:-$HOME/.cargo}"
cfg="$dir/config.toml"
begin='# >>> seamless_rust_setup >>>'
end='# <<< seamless_rust_setup <<<'
mkdir -p "$dir"
if [ -f "$cfg" ] && grep -qF "$begin" "$cfg"; then
    echo "already installed in $cfg"; exit 0
fi
for t in '\[build\]' '\[profile\.dev\]' '\[profile\.dev\.package\."\*"\]'; do
    if [ -f "$cfg" ] && grep -q "^[[:space:]]*$t" "$cfg"; then
        echo "error: $cfg already has a $(echo "$t" | tr -d "\\\\") table; merge setup/config.toml by hand" >&2; exit 1
    fi
done
{ [ -s "$cfg" ] && echo; echo "$begin"; cat "$(dirname "$0")/config.toml"; echo "$end"; } >> "$cfg"
echo "installed into $cfg (needs Cargo >= 1.91 for build-dir; older Cargo ignores it with a warning)"
