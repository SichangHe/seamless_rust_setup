# Seamless Rust setup
(authored by agents unless marked 🧑)

🧑 “I want instant compilation after initial cold start, ability to inject code into running programs for debugging and even monitor asynchronous tasks like `top` does but for agents.” — [human request](HUMAN_REQUEST.md)

This repository studies that goal through runnable experiments. It measures rebuild latency and dependency costs, loads edited Rust files into a paused program, reloads dynamic libraries, and inspects or cancels registered asynchronous tasks. These are research prototypes, not a complete replacement Rust environment.

Start with the [findings and setup](docs/index.md). Each study separates measurements, source evidence, limitations, and proposed work:

- [compilation speed](docs/compile-speed.md)
- [shared compilation artifacts](docs/artifact-pool.md)
- [file-based interactive debugging](docs/interactive.md)
- [hot reload](docs/hot-reload.md)
- [asynchronous task monitoring](docs/task-monitor.md)
- [dependency and binary size](docs/dependency-bloat.md)

The experiments target Linux. Each study gives its own prerequisites and commands. Toolchains, dependency features, and linkers affect the results; a warm build does not imply that arbitrary changes compile instantly.

## Layout

`experiments/` contains runnable code and benchmark drivers. `docs/` contains evidence and design notes. Build products and downloaded workloads stay outside version control. The original request is preserved in `HUMAN_REQUEST.md`.
