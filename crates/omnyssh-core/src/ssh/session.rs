//! Async SSH session management via russh.
//!
//! Provides [`SshSession`] — a thin wrapper around a russh client handle that
//! supports connecting, executing commands, and graceful disconnect.
//! Authentication order: SSH agent → identity file → default keys → password.
//!
//! Hosts with a `ProxyJump` are reached through their bastions: each hop is
//! connected and authenticated in turn, and the next hop rides a
//! `direct-tcpip` channel opened on the previous one (the `ssh -J` model).
//!
//! Connection and command timeouts are enforced:
//! - Connect timeout: 10 seconds (per hop)
//! - Command timeout: 30 seconds

use std::fmt;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use russh::client::{self, Handle};
use russh::ChannelMsg;
use tokio::time;

use crate::ssh::client::Host;
use crate::ssh::identity::{self, IdentityError};
use crate::ssh::password::{self, AskPassword};

// ---------------------------------------------------------------------------
// russh Handler implementation
// ---------------------------------------------------------------------------

/// Shared russh client handler used by every native SSH path (metrics, SFTP,
/// terminal).
///
/// Verifies the server's host key against `~/.ssh/known_hosts`.
/// Unknown hosts are recorded on first connection (trust on first use);
/// changed keys are rejected.
pub(crate) struct KnownHostsHandler {
    /// Hostname used for known_hosts lookup.
    host: String,
    /// Port used for known_hosts lookup.
    port: u16,
    /// Set when the server ends the session with a DISCONNECT of its own, as
    /// OpenSSH does after too many failed logins. A link that just dies leaves
    /// it unset.
    hung_up: Arc<AtomicBool>,
}

#[async_trait]
impl client::Handler for KnownHostsHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        match russh::keys::check_known_hosts(&self.host, self.port, server_public_key) {
            // Key is in known_hosts and matches.
            Ok(true) => Ok(true),
            // Host not seen before — record the key (trust on first use) so a
            // later key change is detected, then accept. Recording is
            // best-effort: a connection must not fail just because
            // known_hosts is unwritable.
            Ok(false) => {
                tracing::warn!(
                    host = %self.host,
                    port = self.port,
                    "Accepting unknown host key for {} (Trust On First Use)", self.host
                );
                match russh::keys::known_hosts::learn_known_hosts(
                    &self.host,
                    self.port,
                    server_public_key,
                ) {
                    Ok(()) => tracing::info!(
                        host = %self.host,
                        port = self.port,
                        "recorded new host key in known_hosts"
                    ),
                    Err(e) => tracing::warn!(
                        host = %self.host,
                        error = %e,
                        "could not record host key in known_hosts"
                    ),
                }
                Ok(true)
            }
            // A previously recorded key changed — refuse; possible MITM.
            Err(russh::keys::Error::KeyChanged { .. }) => {
                tracing::warn!(
                    host = %self.host,
                    port = self.port,
                    "server key mismatch in known_hosts — possible MITM attack, refusing connection"
                );
                Ok(false)
            }
            // Unreadable or corrupt known_hosts — fail closed rather than
            // accept an unverified key.
            Err(e) => {
                tracing::warn!(
                    host = %self.host,
                    error = %e,
                    "known_hosts check failed; refusing connection"
                );
                Ok(false)
            }
        }
    }

    async fn disconnected(
        &mut self,
        reason: client::DisconnectReason<Self::Error>,
    ) -> Result<(), Self::Error> {
        match reason {
            client::DisconnectReason::ReceivedDisconnect(_) => {
                self.hung_up.store(true, Ordering::SeqCst);
                Ok(())
            }
            client::DisconnectReason::Error(e) => Err(e),
        }
    }
}

// ---------------------------------------------------------------------------
// Refused
// ---------------------------------------------------------------------------

/// A connection the server turned away on purpose: it refused every credential,
/// or its host key no longer matches `known_hosts`. A type of its own so a caller
/// that reconnects by itself can stop instead of piling up failed logins.
#[derive(Debug)]
pub(crate) struct Refused(String);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

/// Whether `e` is, or wraps, a refused connection.
pub(crate) fn is_refused(e: &anyhow::Error) -> bool {
    e.chain().any(|cause| {
        cause.is::<Refused>()
            || matches!(
                cause.downcast_ref::<russh::Error>(),
                Some(russh::Error::UnknownKey)
            )
    })
}

/// Prefixes a hop's error with where it failed. The message is flattened, as
/// before, but a refusal or a locked key stays recognisable through the jump
/// chain.
fn at_hop(e: anyhow::Error, context: String) -> anyhow::Error {
    let message = format!("{context}: {e:#}");
    if let Some(path) = passphrase_required(&e) {
        PassphraseRequired {
            path: path.to_owned(),
            message,
        }
        .into()
    } else if let Some(login) = password_required(&e) {
        PasswordRequired {
            login: login.to_owned(),
            message,
        }
        .into()
    } else if is_refused(&e) {
        Refused(message).into()
    } else {
        anyhow!(message)
    }
}

