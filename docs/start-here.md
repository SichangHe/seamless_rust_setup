# Use Rust for a scripting task
(authored by agents unless marked 🧑)

Copy [the script template](../experiments/script_template/) for a small command-line task. It uses stable Cargo, has no dependencies, and includes an executable launcher. These are optional starting choices for future agents, not agent policy. The existing [studies](index.md) cover more specialized needs.

## Copy, run, edit

From this repository's root, choose a destination that does not already exist:

```sh
dest=experiments/my_script
mkdir "$dest" && git archive HEAD experiments/script_template | tar -x --strip-components=2 -C "$dest"
"$dest/run" < README.md
"$dest/run" README.md docs/index.md
```

The example counts lines from standard input or named UTF-8 files. Replace `src/main.rs` with the task's logic and rename the package in `Cargo.toml`. Arguments after `run` go to the Rust program. Relative input paths resolve from the caller's directory. Errors return a nonzero exit status. The launcher rebuilds when needed, then runs the program; a warm build still has Cargo startup overhead.

Prerequisites: Linux, a POSIX shell, and stable Rust/Cargo. The shared intermediate directory was tested with Cargo 1.97.1. The launcher defaults to `${XDG_CACHE_HOME:-$HOME/.cache}/cargo/shared-build`; set `CARGO_BUILD_BUILD_DIR` to choose another directory. Final executables stay in each project's `target/` unless your Cargo configuration overrides it. Existing compiler flags, wrappers, and Cargo configuration still apply.

Use Cargo directly for checks:

```sh
cargo fmt --manifest-path "$dest/Cargo.toml" --check
cargo clippy --manifest-path "$dest/Cargo.toml" -- -D warnings
```

The template's own [smoke check](../experiments/script_template/smoke.sh) exercises input, arguments, failures, and invocation from another directory. Add task-specific checks as the program changes. When adding dependencies, use only required features and retain the updated `Cargo.lock`; fetching new dependencies may require registry access. The initial template needs no registry downloads.

Implementation details are in the [template notes](script-template.md).

## Reduce artifacts across projects

The launcher shares intermediates without editing your machine's configuration. For the same behavior in ordinary Cargo invocations, merge this into your existing Cargo home `config.toml` (normally `~/.cargo/config.toml`), replacing the example with an absolute directory owned by your user:

```toml
[build]
build-dir = "<absolute-cache-directory>/cargo/shared-build"
```

Do not add a second `[build]` table if one already exists. Cargo configuration strings do not expand `$HOME` or `~`. Use Cargo 1.97.1 to match the tested setup. To undo the change, remove just `build-dir`; existing cache files remain until removed separately. Avoid clearing a shared directory while builds are running. The cache is not size-bounded by this template.

The [artifact study](artifact-pool.md#local-reuse-works) reports: “The three separate directories used 869 MiB; the shared target used 449 MiB, measured with `du -sk`, including debug artifacts and incremental state.” It also reports: “Sharing only `build.build-dir` also worked: the second project took 0.66 s and compiled one unit; total storage remained 449 MiB with separate final target directories.” These are local workload measurements, not predicted savings for your task. Compatible inputs can reuse dependencies; changed compiler versions, features, and flags can create additional variants. Concurrent builds may wait for the shared directory's lock.

The template keeps incremental compilation and uses line-table debug information for development: source locations remain available, but full debugger variable inspection needs `debug = 2`. Its release profile strips debug information. For small deployed executables, measure `cargo build --release` and dependency feature choices using the [size study](dependency-bloat.md); release builds usually cost more compilation time. Shared intermediates reduce duplication, not the size of every executable. Adding sccache creates another cache; the [tested comparison](artifact-pool.md) used more total storage than sharing Cargo intermediates alone.

## When a script is not enough

- To inspect and edit state inside a running program, start with the [integrated file and task host](agent-dev.md).
- To replace library code while preserving state, see [hot reload](hot-reload.md).
- To tune an application's edit loop, use the [compilation experiments](compile-speed.md).

Those prototypes need integration with the application. Ordinary scripts need only the template. No measured setup makes arbitrary Rust edits compile instantly; the evidence and remaining limitations stay in the linked studies.
