//! Demo program: `demo SOCKET` runs 2 workers with one task per interesting behavior.
use std::{env::args, future::pending, time::Duration};
use tokio::{sync::mpsc::channel, time::sleep};
use tokio_gen_server::{actor::ActorRunExt, prelude::*};
use tokio_util::sync::CancellationToken;

/// A `tokio_gen_server` actor that counts casts.
struct Counter(u64);

impl Actor for Counter {
    type Call = ();
    type Cast = ();
    type Reply = u64;

    async fn handle_cast(&mut self, (): (), _env: &mut ActorEnv<Self>) -> anyhow::Result<()> {
        self.0 += 1;
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let socket_path = args().nth(1).expect("usage: demo SOCKET");
    let monitor = taskmon::Monitor::serve(socket_path)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()?;
    runtime.block_on(async {
        monitor.spawn("ticker", async {
            loop {
                sleep(Duration::from_millis(10)).await;
            }
        });
        monitor.spawn("half-busy", async {
            loop {
                std::thread::sleep(Duration::from_millis(5));
                sleep(Duration::from_millis(5)).await;
            }
        });
        monitor.spawn("leaked", pending::<()>());
        monitor.spawn("never-yields", async {
            sleep(Duration::from_secs(1)).await;
            loop {
                std::hint::spin_loop();
            }
        });
        // `Counter.spawn()` calls `tokio::spawn` itself, which `taskmon` cannot see,
        // so build the same future from the public parts and spawn it monitored.
        let (msg_sender, msg_receiver) = channel(8);
        let counter = ActorRef::<Counter> {
            msg_sender,
            cancellation_token: CancellationToken::new(),
        };
        let mut env = ActorEnv::<Counter> {
            ref_: counter.clone(),
            msg_receiver,
        };
        monitor.spawn("counter-actor", async move {
            Counter(0).run_and_handle_exit(&mut env).await
        });
        loop {
            sleep(Duration::from_millis(1)).await;
            if counter.cast(()).await.is_err() {
                sleep(Duration::from_secs(3600)).await;
            }
        }
    })
}
