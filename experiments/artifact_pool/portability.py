# 🧑 "Pooling compilation artifacts of public crates globally and reusing them."
"""Probe sccache reuse across registry roots and rustflags on one machine.

Usage: `python3 portability.py NEW_WORK_DIR`. Requires Cargo, rustc, sccache,
and registry access. Keeps sources, outputs, and full logs in `NEW_WORK_DIR`.
"""

import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Result:
    case: str
    hits: int
    misses: int


def run(cmd: list[str], env: dict[str, str], cwd: Path) -> str:
    result = subprocess.run(cmd, env=env, cwd=cwd, capture_output=True, text=True, timeout=60)
    if result.returncode:
        raise RuntimeError(f"{cmd!r} failed:\n{result.stdout}\n{result.stderr}")
    return result.stdout + result.stderr


def probe(case: str, cmd: list[str], env: dict[str, str], cwd: Path, work: Path) -> None:
    _ = run(["sccache", "--zero-stats"], env, work)
    build = run(cmd, env, cwd)
    stats = run(["sccache", "--show-stats"], env, work)
    _ = (work / f"{case}.log").write_text(build + stats)
    counts = {match[1]: int(match[2]) for match in re.finditer(r"^Cache (hits|misses)\s+(\d+)$", stats, re.M)}
    print(Result(case, counts["hits"], counts["misses"]), flush=True)


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    work = Path(sys.argv[1]).resolve()
    work.mkdir(parents=True, exist_ok=False)
    env = {
        key: value for key, value in os.environ.items()
        if not key.startswith(("CARGO_", "RUST", "SCCACHE_"))
        or key.startswith("RUSTUP_")
    }
    env |= {
        "CARGO_INCREMENTAL": "0",
        "CARGO_TERM_COLOR": "never",
        "RUSTC_WRAPPER": "sccache",
        "SCCACHE_DIR": str(work / "cache"),
        "SCCACHE_SERVER_UDS": str(work / "server.sock"),
    }
    _ = (work / "Cargo.toml").write_text('''[package]
name = "portability_probe"
version = "0.1.0"
edition = "2021"
[dependencies]
memchr = "=2.8.3"
''')
    (work / "src").mkdir()
    _ = (work / "src/lib.rs").write_text("pub use memchr::memchr;\n")
    homes = {name: work / name for name in ("home_a", "home_b")}
    env["CARGO_HOME"] = str(homes["home_a"])
    _ = (work / "fetch.log").write_text(run(["cargo", "fetch"], env, work))
    _ = shutil.copytree(homes["home_a"], homes["home_b"])
    sources = {name: next(home.glob("registry/src/*/memchr-2.8.3")) for name, home in homes.items()}
    remap = [f"--remap-path-prefix={source}=registry/memchr-2.8.3" for source in sources.values()]
    _ = (work / "versions.log").write_text(run(["rustc", "-vV"], env, work) + run(["sccache", "--version"], env, work))
    _ = run(["sccache", "--start-server"], env, work)
    try:
        cases: list[tuple[str, str, list[str]]] = [
            ("cold", "home_a", []),
            ("same_root", "home_a", []),
            ("other_root", "home_b", []),
            ("other_root_repeat", "home_b", []),
            ("changed_flags", "home_a", ["-Copt-level=1"]),
            ("changed_flags_repeat", "home_a", ["-Copt-level=1"]),
            ("original_flags_again", "home_a", []),
            ("remap_a", "home_a", remap),
            ("remap_b", "home_b", remap),
        ]
        for case, home, flags in cases:
            cargo_env = env | {"CARGO_HOME": str(homes[home]), "CARGO_ENCODED_RUSTFLAGS": "\x1f".join(flags)}
            cmd = ["cargo", "build", "--offline", "--locked", "-p", "memchr", "--target-dir", str(work / case)]
            probe(case, cmd, cargo_env, work, work)
        direct_env = {key: value for key, value in env.items() if not key.startswith("CARGO_")}
        for case, home, flags in cases[:4] + cases[-2:]:
            case = f"direct_{case}"
            out = work / case
            out.mkdir()
            cmd = [
                "sccache", "rustc", "--crate-name", "memchr", "--crate-type", "rlib",
                "--edition=2021", "--emit=link,dep-info", "--out-dir", str(out),
                "--cfg", 'feature="std"', "--cfg", 'feature="alloc"', *flags, "src/lib.rs",
            ]
            probe(case, cmd, direct_env, sources[home], work)
    finally:
        _ = run(["sccache", "--stop-server"], env, work)


if __name__ == "__main__":
    main()
