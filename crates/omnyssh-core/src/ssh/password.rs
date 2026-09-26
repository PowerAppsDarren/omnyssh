//! Login passwords typed at a prompt.
//!
//! A password is kept in process memory only — never written to disk — and
//! only once a server has accepted it. It is keyed by the login it was typed
//! for: user, host, port and the bastions on the way, so it is never offered to
//! another server that shares an address behind a different bastion.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch};

use crate::event::CoreEvent;

#[derive(Default)]
struct State {
    /// Passwords a server accepted, by login key.
    accepted: HashMap<String, String>,
    /// Prompts waiting for an answer, by request id.
    pending: HashMap<u64, oneshot::Sender<Option<String>>>,
    last_request: u64,
}

fn state() -> MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Bumped whenever a password is accepted, so a connection waiting for one can
/// go again.
fn accepted_signal() -> &'static watch::Sender<()> {
    static ACCEPTED: OnceLock<watch::Sender<()>> = OnceLock::new();
    ACCEPTED.get_or_init(|| watch::channel(()).0)
}

/// The password a server accepted for `key` this session.
pub(crate) fn accepted(key: &str) -> Option<String> {
    state().accepted.get(key).cloned()
}

/// Remembers `password` for `key`; call only once the server took it.
pub(crate) fn remember(key: &str, password: &str) {
    state()
        .accepted
        .insert(key.to_string(), password.to_string());
    accepted_signal().send_replace(());
}

/// Forgets `password` for `key` after the server turned it down. A newer one
/// remembered meanwhile stays.
pub(crate) fn forget(key: &str, password: &str) {
    let mut state = state();
    if state.accepted.get(key).map(String::as_str) == Some(password) {
        state.accepted.remove(key);
    }
}

/// Resolves once a password is remembered for `key`.
pub(crate) async fn remembered(key: &str) {
    // Subscribe before checking, so one remembered in between is not missed.
    let mut rx = accepted_signal().subscribe();
    while accepted(key).is_none() {
        // The sender lives in a static and is never dropped.
        let _ = rx.changed().await;
    }
}

/// An answer that has no prompt to go to.
#[derive(Debug, Error)]
pub enum PasswordError {
    /// The request was answered already, or its connection stopped waiting.
    #[error("no login is waiting for this password")]
    NotRequested,
}

/// Answers the prompt `request_id` of a [`CoreEvent::PasswordRequired`]:
/// `Some(password)` to try it, `None` to cancel the login.
///
/// # Errors
/// [`PasswordError::NotRequested`] when no connection waits on that request.
pub fn answer(request_id: u64, password: Option<String>) -> Result<(), PasswordError> {
    let reply = state()
        .pending
        .remove(&request_id)
        .ok_or(PasswordError::NotRequested)?;
    reply
        .send(password)
        .map_err(|_| PasswordError::NotRequested)
}

/// Asks the user for a login password while a connection authenticates.
#[async_trait]
pub(crate) trait AskPassword: Send {
    /// The password for `login` (`user@host`), or `None` when the user
    /// cancelled. `retry` says the previous one was refused.
    async fn ask(&mut self, login: &str, retry: bool) -> Option<String>;
}

/// Asks through the frontends: sends [`CoreEvent::PasswordRequired`] and waits
/// for [`answer`].
pub struct Prompter {
    tx: mpsc::Sender<CoreEvent>,
    host_name: String,
}

impl Prompter {
    /// A prompter for connections to `host_name`, reporting on `tx`.
    pub fn new(tx: mpsc::Sender<CoreEvent>, host_name: impl Into<String>) -> Self {
        Self {
            tx,
            host_name: host_name.into(),
        }
    }
}

