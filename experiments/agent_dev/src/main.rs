use std::{
    error::Error,
    fs,
    future::pending,
    path::Path,
    thread,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const DEADLINE: Duration = Duration::from_secs(30);

// 🧑 "build one integrated runnable experiment ... reusing those crates"
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args
        .next()
        .ok_or("usage: agent_dev host|smoke|bench SESSION")?;
    let dir = args.next().ok_or("missing session directory")?;
    match mode.as_str() {
        "host" => host(Path::new(&dir)),
        "smoke" => smoke(Path::new(&dir)),
        "bench" => bench(Path::new(&dir)),
        _ => Err("expected host, smoke or bench".into()),
    }
}

fn host(dir: &Path) -> Result<()> {
    let monitor = taskmon::Monitor::serve_files(dir.join("tasks"))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .build()?;
    let _guard = runtime.enter();
    monitor.spawn("pending", pending::<()>());
    let (started, running) = std::sync::mpsc::channel();
    monitor.spawn("never-yields", async move {
        let _ = started.send(());
        loop {
            std::hint::spin_loop();
        }
    });
    running.recv_timeout(DEADLINE)?;
    let mut state = 0_u64;
    // Safety: smoke snippets use the same build and a primitive context, and return.
    unsafe { pry::session(&mut state, dir) };
    runtime.shutdown_background();
    Ok(())
}

fn wait_until(mut ready: impl FnMut() -> bool) -> Result<()> {
    let start = Instant::now();
    while !ready() {
        if start.elapsed() >= DEADLINE {
            return Err("timed out waiting for host".into());
        }
        thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}

/// Measure from immediately before writing the edited source until reading its result.
fn submit(dir: &Path, id: &str, source: &str, expected: &str) -> Result<String> {
    let status_path = dir.join("res").join(format!("{id}.status"));
    if status_path.exists() || dir.join("req").join(format!("{id}.rs")).exists() {
        return Err(format!("source request ID already used: {id}").into());
    }
    let start = Instant::now();
    let temporary = dir.join("req").join(format!("{id}.tmp"));
    fs::write(&temporary, source)?;
    fs::rename(temporary, dir.join("req").join(format!("{id}.rs")))?;
    wait_until(|| status_path.exists())?;
    let status = fs::read_to_string(status_path)?;
    let output = fs::read_to_string(dir.join("res").join(format!("{id}.out")))?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut lines = status.lines();
    let outcome = lines.next().ok_or("missing status")?;
    if outcome != expected {
        return Err(format!("{id}: {status}: {output}").into());
    }
    let compile_ms = lines
        .next()
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or("unavailable");
    println!(
        "request={id} status={outcome} edit_to_result_ms={elapsed_ms:.3} compiler_ms={compile_ms}"
    );
    Ok(output)
}

fn command(dir: &Path, text: &str) -> Result<String> {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let request = dir.join("tasks/req").join(format!("{id}.cmd"));
    let temporary = request.with_extension("tmp");
    fs::write(&temporary, text)?;
    fs::rename(temporary, request)?;
    let output = dir.join("tasks/res").join(format!("{id}.out"));
    wait_until(|| output.exists())?;
    Ok(fs::read_to_string(output)?)
}

fn task_id(snapshot: &str, name: &str) -> Result<String> {
    snapshot
        .lines()
        .find_map(|line| {
            let words: Vec<_> = line.split_whitespace().collect();
            words
                .contains(&name)
                .then(|| words.first().map(|id| (*id).to_owned()))
                .flatten()
        })
        .ok_or_else(|| format!("missing task {name}: {snapshot}").into())
}

fn smoke(dir: &Path) -> Result<()> {
    wait_until(|| dir.join("req").is_dir())?;
    let output = submit(
        dir,
        "mutate",
        "pry::snippet!(|state: &mut u64| { *state += 1; println!(\"state={}\", *state); });",
        "ok",
    )?;
    assert_eq!(output.trim(), "state=1");
    let diagnostics = submit(dir, "broken", "this is not Rust", "compile_error")?;
    assert!(diagnostics.contains("error"));
    let output = submit(
        dir,
        "inspect",
        "pry::snippet!(|state: &mut u64| { println!(\"state={}\", *state); });",
        "ok",
    )?;
    assert_eq!(output.trim(), "state=1");
    let snapshot = command(dir, "top 10 20")?;
    println!("snapshot:\n{snapshot}");
    let pending_id = task_id(&snapshot, "pending")?;
    let busy_id = task_id(&snapshot, "never-yields")?;
    let ack = command(dir, &format!("kill {pending_id}"))?;
    assert_eq!(ack.trim(), format!("abort requested for task {pending_id}"));
    println!("pending: {}", ack.trim());
    wait_until(|| command(dir, "top 10 20").is_ok_and(|s| task_id(&s, "pending").is_err()))?;
    println!("pending: cancellation completed (absent from snapshot)");
    let ack = command(dir, &format!("kill {busy_id}"))?;
    assert_eq!(ack.trim(), format!("abort requested for task {busy_id}"));
    println!("never-yields: {}", ack.trim());
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(250) {
        let snapshot = command(dir, "top 10 20")?;
        assert_eq!(task_id(&snapshot, "never-yields")?, busy_id);
    }
    println!(
        "never-yields: cancel_noncompleting observation_ms={:.3} (still registered)",
        start.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}

/// Exercise fresh source edits and retained state in one running host.
// 🧑 "I want instant compilation after initial cold start"
fn bench(dir: &Path) -> Result<()> {
    wait_until(|| dir.join("req").is_dir())?;
    let mut elapsed_ms = Vec::new();
    let mut expected_state = 0_u64;
    for edit in 1..=30_u64 {
        expected_state += edit;
        let source = format!(
            "pry::snippet!(|state: &mut u64| {{ *state += {edit}; println!(\"state={{}}\", *state); }});"
        );
        let start = Instant::now();
        let output = submit(dir, &format!("edit_{edit}"), &source, "ok")?;
        elapsed_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        if output.trim() != format!("state={expected_state}") {
            return Err(format!("edit {edit}: incorrect retained state: {output}").into());
        }
        if edit % 10 == 0 {
            submit(
                dir,
                &format!("broken_{edit}"),
                "this is not Rust",
                "compile_error",
            )?;
        }
    }
    let output = submit(
        dir,
        "final_inspect",
        "pry::snippet!(|state: &mut u64| { println!(\"state={}\", *state); });",
        "ok",
    )?;
    if output.trim() != format!("state={expected_state}") {
        return Err(format!("incorrect final retained state: {output}").into());
    }
    elapsed_ms.sort_by(f64::total_cmp);
    println!(
        "edits=30 median_ms={:.3} p95_ms={:.3} max_ms={:.3} final_state={expected_state}",
        (elapsed_ms[14] + elapsed_ms[15]) / 2.0,
        elapsed_ms[28],
        elapsed_ms[29]
    );
    Ok(())
}
