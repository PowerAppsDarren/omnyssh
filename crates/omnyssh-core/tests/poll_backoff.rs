//! The reconnect schedule of the metrics poller, driven through `PollManager`.
//!
//! The listener accepts a TCP connection and drops it, so every SSH connect
//! fails the way an unreachable host does — enough to exercise the backoff
//! without an SSH server.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::mpsc;

use omnyssh_core::event::CoreEvent;
use omnyssh_core::ssh::client::Host;
use omnyssh_core::ssh::pool::PollManager;

/// Counts connections, answering none of them.
async fn dead_listener() -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local addr").port();
    let dials = Arc::new(AtomicUsize::new(0));

    let counter = Arc::clone(&dials);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            drop(stream);
        }
    });

    (port, dials)
}

fn unreachable_host(port: u16) -> Host {
    Host {
        name: String::from("probe"),
        hostname: String::from("127.0.0.1"),
        port,
        ..Host::default()
    }
}

/// Drains events so a full channel can never stall the poller under test.
fn drain(mut rx: mpsc::Receiver<CoreEvent>) {
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
}

/// Waits until `dials` reaches `target`, or `limit` of the test clock elapses.
/// Sampling rather than sleeping a fixed span keeps the count off the critical
/// path: the listener increments it from its own task.
async fn wait_for_dials(dials: &AtomicUsize, target: usize, limit: Duration) -> bool {
    tokio::time::timeout(limit, async {
        while dials.load(Ordering::SeqCst) < target {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .is_ok()
}

#[tokio::test(start_paused = true)]
async fn a_refresh_signal_does_not_shorten_the_reconnect_backoff() {
    let (port, dials) = dead_listener().await;
    let (tx, rx) = mpsc::channel(64);
    drain(rx);

    let manager = PollManager::start(vec![unreachable_host(port)], tx, Duration::from_secs(30));
    assert!(
        wait_for_dials(&dials, 1, Duration::from_secs(60)).await,
        "the poller never dialled at all"
    );

    // Twelve nudges over two minutes — the shape of the GUI's refresh timer at
    // its shortest setting.
    for _ in 0..12 {
        manager.refresh_all();
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
    manager.shutdown();

    // Backoff runs 30, 60, 120: four dials at most in the 120 s that follow the
    // first. Honouring the signals instead dials on every one of them.
    let dialled = dials.load(Ordering::SeqCst);
    assert!(
        dialled <= 4,
        "expected the backoff schedule to hold, got {dialled} dials"
    );
}

#[tokio::test(start_paused = true)]
async fn an_unreachable_host_keeps_retrying_on_its_own_schedule() {
    let (port, dials) = dead_listener().await;
    let (tx, rx) = mpsc::channel(64);
    drain(rx);

    let manager = PollManager::start(vec![unreachable_host(port)], tx, Duration::from_secs(30));
    let retried = wait_for_dials(&dials, 2, Duration::from_secs(300)).await;
    manager.shutdown();

    // A host that stops answering is retried, not abandoned.
    assert!(retried, "the poller stopped retrying an unreachable host");
}