// ---------------------------------------------------------------------------
// PassphraseRequired
// ---------------------------------------------------------------------------

/// No credential got in, and an encrypted key was skipped for want of its
/// passphrase. Unlike [`Refused`] it is not final: once the key is unlocked
/// ([`crate::ssh::identity::unlock`]) the same login can succeed.
#[derive(Debug)]
pub(crate) struct PassphraseRequired {
    /// Canonical path of the encrypted key.
    path: String,
    message: String,
}

impl PassphraseRequired {
    fn new(path: String) -> Self {
        // The path goes last: frontends cut messages at the first ':', and a
        // Windows path has one.
        let message = format!("SSH key requires a passphrase: {path}");
        Self { path, message }
    }
}

impl fmt::Display for PassphraseRequired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PassphraseRequired {}

/// The encrypted key a failed connection is waiting on, if that is why it failed.
pub fn passphrase_required(e: &anyhow::Error) -> Option<&str> {
    e.chain()
        .find_map(|cause| cause.downcast_ref::<PassphraseRequired>())
        .map(|locked| locked.path.as_str())
}

// ---------------------------------------------------------------------------
// PasswordRequired
// ---------------------------------------------------------------------------

/// No key got in and there was no password to try. Not final either: once one
/// is typed for the login elsewhere ([`password::remembered`]) the same
/// connection can go again.
#[derive(Debug)]
pub(crate) struct PasswordRequired {
    /// The login key the password is remembered under.
    login: String,
    message: String,
}

impl fmt::Display for PasswordRequired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PasswordRequired {}

/// The login a failed connection needs a password for, if that is why it failed.
pub(crate) fn password_required(e: &anyhow::Error) -> Option<&str> {
    e.chain()
        .find_map(|cause| cause.downcast_ref::<PasswordRequired>())
        .map(|missing| missing.login.as_str())
}

// ---------------------------------------------------------------------------
// Passwords
// ---------------------------------------------------------------------------

/// Which login passwords a connection may use.
pub(crate) enum Passwords<'a> {
    /// The host's saved password and one typed for the login this session.
    /// Background work: pollers, tunnels that retry, snippets.
    Remembered,
    /// The host's saved password only. Key setup checks a new key this way; a
    /// remembered password would let a broken key pass.
    SavedOnly,
    /// As [`Passwords::Remembered`], then ask the user. A connection the user
    /// started and is watching.
    Ask(&'a mut dyn AskPassword),
}

/// How many passwords the user may type for one login before it fails.
const PASSWORD_PROMPTS: usize = 3;

/// The key a typed password is remembered under: the login plus every bastion
/// on the way, so a private address behind another bastion never gets it.
fn login_key(host: &Host, via: &[Host]) -> String {
    let login = |h: &Host| format!("{}@{}:{}", h.user, h.hostname, h.port);
    std::iter::once(login(host))
        .chain(via.iter().rev().map(login))
        .collect::<Vec<_>>()
        .join(" via ")
}

// ---------------------------------------------------------------------------
// SshConnection
// ---------------------------------------------------------------------------

/// An authenticated russh connection to one host, plus the jump-host
/// connections it is tunnelled through (empty for a direct connection).
///
/// The bastion handles are owned for the whole lifetime of the connection so
/// the chain outlives nothing it carries. Teardown runs the other way: the
/// target's session task holds the `direct-tcpip` stream of the hop below it,
/// so dropping this struct closes the target first and cascades outward.
/// Derefs to the target's [`Handle`], so callers open channels on it exactly as
/// before.
pub(crate) struct SshConnection {
    handle: Handle<KnownHostsHandler>,
    /// Bastions, nearest-first. Never used directly — kept alive by ownership.
    _jumps: Vec<Handle<KnownHostsHandler>>,
}

impl std::ops::Deref for SshConnection {
    type Target = Handle<KnownHostsHandler>;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

// ---------------------------------------------------------------------------
// SshSession
// ---------------------------------------------------------------------------

/// An authenticated SSH session ready for command execution.
///
/// Holds the russh client handle for the duration of its lifetime.
/// Drop → the connection is cleaned up by russh's internal tasks.
///
/// Wrapped in Arc to allow sharing across multiple operations (discovery + metrics).
#[derive(Clone)]
pub struct SshSession {
    handle: Arc<SshConnection>,
}

impl SshSession {
    /// Connect and authenticate to `host`.
    ///
    /// Authentication is attempted in order:
    /// 1. SSH agent (unix only, via `SSH_AUTH_SOCK`).
    /// 2. Identity file specified in the host config (`identity_file`).
    /// 3. Default key files (`~/.ssh/id_ed25519`, `id_rsa`, etc.).
    /// 4. Password: one typed for this login earlier in the session, then the
    ///    one in the host config.
    ///
    /// A host with a `ProxyJump` is reached through its bastion chain; each hop
    /// authenticates the same way.
    ///
    /// Returns an error when no method succeeds or the connection times out.
    ///
    /// # Errors
    /// - Connection timeout (> 10 s per hop)
    /// - Authentication failure
    /// - Network error
    /// - An unresolvable `ProxyJump` chain (cycle or too many hops)
    pub async fn connect(host: &Host) -> anyhow::Result<Self> {
        Self::connect_with(host, Passwords::Remembered).await
    }

