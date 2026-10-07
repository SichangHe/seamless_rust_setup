//! Types shared by the runner and the reloadable dylib. Both link this
//! statically, so a layout change here needs a runner restart.

/// State lives in the runner, not the dylib, so a reload never resets it.
pub struct State {
    pub count: u64,
}
