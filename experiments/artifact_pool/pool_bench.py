# 🧑 "Pooling compilation artifacts of public crates globally and reusing them."
"""Measure build time and disk use of 3 projects sharing public crates under each pooling setup.

Usage: `python3 pool_bench.py WORK_DIR [SETUP_NAME ...]`. Prints one line per measurement.
`WORK_DIR` receives generated projects and all build outputs; delete it afterwards.
"""

import os
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

MANIFEST_FULL = """\
[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
clap = {{ version = "4", features = ["derive"] }}
regex = "1"
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
tokio = {{ version = "1", features = ["full"] }}
"""
# Same crates as `MANIFEST_FULL` but fewer `tokio` features, plus `rand`.
MANIFEST_SLIM = """\
[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
clap = {{ version = "4", features = ["derive"] }}
rand = "0.9"
regex = "1"
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
tokio = {{ version = "1", features = ["rt", "macros"] }}
"""
MAIN = """\
use clap::Parser;
#[derive(Parser, serde::Serialize)]
struct Args {
    #[arg(long, default_value = "a+")]
    pattern: String,
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    regex::Regex::new(&args.pattern)?;
    println!("{}", serde_json::to_string(&args)?);
    Ok(())
}
"""
PROJECTS = {"p1": MANIFEST_FULL, "p2": MANIFEST_FULL, "p3": MANIFEST_SLIM}
SCCACHE_PORT = "14227"


@dataclass(frozen=True)
class Setup:
    name: str
    toolchain: str = "stable"
    shared_target: bool = False
    shared_build: bool = False
    sccache: bool = False
    z_flags: tuple[str, ...] = ()


SETUPS = [
    Setup("separate"),
    Setup("shared_target_dir", shared_target=True),
    Setup("shared_build_dir", shared_build=True),
    Setup("sccache", sccache=True),
    Setup("nightly_separate", toolchain="nightly"),
    Setup("nightly_shared_build_dir", toolchain="nightly", shared_build=True),
    Setup(
        "nightly_shared_build_dir_fine_lock",
        toolchain="nightly",
        shared_build=True,
        z_flags=("-Zbuild-dir-new-layout", "-Zfine-grain-locking"),
    ),
]


@dataclass(frozen=True)
class Built:
    setup: str
    phase: str
    project: str
    wall_s: float
    n_compiled: int
    n_lock_waits: int


@dataclass(frozen=True)
class Disk:
    setup: str
    phase: str
    total_mb: int


def build_env(setup: Setup, out: Path, project: str) -> dict[str, str]:
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(out / ("target" if setup.shared_target else f"target_{project}"))
    if setup.shared_build:
        env["CARGO_BUILD_BUILD_DIR"] = str(out / "build")
    if setup.sccache:
        env |= {
            "RUSTC_WRAPPER": "sccache",
            "SCCACHE_DIR": str(out / "sccache"),
            "SCCACHE_SERVER_PORT": SCCACHE_PORT,
        }
    return env


def spawn(setup: Setup, work: Path, out: Path, project: str) -> subprocess.Popen[str]:
    cmd = ["cargo", f"+{setup.toolchain}", "build", "--offline", *setup.z_flags]
    return subprocess.Popen(
        cmd,
        cwd=work / project,
        env=build_env(setup, out, project),
        stderr=subprocess.PIPE,
        stdout=subprocess.DEVNULL,
        text=True,
    )


def finish(setup: Setup, phase: str, project: str, proc: subprocess.Popen[str], t0: float) -> Built:
    try:
        _, err = proc.communicate(timeout=120)
    except subprocess.TimeoutExpired:
        proc.kill()
        _, err = proc.communicate()
        sys.exit(f"{setup.name} {project} exceeded 120 seconds:\n{err[-2000:]}")
    wall_s = time.perf_counter() - t0
    if proc.returncode != 0:
        sys.exit(f"{setup.name} {project} failed:\n{err}")
    lines = [line.strip() for line in err.splitlines()]
    return Built(
        setup.name,
        phase,
        project,
        round(wall_s, 2),
        sum(line.startswith("Compiling") for line in lines),
        sum(line.startswith("Blocking") for line in lines),
    )


def disk(setup: Setup, phase: str, out: Path) -> Disk:
    kb = subprocess.run(["du", "-sk", out], capture_output=True, text=True, check=True).stdout
    return Disk(setup.name, phase, int(kb.split()[0]) // 1024)


def sccache_server(setup: Setup, out: Path, action: str) -> None:
    _ = subprocess.run(
        ["sccache", action], env=build_env(setup, out, "p1"), capture_output=True, check=False, timeout=10
    )


def measure(setup: Setup, work: Path) -> None:
    out = work / "out" / setup.name
    for phase in ("sequential", "concurrent"):
        shutil.rmtree(out, ignore_errors=True)
        out.mkdir(parents=True)
        if setup.sccache:
            sccache_server(setup, out, "--stop-server")
            sccache_server(setup, out, "--start-server")
        if phase == "sequential":
            # `p1` again at the end checks that building `p2`, `p3` did not invalidate it.
            for project in (*PROJECTS, "p1"):
                t0 = time.perf_counter()
                print(finish(setup, phase, project, spawn(setup, work, out, project), t0))
        else:
            t0 = time.perf_counter()
            procs = {project: spawn(setup, work, out, project) for project in PROJECTS}
            with ThreadPoolExecutor(max_workers=len(procs)) as executor:
                results = [
                    executor.submit(finish, setup, phase, project, proc, t0)
                    for project, proc in procs.items()
                ]
                for result in results:
                    print(result.result())
        if setup.sccache:
            stats = subprocess.run(
                ["sccache", "--show-stats"], env=build_env(setup, out, "p1"),
                capture_output=True, text=True, timeout=10,
            )
            print(stats.stdout, flush=True)
            sccache_server(setup, out, "--stop-server")
        print(disk(setup, phase, out), flush=True)


def generate(work: Path) -> None:
    """Write the projects with one shared lock file so that crate versions match."""
    for name, manifest in PROJECTS.items():
        src = work / name / "src"
        src.mkdir(parents=True, exist_ok=True)
        _ = (work / name / "Cargo.toml").write_text(manifest.format(name=name))
        _ = (src / "main.rs").write_text(MAIN)
        lock = work / "p1" / "Cargo.lock"
        if name != "p1":
            _ = shutil.copy(lock, work / name / "Cargo.lock")
        _ = subprocess.run(["cargo", "fetch"], cwd=work / name, check=True, capture_output=True, timeout=60)


def main() -> None:
    work = Path(sys.argv[1]).resolve()
    names = sys.argv[2:]
    generate(work)
    for setup in SETUPS:
        if not names or setup.name in names:
            measure(setup, work)


if __name__ == "__main__":
    main()