    /// [`SshSession::connect`] with a say over which passwords it may use.
    pub(crate) async fn connect_with(
        host: &Host,
        passwords: Passwords<'_>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            handle: Arc::new(connect_and_auth(host, passwords).await?),
        })
    }

    /// Execute a shell command on the remote host and return its stdout.
    ///
    /// A new SSH channel is opened for each call so sessions can be
    /// reused across multiple commands. The remote exit status is ignored —
    /// use [`SshSession::run_command_checked`] when it carries the result.
    ///
    /// # Errors
    /// Returns an error on channel failure or if the command times out (30 s).
    pub async fn run_command(&self, cmd: &str) -> anyhow::Result<String> {
        Ok(self.exec(cmd).await?.0)
    }

    /// Like [`SshSession::run_command`] but returns an error when the remote
    /// command exits with a non-zero status. Use for `test`-style probes whose
    /// exit code is the answer (e.g. `sudo -n true`).
    ///
    /// # Errors
    /// As [`SshSession::run_command`], plus a non-zero remote exit status.
    pub async fn run_command_checked(&self, cmd: &str) -> anyhow::Result<String> {
        let (output, status) = self.exec(cmd).await?;
        match status {
            // A missing exit status is treated as success — failing a command
            // that likely worked is worse than missing a rare edge case.
            None | Some(0) => Ok(output),
            Some(code) => Err(anyhow!("remote command exited with status {code}")),
        }
    }

    /// Opens a channel, runs `cmd`, and returns its stdout and exit status.
    async fn exec(&self, cmd: &str) -> anyhow::Result<(String, Option<u32>)> {
        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .context("open SSH channel")?;

        channel.exec(true, cmd).await.context("exec SSH command")?;

        time::timeout(Duration::from_secs(30), collect_output(&mut channel))
            .await
            .map_err(|_| anyhow!("command timed out (30 s): {}", cmd))?
            .context("read command output")
    }

    /// Opens a new SSH channel, requests the SFTP subsystem, and returns the
    /// channel as an async stream suitable for [`russh_sftp::client::SftpSession::new`].
    ///
    /// The `SshSession` **must** remain alive for the entire lifetime of the
    /// SFTP session — dropping it closes the underlying TCP connection.
    ///
    /// # Errors
    /// Returns an error if the channel cannot be opened or if the server rejects
    /// the SFTP subsystem request.
    pub async fn open_sftp_channel(
        &self,
    ) -> anyhow::Result<russh::ChannelStream<russh::client::Msg>> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .context("open SFTP session channel")?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .context("request SFTP subsystem")?;
        Ok(channel.into_stream())
    }

    /// Gracefully close the SSH connection.
    pub async fn disconnect(self) {
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
    }
}

// ---------------------------------------------------------------------------
// Connection + authentication
// ---------------------------------------------------------------------------

/// Per-hop budget for the TCP connect and SSH handshake.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Connect to `host`, verify its host key, and authenticate — through the
/// host's `ProxyJump` chain when it has one.
///
/// Shared by [`SshSession::connect`] (metrics/SFTP) and the terminal so every
/// native SSH path honors the same keys, agent, passwords, known_hosts policy,
/// and bastions.
///
/// # Errors
/// Connection timeout (> 10 s per hop), host-key rejection, authentication
/// failure, or an unresolvable `ProxyJump` chain.
pub(crate) async fn connect_and_auth(
    host: &Host,
    mut passwords: Passwords<'_>,
) -> anyhow::Result<SshConnection> {
    let chain = jump_chain(host).await?;
    let config = client_config();

    // Walk the bastions outward: the first is reached directly, every later one
    // through its predecessor. The target then rides the last hop.
    let mut jumps: Vec<Handle<KnownHostsHandler>> = Vec::with_capacity(chain.len());
    for (i, hop) in chain.iter().enumerate() {
        let key = login_key(hop, &chain[..i]);
        let handle = match jumps.last() {
            None => connect_direct(&config, hop, &key, &mut passwords).await,
            Some(via) => connect_tunnelled(&config, via, hop, &key, &mut passwords).await,
        }
        .map_err(|e| at_hop(e, format!("ProxyJump via '{}' failed", hop.name)))?;
        jumps.push(handle);
    }

    let key = login_key(host, &chain);
    let handle = match (jumps.last(), chain.last()) {
        (Some(via), Some(last)) => connect_tunnelled(&config, via, host, &key, &mut passwords)
            .await
            .map_err(|e| at_hop(e, format!("connecting via '{}' failed", last.name)))?,
        _ => connect_direct(&config, host, &key, &mut passwords).await?,
    };

    Ok(SshConnection {
        handle,
        _jumps: jumps,
    })
}

