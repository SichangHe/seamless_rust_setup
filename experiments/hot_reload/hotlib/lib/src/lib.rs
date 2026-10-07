//! Reloadable logic. The runner owns all state and passes it in by `&mut`,
//! so we can swap this code without losing the running program's data.

pub use types::State;

#[no_mangle]
pub fn step(state: &mut State) -> String {
    // 🧑 "code hot reloading ... loading and running dylib ... at run time".
    // Edit the body below and the runner picks it up without restarting.
    state.count += 1;
    format!("v1: count={}", state.count)
}
