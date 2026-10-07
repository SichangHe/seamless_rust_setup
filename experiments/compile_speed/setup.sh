#!/usr/bin/env bash
# Fetch linkers into `tools/` and workloads into `work/`; idempotent.
set -euo pipefail
cd "$(dirname "$0")"
MOLD=3.0.0 WILD=0.10.0 RIPGREP=15.1.0
mkdir -p tools work
[ -d tools/mold ] || {
    curl -sL "https://github.com/rui314/mold/releases/download/v$MOLD/mold-$MOLD-x86_64-linux.tar.gz" | tar xz -C tools
    mv "tools/mold-$MOLD-x86_64-linux" tools/mold
}
# gcc finds the linker as `ld` in a `-B` directory; mold ships such a directory, wild does not.
[ -d tools/wild ] || {
    curl -sL "https://github.com/wild-linker/wild/releases/download/$WILD/wild-linker-$WILD-x86_64-unknown-linux-gnu.tar.gz" | tar xz -C tools
    mv "tools/wild-linker-$WILD-x86_64-unknown-linux-gnu" tools/wild
    ln -s wild tools/wild/ld
}
rustup toolchain install nightly --component rustc-codegen-cranelift
[ -d work/ripgrep ] || {
    git clone -q --depth 1 --branch "$RIPGREP" https://github.com/BurntSushi/ripgrep work/ripgrep
    # Edit markers `black_box(<n>u64)` for `bench.py`, each inside a function that runs.
    sed -i '0,/^fn main() -> ExitCode {$/s//&\n    std::hint::black_box(0u64);/' work/ripgrep/crates/core/main.rs
    sed -i '0,/^    pub fn new(glob: &str) -> Result<Glob, Error> {$/s//&\n        std::hint::black_box(0u64);/' work/ripgrep/crates/globset/src/glob.rs
    grep -q 'black_box(0u64)' work/ripgrep/crates/core/main.rs
    grep -q 'black_box(0u64)' work/ripgrep/crates/globset/src/glob.rs
}
rm -rf work/webapp
cp -r webapp work/webapp
(cd work/webapp && cargo fetch -q)
(cd work/ripgrep && cargo fetch -q)