/// Wall-clock budget one [`SshSession::connect`] needs for `host`: the per-hop
/// connect timeout and agent bound once for every bastion in its `ProxyJump`
/// chain, plus the target.
///
/// Callers that wrap the connect in a timeout of their own must scale it by
/// this — a fixed budget trips on a bastion chain before the connection has had
/// the time [`connect_and_auth`] is entitled to.
pub(crate) async fn connect_budget(host: &Host) -> Duration {
    // A chain that fails to resolve costs nothing to connect; the caller's own
    // attempt reports why.
    let hops = jump_chain(host).await.map_or(0, |chain| chain.len());
    // The agent's bound counts too: a caller that gives up mid-signature drops
    // russh while it waits for us.
    (CONNECT_TIMEOUT + AGENT_BUDGET) * (hops as u32 + 1)
}

/// The shared russh client configuration (timeouts + keepalives).
fn client_config() -> Arc<client::Config> {
    Arc::new(client::Config {
        // No inactivity timeout: russh skips resetting it on the iteration that
        // sends a keepalive, so a peer that never answers `keepalive@openssh.com`
        // (common in appliance SSH stacks) was torn down after 30 s even while
        // its commands still ran. Liveness stays bounded by `keepalive_max`.
        keepalive_interval: Some(Duration::from_secs(15)),
        keepalive_max: 3,
        ..Default::default()
    })
}

/// Resolves `host`'s `ProxyJump` into the hops to connect before it.
///
/// The jump aliases are looked up in the merged host list, so a bastion defined
/// elsewhere in `~/.ssh/config` (or in `hosts.toml`) contributes its own
/// HostName/User/Port/IdentityFile. Loading is skipped entirely for the common
/// no-`ProxyJump` case.
///
/// # Errors
/// An unreadable host list, or a chain that cannot be resolved. Both fail the
/// connection: resolving a bastion alias against nothing would fall back to
/// dialling the alias as a hostname, which is a different machine.
async fn jump_chain(host: &Host) -> anyhow::Result<Vec<Host>> {
    if crate::ssh::jump::jump_value(host).is_none() {
        return Ok(Vec::new());
    }
    // load_all_hosts() is blocking file I/O — keep it off the async worker.
    let known = tokio::task::spawn_blocking(crate::config::load_all_hosts)
        .await
        .context("host list load panicked")?
        .map_err(|e| anyhow!("could not load hosts for ProxyJump resolution: {e:#}"))?;

    let chain = crate::ssh::jump::resolve_chain(host, &known)?;
    tracing::debug!(
        host = %host.name,
        via = %chain.iter().map(|h| h.name.as_str()).collect::<Vec<_>>().join(" -> "),
        "resolved ProxyJump chain"
    );
    Ok(chain)
}

/// Opens a TCP connection to `host` and authenticates.
async fn connect_direct(
    config: &Arc<client::Config>,
    host: &Host,
    key: &str,
    passwords: &mut Passwords<'_>,
) -> anyhow::Result<Handle<KnownHostsHandler>> {
    let dial = || dial_direct(config, host);
    finish_auth(dial().await?, host, key, dial, passwords).await
}

/// Reaches `host` through the already-connected bastion `via`: a `direct-tcpip`
/// channel on the bastion carries a second SSH session to the target, which is
/// verified and authenticated in its own right.
async fn connect_tunnelled(
    config: &Arc<client::Config>,
    via: &Handle<KnownHostsHandler>,
    host: &Host,
    key: &str,
    passwords: &mut Passwords<'_>,
) -> anyhow::Result<Handle<KnownHostsHandler>> {
    let dial = || dial_tunnelled(config, via, host);
    finish_auth(dial().await?, host, key, dial, passwords).await
}

/// A connection that has shaken hands and passed the host-key check, not yet
/// authenticated.
struct Dialed {
    handle: Handle<KnownHostsHandler>,
    /// Set if the server ends the session with a DISCONNECT of its own.
    hung_up: Arc<AtomicBool>,
}

