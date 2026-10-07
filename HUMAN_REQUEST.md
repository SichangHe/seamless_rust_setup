# Human request

(authored by human unless marked 🤖)

Separately, I want dedicated agents, also fully autonomous, to study how to make Rust the perfect programming environment by addressing all its major shortfalls: compilation speed, interactive use and pry, compilation artifact bloat. Call it seamless_rust_setup and make it public. It can use existing tools or invent new ones. Ideas include dynamic linking, code hot reloading (maybe see dioxus or something called something like that and think maybe what Flutter offers), loading and running dylib or something at run time to dynamically run code, Erlang-style shell attachment and process monitoring and killing (async tasks), much better interactive use by editing and loading a file or files instead of doing it REPL style and optimized for agents, fork large dependencies that we only use very small portions of (see ones used in my shame crate, and also perhaps try to find time crate smaller than chrono but with enough functionalities, maybe an extension of the time crate; tokio always bloats the binary so much and it’s unclear why we need so much junk when we don’t really need much but its ecosystem is big so maybe we somehow produce something other crates think is tokio?), Pooling compilation artifacts of public crates globally and reusing them. I want instant compilation after initial cold start, ability to inject code into running programs for debugging and even monitor asynchronous tasks like `top` does but for agents.

Agents decide for themselves autonomously and only ask me to clarify the top-level goal instead of bothering me.

Use a combination of Opus low and Fable medium agents for all these and let these agents consult other context-free agents for decisions and avoid bothering me

## 🤖 Facts

- 🤖 the shame crate is https://github.com/SichangHe/shame.rs
- 🤖 the human's Rust notes are at `/ssd1/sichanghe.github.io/src/notes/programming/rust.md`
- 🤖 this repo will be published at github.com/SichangHe/seamless_rust_setup
