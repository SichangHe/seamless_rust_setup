//! Runs forever, calling `step` from the reloadable dylib each tick.
//! Edit `lib/src/lib.rs`, run `cargo build -p reloadable`, and the running
//! process picks up the new code without restarting or losing `State`.

use std::{thread::sleep, time::Duration};

use reloadable::State;

#[hot_lib_reloader::hot_module(
    dylib = "reloadable",
    lib_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/debug")
)]
mod hot {
    pub use reloadable::State;
    hot_functions_from_file!("lib/src/lib.rs");
}

fn main() {
    let mut state = State { count: 0 };
    loop {
        let out = hot::step(&mut state);
        println!("{out}");
        sleep(Duration::from_millis(300));
    }
}
