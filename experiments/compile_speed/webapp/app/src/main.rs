//! Small but typical web service: the benchmark workload whose build time
//! is dominated by third-party dependencies.
use std::{hint::black_box, net::SocketAddr};

use axum::{
    Json, Router,
    extract::{Path, Query},
    routing::{get, post},
};
use clap::Parser;
#[cfg(feature = "dylib")]
use deps_dylib as _;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing::info;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:0")]
    addr: SocketAddr,
    /// Bind, then exit without serving; used to measure start-up time.
    #[arg(long)]
    once: bool,
}

#[derive(Deserialize)]
struct Search {
    pattern: String,
    text: String,
}

#[derive(Serialize)]
struct Found {
    matches: Vec<String>,
    edit: u64,
}

async fn search(Json(Search { pattern, text }): Json<Search>) -> Result<Json<Found>, String> {
    let regex = Regex::new(&pattern).map_err(|err| err.to_string())?;
    let matches = regex.find_iter(&text).map(|m| m.as_str().into()).collect();
    Ok(Json(Found {
        matches,
        edit: black_box(0u64),
    }))
}

#[derive(Deserialize)]
struct Times {
    n: usize,
}

async fn repeat(Path(word): Path<String>, Query(Times { n }): Query<Times>) -> String {
    word.repeat(n)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();
    let Args { addr, once } = Args::parse();
    let router = Router::new()
        .route("/search", post(search))
        .route("/repeat/{word}", get(repeat));
    let listener = TcpListener::bind(addr).await?;
    info!(addr = %listener.local_addr()?, "listening");
    if once {
        return Ok(());
    }
    axum::serve(listener, router).await
}
