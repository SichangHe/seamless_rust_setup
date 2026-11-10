#!/bin/sh
# Remove what install.sh added; leaves the rest of the config untouched.
set -eu
cfg="${CARGO_HOME:-$HOME/.cargo}/config.toml"
[ -f "$cfg" ] && grep -qF '# >>> seamless_rust_setup >>>' "$cfg" || { echo "not installed"; exit 0; }
tmp="$cfg.tmp.$$"
sed '/^# >>> seamless_rust_setup >>>$/,/^# <<< seamless_rust_setup <<<$/d' "$cfg" > "$tmp"
if grep -q '[^[:space:]]' "$tmp"; then mv "$tmp" "$cfg"; else rm -f "$tmp" "$cfg"; fi
echo "removed from $cfg. build caches remain in ${CARGO_HOME:-$HOME/.cargo}/build/; delete it to reclaim space"
