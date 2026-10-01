//! SFTP file manager operations.
//!
//! Provides [`SftpManager`] — a persistent background task that owns an SSH+SFTP
//! session and processes [`SftpCommand`] messages sent from the UI thread.
//!
//! All operations are non-blocking from the UI perspective.
//! Progress is reported via [`CoreEvent::FileTransferProgress`].

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use russh_sftp::protocol::{FileAttributes, FileType};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::time;

use crate::event::{CoreEvent, TransferId};
use crate::ssh::client::Host;
use crate::ssh::password::Prompter;
use crate::ssh::session::{Passwords, SshSession};

/// How long the SFTP channel and subsystem may take once logged in.
const OPEN_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// FileEntry — represents one file or directory in a panel listing
// ---------------------------------------------------------------------------

/// Metadata for a single file or directory in a file panel.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Base file name (not the full path).
    pub name: String,
    /// Absolute path string (used as the stable identifier for marked sets).
    pub path: String,
    /// File size in bytes (`0` for directories).
    pub size: u64,
    /// `true` when this entry is a directory.
    pub is_dir: bool,
    /// Last modification time, in seconds since the Unix epoch, when known.
    pub modified: Option<i64>,
}

impl FileEntry {
    /// The synthetic `..` entry pointing at `parent`.
    fn parent(parent: &str) -> Self {
        Self {
            name: "..".to_string(),
            path: parent.to_string(),
            size: 0,
            is_dir: true,
            modified: None,
        }
    }
}

/// Seconds since the Unix epoch for a file time, or `None` if it is unavailable.
fn unix_secs(time: std::io::Result<std::time::SystemTime>) -> Option<i64> {
    let time = time.ok()?;
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).ok(),
        Err(e) => i64::try_from(e.duration().as_secs()).ok().map(|s| -s),
    }
}

// ---------------------------------------------------------------------------
// SftpCommand — sent from UI thread → SftpManager background task
// ---------------------------------------------------------------------------

/// Commands processed by the [`SftpManager`] background task.
pub enum SftpCommand {
    /// List the entries in a remote directory.
    ListDir(String),
    /// Download a remote file, or a remote folder with everything in it, to a local
    /// path. An existing folder is merged into and an existing file replaced. Inside a
    /// folder, symlinks, special files and names this system cannot create are
    /// skipped; the transfer goes on past them and past failed files, and its
    /// [`CoreEvent::SftpOpDone`] error counts them.
    Download {
        remote: String,
        local: String,
        transfer_id: TransferId,
    },
    /// Upload a local file or folder to a remote path, by the rules of `Download`.
    Upload {
        local: String,
        remote: String,
        transfer_id: TransferId,
    },
    /// Delete a remote file (falls back to removing an empty directory).
    Delete(String),
    /// Create a remote directory.
    MkDir(String),
    /// Rename / move a remote path.
    Rename { from: String, to: String },
    /// Read the first 4 096 bytes of a remote file for preview.
    ReadPreview(String),
    /// Shut down the task gracefully.
    Disconnect,
}

// ---------------------------------------------------------------------------
// SftpManager — handle held by App to communicate with the background task
// ---------------------------------------------------------------------------

/// Manages a persistent SSH+SFTP background task.
///
/// Use [`SftpManager::connect`] to create, [`SftpManager::send`] to enqueue
/// commands, and [`SftpManager::disconnect`] for a clean shutdown.
#[derive(Debug)]
pub struct SftpManager {
    cmd_tx: mpsc::Sender<SftpCommand>,
}

impl SftpManager {
    /// Connects to `host` via SSH + SFTP subsystem and spawns the background task.
    /// A login the keys do not get into asks for the password through `prompter`.
    ///
    /// On success sends [`CoreEvent::SftpConnected`] through `event_tx`.
    /// On failure the task sends [`CoreEvent::SftpDisconnected`].
    ///
    /// # Errors
    /// Returns an error if the SSH connection fails before the task is spawned,
    /// including a cancelled password prompt.
    pub async fn connect(
        host: &Host,
        event_tx: mpsc::Sender<CoreEvent>,
        mut prompter: Prompter,
    ) -> anyhow::Result<Self> {
        let session = SshSession::connect_with(host, Passwords::Ask(&mut prompter))
            .await
            .context("SFTP SSH connect")?;
        // The login is bounded step by step; the channel must not hang either.
        let sftp = time::timeout(OPEN_TIMEOUT, async {
            let stream = session
                .open_sftp_channel()
                .await
                .context("open SFTP channel")?;
            russh_sftp::client::SftpSession::new(stream)
                .await
                .context("create SFTP session")
        })
        .await
        .map_err(|_| anyhow::anyhow!("SFTP did not start within {}s", OPEN_TIMEOUT.as_secs()))??;

        let (cmd_tx, cmd_rx) = mpsc::channel::<SftpCommand>(64);
        let host_name = host.name.clone();

        // `session` and `sftp` are owned by this async block.  If the task
        // panics, Rust's unwind machinery calls their Drop impls before the
        // panic propagates to tokio — the TCP connection is therefore always
        // released even in the panic path.  No explicit catch_unwind needed.
        tokio::spawn(async move {
            let _ = event_tx
                .send(CoreEvent::SftpConnected {
                    host_name: host_name.clone(),
                })
                .await;
            sftp_task_loop(session, sftp, cmd_rx, event_tx.clone()).await;
            tracing::info!("SFTP task for '{}' exited", host_name);
        });

        Ok(Self { cmd_tx })
    }

