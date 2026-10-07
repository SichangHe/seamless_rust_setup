//! Stand-in for a real project: some state built from dependencies.
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: u64,
    pub customer: String,
    pub total_cents: i64,
}

#[derive(Debug)]
pub struct State {
    pub orders: Vec<Order>,
    pub by_customer: HashMap<String, Vec<usize>>,
    pub email_re: Regex,
    pub step: u64,
}

impl State {
    pub fn load(json: &str) -> Result<Self, serde_json::Error> {
        let orders: Vec<Order> = serde_json::from_str(json)?;
        let mut by_customer: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, o) in orders.iter().enumerate() {
            by_customer.entry(o.customer.clone()).or_default().push(i);
        }
        Ok(Self {
            orders,
            by_customer,
            email_re: Regex::new(r"^[^@\s]+@[^@\s]+\.[a-z]+$").expect("static regex"),
            step: 0,
        })
    }

    pub fn revenue_cents(&self) -> i64 {
        self.orders.iter().map(|o| o.total_cents).sum()
    }
}
