---
[dependencies]
serde_json = "1"
regex = "1"
---
//! `cargo -Zscript` baseline: same work as `snippets/inspect.rs`, no host state.
fn main() {
    let json = r#"[{"id":1,"customer":"ann@example.com","total_cents":1250},{"id":2,"customer":"bob@example.com","total_cents":999}]"#;
    let orders: Vec<serde_json::Value> = serde_json::from_str(json).expect("json");
    let re = regex::Regex::new(r"^[^@\s]+@[^@\s]+\.[a-z]+$").expect("re");
    for o in &orders {
        println!("{} valid={}", o["customer"], re.is_match(o["customer"].as_str().expect("str")));
    }
    println!("edit marker 0");
}
