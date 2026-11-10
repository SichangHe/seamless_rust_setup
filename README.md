# Seamless Rust setup
(authored by agents unless marked 🧑)

🧑 “I want instant compilation after initial cold start, ability to inject code into running programs for debugging and even monitor asynchronous tasks like `top` does but for agents.” — [human request](HUMAN_REQUEST.md)

This repository studies that goal through runnable experiments. It measures rebuild latency and dependency costs, loads edited Rust files into a paused program, reloads dynamic libraries, and inspects or cancels registered asynchronous tasks. These are research prototypes, not a complete replacement Rust environment.

Start with the [practical guide and script template](docs/start-here.md) to use this work in a new task. For measurements and prototypes, read the [findings and setup](docs/index.md). Each study separates measurements, source evidence, limitations, and proposed work:

- [compilation speed](docs/compile-speed.md)
- [shared compilation artifacts](docs/artifact-pool.md)
- [file-based interactive debugging](docs/interactive.md)
- [hot reload](docs/hot-reload.md)
- [asynchronous task monitoring](docs/task-monitor.md)
- [integrated file and task control](docs/agent-dev.md)
- [dependency and binary size](docs/dependency-bloat.md)

## Install the build settings

`setup/install.sh` adds smaller debug info and moves build intermediates out of each repository into `~/.cargo/build/`, by appending a marked block to Cargo's user config. `setup/uninstall.sh` removes only that block. Measured effect on four real workspaces: build output 1.7-2.5x smaller, edit rebuilds 9-34% faster; panics keep file and line, but debuggers lose local variables. Requires Cargo 1.91 or newer. Measurements: [rust_build_eval](https://github.com/SichangHe/rust_build_eval).

```sh
setup/install.sh    # undo: setup/uninstall.sh
```

The experiments target Linux. Each study gives its own prerequisites and commands. Toolchains, dependency features, and linkers affect the results; a warm build does not imply that arbitrary changes compile instantly.

## Layout

`experiments/` contains runnable code and benchmark drivers. `docs/` contains evidence and design notes. Build products and downloaded workloads stay outside version control. The original request is preserved in `HUMAN_REQUEST.md`.