    /// Enqueues a command (fire-and-forget). Silently drops if the task exited.
    pub fn send(&self, cmd: SftpCommand) {
        let _ = self.cmd_tx.try_send(cmd);
    }

    /// Sends [`SftpCommand::Disconnect`] and drops the sender.
    pub fn disconnect(self) {
        let _ = self.cmd_tx.try_send(SftpCommand::Disconnect);
    }
}

// ---------------------------------------------------------------------------
// Background task loop
// ---------------------------------------------------------------------------

async fn sftp_task_loop(
    _ssh: SshSession, // kept alive to hold the SSH connection open
    sftp: russh_sftp::client::SftpSession,
    mut cmd_rx: mpsc::Receiver<SftpCommand>,
    event_tx: mpsc::Sender<CoreEvent>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            SftpCommand::ListDir(path) => match do_list_dir(&sftp, &path).await {
                Ok(entries) => {
                    let _ = event_tx
                        .send(CoreEvent::FileDirListed { path, entries })
                        .await;
                }
                Err(e) => {
                    let _ = event_tx
                        .send(CoreEvent::SftpDisconnected {
                            reason: format!("ListDir failed: {e:#}"),
                        })
                        .await;
                }
            },

            SftpCommand::Download {
                remote,
                local,
                transfer_id,
            } => {
                let result = do_download(&sftp, &remote, &local, transfer_id, &event_tx)
                    .await
                    .map_err(|e| format!("{e:#}"));
                let _ = event_tx.send(CoreEvent::SftpOpDone { result }).await;
            }

            SftpCommand::Upload {
                local,
                remote,
                transfer_id,
            } => {
                let result = do_upload(&local, &sftp, &remote, transfer_id, &event_tx)
                    .await
                    .map_err(|e| format!("{e:#}"));
                let _ = event_tx.send(CoreEvent::SftpOpDone { result }).await;
            }

            SftpCommand::Delete(path) => {
                // Try remove_file first; on failure try remove_dir (empty dirs only).
                let result = match sftp.remove_file(&path).await {
                    Ok(()) => Ok(()),
                    Err(_) => sftp.remove_dir(&path).await.map_err(|e| e.to_string()),
                };
                let _ = event_tx.send(CoreEvent::SftpOpDone { result }).await;
            }

            SftpCommand::MkDir(path) => {
                let result = sftp.create_dir(&path).await.map_err(|e| e.to_string());
                let _ = event_tx.send(CoreEvent::SftpOpDone { result }).await;
            }

            SftpCommand::Rename { from, to } => {
                let result = sftp.rename(&from, &to).await.map_err(|e| e.to_string());
                let _ = event_tx.send(CoreEvent::SftpOpDone { result }).await;
            }

            SftpCommand::ReadPreview(path) => {
                if let Ok(content) = do_read_preview(&sftp, &path).await {
                    let _ = event_tx
                        .send(CoreEvent::FilePreviewReady { path, content })
                        .await;
                }
            }

            SftpCommand::Disconnect => break,
        }
    }
}

// ---------------------------------------------------------------------------
// SFTP helpers
// ---------------------------------------------------------------------------