/// Opens a TCP connection to `host` and verifies its host key.
async fn dial_direct(config: &Arc<client::Config>, host: &Host) -> anyhow::Result<Dialed> {
    let addr = format!("{}:{}", host.hostname, host.port);
    let hung_up = Arc::new(AtomicBool::new(false));
    let handle = time::timeout(
        CONNECT_TIMEOUT,
        client::connect(
            Arc::clone(config),
            addr,
            known_hosts_handler(host, &hung_up),
        ),
    )
    .await
    .map_err(|_| anyhow!("SSH connection timed out (10 s)"))?
    .context("SSH connection failed")?;
    Ok(Dialed { handle, hung_up })
}

/// Opens a `direct-tcpip` channel to `host` on the bastion `via` and runs the
/// SSH handshake over it.
async fn dial_tunnelled(
    config: &Arc<client::Config>,
    via: &Handle<KnownHostsHandler>,
    host: &Host,
) -> anyhow::Result<Dialed> {
    // The originator address is informational; ssh(1) reports the loopback it
    // forwards from, and servers only log it.
    //
    // Timed out like the handshake it precedes: the bastion answers only once
    // its own connect() to the target resolves, so a firewalled target would
    // otherwise park the caller for the bastion's whole SYN budget.
    let channel = time::timeout(
        CONNECT_TIMEOUT,
        via.channel_open_direct_tcpip(host.hostname.clone(), host.port as u32, "127.0.0.1", 0),
    )
    .await
    .map_err(|_| anyhow!("SSH connection timed out (10 s)"))?
    .with_context(|| format!("open tunnel to {}:{}", host.hostname, host.port))?;

    let hung_up = Arc::new(AtomicBool::new(false));
    let handle = time::timeout(
        CONNECT_TIMEOUT,
        client::connect_stream(
            Arc::clone(config),
            channel.into_stream(),
            known_hosts_handler(host, &hung_up),
        ),
    )
    .await
    .map_err(|_| anyhow!("SSH connection timed out (10 s)"))?
    .context("SSH connection failed")?;
    Ok(Dialed { handle, hung_up })
}

/// The host-key verifier for `host`. The lookup uses the target's own
/// hostname/port even over a tunnel, so `known_hosts` entries match what an
/// `ssh -J` would record.
fn known_hosts_handler(host: &Host, hung_up: &Arc<AtomicBool>) -> KnownHostsHandler {
    KnownHostsHandler {
        host: host.hostname.clone(),
        port: host.port,
        hung_up: Arc::clone(hung_up),
    }
}

/// Authenticates the `first` connection as `host` (remembered passwords under
/// `key`), converting a refusal into an error. `dial` opens another connection
/// to the same hop, for the keyboard-interactive fallback.
async fn finish_auth<F, Fut>(
    first: Dialed,
    host: &Host,
    key: &str,
    dial: F,
    passwords: &mut Passwords<'_>,
) -> anyhow::Result<Handle<KnownHostsHandler>>
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<Dialed>>,
{
    let Dialed {
        mut handle,
        hung_up,
    } = first;
    let encrypted_key = match authenticate(&mut handle, host).await? {
        KeyAuth::Accepted => return Ok(handle),
        KeyAuth::Rejected { encrypted_key } => encrypted_key,
    };

    // The server login password, never a key passphrase, and tried last: keys
    // are what OmnySSH steers users towards. One typed this session goes first;
    // it is newer than any saved one.
    let typed = match passwords {
        Passwords::SavedOnly => None,
        _ => password::accepted(key),
    };
    let saved = host.password.clone().filter(|p| typed.as_ref() != Some(p));
    let known = typed.is_some() || saved.is_some();
    for (password, was_typed) in typed
        .map(|p| (p, true))
        .into_iter()
        .chain(saved.map(|p| (p, false)))
    {
        match try_password(&mut handle, &dial, host, &password).await {
            Offer::Here => return Ok(password_login(host, handle)),
            Offer::There(fresh) => return Ok(password_login(host, fresh)),
            Offer::No if was_typed => password::forget(key, &password),
            Offer::No => {}
        }
    }

    if let Some(path) = encrypted_key {
        return Err(PassphraseRequired::new(path).into());
    }

    if let Passwords::Ask(ask) = passwords {
        let login = format!("{}@{}", host.user, host.hostname);
        let mut retry = known;
        for _ in 0..PASSWORD_PROMPTS {
            let Some(password) = ask.ask(&login, retry).await else {
                return Err(Refused(format!("SSH login cancelled for {}", host.name)).into());
            };
            match try_password(&mut handle, &dial, host, &password).await {
                Offer::Here => {
                    password::remember(key, &password);
                    return Ok(password_login(host, handle));
                }
                Offer::There(fresh) => {
                    password::remember(key, &password);
                    return Ok(password_login(host, fresh));
                }
                Offer::No => retry = true,
            }
        }
        return Err(Refused(format!("SSH authentication failed for {}", host.name)).into());
    }

    if !known && !matches!(passwords, Passwords::SavedOnly) {
        return Err(PasswordRequired {
            login: key.to_string(),
            message: format!(
                "SSH authentication failed for {}: no key was accepted and no password is saved",
                host.name
            ),
        }
        .into());
    }

    let message = format!("SSH authentication failed for {}", host.name);
    // Every attempt folds a dropped link into "not accepted". A connection
    // that is gone refused us only if the server hung up itself, as OpenSSH
    // does after too many failed logins. russh records that as the session
    // winds down, so let it finish first.
    if handle.is_closed() {
        let _ = time::timeout(Duration::from_secs(1), &mut handle).await;
        if !hung_up.load(Ordering::SeqCst) {
            return Err(anyhow!(message));
        }
    }
    Err(Refused(message).into())
}

