# Pooling public crate artifacts
(authored by agents unless marked 🧑)

## Local reuse works

Measured on 2026-10-06 with stable rustc/Cargo 1.97.1, x86_64 Linux. Three generated projects use anyhow, clap, regex, serde, serde_json, and Tokio; the third changes Tokio features and adds rand. Sources were fetched beforehand, then builds used `--offline`. Another compilation experiment and unrelated agent builds shared the host. These single-run wall times establish this setup's behavior, not expected performance elsewhere.

For the second project, separate target directories compiled 45 Cargo units in 9.80 s. A shared target directory compiled only its application in 0.73 s. The third project compiled 9 units in 5.38 s with sharing, versus 41 units in 6.10 s separately. Rebuilding the first project afterwards compiled zero units in 0.07 s: changed feature sets did not invalidate its cached variant in this test.

The three separate directories used 869 MiB; the shared target used 449 MiB, measured with `du -sk`, including debug artifacts and incremental state. Sharing therefore saved approximately 48% here. This is local reuse among matching dependency versions, not reuse of arbitrary public crates across machines.

Sharing only `build.build-dir` also worked: the second project took 0.66 s and compiled one unit; total storage remained 449 MiB with separate final target directories. The original sccache 0.12.0 setup took 12.31 s for the second project and used 999 MiB including its cache. Those measurements used a harness configuration that prevented cross-project cache hits, diagnosed below.

A 2026-10-24 rerun with Cargo 1.97.1 again showed local reuse: the second project took 6.96 s separately, 0.54 s with a shared target directory, and 0.51 s with shared intermediates. Separate outputs occupied 861 MiB versus 443 MiB for either sharing setup. The sccache run explicitly disabled incremental compilation but still reported `Cache hits                            0` and `Cache misses                        125` for its sequential phase; its second project took 8.97 s and total storage was 969 MiB. These were single runs on a shared host, with the sccache and Cargo experiments overlapping.

The cause was the harness's different `CARGO_TARGET_DIR` environment value for each project. sccache 0.12.0's [Rust cache-key implementation](https://github.com/mozilla/sccache/blob/v0.12.0/src/compiler/rust.rs#L1516-L1537) hashes `CARGO_` variables except `CARGO_MAKEFLAGS`, `CARGO_REGISTRIES_*`, and `CARGO_BUILD_JOBS`. A controlled test built only memchr 2.8.3 from p1, p2, then p1 into three fresh output directories: using the environment variable produced three misses; unsetting it and passing Cargo's `--target-dir` produced one miss and two hits. The harness now uses the latter. Dependency sources, compiler, features, and registry paths were unchanged, so this establishes reuse across local projects, not across machines with different source locations.

With that fix, the full sequential sccache run reported `Cache hits                           68` and `Cache misses                         57`; the second project took 3.50 s and total storage was 907 MiB. Reuse works, but this setup still used more storage and took longer than sharing Cargo intermediates. Both sequential and concurrent phases completed successfully. Cross-machine global reuse has not been tested.

## Reproduce

To reuse intermediate artifacts across your own projects with the tested Cargo version, run this from each project:

```sh
CARGO_BUILD_BUILD_DIR="$HOME/.cache/cargo/shared-build" cargo build
```

Use the same directory for each invocation. Final binaries remain in each project's target directory. Cargo still rebuilds incompatible dependency variants, and concurrent builds may wait for its locks. This shares artifacts on one machine under one user; a public cross-machine service is not implemented.

From the repository root:

Prerequisites: Python 3.10 or newer and rustup with a recent stable Cargo. This experiment was tested with Cargo 1.97.1; use that version to reproduce the shared `build-dir` configuration. Registry access is required for the initial fetch. `du` is used for storage measurement. To include the optional `sccache` setup, install sccache first (tested: 0.12.0) and append `sccache` to the command.

```sh
timeout 300s python3 -u experiments/artifact_pool/pool_bench.py /tmp/rust-pool separate shared_target_dir shared_build_dir
```

The script generates isolated projects and locks matching existing dependencies. Choose a disposable work directory: generated manifests and sources are overwritten. It deletes only each setup's output directory between sequential and concurrent phases. Omitting setup names selects the three stable Cargo setups; unknown names fail before files are written. `--target-dir` selects separate or shared final outputs; `CARGO_BUILD_BUILD_DIR` shares intermediate outputs while keeping final target directories separate. Inherited target-directory, build-directory, and compiler-wrapper environment settings are cleared; Cargo config files and rustflags still apply. Incremental compilation is enabled for Cargo setups and disabled for sccache, as required by sccache. The sccache setup uses an isolated cache directory and server port 14227. Do not run two sccache instances of this script at once.

`n_compiled` counts Cargo's “Compiling” messages, so it measures Cargo compiler requests rather than sccache misses. `n_lock_waits` counts all Cargo “Blocking” messages, including package-cache locks; it does not distinguish which lock caused waiting. Earlier concurrent observations used sequential process collection and overestimated early completion times; the harness now collects process completion concurrently. Sequential rows above are unaffected.

## Existing mechanisms and global-pool proposal

The official [Cargo configuration](https://doc.rust-lang.org/cargo/reference/config.html#buildbuild-dir) defines `build.build-dir` as: “The directory where intermediate build artifacts will be stored.” This setting is present in current stable documentation and worked with our stable toolchain; a nightly flag is unnecessary for the basic shared directory.

Cargo's [build cache documentation](https://doc.rust-lang.org/cargo/reference/build-cache.html#shared-cache) says: “A third party tool, sccache, can be used to share built dependencies across different workspaces.” sccache's [Rust documentation](https://github.com/mozilla/sccache/blob/main/docs/Rust.md) warns: “`rustc`'s incremental compilation needs to be disabled.” It also excludes compilations that invoke the linker, including binaries and procedural macros. These limits mean whole-app instant builds do not follow from a warm sccache cache.

Agent recommendation: begin with a trusted same-user shared intermediate directory and measure contention. Keep final executables separate to avoid projects overwriting identically named outputs. A global distributed public pool is a separate proposal: artifacts need matching compiler, target, dependency versions/features, code-generation options, and build-script inputs. Matching crate name/version alone is insufficient. Reusing native code also introduces a trust boundary; the pool must authenticate producers or rebuild/verify artifacts before consumption. These requirements have not been implemented or evaluated here.

A useful next experiment is separate-process concurrent builds with corrected timing, cache-hit counters, changed rustflags/toolchains, build-script environment changes, and bounded cache eviction. It should separate storage saved by pooling from storage merely moved into a second cache.
