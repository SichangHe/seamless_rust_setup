#!/usr/bin/env python3
"""Build generated cases in isolated targets; save size observations."""
import csv
import os
from pathlib import Path
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent


def main() -> None:
    names = sys.argv[1:]
    if not names:
        raise SystemExit("Provide one or more case names")
    _ = subprocess.run([sys.executable, str(HERE / "gen_cases.py"), *names], check=True)
    failed = False
    for name in names:
        case = HERE / "cases" / name
        target = case / "target"
        if target.exists():
            raise SystemExit(f"Remove {target} before measuring a cold build")
        observation = case / "measurement.csv"
        observation.unlink(missing_ok=True)
        env = os.environ | {"CARGO_TARGET_DIR": str(target), "CARGO_BUILD_JOBS": "2"}
        log = case / "build.log"
        start = time.monotonic()
        with log.open("w") as out:
            result = subprocess.run(
                ["cargo", "build", "--profile", "min", "--offline"],
                cwd=case, env=env, stdout=out, stderr=out, timeout=300,
            )
        elapsed_s = time.monotonic() - start
        if result.returncode:
            print(f"FAIL {name}: {log}", flush=True)
            failed = True
            continue
        binary = target / "min" / f"case_{name}"
        _ = subprocess.run([str(binary)], check=True, stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL, timeout=15)
        target_bytes = sum(p.stat().st_size for p in target.rglob("*") if p.is_file())
        row = (name, round(elapsed_s, 3), binary.stat().st_size, target_bytes)
        with observation.open("w") as out:
            writer = csv.writer(out)
            writer.writerow(("case", "build_s", "binary_bytes", "target_logical_bytes"))
            writer.writerow(row)
        print(row, flush=True)
    if failed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