#[async_trait]
impl AskPassword for Prompter {
    async fn ask(&mut self, login: &str, retry: bool) -> Option<String> {
        let (reply, answer) = oneshot::channel();
        let request_id = {
            let mut state = state();
            state.last_request += 1;
            let id = state.last_request;
            state.pending.insert(id, reply);
            id
        };
        let _open = OpenPrompt {
            request_id,
            tx: self.tx.clone(),
        };
        let asked = self
            .tx
            .send(CoreEvent::PasswordRequired {
                request_id,
                host_name: self.host_name.clone(),
                login: login.to_string(),
                retry,
            })
            .await;
        if asked.is_err() {
            return None;
        }
        answer.await.ok().flatten()
    }
}

/// A prompt on screen. Dropped unanswered — the connection gave up, say a
/// tunnel was stopped — it takes the prompt down again.
struct OpenPrompt {
    request_id: u64,
    tx: mpsc::Sender<CoreEvent>,
}

impl Drop for OpenPrompt {
    fn drop(&mut self) {
        if state().pending.remove(&self.request_id).is_some() {
            let _ = self
                .tx
                .try_send(CoreEvent::PasswordPromptClosed(self.request_id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn next(rx: &mut mpsc::Receiver<CoreEvent>) -> Option<CoreEvent> {
        rx.try_recv().ok()
    }

    async fn asked(rx: &mut mpsc::Receiver<CoreEvent>) -> u64 {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Some(CoreEvent::PasswordRequired { request_id, .. })) => request_id,
            other => panic!("expected a prompt, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_answer_reaches_the_connection_that_asked() {
        let (tx, mut rx) = mpsc::channel(8);
        let mut prompter = Prompter::new(tx, "web-1");
        let asking = tokio::spawn(async move { prompter.ask("root@10.0.0.1", false).await });

        let id = asked(&mut rx).await;
        answer(id, Some(String::from("secret"))).expect("answer");
        assert_eq!(asking.await.expect("ran").as_deref(), Some("secret"));
        assert!(
            next(&mut rx).is_none(),
            "an answered prompt is not closed again"
        );
        assert!(matches!(answer(id, None), Err(PasswordError::NotRequested)));
    }

    #[tokio::test]
    async fn a_cancel_ends_the_login() {
        let (tx, mut rx) = mpsc::channel(8);
        let mut prompter = Prompter::new(tx, "web-1");
        let asking = tokio::spawn(async move { prompter.ask("root@10.0.0.1", true).await });

        let id = asked(&mut rx).await;
        answer(id, None).expect("cancel");
        assert_eq!(asking.await.expect("ran"), None);
    }

    #[tokio::test]
    async fn a_connection_that_stops_waiting_takes_its_prompt_down() {
        let (tx, mut rx) = mpsc::channel(8);
        let mut prompter = Prompter::new(tx, "web-1");
        let asking = tokio::spawn(async move { prompter.ask("root@10.0.0.1", false).await });

        let id = asked(&mut rx).await;
        asking.abort();
        let _ = asking.await;
        match next(&mut rx) {
            Some(CoreEvent::PasswordPromptClosed(closed)) => assert_eq!(closed, id),
            other => panic!("expected the prompt to close, got {other:?}"),
        }
        assert!(matches!(
            answer(id, Some(String::from("late"))),
            Err(PasswordError::NotRequested)
        ));
    }

    #[tokio::test]
    async fn a_waiter_wakes_when_its_login_is_remembered() {
        let key = "waiter@10.9.9.9:22";
        let waiter = tokio::spawn(async move { remembered(key).await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!waiter.is_finished());

        remember("someone-else@10.9.9.9:22", "x");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!waiter.is_finished(), "another login does not wake it");

        remember(key, "secret");
        tokio::time::timeout(Duration::from_secs(5), waiter)
            .await
            .expect("the waiter wakes")
            .expect("the waiter ran");
    }

    #[test]
    fn a_refused_password_is_forgotten_but_a_newer_one_stays() {
        let key = "forget@10.8.8.8:22";
        remember(key, "old");
        forget(key, "old");
        assert_eq!(accepted(key), None);

        remember(key, "new");
        forget(key, "old");
        assert_eq!(accepted(key).as_deref(), Some("new"));
    }
}
