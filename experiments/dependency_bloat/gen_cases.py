#!/usr/bin/env python3
"""Generate one tiny Cargo package per dependency case under `cases/`.

Each case is a binary that uses its dependency just enough that the linker
cannot drop it. `measure.py` then builds every case from scratch and records
compile time, `target/` size, binary size, and crate count.
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
CASES_DIR = HERE / "cases"

# 🧑 "fork large dependencies that we only use very small portions of (see
# ones used in my shame crate, ... time crate smaller than chrono ... tokio
# always bloats the binary so much"
# name -> (dependency lines for `[dependencies]`, body of `fn main()`)
CASES: dict[str, tuple[str, str]] = {
    "hello": ("", 'println!("hello {}", std::env::args().count());'),
    # --- shame and its dependencies ---
    "shame": (
        'shame = "0.0.4"',
        """use shame::prelude::*;
    init_tracing();
    let pat = std::env::args().nth(1).unwrap_or_default();
    let re = Regex::new(&pat).expect("regex");
    info!(matched = re.is_match("abc"), "ran");
    let r: Result<()> = (|| bail!("boom {pat}"))();
    if let Err(e) = r { eprintln!("{e:?}"); }""",
    ),
    "anyhow": (
        'anyhow = "1"',
        """let r: anyhow::Result<()> = (|| anyhow::bail!("boom {}", std::env::args().count()))();
    if let Err(e) = r { eprintln!("{e:?}"); }""",
    ),
    "anyhow_backtrace": (
        'anyhow = { version = "1", features = ["backtrace"] }',
        """let r: anyhow::Result<()> = (|| anyhow::bail!("boom {}", std::env::args().count()))();
    if let Err(e) = r { eprintln!("{e:?}"); }""",
    ),
    "regex": (
        'regex = "1"',
        """let pat = std::env::args().nth(1).unwrap_or_default();
    let re = regex::Regex::new(&pat).expect("regex");
    println!("{}", re.is_match("abc"));""",
    ),
    "regex_lite": (
        'regex-lite = "0.1"',
        """let pat = std::env::args().nth(1).unwrap_or_default();
    let re = regex_lite::Regex::new(&pat).expect("regex");
    println!("{}", re.is_match("abc"));""",
    ),
    "tracing": (
        'tracing = "0.1"',
        'tracing::info!(n = std::env::args().count(), "ran");',
    ),
    "tracing_sub_fmt": (
        'tracing = "0.1"\ntracing-subscriber = "0.3"',
        """tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    tracing::info!(n = std::env::args().count(), "ran");""",
    ),
    "tracing_sub_envfilter": (
        'tracing = "0.1"\ntracing-subscriber = { version = "0.3", features = ["env-filter"] }',
        """tracing_subscriber::fmt().with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    tracing::info!(n = std::env::args().count(), "ran");""",
    ),
    "tracing_sub_minimal": (
        'tracing = "0.1"\ntracing-subscriber = { version = "0.3", default-features = false, features = ["fmt", "std"] }',
        """tracing_subscriber::fmt().with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO).init();
    tracing::info!(n = std::env::args().count(), "ran");""",
    ),
    "thiserror": (
        'thiserror = "1"',
        """#[derive(Debug, thiserror::Error)]
    enum E { #[error("bad {0}")] Bad(usize) }
    eprintln!("{}", E::Bad(std::env::args().count()));""",
    ),
    "derive_new": (
        'derive-new = "0.7"',
        """#[derive(derive_new::new, Debug)]
    struct S { a: usize }
    println!("{:?}", S::new(std::env::args().count()));""",
    ),
    "derive_where": (
        'derive-where = "1"',
        """#[derive_where::derive_where(Debug, Default)]
    struct S<T> { a: usize, _t: std::marker::PhantomData<T> }
    println!("{:?}", S::<String>::default());""",
    ),
    "derive_everything": (
        'derive_everything = "0.1"',
        """#[derive_everything::derive_everything]
    struct S { a: usize }
    println!("{:?}", S { a: std::env::args().count() });""",
    ),
    "pub_fields": (
        'pub-fields = "0.1"',
        """mod m { #[pub_fields::pub_fields] #[derive(Debug)] pub struct S { a: usize } }
    println!("{:?}", m::S { a: std::env::args().count() });""",
    ),
    # --- time libraries ---
    "chrono": (
        'chrono = "0.4"',
        """let now = chrono::Local::now();
    let utc: chrono::DateTime<chrono::Utc> = "2024-02-29T12:34:56Z".parse().expect("parse");
    println!("{} {} {}", now.to_rfc3339(), (utc + chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S"), now.offset());""",
    ),
    "chrono_min": (
        'chrono = { version = "0.4", default-features = false, features = ["std", "clock"] }',
        """let now = chrono::Local::now();
    let utc: chrono::DateTime<chrono::Utc> = "2024-02-29T12:34:56Z".parse().expect("parse");
    println!("{} {} {}", now.to_rfc3339(), (utc + chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S"), now.offset());""",
    ),
    "time": (
        'time = { version = "0.3", features = ["formatting", "parsing", "macros", "local-offset"] }',
        """use time::format_description::well_known::Rfc3339;
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let utc = time::OffsetDateTime::parse("2024-02-29T12:34:56Z", &Rfc3339).expect("parse");
    let f = time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    println!("{} {} {}", now.format(&Rfc3339).expect("fmt"), (utc + time::Duration::days(1)).format(&f).expect("fmt"), now.offset());""",
    ),
    "time_min": (
        'time = { version = "0.3", features = ["formatting", "parsing"] }',
        """use time::format_description::well_known::Rfc3339;
    let now = time::OffsetDateTime::now_utc();
    let utc = time::OffsetDateTime::parse("2024-02-29T12:34:56Z", &Rfc3339).expect("parse");
    println!("{} {} {}", now.format(&Rfc3339).expect("fmt"), (utc + time::Duration::days(1)).format(&Rfc3339).expect("fmt"), now.offset());""",
    ),
    "jiff": (
        'jiff = "0.2"',
        """let now = jiff::Zoned::now();
    let utc: jiff::Timestamp = "2024-02-29T12:34:56Z".parse().expect("parse");
    println!("{} {} {}", now, (utc + jiff::SignedDuration::from_hours(24)).strftime("%Y-%m-%d %H:%M:%S"), now.offset());""",
    ),
    "jiff_min": (
        'jiff = { version = "0.2", default-features = false, features = ["std"] }',
        """let now = jiff::Timestamp::now();
    let utc: jiff::Timestamp = "2024-02-29T12:34:56Z".parse().expect("parse");
    println!("{} {}", now, (utc + jiff::SignedDuration::from_hours(24)).strftime("%Y-%m-%d %H:%M:%S"));""",
    ),
    "humantime": (
        'humantime = "2"',
        """let now = std::time::SystemTime::now();
    let then = humantime::parse_rfc3339("2024-02-29T12:34:56Z").expect("parse");
    println!("{} {}", humantime::format_rfc3339_seconds(now), humantime::format_rfc3339(then + std::time::Duration::from_secs(86400)));""",
    ),
    # --- async runtimes ---
    "tokio_full": (
        'tokio = { version = "1", features = ["full"] }',
        """let rt = tokio::runtime::Runtime::new().expect("rt");
    rt.block_on(async {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<usize>(1);
        let h = tokio::spawn(async move { tx.send(std::env::args().count()).await.ok(); });
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        println!("{:?}", rx.recv().await);
        h.await.expect("join");
    });""",
    ),
    "tokio_rt": (
        'tokio = { version = "1", features = ["rt"] }',
        """let rt = tokio::runtime::Builder::new_current_thread().build().expect("rt");
    rt.block_on(async {
        let h = tokio::spawn(async move { std::env::args().count() });
        println!("{:?}", h.await.expect("join"));
    });""",
    ),
    "tokio_rt_mt": (
        'tokio = { version = "1", features = ["rt-multi-thread", "macros"] }',
        """#[tokio::main]
    async fn run() -> usize { tokio::spawn(async move { std::env::args().count() }).await.expect("join") }
    println!("{:?}", run());""",
    ),
    "tokio_common": (
        'tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time", "net", "io-util", "fs"] }',
        """#[tokio::main]
    async fn run() -> Option<usize> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<usize>(1);
        let h = tokio::spawn(async move { tx.send(std::env::args().count()).await.ok(); });
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        let _ = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let _ = tokio::fs::metadata(".").await;
        let r = rx.recv().await;
        h.await.expect("join");
        r
    }
    println!("{:?}", run());""",
    ),
    "smol": (
        'smol = "2"',
        """smol::block_on(async {
        let (tx, rx) = smol::channel::bounded::<usize>(1);
        let h = smol::spawn(async move { tx.send(std::env::args().count()).await.ok(); });
        smol::Timer::after(std::time::Duration::from_millis(1)).await;
        let _ = smol::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        println!("{:?}", rx.recv().await);
        h.await;
    });""",
    ),
    "async_executor": (
        'async-executor = "1"\nfutures-lite = "2"',
        """let ex = async_executor::Executor::new();
    let out = futures_lite::future::block_on(ex.run(async {
        ex.spawn(async move { std::env::args().count() }).await
    }));
    println!("{out:?}");""",
    ),
    "std_thread": (
        "",
        """let (tx, rx) = std::sync::mpsc::sync_channel::<usize>(1);
    let h = std::thread::spawn(move || { tx.send(std::env::args().count()).ok(); });
    std::thread::sleep(std::time::Duration::from_millis(1));
    let _ = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    println!("{:?}", rx.recv());
    h.join().expect("join");""",
    ),
}

CASES["tokio_selected"] = (
    'tokio = { version = "1", features = ["rt-multi-thread", "sync", "time"] }',
    CASES["tokio_full"][1],
)

CARGO_TOML = """[package]
name = "case_{name}"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]

[dependencies]
{deps}

[profile.min]
inherits = "release"
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
"""

MAIN_RS = """fn main() {{
    {body}
}}
"""


def main() -> int:
    names = sys.argv[1:] or list(CASES)
    for name in names:
        deps, body = CASES[name]
        d = CASES_DIR / name
        (d / "src").mkdir(parents=True, exist_ok=True)
        _ = (d / "Cargo.toml").write_text(CARGO_TOML.format(name=name, deps=deps))
        _ = (d / "src" / "main.rs").write_text(MAIN_RS.format(body=body))
    print(f"generated {len(names)} cases in {CASES_DIR}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
