# 🧑 "Pooling compilation artifacts of public crates globally and reusing them."
"""Validate transferred sccache entries in independent bwrap environments.

Usage: `python3 isolated.py PORTABILITY_WORK_DIR NEW_WORK_DIR` after running
`portability.py`. Requires Linux, bwrap, and the same Cargo/rustc/sccache tools.
Uses only local directories, with no Docker or remote cache service.
"""

import os
import shutil
import sys
from pathlib import Path

from portability import probe, run  # pyright: ignore[reportImplicitRelativeImport]


def worker(case: str, flags: str, expected_hits: int) -> None:
    """Run inside a fresh mount namespace with all test paths under `/tmp/pool`."""
    root = Path("/tmp/pool")
    env = {
        key: value for key, value in os.environ.items()
        if not key.startswith(("CARGO_", "RUST", "SCCACHE_"))
        or key.startswith("RUSTUP_")
    }
    env |= {
        "CARGO_HOME": str(root / "cargo"),
        "CARGO_INCREMENTAL": "0",
        "CARGO_TERM_COLOR": "never",
        "CARGO_ENCODED_RUSTFLAGS": flags,
        "RUSTC_WRAPPER": "sccache",
        "SCCACHE_DIR": str(root / "cache"),
        "SCCACHE_SERVER_UDS": str(root / "server.sock"),
    }
    _ = run(["sccache", "--start-server"], env, root)
    try:
        cmd = ["cargo", "build", "--offline", "--locked", "-p", "memchr", "--target-dir", str(root / case)]
        result = probe(case, cmd, env, root / "project", root)
        if (result.hits, result.misses) != (expected_hits, 1 - expected_hits):
            sys.exit(f"unexpected cache result: {result}")
    finally:
        _ = run(["sccache", "--stop-server"], env, root)


def isolated(root: Path, case: str, flags: str, expected_hits: int) -> None:
    """Start an independent server and mount namespace for each build."""
    cmd = [
        "bwrap", "--ro-bind", "/", "/", "--tmpfs", "/tmp", "--bind", str(root), "/tmp/pool",
        "--ro-bind", str(Path(__file__).resolve().parent), "/tmp/experiment",
        "--unshare-user", "--unshare-pid", "--unshare-net", "--proc", "/proc", "--dev", "/dev",
        "--die-with-parent", "--chdir", "/tmp/pool",
        sys.executable, "/tmp/experiment/isolated.py", "--worker", case, flags, str(expected_hits),
    ]
    print(run(cmd, dict(os.environ), root).strip(), flush=True)


def main() -> None:
    if len(sys.argv) == 5 and sys.argv[1] == "--worker":
        worker(sys.argv[2], sys.argv[3], int(sys.argv[4]))
        return
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    source = Path(sys.argv[1]).resolve()
    work = Path(sys.argv[2]).resolve()
    work.mkdir(parents=True, exist_ok=False)
    producer, consumer = work / "producer", work / "consumer"
    for root in (producer, consumer):
        _ = shutil.copytree(source / "home_a", root / "cargo")
        _ = shutil.copytree(source / "src", root / "project/src")
        for name in ("Cargo.toml", "Cargo.lock"):
            _ = shutil.copy2(source / name, root / "project" / name)
    isolated(producer, "cold", "", 0)
    _ = shutil.copytree(producer / "cache", consumer / "cache")
    isolated(consumer, "transferred", "", 1)
    isolated(consumer, "changed_flags", "-Copt-level=1", 0)
    isolated(consumer, "repeated_flags", "-Copt-level=1", 1)
    crate = next((consumer / "cargo").glob("registry/src/*/memchr-2.8.3/src/lib.rs"))
    original = crate.read_text()
    _ = crate.write_text(original + "\npub const POOL_PROBE: usize = 99;\n")
    isolated(consumer, "changed_source", "", 0)
    isolated(consumer, "repeated_source", "", 1)
    _ = crate.write_text(original)
    isolated(consumer, "restored_source", "", 1)


if __name__ == "__main__":
    main()