fn password_login(host: &Host, handle: Handle<KnownHostsHandler>) -> Handle<KnownHostsHandler> {
    tracing::info!(
        host = %host.name,
        "Connected via password authentication — consider setting up SSH key"
    );
    handle
}

/// Where an offered password got in, if anywhere.
enum Offer {
    /// On the connection it was offered on.
    Here,
    /// On a fresh connection, by keyboard-interactive.
    There(Handle<KnownHostsHandler>),
    No,
}

/// Offers `password` the way ssh(1) does: by the password method, then by
/// keyboard-interactive, which is all some servers take (UniFi consoles turn the
/// password method off). russh 0.46 answers keyboard-interactive only as the
/// first method of a connection, so that part runs on a fresh one. The same
/// holds when the server already hung up on the key attempts.
async fn try_password<F, Fut>(
    handle: &mut Handle<KnownHostsHandler>,
    dial: &F,
    host: &Host,
    password: &str,
) -> Offer
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<Dialed>>,
{
    let offered = !handle.is_closed();
    if offered && try_password_auth(handle, &host.user, password).await {
        return Offer::Here;
    }
    let mut fresh = match dial().await {
        Ok(dialed) => dialed.handle,
        Err(e) => {
            tracing::debug!(host = %host.name, error = %e, "keyboard-interactive dial failed");
            return Offer::No;
        }
    };
    if keyboard_interactive(&mut fresh, &host.user, password).await
        || (!offered && try_password_auth(&mut fresh, &host.user, password).await)
    {
        return Offer::There(fresh);
    }
    Offer::No
}

// ---------------------------------------------------------------------------
// Authentication helpers
// ---------------------------------------------------------------------------

enum KeyAuth {
    Accepted,
    /// No key got in; `encrypted_key` is one that was skipped for want of its
    /// passphrase.
    Rejected {
        encrypted_key: Option<String>,
    },
}

/// Tries the agent, the identity file and the default keys, in that order.
async fn authenticate(
    handle: &mut Handle<KnownHostsHandler>,
    host: &Host,
) -> anyhow::Result<KeyAuth> {
    let user = host.user.clone();
    let mut encrypted_key: Option<String> = None;

    // 1. Try SSH agent first — it handles passphrase-protected keys and is the
    //    most common auth method for non-interactive clients.
    #[cfg(unix)]
    {
        if try_agent_auth(handle, &user).await.unwrap_or(false) {
            return Ok(KeyAuth::Accepted);
        }
    }

    // 2. Try explicit identity_file from host config.
    if let Some(key_path) = &host.identity_file {
        match try_key_auth(handle, &user, key_path).await {
            Ok(true) => return Ok(KeyAuth::Accepted),
            Ok(false) => {}
            Err(e) => note_encrypted(&mut encrypted_key, e),
        }
    }

    // 3. Try default key files — mirrors what the `ssh` binary does when no
    //    -i flag is given. Skips files that don't exist. A locked one is worth a
    //    prompt only without an identity file: ssh(1) would not offer it then.
    for key_path in default_key_paths() {
        if key_path.exists() {
            let path_str = key_path.to_string_lossy().into_owned();
            match try_key_auth(handle, &user, &path_str).await {
                Ok(true) => return Ok(KeyAuth::Accepted),
                Ok(false) => {}
                Err(e) if host.identity_file.is_none() => note_encrypted(&mut encrypted_key, e),
                Err(_) => {}
            }
        }
    }

    Ok(KeyAuth::Rejected { encrypted_key })
}

fn note_encrypted(encrypted_key: &mut Option<String>, err: anyhow::Error) {
    match err.downcast_ref::<IdentityError>() {
        Some(IdentityError::Encrypted(path)) if encrypted_key.is_none() => {
            *encrypted_key = Some(path.clone());
        }
        _ => {
            tracing::debug!(error = %err, "public-key authentication attempt failed");
        }
    }
}

/// Returns the standard default SSH private key paths in priority order.
/// FIDO (`id_*_sk`) keys are left out: russh cannot sign with them, so a
/// locked one would ask for a passphrase that could never help.
fn default_key_paths() -> Vec<std::path::PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };
    let ssh = home.join(".ssh");
    ["id_ed25519", "id_rsa", "id_ecdsa"]
        .iter()
        .map(|name| ssh.join(name))
        .collect()
}

