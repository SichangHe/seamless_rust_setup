#!/bin/sh
# Install the global Rust build settings into Cargo's user config, plus the wild linker for dev builds (Linux x86_64/aarch64).
# Re-running upgrades. Undo: ./uninstall.sh
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
dir="${CARGO_HOME:-$HOME/.cargo}"
cfg="$dir/config.toml"
home="$dir/seamless_rust_setup"
begin='# >>> seamless_rust_setup >>>'
end='# <<< seamless_rust_setup <<<'
wild_version=0.10.0
mkdir -p "$dir"
if [ -f "$cfg" ] && grep -qF "$begin" "$cfg"; then "$here/uninstall.sh" >/dev/null; fi
for t in '\[build\]' '\[profile\.dev\]' '\[profile\.dev\.package\."\*"\]'; do
    if [ -f "$cfg" ] && grep -q "^[[:space:]]*$t" "$cfg"; then
        echo "error: $cfg already has a $(echo "$t" | tr -d "\\\\") table; merge setup/config.toml by hand" >&2; exit 1
    fi
done

# wild: pinned release, checksum-verified; reuse a matching `wild` on PATH. Any failure here only skips wild.
install_wild() {
    [ "$(uname -s)" = Linux ] || { echo "wild: skipped, not Linux" >&2; return 1; }
    arch="$(uname -m)"
    case "$arch" in x86_64|aarch64) ;; *) echo "wild: skipped, unsupported CPU $arch" >&2; return 1 ;; esac
    triple="$arch-unknown-linux-gnu"
    if grep -q "^[[:space:]]*\[target\.$triple\]" "$cfg" 2>/dev/null; then
        echo "wild: skipped, $cfg already has [target.$triple]; set \`linker = \"$home/linker\"\` by hand" >&2; return 1
    fi
    mkdir -p "$home/bin"
    found="$(command -v wild 2>/dev/null || true)"
    if [ -n "$found" ] && "$found" --version 2>/dev/null | grep -q "^Wild $wild_version "; then
        cp "$found" "$home/bin/ld"
    else
        tar="wild-linker-$wild_version-$triple.tar.gz"
        tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
        url="https://github.com/davidlattimore/wild/releases/download/$wild_version/$tar"
        { curl -fsSL "$url" -o "$tmp/$tar" || wget -q "$url" -O "$tmp/$tar"; } 2>/dev/null || { echo "wild: skipped, download failed ($url)" >&2; return 1; }
        want="$(grep " $tar\$" "$here/wild.sha256" | cut -d' ' -f1)"
        got="$(sha256sum "$tmp/$tar" | cut -d' ' -f1)"
        [ -n "$want" ] && [ "$want" = "$got" ] || { echo "wild: skipped, checksum mismatch for $tar" >&2; return 1; }
        tar -xzf "$tmp/$tar" -C "$tmp"
        cp "$tmp/wild-linker-$wild_version-$triple/wild" "$home/bin/ld"
    fi
    chmod +x "$home/bin/ld"
    "$home/bin/ld" --version >/dev/null 2>&1 || { echo "wild: skipped, binary does not run" >&2; return 1; }
    cp "$here/linker" "$home/linker"; chmod +x "$home/linker"
    printf '\n[target.%s]\n# dev builds link with wild; release and any wild failure use the default linker\nlinker = "%s/linker"\n' "$triple" "$home"
}

wild_cfg="$(install_wild)" || wild_cfg=""
{ [ -s "$cfg" ] && echo; echo "$begin"; cat "$here/config.toml"; [ -z "$wild_cfg" ] || printf '%s\n' "$wild_cfg"; echo "$end"; } >> "$cfg"
echo "installed into $cfg (needs Cargo >= 1.91 for build-dir; older Cargo ignores it with a warning)"
