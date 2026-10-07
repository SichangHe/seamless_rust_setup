//! Builds a `State`, then pauses at a pry point; `pry-eval --continue` resumes
//! and the loop pauses again at the next step, so edits to `State` are visible.
use app::State;

const ORDERS: &str = r#"[
  {"id": 1, "customer": "ann@example.com", "total_cents": 1250},
  {"id": 2, "customer": "bob@example.com", "total_cents": 999},
  {"id": 3, "customer": "ann@example.com", "total_cents": 4000}
]"#;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "pry".into());
    let mut state = State::load(ORDERS).expect("static json");
    loop {
        println!(
            "step {}: revenue {} cents",
            state.step,
            state.revenue_cents()
        );
        unsafe { pry::session(&mut state, &dir) };
        state.step += 1;
    }
}
