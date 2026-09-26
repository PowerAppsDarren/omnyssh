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
use omnyssh_core::ssh::client::{ConnectionStatus, Host, MonitorMode};
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

// Real clock, unlike its neighbours: a paused clock auto-advances whenever the
// runtime idles, which it does while the loopback connect sits in the IO driver —
// so `TCP_PROBE_TIMEOUT` elapses in virtual time and the open port reads as dead.
#[tokio::test]
async fn a_reachability_host_is_probed_without_an_ssh_session() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local addr").port();
    tokio::spawn(async move { while listener.accept().await.is_ok() {} });

    let host = Host {
        monitoring: MonitorMode::TcpPort,
        // The probe port wins over the SSH port, which is deliberately dead here.
        port: 1,
        monitor_port: Some(port),
        ..unreachable_host(port)
    };
    let (tx, mut rx) = mpsc::channel(64);
    let manager = PollManager::start(vec![host], tx, Duration::from_secs(30));

    // Bounded: the poller never closes the channel, so an unbounded wait would
    // hang instead of failing when the expected status stops arriving. Real seconds
    // now, and comfortably clear of `TCP_PROBE_TIMEOUT`: a budget equal to it would
    // expire on the same dial it is meant to watch retry.
    let reachable = tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(event) = rx.recv().await {
            if let CoreEvent::HostStatusChanged(_, ConnectionStatus::Connected) = event {
                return true;
            }
        }
        false
    })
    .await;
    assert_eq!(
        reachable,
        Ok(true),
        "the probe never reported the port as reachable"
    );

    // The probe has settled, so the clock can be paused for the rest: waiting out
    // the cycles below in real seconds would be a two-minute test. A dial that now
    // times out against the virtual clock only reports a status, which is what this
    // half tolerates anyway.
    tokio::time::pause();

    // And over the cycles that follow, it reports status only — metrics would be
    // invented. Elapsing without a metrics event is the pass.
    let stray_metrics = tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(event) = rx.recv().await {
            if matches!(event, CoreEvent::MetricsUpdate(..)) {
                return true;
            }
        }
        false
    })
    .await;
    manager.shutdown();

    assert_ne!(
        stray_metrics,
        Ok(true),
        "a tcp-probed host must not report metrics"
    );
}

#[tokio::test(start_paused = true)]
async fn a_reachability_host_reports_a_closed_port_as_failed() {
    // Port 1 is closed and cannot be re-bound by anything else mid-test.
    let port = 1;

    let host = Host {
        monitoring: MonitorMode::TcpPort,
        ..unreachable_host(port)
    };
    let (tx, mut rx) = mpsc::channel(64);
    let manager = PollManager::start(vec![host], tx, Duration::from_secs(30));

    let failed = tokio::time::timeout(Duration::from_secs(60), async {
        while let Some(event) = rx.recv().await {
            if let CoreEvent::HostStatusChanged(_, ConnectionStatus::Failed(_)) = event {
                return true;
            }
        }
        false
    })
    .await;
    manager.shutdown();

    assert_eq!(
        failed,
        Ok(true),
        "a closed port was not reported as unreachable"
    );
}

#[tokio::test(start_paused = true)]
async fn a_reachability_host_behind_a_bastion_says_so_instead_of_probing_direct() {
    let host = Host {
        monitoring: MonitorMode::TcpPort,
        proxy_jump: Some(String::from("bastion")),
        ..unreachable_host(22)
    };
    let (tx, mut rx) = mpsc::channel(64);
    let manager = PollManager::start(vec![host], tx, Duration::from_secs(30));

    let explained = tokio::time::timeout(Duration::from_secs(60), async {
        while let Some(event) = rx.recv().await {
            if let CoreEvent::HostStatusChanged(_, ConnectionStatus::Failed(message)) = event {
                // Probing the target address direct would report on whatever
                // else answers it, which is worse than saying nothing.
                return message.contains("ProxyJump");
            }
        }
        false
    })
    .await;
    manager.shutdown();

    assert_eq!(explained, Ok(true), "a bastion host was probed direct");
}
