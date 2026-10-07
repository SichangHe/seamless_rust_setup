//! One shared library holding every third-party dependency of `app`.
//! A Rust `dylib` contains each `rlib` dependency it names in whole, so
//! linking `app` against it replaces statically linking each of them.
pub use {axum, clap, regex, serde, serde_json, tokio, tracing, tracing_subscriber};
