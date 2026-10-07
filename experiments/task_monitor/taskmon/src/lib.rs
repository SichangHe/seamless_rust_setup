//! `top` and `kill` for Tokio tasks, served as plain text over a Unix socket.
//! Design: `docs/task-monitor.md`.
// 🧑 "Erlang-style shell attachment and process monitoring and killing (async tasks)" ...
// "monitor asynchronous tasks like `top` does but for agents"
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    future::{poll_fn, Future},
    io::{self, BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    panic::Location,
    path::Path,
    pin::pin,
    sync::{
        atomic::{AtomicU64, Ordering::Relaxed},
        mpsc::{channel, Sender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tokio::task::{AbortHandle, JoinHandle};

/// Counters of one task. Only the thread polling the task writes them.
struct Stats {
    name: String,
    spawned_at: &'static Location<'static>,
    spawned: Instant,
    n_polls: AtomicU64,
    busy_ns: AtomicU64,
    /// When the running poll started, in ns since `spawned` plus 1; `0` if not being polled.
    poll_start_ns: AtomicU64,
}

struct Entry {
    stats: Arc<Stats>,
    abort: AbortHandle,
}

/// One task as seen at one instant.
#[derive(Clone, Debug)]
struct Sample {
    id: u64,
    name: String,
    spawned_at: &'static Location<'static>,
    age: Duration,
    n_polls: u64,
    /// Includes the running poll; measures wall time, including blocking waits.
    busy_ns: u64,
    /// How long the running poll has lasted, if the task is being polled.
    in_poll_ns: Option<u64>,
}

enum Msg {
    Spawned(Entry),
    Snapshot(Sender<Vec<Sample>>),
    Kill(u64, Sender<bool>),
}

/// Handle to spawn monitored tasks. Cheap to clone.
#[derive(Clone)]
pub struct Monitor {
    registry: Sender<Msg>,
}

impl Monitor {
    /// Start the registry and the socket server at `socket_path` on their own OS threads,
    /// so they answer even when every runtime worker is blocked.
    pub fn serve(socket_path: impl AsRef<Path>) -> io::Result<Self> {
        let listener = UnixListener::bind(socket_path)?;
        let (registry, msgs) = channel();
        thread::Builder::new()
            .name("taskmon-registry".into())
            .spawn(move || run_registry(msgs))?;
        let monitor = Self { registry };
        let registry = monitor.registry.clone();
        thread::Builder::new()
            .name("taskmon-listener".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let registry = registry.clone();
                    thread::spawn(move || {
                        let _ = handle_client(stream, &registry);
                    });
                }
            })?;
        Ok(monitor)
    }

    /// Like [`tokio::spawn`], but the task shows up in `top` under `name` and `kill` can abort it.
    #[track_caller]
    pub fn spawn<F>(&self, name: impl Into<String>, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let stats = Arc::new(Stats {
            name: name.into(),
            spawned_at: Location::caller(),
            spawned: Instant::now(),
            n_polls: AtomicU64::new(0),
            busy_ns: AtomicU64::new(0),
            poll_start_ns: AtomicU64::new(0),
        });
        let handle = tokio::spawn({
            let stats = stats.clone();
            async move {
                let mut future = pin!(future);
                poll_fn(|cx| {
                    let start = Instant::now();
                    let start_ns = (start - stats.spawned).as_nanos() as u64 + 1;
                    stats.poll_start_ns.store(start_ns, Relaxed);
                    let poll = future.as_mut().poll(cx);
                    let busy_ns = stats.busy_ns.load(Relaxed) + start.elapsed().as_nanos() as u64;
                    stats.busy_ns.store(busy_ns, Relaxed);
                    stats
                        .n_polls
                        .store(stats.n_polls.load(Relaxed) + 1, Relaxed);
                    stats.poll_start_ns.store(0, Relaxed);
                    poll
                })
                .await
            }
        });
        let abort = handle.abort_handle();
        let _ = self.registry.send(Msg::Spawned(Entry { stats, abort }));
        handle
    }
}

/// The registry actor: sole owner of the task table.
/// Finished tasks are dropped at each snapshot and whenever the table doubles.
fn run_registry(msgs: std::sync::mpsc::Receiver<Msg>) {
    let mut tasks = BTreeMap::new();
    let mut next_id = 1u64;
    let mut prune_at_len = 1024;
    for msg in msgs {
        match msg {
            Msg::Spawned(entry) => {
                tasks.insert(next_id, entry);
                next_id += 1;
                if tasks.len() >= prune_at_len {
                    tasks.retain(|_, entry: &mut Entry| !entry.abort.is_finished());
                    prune_at_len = (tasks.len() * 2).max(1024);
                }
            }
            Msg::Snapshot(reply) => {
                tasks.retain(|_, entry| !entry.abort.is_finished());
                let samples = tasks.iter().map(|(id, entry)| sample(*id, &entry.stats));
                let _ = reply.send(samples.collect());
            }
            Msg::Kill(id, reply) => {
                let found = tasks.get(&id).map(|entry| entry.abort.abort()).is_some();
                let _ = reply.send(found);
            }
        }
    }
}

/// The counters are read one by one without a lock,
/// so a sample taken while the task is polled may be off by one poll.
fn sample(id: u64, stats: &Stats) -> Sample {
    let age = stats.spawned.elapsed();
    let in_poll_ns = match stats.poll_start_ns.load(Relaxed) {
        0 => None,
        start_ns => Some((age.as_nanos() as u64).saturating_sub(start_ns - 1)),
    };
    Sample {
        id,
        name: stats.name.clone(),
        spawned_at: stats.spawned_at,
        age,
        n_polls: stats.n_polls.load(Relaxed),
        busy_ns: stats.busy_ns.load(Relaxed) + in_poll_ns.unwrap_or(0),
        in_poll_ns,
    }
}

const HELP: &str = "\
top [INTERVAL_MS=1000] [N_ROWS=20]  tasks sorted by time inside poll
kill ID                             request cancellation when poll returns
";

/// Answer each command line from `stream` until the client closes its writing side.
fn handle_client(stream: UnixStream, registry: &Sender<Msg>) -> io::Result<()> {
    let mut out = stream.try_clone()?;
    for line in BufReader::new(stream).lines() {
        let line = line?;
        let mut words = line.split_whitespace();
        let reply = match (words.next(), words.next(), words.next()) {
            (Some("top"), interval_ms, n_rows) => {
                let interval_ms = interval_ms.and_then(|w| w.parse().ok()).unwrap_or(1000);
                let n_rows = n_rows.and_then(|w| w.parse().ok()).unwrap_or(20);
                let interval = Duration::from_millis(interval_ms.max(1));
                let before = snapshot(registry);
                let start = Instant::now();
                thread::sleep(interval);
                match (before, snapshot(registry)) {
                    (Some(before), Some(after)) => {
                        render_top(&before, after, start.elapsed(), n_rows)
                    }
                    _ => "registry is gone\n".into(),
                }
            }
            (Some("kill"), Some(id), None) => {
                match id.parse().ok().and_then(|id| kill(registry, id)) {
                    Some(true) => format!("abort requested for task {id}\n"),
                    _ => format!("no task {id}\n"),
                }
            }
            _ => HELP.into(),
        };
        out.write_all(reply.as_bytes())?;
    }
    Ok(())
}

fn snapshot(registry: &Sender<Msg>) -> Option<Vec<Sample>> {
    let (reply, samples) = channel();
    registry.send(Msg::Snapshot(reply)).ok()?;
    samples.recv().ok()
}

fn kill(registry: &Sender<Msg>, id: u64) -> Option<bool> {
    let (reply, found) = channel();
    registry.send(Msg::Kill(id, reply)).ok()?;
    found.recv().ok()
}

/// BUSY% is wall time inside poll divided by the elapsed sampling interval.
/// Blocking waits count, so this is not CPU utilization.
/// `after` must be sorted by ID like `before`, as the registry returns them.
fn render_top(
    before: &[Sample],
    mut after: Vec<Sample>,
    interval: Duration,
    n_rows: usize,
) -> String {
    let busy_before = |id| match before.binary_search_by_key(&id, |sample| sample.id) {
        Ok(index) => before[index].busy_ns,
        Err(_) => 0,
    };
    let busy_percent = |sample: &Sample| {
        100.0 * sample.busy_ns.saturating_sub(busy_before(sample.id)) as f64
            / interval.as_nanos().max(1) as f64
    };
    let total_busy_percent: f64 = after.iter().map(busy_percent).sum();
    after.sort_by(|a, b| {
        busy_percent(b)
            .total_cmp(&busy_percent(a))
            .then(b.busy_ns.cmp(&a.busy_ns))
    });
    let mut text = format!(
        "{} tasks, {total_busy_percent:.1}% inside poll, showing {}\n\
         {:>6} {:>6} {:>9} {:>9} {:>8} {:>8}  NAME @ SPAWNED_AT\n",
        after.len(),
        after.len().min(n_rows),
        "ID",
        "BUSY%",
        "POLLS",
        "BUSY_MS",
        "AGE_S",
        "IN_POLL",
    );
    for sample in after.iter().take(n_rows) {
        let in_poll = match sample.in_poll_ns {
            Some(ns) => format!("{:.3}s", ns as f64 / 1e9),
            None => "-".into(),
        };
        let _ = writeln!(
            text,
            "{:>6} {:>6.1} {:>9} {:>9.1} {:>8.1} {:>8}  {} @ {}:{}",
            sample.id,
            busy_percent(sample),
            sample.n_polls,
            sample.busy_ns as f64 / 1e6,
            sample.age.as_secs_f64(),
            in_poll,
            sample.name,
            sample.spawned_at.file(),
            sample.spawned_at.line(),
        );
    }
    text
}
