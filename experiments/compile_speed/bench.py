#!/usr/bin/env python3
# 🧑 "I want instant compilation after initial cold start"
"""Time rebuild-after-edit for each workload under each build config.

Usage: `./bench.py [CONFIG...] > results/NAME.tsv`; no argument runs every config.
Run `./setup.sh` first. Each (workload, config) gets its own target directory,
so configs never share or invalidate artifacts.
"""

import re
import statistics
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORK = HERE / "work"
N_TIMED_EDITS = 5
N_TIMED_RUNS = 10
NO_LLD = "-Clinker-features=-lld"
MOLD = [NO_LLD, f"-Clink-arg=-B{HERE}/tools/mold/libexec/mold"]
WILD = [NO_LLD, f"-Clink-arg=-B{HERE}/tools/wild"]
LINE_TABLES = ['profile.dev.debug="line-tables-only"']


@dataclass(frozen=True)
class Config:
    """One way to invoke cargo. `dylib` puts all third-party crates in one shared library."""

    toolchain: str = "stable"
    rustflags: list[str] = field(default_factory=list)
    cargo_config: list[str] = field(default_factory=list)
    command: str = "build"
    dylib: bool = False
    env: dict[str, str] = field(default_factory=dict)


CONFIGS = {
    "default": Config(),
    "check": Config(command="check"),
    "no_incremental": Config(env={"CARGO_INCREMENTAL": "0"}),
    "ld_bfd": Config(rustflags=[NO_LLD]),
    "mold": Config(rustflags=MOLD),
    "wild": Config(rustflags=WILD),
    "debug_none": Config(cargo_config=["profile.dev.debug=0"]),
    "debug_line_tables": Config(cargo_config=LINE_TABLES),
    "split_debuginfo": Config(cargo_config=['profile.dev.split-debuginfo="unpacked"']),
    "dyn_std": Config(rustflags=["-Cprefer-dynamic"]),
    "dylib": Config(dylib=True),
    "nightly": Config(toolchain="nightly"),
    "threads8": Config(toolchain="nightly", rustflags=["-Zthreads=8"]),
    "cranelift": Config(toolchain="nightly", rustflags=["-Zcodegen-backend=cranelift"]),
    "stable_combo": Config(rustflags=WILD, cargo_config=LINE_TABLES, dylib=True),
    "stable_combo_mold": Config(rustflags=MOLD, cargo_config=LINE_TABLES, dylib=True),
    "nightly_combo": Config(
        toolchain="nightly",
        rustflags=[*WILD, "-Zcodegen-backend=cranelift", "-Zthreads=8"],
        cargo_config=LINE_TABLES,
        dylib=True,
    ),
    "nightly_combo_mold": Config(
        toolchain="nightly",
        rustflags=[*MOLD, "-Zcodegen-backend=cranelift", "-Zthreads=8"],
        cargo_config=LINE_TABLES,
        dylib=True,
    ),
}


@dataclass(frozen=True)
class Workload:
    """A cargo project, the file `bench.py` edits in it, and its binary's quick-exit args."""

    name: str
    project: str
    edit_file: str
    binary: str
    run_args: list[str]
    has_deps_dylib: bool = False


WORKLOADS = [
    Workload("webapp", "webapp", "app/src/main.rs", "app", ["--once"], has_deps_dylib=True),
    Workload("rg_top", "ripgrep", "crates/core/main.rs", "rg", ["--version"]),
    Workload("rg_deep", "ripgrep", "crates/globset/src/glob.rs", "rg", ["--version"]),
]


def edit(path: Path) -> None:
    """Change the body of a function that runs, by bumping its marker number."""
    text, n_subs = re.subn(
        r"black_box\((\d+)u64\)", lambda m: f"black_box({int(m[1]) + 1}u64)", path.read_text()
    )
    assert n_subs == 1, path
    _ = path.write_text(text)


def timed_ms(cmd: list[str], cwd: Path, env: dict[str, str]) -> float | None:
    """Wall time of `cmd`, or `None` if it failed (stderr tail goes to our stderr)."""
    start = time.perf_counter()
    done = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True, timeout=120)
    elapsed_ms = (time.perf_counter() - start) * 1e3
    if done.returncode != 0:
        print(f"FAILED {cmd}\n{done.stderr[-2000:]}", file=sys.stderr)
        return None
    return elapsed_ms


def bench(workload: Workload, name: str, config: Config, base_env: dict[str, str]) -> str:
    """One TSV row; times are `fail` if any step failed."""
    project = WORK / workload.project
    target = WORK / "target" / workload.name / name
    dylib = config.dylib and workload.has_deps_dylib
    rustflags = config.rustflags + (["-Cprefer-dynamic"] if dylib else [])
    cargo = ["cargo", f"+{config.toolchain}", config.command]
    cargo += ["--features", "dylib"] if dylib else []
    for item in [f"build.rustflags={rustflags!r}".replace("'", '"'), *config.cargo_config]:
        cargo += ["--config", item]
    target_libdir = subprocess.check_output(
        ["rustc", f"+{config.toolchain}", "--print", "target-libdir"], text=True, timeout=10
    ).strip()
    env = base_env | config.env | {"CARGO_TARGET_DIR": str(target)}
    env["LD_LIBRARY_PATH"] = f"{target}/debug/deps:{target_libdir}:{base_env.get('LD_LIBRARY_PATH', '')}"
    _ = subprocess.run(["rm", "-rf", target], check=True, timeout=30)
    cold_ms = timed_ms(cargo, project, env)
    noop_ms = timed_ms(cargo, project, env)
    rebuilds_ms: list[float | None] = []
    for _ in range(1 + N_TIMED_EDITS):
        edit(project / workload.edit_file)
        rebuilds_ms.append(timed_ms(cargo, project, env))
    binary = target / "debug" / workload.binary
    runs_ms = (
        [timed_ms([str(binary), *workload.run_args], project, env) for _ in range(N_TIMED_RUNS)]
        if config.command == "build"
        else []
    )
    cells = [workload.name, name]
    if cold_ms is None or noop_ms is None or None in [*rebuilds_ms, *runs_ms]:
        return "\t".join([*cells, "fail"])
    timed = [ms for ms in rebuilds_ms[1:] if ms is not None]
    cells += [f"{cold_ms / 1e3:.1f}", f"{noop_ms:.0f}"]
    cells += [f"{statistics.median(timed):.0f}", f"{min(timed):.0f}"]
    if runs_ms:
        cells += [
            f"{statistics.median(ms for ms in runs_ms if ms is not None):.1f}",
            f"{binary.stat().st_size / 1e6:.1f}",
        ]
    return "\t".join(cells)


def main() -> None:
    import os

    names = sys.argv[1:] or list(CONFIGS)
    print("workload\tconfig\tcold_s\tnoop_ms\trebuild_median_ms\trebuild_min_ms\trun_ms\tbin_mb")
    for name in names:
        for workload in WORKLOADS:
            if CONFIGS[name].dylib and not workload.has_deps_dylib and name == "dylib":
                continue
            print(bench(workload, name, CONFIGS[name], dict(os.environ)), flush=True)


if __name__ == "__main__":
    main()