async fn do_list_dir(
    sftp: &russh_sftp::client::SftpSession,
    path: &str,
) -> anyhow::Result<Vec<FileEntry>> {
    let read_dir = sftp
        .read_dir(path)
        .await
        .with_context(|| format!("read remote dir '{path}'"))?;

    let mut entries: Vec<FileEntry> = Vec::new();

    // ".." parent entry (omit at root "/")
    if let Some(parent) = std::path::Path::new(path).parent() {
        let parent_str = parent.to_string_lossy();
        let parent_str = if parent_str.is_empty() {
            "/"
        } else {
            &parent_str
        };
        entries.push(FileEntry::parent(parent_str));
    }

    for entry in read_dir {
        let name = entry.file_name();
        let ft = entry.file_type();
        let meta = entry.metadata();

        let full_path = join_remote(path, &name);

        entries.push(FileEntry {
            name,
            path: full_path,
            size: meta.size.unwrap_or(0),
            is_dir: ft.is_dir(),
            modified: meta.mtime.map(i64::from),
        });
    }

    // Sort: ".." first, then dirs, then files — all alphabetically.
    entries.sort_by(|a, b| {
        if a.name == ".." {
            return std::cmp::Ordering::Less;
        }
        if b.name == ".." {
            return std::cmp::Ordering::Greater;
        }
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    Ok(entries)
}

/// `name` joined onto the remote directory `dir`.
fn join_remote(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// Whether a name the server gave can be created locally as itself: one plain path
/// component, so it neither climbs out of the destination nor, as Windows' `C:x` does,
/// replaces it.
fn is_safe_name(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    let single = matches!(
        (parts.next(), parts.next()),
        (Some(std::path::Component::Normal(part)), None) if part == name
    );
    single && !name.contains('\0') && (!cfg!(windows) || is_windows_name(name))
}

/// Windows' own rules: no `:` (drives, alternate streams) or other reserved character,
/// no trailing dot or space (Win32 drops them, so `a.` would overwrite `a`), and no
/// device name, with or without an extension (`aux.c` opens the AUX device).
fn is_windows_name(name: &str) -> bool {
    if name.chars().any(|c| c < ' ' || "<>:\"/\\|?*".contains(c)) || name.ends_with(['.', ' ']) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end_matches(' ');
    let upper = stem.to_ascii_uppercase();
    let device = match upper.get(..3) {
        Some("CON" | "PRN" | "AUX" | "NUL") => upper.len() == 3,
        Some("COM" | "LPT") => {
            let port = &upper[3..];
            port.chars().count() == 1 && "0123456789\u{b9}\u{b2}\u{b3}".contains(port)
        }
        _ => false,
    };
    !device
}

/// `name` from the server joined onto the local folder `dir`, unless it would be
/// anything but a direct child of `dir`.
fn join_local(dir: &Path, name: &str) -> Option<PathBuf> {
    let path = dir.join(name);
    (is_safe_name(name) && path.parent() == Some(dir)).then_some(path)
}

/// Rejects a download destination whose own name could not be created as itself: the
/// frontends build it from the name the server listed.
fn check_local_name(local: &str) -> anyhow::Result<()> {
    let name = Path::new(local).file_name().and_then(|n| n.to_str());
    if !name.is_some_and(is_safe_name) {
        anyhow::bail!("'{local}' is not a name that can be created here");
    }
    Ok(())
}

/// Rejects a transfer whose local path climbs out with `..` or either path holds a
/// null byte.
fn check_paths(local: &str, remote: &str) -> anyhow::Result<()> {
    if Path::new(local)
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        anyhow::bail!("Local path contains '..': {local}");
    }
    if local.contains('\0') || remote.contains('\0') {
        anyhow::bail!("Path contains null bytes");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Transfers
// ---------------------------------------------------------------------------

/// How deep a folder transfer goes: past any real tree, short of one a server makes
/// up to never end.
const MAX_DEPTH: usize = 64;

/// What a folder transfer left out. It goes on past every skip and failure and
/// reports them in its one result.
#[derive(Debug, Default)]
struct Skipped {
    count: usize,
    first: Option<String>,
}

impl Skipped {
    fn add(&mut self, path: &str, reason: impl std::fmt::Display) {
        self.count += 1;
        self.first
            .get_or_insert_with(|| format!("'{path}': {reason}"));
    }

    fn into_result(self) -> anyhow::Result<()> {
        let Some(first) = self.first else {
            return Ok(());
        };
        let items = if self.count == 1 { "item" } else { "items" };
        anyhow::bail!("{} {items} skipped or failed, first {first}", self.count)
    }
}

/// One folder or file of a folder transfer.
#[derive(Debug, PartialEq)]
struct Planned {
    src: String,
    dst: String,
    size: u64,
}

/// Everything a folder transfer creates, worked out before the first byte moves so
/// the progress bar covers the whole folder. `dirs` lists parents before children.
#[derive(Debug, Default)]
struct TreePlan {
    dirs: Vec<Planned>,
    files: Vec<Planned>,
}

impl TreePlan {
    fn total(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}

/// Byte progress across every file of one transfer.
struct Progress<'a> {
    transfer_id: TransferId,
    done: u64,
    total: u64,
    event_tx: &'a mpsc::Sender<CoreEvent>,
}

impl<'a> Progress<'a> {
    fn new(transfer_id: TransferId, total: u64, event_tx: &'a mpsc::Sender<CoreEvent>) -> Self {
        Self {
            transfer_id,
            done: 0,
            total,
            event_tx,
        }
    }

    async fn advance(&mut self, n: usize) {
        self.done += n as u64;
        let _ = self
            .event_tx
            .send(CoreEvent::FileTransferProgress(
                self.transfer_id,
                self.done,
                self.total,
            ))
            .await;
    }
}

/// Walks the local folder `root`, mapping it onto the remote folder `dst_root`.
/// Nothing is followed: a symlink is skipped, as is anything else that is not a
/// plain file or folder.
async fn plan_local_tree(
    root: &str,
    dst_root: &str,
    skipped: &mut Skipped,
) -> anyhow::Result<TreePlan> {
    let mut plan = TreePlan::default();
    let mut stack = vec![(PathBuf::from(root), dst_root.to_string(), 0)];
    while let Some((src, dst, depth)) = stack.pop() {
        // Every name on the way is UTF-8, so the path converts losslessly.
        let src_str = src.to_string_lossy().into_owned();
        let mut read_dir = match tokio::fs::read_dir(&src).await {
            Ok(read_dir) => read_dir,
            Err(e) => {
                skipped.add(&src_str, format_args!("read local dir: {e}"));
                continue;
            }
        };
        plan.dirs.push(Planned {
            src: src_str.clone(),
            dst: dst.clone(),
            size: 0,
        });
        loop {
            let entry = match read_dir.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(e) => {
                    skipped.add(&src_str, format_args!("read local dir: {e}"));
                    break;
                }
            };
            let path = entry.path();
            let shown = path.to_string_lossy();
            let Ok(name) = entry.file_name().into_string() else {
                skipped.add(&shown, "name is not valid UTF-8");
                continue;
            };
            // `DirEntry::metadata` does not follow symlinks.
            let meta = match entry.metadata().await {
                Ok(meta) => meta,
                Err(e) => {
                    skipped.add(&shown, e);
                    continue;
                }
            };
            let remote = join_remote(&dst, &name);
            let file_type = meta.file_type();
            if file_type.is_dir() && depth < MAX_DEPTH {
                stack.push((path.clone(), remote, depth + 1));
            } else if file_type.is_dir() {
                skipped.add(&shown, "nested too deep");
            } else if file_type.is_file() {
                plan.files.push(Planned {
                    src: shown.into_owned(),
                    dst: remote,
                    size: meta.len(),
                });
            } else if file_type.is_symlink() {
                skipped.add(&shown, "symbolic link, not followed");
            } else {
                skipped.add(&shown, "not a regular file or folder");
            }
        }
    }
    Ok(plan)
}

/// Walks the remote folder `root`, mapping it onto the local folder `dst_root`;
/// `list` reads one remote folder. A symlink is skipped (servers report links, not
/// their targets), as is anything that is not a plain file or folder and any name
/// that cannot be created here.
async fn plan_remote_tree<F, Fut>(
    mut list: F,
    root: &str,
    dst_root: &Path,
    skipped: &mut Skipped,
) -> anyhow::Result<TreePlan>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = anyhow::Result<Vec<(String, FileAttributes)>>>,
{
    let mut plan = TreePlan::default();
    let mut stack = vec![(root.to_string(), dst_root.to_path_buf(), 0)];
    while let Some((src, dst, depth)) = stack.pop() {
        let entries = match list(src.clone()).await {
            Ok(entries) => entries,
            Err(e) => {
                skipped.add(&src, format_args!("read remote dir: {e:#}"));
                continue;
            }
        };
        plan.dirs.push(Planned {
            src: src.clone(),
            dst: dst.to_string_lossy().into_owned(),
            size: 0,
        });
        for (name, attrs) in entries {
            let path = join_remote(&src, &name);
            let Some(local) = join_local(&dst, &name) else {
                skipped.add(&path, "name not allowed here");
                continue;
            };
            match attrs.file_type() {
                FileType::Dir if depth < MAX_DEPTH => {
                    stack.push((path, local, depth + 1));
                }
                FileType::Dir => skipped.add(&path, "nested too deep"),
                FileType::File => plan.files.push(Planned {
                    src: path,
                    dst: local.to_string_lossy().into_owned(),
                    size: attrs.size.unwrap_or(0),
                }),
                FileType::Symlink => skipped.add(&path, "symbolic link, not followed"),
                FileType::Other => skipped.add(&path, "not a regular file or folder"),
            }
        }
    }
    Ok(plan)
}

/// Downloads a remote file or, recursively, a remote folder to `local`.
async fn do_download(
    sftp: &russh_sftp::client::SftpSession,
    remote: &str,
    local: &str,
    transfer_id: TransferId,
    event_tx: &mpsc::Sender<CoreEvent>,
) -> anyhow::Result<()> {
    check_paths(local, remote)?;
    check_local_name(local)?;

    // Size and type best-effort: a failed stat downloads as a file, and the open below
    // reports why. Without permissions the type is unknown too.
    let meta = sftp.metadata(remote).await.ok();
    match meta
        .as_ref()
        .map(|m| (m.permissions.is_some(), m.file_type()))
    {
        Some((_, FileType::Dir)) => {}
        Some((true, FileType::Symlink | FileType::Other)) => {
            anyhow::bail!("'{remote}' is not a regular file or folder")
        }
        _ => {
            let total = meta.and_then(|m| m.size).unwrap_or(0);
            let mut progress = Progress::new(transfer_id, total, event_tx);
            return download_file(sftp, remote, local, &mut progress).await;
        }
    }

    let mut skipped = Skipped::default();
    let list = |dir: String| async move {
        let entries = sftp.read_dir(dir).await?;
        anyhow::Ok(entries.map(|e| (e.file_name(), e.metadata())).collect())
    };
    let plan = plan_remote_tree(list, remote, Path::new(local), &mut skipped).await?;
    let mut progress = Progress::new(transfer_id, plan.total(), event_tx);
    for dir in &plan.dirs {
        if let Err(e) = tokio::fs::create_dir_all(&dir.dst).await {
            skipped.add(&dir.src, format_args!("create local dir: {e}"));
        }
    }
    for file in &plan.files {
        if let Err(e) = download_file(sftp, &file.src, &file.dst, &mut progress).await {
            skipped.add(&file.src, format_args!("{e:#}"));
        }
    }
    skipped.into_result()
}

async fn download_file(
    sftp: &russh_sftp::client::SftpSession,
    remote: &str,
    local: &str,
    progress: &mut Progress<'_>,
) -> anyhow::Result<()> {
    let mut remote_file = sftp
        .open(remote)
        .await
        .context("open remote file for download")?;
    let mut local_file = tokio::fs::File::create(local)
        .await
        .context("create local file")?;

    let mut buf = vec![0u8; 65_536];
    loop {
        let n = remote_file
            .read(&mut buf)
            .await
            .context("read remote file")?;
        if n == 0 {
            break;
        }
        local_file
            .write_all(&buf[..n])
            .await
            .context("write local file")?;
        progress.advance(n).await;
    }

    // The last write may still be in flight; this is where it fails, if it does.
    local_file.flush().await.context("write local file")
}

/// Uploads a local file or, recursively, a local folder to `remote`.
async fn do_upload(
    local: &str,
    sftp: &russh_sftp::client::SftpSession,
    remote: &str,
    transfer_id: TransferId,
    event_tx: &mpsc::Sender<CoreEvent>,
) -> anyhow::Result<()> {
    check_paths(local, remote)?;

    // Follows a symlink: a linked folder the user picked is uploaded as a folder.
    let meta = tokio::fs::metadata(local)
        .await
        .context("open local file for upload")?;
    if meta.is_file() {
        let mut progress = Progress::new(transfer_id, meta.len(), event_tx);
        return upload_file(local, sftp, remote, &mut progress).await;
    }
    if !meta.is_dir() {
        anyhow::bail!("'{local}' is not a regular file or folder");
    }

    let mut skipped = Skipped::default();
    let plan = plan_local_tree(local, remote, &mut skipped).await?;
    let mut progress = Progress::new(transfer_id, plan.total(), event_tx);
    for dir in &plan.dirs {
        if let Err(e) = create_remote_dir(sftp, &dir.dst).await {
            skipped.add(&dir.src, format_args!("{e:#}"));
        }
    }
    for file in &plan.files {
        if let Err(e) = upload_file(&file.src, sftp, &file.dst, &mut progress).await {
            skipped.add(&file.src, format_args!("{e:#}"));
        }
    }
    skipped.into_result()
}

/// Creates the remote folder `path`, or merges into the one there.
async fn create_remote_dir(
    sftp: &russh_sftp::client::SftpSession,
    path: &str,
) -> anyhow::Result<()> {
    if let Err(e) = sftp.create_dir(path).await {
        if sftp
            .metadata(path)
            .await
            .is_ok_and(|m| m.file_type().is_dir())
        {
            return Ok(());
        }
        return Err(e).context("create remote dir");
    }
    Ok(())
}

async fn upload_file(
    local: &str,
    sftp: &russh_sftp::client::SftpSession,
    remote: &str,
    progress: &mut Progress<'_>,
) -> anyhow::Result<()> {
    let mut local_file = tokio::fs::File::open(local)
        .await
        .context("open local file for upload")?;
    let mut remote_file = sftp
        .create(remote)
        .await
        .context("create remote file for upload")?;

    let mut buf = vec![0u8; 65_536];
    loop {
        let n = local_file.read(&mut buf).await.context("read local file")?;
        if n == 0 {
            break;
        }
        remote_file
            .write_all(&buf[..n])
            .await
            .context("write remote file")?;
        progress.advance(n).await;
    }

    Ok(())
}

async fn do_read_preview(
    sftp: &russh_sftp::client::SftpSession,
    path: &str,
) -> anyhow::Result<String> {
    let mut file = sftp.open(path).await.context("open for preview")?;
    let mut buf = vec![0u8; 4_096];
    let n = file.read(&mut buf).await.context("read preview bytes")?;
    buf.truncate(n);
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

// ---------------------------------------------------------------------------
// Local filesystem helpers (called via inline tokio::spawn in App)
// ---------------------------------------------------------------------------

/// Lists the entries of a local directory, sorted dirs-first then alphabetically.
///
/// Prepends a `".."` entry for the parent directory (omitted at filesystem root).
///
/// # Errors
/// Returns an error if the directory cannot be read (e.g. permission denied).
pub async fn list_local_dir(path: &str) -> anyhow::Result<Vec<FileEntry>> {
    let mut read_dir = tokio::fs::read_dir(path)
        .await
        .with_context(|| format!("read local dir '{path}'"))?;

    let mut entries: Vec<FileEntry> = Vec::new();

    // ".." parent entry.
    if let Some(parent) = std::path::Path::new(path).parent() {
        let parent_str = parent.to_string_lossy();
        let parent_str = if parent_str.is_empty() {
            "/"
        } else {
            &parent_str
        };
        entries.push(FileEntry::parent(parent_str));
    }

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .context("read local dir entry")?
    {
        let file_type = entry.file_type().await.ok();
        let is_dir = file_type.as_ref().map(|ft| ft.is_dir()).unwrap_or(false);
        let meta = entry.metadata().await.ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);

        let name = entry.file_name().to_string_lossy().into_owned();
        let path_str = entry.path().to_string_lossy().into_owned();

        entries.push(FileEntry {
            name,
            path: path_str,
            size,
            is_dir,
            modified: meta.and_then(|m| unix_secs(m.modified())),
        });
    }

    // Sort: ".." first, then dirs, then files — case-insensitive alphabetically.
    entries.sort_by(|a, b| {
        if a.name == ".." {
            return std::cmp::Ordering::Less;
        }
        if b.name == ".." {
            return std::cmp::Ordering::Greater;
        }
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    Ok(entries)
}

/// The roots the local file system can be browsed from: every drive letter on
/// Windows, where `..` stops at the drive the pane is on, and `/` elsewhere.
pub fn local_roots() -> Vec<String> {
    #[cfg(windows)]
    {
        // SAFETY: GetLogicalDrives takes no arguments and only returns a bitmask.
        let mask = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        drive_roots(mask)
    }
    #[cfg(not(windows))]
    {
        vec!["/".to_string()]
    }
}

/// `C:\`-style roots for the drives set in `mask` (bit 0 is `A:`).
#[cfg_attr(not(windows), allow(dead_code))]
fn drive_roots(mask: u32) -> Vec<String> {
    (b'A'..=b'Z')
        .enumerate()
        .filter(|(bit, _)| mask & (1 << bit) != 0)
        .map(|(_, letter)| format!("{}:\\", letter as char))
        .collect()
}

/// Reads up to 4 096 bytes from a local file and returns them as a UTF-8 string.
///
/// Non-UTF-8 bytes are replaced with the Unicode replacement character.
///
/// # Errors
/// Returns an error if the file cannot be opened or read.
pub async fn preview_local_file(path: &str) -> anyhow::Result<String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .context("open local file for preview")?;
    let mut buf = vec![0u8; 4_096];
    let n = file
        .read(&mut buf)
        .await
        .context("read local preview bytes")?;
    buf.truncate(n);
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_roots_follow_the_mask() {
        assert_eq!(drive_roots(0b1100), ["C:\\", "D:\\"]);
        assert_eq!(drive_roots(1 | 1 << 25), ["A:\\", "Z:\\"]);
        assert!(drive_roots(0).is_empty());
    }

    fn attrs(permissions: u32, size: u64) -> FileAttributes {
        FileAttributes {
            permissions: Some(permissions),
            size: Some(size),
            ..FileAttributes::empty()
        }
    }

    #[tokio::test]
    async fn local_listing_carries_modification_time() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("a.txt"), b"hello").expect("write scratch file");

        let entries = list_local_dir(&dir.path().to_string_lossy())
            .await
            .expect("list scratch dir");
        let file = entries
            .iter()
            .find(|e| e.name == "a.txt")
            .expect("file listed");
        assert_eq!(file.size, 5);
        assert!(!file.is_dir);
        assert!(file.modified.is_some_and(|t| t > 0));
        let parent = entries.first().expect("parent entry");
        assert_eq!(parent.name, "..");
        assert!(parent.modified.is_none());
    }

    #[tokio::test]
    async fn the_local_plan_maps_files_and_folders_and_reports_the_rest() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("sub").join("deep")).expect("create scratch tree");
        std::fs::write(dir.join("a.txt"), b"abc").expect("write scratch file");
        std::fs::write(dir.join("sub").join("deep").join("b.bin"), b"hello")
            .expect("write scratch file");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir, dir.join("sub").join("loop")).expect("symlink");
            std::os::unix::net::UnixListener::bind(dir.join("sock")).expect("socket");
        }

        let mut skipped = Skipped::default();
        let root = dir.to_str().expect("utf-8 temp dir");
        let plan = plan_local_tree(root, "/srv/up", &mut skipped)
            .await
            .expect("plan tree");
        assert_eq!(plan.dirs[0].dst, "/srv/up");
        let mut dirs: Vec<_> = plan.dirs.iter().map(|d| d.dst.as_str()).collect();
        dirs.sort();
        assert_eq!(dirs, ["/srv/up", "/srv/up/sub", "/srv/up/sub/deep"]);
        // Parents come before their children, so creating in order never fails.
        let sub = plan.dirs.iter().position(|d| d.dst == "/srv/up/sub");
        let deep = plan.dirs.iter().position(|d| d.dst == "/srv/up/sub/deep");
        assert!(sub < deep);
        let mut files: Vec<_> = plan
            .files
            .iter()
            .map(|f| (f.dst.as_str(), f.size))
            .collect();
        files.sort();
        assert_eq!(files, [("/srv/up/a.txt", 3), ("/srv/up/sub/deep/b.bin", 5)]);
        assert_eq!(plan.total(), 8);
        // The link back up and the socket are left out, and said so.
        assert_eq!(skipped.count, if cfg!(unix) { 2 } else { 0 });
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn a_local_name_that_is_not_utf8_is_reported_not_fatal() {
        use std::os::unix::ffi::OsStrExt;
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("ok.txt"), b"ok").expect("write");
        let bad = std::ffi::OsStr::from_bytes(b"bad\xff.txt");
        std::fs::write(tmp.path().join(bad), b"bad").expect("write");

        let mut skipped = Skipped::default();
        let root = tmp.path().to_str().expect("utf-8 temp dir");
        let plan = plan_local_tree(root, "/up", &mut skipped)
            .await
            .expect("plan tree");
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].dst, "/up/ok.txt");
        let err = skipped.into_result().expect_err("the bad name is reported");
        assert!(
            err.to_string().ends_with("': name is not valid UTF-8"),
            "{err}"
        );
    }

    /// A remote file system: folder -> listing. A folder not in it cannot be read.
    fn fake_server(
        tree: Vec<(&'static str, Vec<(&'static str, FileAttributes)>)>,
    ) -> impl FnMut(String) -> std::future::Ready<anyhow::Result<Vec<(String, FileAttributes)>>>
    {
        move |dir| {
            let listing = tree
                .iter()
                .find(|(path, _)| *path == dir)
                .map(|(_, entries)| {
                    entries
                        .iter()
                        .map(|(name, attrs)| (name.to_string(), attrs.clone()))
                        .collect()
                });
            std::future::ready(listing.ok_or_else(|| anyhow::anyhow!("Permission denied")))
        }
    }

    #[tokio::test]
    async fn the_remote_plan_maps_a_tree_and_reports_what_it_leaves_out() {
        let server = fake_server(vec![
            (
                "/srv/app",
                vec![
                    ("run.sh", attrs(0o100_755, 3)),
                    ("conf", attrs(0o40_700, 0)),
                    ("current", attrs(0o120_777, 0)),
                    ("fifo", attrs(0o10_644, 0)),
                    ("a/b", attrs(0o100_644, 1)),
                ],
            ),
            (
                "/srv/app/conf",
                vec![
                    ("app.toml", attrs(0o100_600, 5)),
                    ("locked", attrs(0o40_000, 0)),
                ],
            ),
        ]);
        let dst = Path::new("dl").join("app");
        let mut skipped = Skipped::default();
        let plan = plan_remote_tree(server, "/srv/app", &dst, &mut skipped)
            .await
            .expect("plan tree");

        let local = |rel: &[&str]| {
            rel.iter()
                .fold(dst.clone(), |p, part| p.join(part))
                .to_string_lossy()
                .into_owned()
        };
        let dirs: Vec<_> = plan.dirs.iter().map(|d| d.dst.clone()).collect();
        assert_eq!(dirs, [local(&[]), local(&["conf"])]);
        assert_eq!(
            plan.files,
            [
                Planned {
                    src: "/srv/app/run.sh".into(),
                    dst: local(&["run.sh"]),
                    size: 3,
                },
                Planned {
                    src: "/srv/app/conf/app.toml".into(),
                    dst: local(&["conf", "app.toml"]),
                    size: 5,
                },
            ]
        );
        // The symlink, the FIFO, the name with a separator and the unreadable folder:
        // left out, counted, and the first one named.
        assert_eq!(skipped.count, 4);
        let err = skipped.into_result().expect_err("skips are reported");
        assert_eq!(
            err.to_string(),
            "4 items skipped or failed, first '/srv/app/current': symbolic link, not followed"
        );
    }

    #[tokio::test]
    async fn the_remote_plan_stops_at_the_depth_cap() {
        // A server that makes up one more folder at every level.
        let server = |_dir: String| {
            std::future::ready(anyhow::Ok(vec![("d".to_string(), attrs(0o40_755, 0))]))
        };
        let mut skipped = Skipped::default();
        let plan = plan_remote_tree(server, "/x", Path::new("dl"), &mut skipped)
            .await
            .expect("plan tree");
        assert_eq!(plan.dirs.len(), MAX_DEPTH + 1);
        let err = skipped.into_result().expect_err("the cut is reported");
        assert!(err
            .to_string()
            .starts_with("1 item skipped or failed, first '/x/d/d/"));
        assert!(err.to_string().ends_with("/d': nested too deep"), "{err}");
    }

    #[test]
    fn skips_are_counted_and_the_first_reason_kept_whole() {
        assert!(Skipped::default().into_result().is_ok());

        let mut skipped = Skipped::default();
        let cause = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let err = anyhow::Error::from(cause).context("open remote file for download");
        skipped.add("/srv/a", format_args!("{err:#}"));
        let one = format!(
            "1 item skipped or failed, first '/srv/a': open remote file for download: {}",
            std::io::Error::from(std::io::ErrorKind::PermissionDenied)
        );
        let mut again = Skipped::default();
        again.add("/srv/a", format_args!("{err:#}"));
        assert_eq!(again.into_result().expect_err("one").to_string(), one);

        skipped.add("/srv/b", "symbolic link, not followed");
        assert_eq!(
            skipped.into_result().expect_err("two").to_string(),
            one.replacen("1 item", "2 items", 1)
        );
    }

    /// An SFTP session on this machine's own files through OpenSSH's sftp-server,
    /// where one is installed.
    #[cfg(unix)]
    async fn local_sftp() -> Option<russh_sftp::client::SftpSession> {
        let server = [
            "/usr/lib/openssh/sftp-server",
            "/usr/libexec/openssh/sftp-server",
            "/usr/libexec/sftp-server",
        ]
        .into_iter()
        .find(|path| Path::new(path).exists())?;
        let mut child = tokio::process::Command::new(server)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .ok()?;
        // The server exits once the session drops its end of the pipes.
        let pipes = tokio::io::join(child.stdout.take()?, child.stdin.take()?);
        russh_sftp::client::SftpSession::new(pipes).await.ok()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_folder_round_trips_through_sftp_with_its_skips_reported() {
        use std::os::unix::fs::PermissionsExt;
        let Some(sftp) = local_sftp().await else {
            eprintln!("no sftp-server on this machine; skipped");
            return;
        };
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = |rel: &str| tmp.path().join(rel);
        let str_of = |p: &Path| p.to_str().expect("utf-8 temp dir").to_string();
        let set_mode = |p: PathBuf, mode| {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).expect("chmod")
        };
        std::fs::create_dir_all(path("src/private")).expect("mkdir");
        std::fs::write(path("src/run.sh"), "#!/bin/sh\n").expect("write");
        std::fs::write(path("src/private/data.bin"), vec![7u8; 200_000]).expect("write");
        std::os::unix::fs::symlink(path("src/run.sh"), path("src/link")).expect("symlink");

        let (tx, _rx) = mpsc::channel(64);
        let err = do_upload(&str_of(&path("src")), &sftp, &str_of(&path("up")), 1, &tx)
            .await
            .expect_err("the link is reported");
        let link = str_of(&path("src/link"));
        assert_eq!(
            err.to_string(),
            format!("1 item skipped or failed, first '{link}': symbolic link, not followed")
        );
        assert_eq!(
            std::fs::read(path("up/private/data.bin"))
                .expect("read")
                .len(),
            200_000
        );
        assert!(!path("up/link").exists());

        // Back down, into a folder that already holds one of the files.
        std::fs::create_dir_all(path("down")).expect("mkdir");
        std::fs::write(path("down/run.sh"), "old").expect("write");
        do_download(&sftp, &str_of(&path("up")), &str_of(&path("down")), 2, &tx)
            .await
            .expect("downloaded");
        assert_eq!(
            std::fs::read(path("down/run.sh")).expect("read"),
            b"#!/bin/sh\n"
        );
        assert_eq!(
            std::fs::read(path("down/private/data.bin"))
                .expect("read")
                .len(),
            200_000
        );

        // An unreadable folder is reported with the server's reason; the rest still
        // arrives. Root reads it anyway, so only a plain user can check this.
        set_mode(path("up/private"), 0o000);
        let readable = std::fs::read_dir(path("up/private")).is_ok();
        let result =
            do_download(&sftp, &str_of(&path("up")), &str_of(&path("again")), 3, &tx).await;
        set_mode(path("up/private"), 0o700);
        if !readable {
            let err = result.expect_err("the folder is reported");
            let private = str_of(&path("up/private"));
            assert!(
                err.to_string().starts_with(&format!(
                    "1 item skipped or failed, first '{private}': read remote dir: Permission denied"
                )),
                "{err}"
            );
            assert!(path("again/run.sh").exists());
        }
    }

    #[test]
    fn a_server_name_must_be_one_plain_component() {
        for name in ["report.txt", "..hidden", "with space", "caf\u{e9}.txt"] {
            assert!(is_safe_name(name), "{name:?} should be safe");
        }
        for name in ["", ".", "..", "a/b", "/abs", "dir/", "a\0b"] {
            assert!(!is_safe_name(name), "{name:?} should not be safe");
        }
        assert_eq!(join_remote("/", "x"), "/x");
        assert_eq!(join_remote("/srv", "x"), "/srv/x");
    }

    #[test]
    fn windows_names_refuse_drives_devices_and_trimmed_endings() {
        for name in [
            "C:x",
            "D:",
            "a:b",
            "..\\x",
            "a\\b",
            "CON",
            "aux.c",
            "NUL.tar.gz",
            "Com1",
            "lpt9.txt",
            "COM\u{b9}",
            "CON .txt",
            "a.",
            "a ",
            "a<b",
            "a?b",
            "a\"b",
            "tab\tname",
        ] {
            assert!(
                !is_windows_name(name),
                "{name:?} should be refused on Windows"
            );
        }
        for name in [
            "report.txt",
            "CONFIG",
            "COM10",
            "lpt",
            "aux_c",
            ".hidden",
            "a.b",
            "a b",
        ] {
            assert!(
                is_windows_name(name),
                "{name:?} should be allowed on Windows"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn on_windows_a_server_name_never_leaves_the_destination() {
        let dst = Path::new(r"C:\Users\me\dl");
        for name in [
            "C:x", "D:", "a:b", r"..\x", r"\x", r"\\?\x", "CON", "aux.c", "a.",
        ] {
            assert!(
                join_local(dst, name).is_none(),
                "{name:?} should be refused"
            );
        }
        assert_eq!(join_local(dst, "x.txt"), Some(dst.join("x.txt")));
        assert!(check_local_name(r"C:\Users\me\D:evil").is_err());
        assert!(check_local_name(r"C:\Users\me\nul.txt").is_err());
        assert!(check_local_name(r"C:\Users\me\notes.txt").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn on_unix_backslashes_colons_and_device_names_are_plain() {
        let dst = Path::new("/home/me/dl");
        for name in ["a\\b", "..\\x", "C:x", "a:b", "CON", "a."] {
            assert_eq!(join_local(dst, name), Some(dst.join(name)), "{name:?}");
        }
        for name in ["..", "a/b", ""] {
            assert!(
                join_local(dst, name).is_none(),
                "{name:?} should be refused"
            );
        }
        assert!(check_local_name("/home/me/dl/C:x").is_ok());
        assert!(check_local_name("/").is_err());
    }

    #[test]
    fn check_paths_rejects_parent_dirs_and_nulls() {
        assert!(check_paths("/tmp/a", "/srv/a").is_ok());
        assert!(check_paths("/tmp/../etc/a", "/srv/a").is_err());
        assert!(check_paths("/tmp/a", "/srv/a\0").is_err());
    }

    #[test]
    fn unix_secs_handles_times_before_the_epoch() {
        let before = std::time::UNIX_EPOCH - Duration::from_secs(10);
        assert_eq!(unix_secs(Ok(before)), Some(-10));
        assert_eq!(unix_secs(Ok(std::time::UNIX_EPOCH)), Some(0));
        assert_eq!(unix_secs(Err(std::io::Error::other("no"))), None);
    }

    #[cfg(windows)]
    #[test]
    fn the_system_drive_is_a_root() {
        let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        assert!(local_roots().contains(&format!("{}\\", system.to_uppercase())));
    }
}
