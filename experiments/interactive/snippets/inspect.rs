use app::State;

pry::snippet!(|st: &mut State| {
    println!("{} orders, revenue {}", st.orders.len(), st.revenue_cents());
    for (c, idx) in &st.by_customer {
        let valid = st.email_re.is_match(c);
        println!("{c} valid={valid} orders={idx:?}");
    }
    println!("{}", serde_json::to_string_pretty(&st.orders[0]).expect("json"));
});