async fn try_key_auth(
    handle: &mut Handle<KnownHostsHandler>,
    user: &str,
    key_path: &str,
) -> anyhow::Result<bool> {
    // load_secret_key is synchronous (file I/O) — offload to blocking pool.
    let path = key_path.to_string();
    let key_pair = tokio::task::spawn_blocking(move || identity::load_key_pair(&path))
        .await
        .context("spawn_blocking panicked")??;

    let ok = handle
        .authenticate_publickey(user, Arc::new(key_pair))
        .await
        .context("authenticate_publickey")?;
    Ok(ok)
}

/// How long the SSH agent gets to answer the connect and the key listing. An
/// agent that accepts and never replies must not stall the whole login.
#[cfg(unix)]
const AGENT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long one agent signature may take. It can wait on the user (a confirm
/// dialog, Touch ID), so it gets longer than the listing.
#[cfg(unix)]
const SIGN_TIMEOUT: Duration = Duration::from_secs(15);

/// Upper bound on the agent's share of one hop's login, for callers that time
/// the whole connect.
const AGENT_BUDGET: Duration = Duration::from_secs(20);

#[cfg(unix)]
async fn try_agent_auth(
    handle: &mut Handle<KnownHostsHandler>,
    user: &str,
) -> anyhow::Result<bool> {
    use russh::keys::agent::client::AgentClient;

    let (agent, identities) = time::timeout(AGENT_TIMEOUT, async {
        let mut agent = AgentClient::connect_env()
            .await
            .context("connect to SSH agent")?;
        let identities = agent
            .request_identities()
            .await
            .context("request agent identities")?;
        Ok::<_, anyhow::Error>((agent, identities))
    })
    .await
    .map_err(|_| anyhow!("SSH agent did not answer"))??;

    let failed = Arc::new(tokio::sync::Notify::new());
    let mut signer = AgentSigner {
        agent: Some(agent),
        failed: Arc::clone(&failed),
    };
    for pubkey in identities {
        let attempt = handle.authenticate_future(user, pubkey, signer);
        tokio::pin!(attempt);
        let (back, result) = tokio::select! {
            biased;
            done = &mut attempt => done,
            () = failed.notified() => {
                // russh got the buffer back unsigned, sent nothing and now waits
                // for a reply that will not come. Let it finish handing the
                // buffer over, then leave the agent out of this login.
                let _ = time::timeout(Duration::from_millis(100), &mut attempt).await;
                tracing::debug!("SSH agent did not sign; trying other methods");
                return Ok(false);
            }
        };
        signer = back;
        if matches!(result, Ok(true)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Signs through the SSH agent without ever leaving russh waiting.
///
/// russh 0.46 treats a signer error as final for the connection: it keeps
/// waiting for the signature and swallows every later auth request, so one
/// refused or stalled signature hung the login. Handing the buffer back
/// unchanged makes russh send nothing and carry on, and [`try_agent_auth`] moves
/// on to the other methods.
#[cfg(unix)]
struct AgentSigner {
    agent: Option<russh::keys::agent::client::AgentClient<tokio::net::UnixStream>>,
    failed: Arc<tokio::sync::Notify>,
}

#[cfg(unix)]
impl russh::Signer for AgentSigner {
    type Error = russh::AgentAuthError;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = (Self, Result<russh::CryptoVec, Self::Error>)> + Send>,
    >;

    fn auth_publickey_sign(
        mut self,
        key: &russh::keys::key::PublicKey,
        to_sign: russh::CryptoVec,
    ) -> Self::Future {
        let key = key.clone();
        Box::pin(async move {
            let mut signed = None;
            if let Some(agent) = self.agent.take() {
                // A timed-out request leaves the agent connection mid-reply, so
                // it is dropped with the future.
                if let Ok((agent, result)) =
                    time::timeout(SIGN_TIMEOUT, agent.sign_request(&key, to_sign.clone())).await
                {
                    self.agent = Some(agent);
                    // An agent reply russh cannot read comes back unchanged.
                    signed = result.ok().filter(|data| data.len() != to_sign.len());
                }
            }
            match signed {
                Some(data) => (self, Ok(data)),
                None => {
                    self.failed.notify_one();
                    (self, Ok(to_sign))
                }
            }
        })
    }
}

/// Password-method login; a failed request counts as refused.
async fn try_password_auth(
    handle: &mut Handle<KnownHostsHandler>,
    user: &str,
    password: &str,
) -> bool {
    handle
        .authenticate_password(user, password)
        .await
        .unwrap_or(false)
}

/// Rounds of server prompts one keyboard-interactive login may take.
const KBD_ROUNDS: usize = 4;

/// How long each keyboard-interactive reply may take; a wrong password makes
/// PAM stall for a few seconds.
const KBD_TIMEOUT: Duration = Duration::from_secs(10);

/// Keyboard-interactive login that answers the server's password prompt with
/// `password`. Must be the first method on the connection (russh 0.46).
async fn keyboard_interactive(
    handle: &mut Handle<KnownHostsHandler>,
    user: &str,
    password: &str,
) -> bool {
    use russh::client::KeyboardInteractiveAuthResponse as Reply;

    let mut password = Some(password);
    let mut reply = time::timeout(
        KBD_TIMEOUT,
        handle.authenticate_keyboard_interactive_start(user, None),
    )
    .await;
    for _ in 0..KBD_ROUNDS {
        let prompts = match reply {
            Ok(Ok(Reply::Success)) => return true,
            Ok(Ok(Reply::InfoRequest { prompts, .. })) => prompts,
            _ => return false,
        };
        // Always answered, even when we cannot: russh waits for the answer and
        // swallows everything else until it has one.
        let answers = kbd_answers(&prompts, &mut password);
        reply = time::timeout(
            KBD_TIMEOUT,
            handle.authenticate_keyboard_interactive_respond(answers),
        )
        .await;
    }
    false
}

/// Answers to one round of keyboard-interactive prompts: the password goes, once,
/// to a lone hidden prompt; anything else (a code, a visible question) gets a
/// blank the server will refuse.
fn kbd_answers(prompts: &[russh::client::Prompt], password: &mut Option<&str>) -> Vec<String> {
    match prompts {
        [only] if !only.echo => match password.take() {
            Some(password) => vec![password.to_string()],
            None => vec![String::new()],
        },
        _ => vec![String::new(); prompts.len()],
    }
}

// ---------------------------------------------------------------------------
// Output collection
// ---------------------------------------------------------------------------

async fn collect_output(
    channel: &mut russh::Channel<russh::client::Msg>,
) -> anyhow::Result<(String, Option<u32>)> {
    let mut buf = Vec::new();
    let mut exit_status = None;
    loop {
        match channel.wait().await {
            Some(ChannelMsg::Data { ref data }) => {
                buf.extend_from_slice(data);
            }
            Some(ChannelMsg::ExtendedData { .. }) => {
                // stderr — discard to avoid corrupting stdout-only parser input
            }
            Some(ChannelMsg::ExitStatus { exit_status: code }) => {
                // Record it but keep reading: trailing stdout may still arrive
                // before the channel is closed.
                exit_status = Some(code);
            }
            Some(ChannelMsg::Eof) => {
                // Continue reading — ExitStatus may arrive after Eof.
            }
            Some(ChannelMsg::Close) | None => break,
            _ => {}
        }
    }
    // Use .lines() semantics: replace \r\n → \n for cross-platform safety.
    let raw = String::from_utf8_lossy(&buf);
    let normalised: String = raw.lines().flat_map(|l| [l, "\n"]).collect();
    Ok((normalised, exit_status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locked_key_stays_recognisable_through_a_jump_host() {
        let locked = anyhow::Error::from(PassphraseRequired::new(String::from("/k/id")));
        let hop = at_hop(locked, String::from("ProxyJump via 'bastion' failed"));
        assert_eq!(passphrase_required(&hop), Some("/k/id"));
        assert!(!is_refused(&hop));
        assert_eq!(
            hop.to_string(),
            "ProxyJump via 'bastion' failed: SSH key requires a passphrase: /k/id"
        );
    }

    fn prompt(text: &str, echo: bool) -> russh::client::Prompt {
        russh::client::Prompt {
            prompt: text.to_string(),
            echo,
        }
    }

    #[test]
    fn the_password_answers_one_hidden_prompt_only() {
        let mut password = Some("secret");
        let hidden = [prompt("Password: ", false)];
        assert_eq!(kbd_answers(&hidden, &mut password), ["secret"]);
        // A second ask (a code, a retry) must not get it again.
        assert_eq!(kbd_answers(&hidden, &mut password), [""]);
    }

    #[test]
    fn other_prompts_are_answered_blank() {
        let mut password = Some("secret");
        assert!(kbd_answers(&[], &mut password).is_empty());
        assert_eq!(
            kbd_answers(&[prompt("Username: ", true)], &mut password),
            [""]
        );
        let two = [prompt("Password: ", false), prompt("Code: ", false)];
        assert_eq!(kbd_answers(&two, &mut password), ["", ""]);
        assert_eq!(
            password,
            Some("secret"),
            "never spent on a prompt it did not answer"
        );
    }

    #[test]
    fn a_locked_key_is_found_under_added_context() {
        let e = anyhow::Error::from(PassphraseRequired::new(String::from("/k/id")))
            .context("SFTP SSH connect");
        assert_eq!(passphrase_required(&e), Some("/k/id"));
    }
}
