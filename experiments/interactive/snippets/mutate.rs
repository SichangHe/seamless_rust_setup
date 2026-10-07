use app::{Order, State};

pry::snippet!(|st: &mut State| {
    st.orders.push(Order { id: 4, customer: "cat@example.com".into(), total_cents: 1 });
    let i = st.orders.len() - 1;
    st.by_customer.entry("cat@example.com".into()).or_default().push(i);
    println!("now {} orders, revenue {}", st.orders.len(), st.revenue_cents());
});
