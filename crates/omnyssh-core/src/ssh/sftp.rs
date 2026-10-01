//! SFTP file manager operations.
//!
//! Provides [`SftpManager`] — a persistent background task that owns an SSH+SFTP
//! session and processes [`SftpCommand`] messages sent from the UI thread.
//!
//! All operations are non-blocking from the UI perspective.
//! Progress is reported via [`CoreEvent::FileTransferProgress`].

use std::time::Duration;

use anyhow::Context;
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
    /// Creation time, in seconds since the Unix epoch. Local entries only, and only
    /// where the file system records it: SFTP v3 has no creation time.
    pub created: Option<i64>,
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
            created: None,
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

/// Builds a local [`FileEntry`] from its path and (possibly unreadable) metadata.
fn local_entry(path: &std::path::Path, meta: Option<&std::fs::Metadata>) -> FileEntry {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    FileEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        size: meta.map(|m| m.len()).unwrap_or(0),
        is_dir: meta.map(|m| m.is_dir()).unwrap_or(false),
        modified: meta.and_then(|m| unix_secs(m.modified())),
        created: meta.and_then(|m| unix_secs(m.created())),
    }
}

// ---------------------------------------------------------------------------
// SftpCommand — sent from UI thread → SftpManager background task
// ---------------------------------------------------------------------------

/// Commands processed by the [`SftpManager`] background task.
pub enum SftpCommand {
    /// List the entries in a remote directory.
    ListDir(String),
    /// Download a remote file to a local path.
    Download {
        remote: String,
        local: String,
        transfer_id: TransferId,
    },
    /// Upload a local file to a remote path.
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
                            reason: format!("ListDir failed: {e}"),
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
                    .map_err(|e| e.to_string());
                let _ = event_tx.send(CoreEvent::SftpOpDone { result }).await;
            }

            SftpCommand::Upload {
                local,
                remote,
                transfer_id,
            } => {
                let result = do_upload(&local, &sftp, &remote, transfer_id, &event_tx)
                    .await
                    .map_err(|e| e.to_string());
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
            created: None,
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

/// A directory entry name that is safe to join onto a destination: not `.` or `..`
/// and free of separators, so a hostile server cannot steer a download elsewhere.
fn is_plain_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0'])
}

/// Rejects a transfer whose local path climbs out with `..` or either path holds a
/// null byte.
fn check_paths(local: &str, remote: &str) -> anyhow::Result<()> {
    if std::path::Path::new(local)
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

/// One file of a folder transfer.
#[derive(Debug, PartialEq)]
struct PlannedFile {
    src: String,
    dst: String,
    size: u64,
}

/// Everything a folder transfer creates, worked out before the first byte moves so
/// the progress bar covers the whole folder. `dirs` lists parents before children.
#[derive(Debug, Default)]
struct TreePlan {
    dirs: Vec<String>,
    files: Vec<PlannedFile>,
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
/// Symlinks and special files are skipped, so a link cycle cannot loop forever.
async fn plan_local_tree(root: &std::path::Path, dst_root: &str) -> anyhow::Result<TreePlan> {
    let mut plan = TreePlan::default();
    let mut stack = vec![(root.to_path_buf(), dst_root.to_string())];
    while let Some((src, dst)) = stack.pop() {
        let mut read_dir = tokio::fs::read_dir(&src)
            .await
            .with_context(|| format!("read local dir '{}'", src.display()))?;
        plan.dirs.push(dst.clone());
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .with_context(|| format!("read local dir '{}'", src.display()))?
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_plain_name(&name) {
                continue;
            }
            // `DirEntry::file_type` does not follow symlinks.
            let file_type = entry
                .file_type()
                .await
                .with_context(|| format!("stat '{}'", entry.path().display()))?;
            if file_type.is_dir() {
                stack.push((entry.path(), join_remote(&dst, &name)));
            } else if file_type.is_file() {
                let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
                plan.files.push(PlannedFile {
                    src: entry.path().to_string_lossy().into_owned(),
                    dst: join_remote(&dst, &name),
                    size,
                });
            }
        }
    }
    Ok(plan)
}

/// Walks the remote folder `root`, mapping it onto the local folder `dst_root`.
/// Symlinks and special files are skipped, as are names that are not plain.
async fn plan_remote_tree(
    sftp: &russh_sftp::client::SftpSession,
    root: &str,
    dst_root: &std::path::Path,
) -> anyhow::Result<TreePlan> {
    let mut plan = TreePlan::default();
    let mut stack = vec![(root.to_string(), dst_root.to_path_buf())];
    while let Some((src, dst)) = stack.pop() {
        let read_dir = sftp
            .read_dir(src.as_str())
            .await
            .with_context(|| format!("read remote dir '{src}'"))?;
        plan.dirs.push(dst.to_string_lossy().into_owned());
        for entry in read_dir {
            let name = entry.file_name();
            if !is_plain_name(&name) {
                continue;
            }
            let file_type = entry.file_type();
            if file_type.is_dir() {
                stack.push((join_remote(&src, &name), dst.join(&name)));
            } else if file_type.is_file() {
                plan.files.push(PlannedFile {
                    src: join_remote(&src, &name),
                    dst: dst.join(&name).to_string_lossy().into_owned(),
                    size: entry.metadata().size.unwrap_or(0),
                });
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

    // Size for progress and file-or-folder (best-effort: a failed stat downloads as a
    // file, and the open below reports the real error).
    let meta = sftp.metadata(remote).await.ok();
    if meta.as_ref().is_some_and(|m| m.file_type().is_dir()) {
        let plan = plan_remote_tree(sftp, remote, std::path::Path::new(local)).await?;
        let mut progress = Progress::new(transfer_id, plan.total(), event_tx);
        for dir in &plan.dirs {
            check_paths(dir, remote)?;
            tokio::fs::create_dir_all(dir)
                .await
                .with_context(|| format!("create local dir '{dir}'"))?;
        }
        for file in &plan.files {
            check_paths(&file.dst, &file.src)?;
            download_file(sftp, &file.src, &file.dst, &mut progress)
                .await
                .with_context(|| format!("download '{}'", file.src))?;
        }
        return Ok(());
    }

    let total = meta.and_then(|m| m.size).unwrap_or(0);
    let mut progress = Progress::new(transfer_id, total, event_tx);
    download_file(sftp, remote, local, &mut progress).await
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

    Ok(())
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
    if meta.is_dir() {
        let plan = plan_local_tree(std::path::Path::new(local), remote).await?;
        let mut progress = Progress::new(transfer_id, plan.total(), event_tx);
        for dir in &plan.dirs {
            // An existing folder is merged into, as an existing file is overwritten.
            if !sftp.try_exists(dir.as_str()).await.unwrap_or(false) {
                sftp.create_dir(dir.as_str())
                    .await
                    .with_context(|| format!("create remote dir '{dir}'"))?;
            }
        }
        for file in &plan.files {
            upload_file(&file.src, sftp, &file.dst, &mut progress)
                .await
                .with_context(|| format!("upload '{}'", file.src))?;
        }
        return Ok(());
    }

    let mut progress = Progress::new(transfer_id, meta.len(), event_tx);
    upload_file(local, sftp, remote, &mut progress).await
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

        let mut file = local_entry(&entry.path(), meta.as_ref());
        file.name = entry.file_name().to_string_lossy().into_owned();
        file.is_dir = is_dir;
        entries.push(file);
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

/// Stats each local path (following symlinks), e.g. files dropped onto the app from
/// the OS. Paths that cannot be read are left out.
pub async fn stat_local_paths(paths: &[String]) -> Vec<FileEntry> {
    let mut entries = Vec::with_capacity(paths.len());
    for path in paths {
        if let Ok(meta) = tokio::fs::metadata(path).await {
            entries.push(local_entry(std::path::Path::new(path), Some(&meta)));
        }
    }
    entries
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

    fn scratch_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("omnyssh-sftp-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[tokio::test]
    async fn local_listing_carries_modification_time() {
        let dir = scratch_dir("list");
        std::fs::write(dir.join("a.txt"), b"hello").expect("write scratch file");

        let entries = list_local_dir(&dir.to_string_lossy())
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
        assert!(parent.modified.is_none() && parent.created.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn stat_local_paths_tells_files_from_dirs_and_skips_missing() {
        let dir = scratch_dir("stat");
        let file = dir.join("b.bin");
        std::fs::write(&file, b"xy").expect("write scratch file");
        let missing = dir.join("missing");

        let paths = [
            file.to_string_lossy().into_owned(),
            dir.to_string_lossy().into_owned(),
            missing.to_string_lossy().into_owned(),
        ];
        let entries = stat_local_paths(&paths).await;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "b.bin");
        assert!(!entries[0].is_dir);
        assert_eq!(entries[0].size, 2);
        assert!(entries[1].is_dir);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn local_tree_plan_maps_every_file_and_folder() {
        let dir = scratch_dir("plan");
        std::fs::create_dir_all(dir.join("sub").join("deep")).expect("create scratch tree");
        std::fs::write(dir.join("a.txt"), b"abc").expect("write scratch file");
        std::fs::write(dir.join("sub").join("deep").join("b.bin"), b"hello")
            .expect("write scratch file");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&dir, dir.join("sub").join("loop")).expect("create symlink");

        let plan = plan_local_tree(&dir, "/srv/up").await.expect("plan tree");
        assert_eq!(plan.dirs[0], "/srv/up");
        let mut dirs = plan.dirs.clone();
        dirs.sort();
        assert_eq!(dirs, ["/srv/up", "/srv/up/sub", "/srv/up/sub/deep"]);
        // Parents come before their children, so creating in order never fails.
        let sub = plan.dirs.iter().position(|d| d == "/srv/up/sub");
        let deep = plan.dirs.iter().position(|d| d == "/srv/up/sub/deep");
        assert!(sub < deep);
        let mut files: Vec<_> = plan
            .files
            .iter()
            .map(|f| (f.dst.as_str(), f.size))
            .collect();
        files.sort();
        assert_eq!(files, [("/srv/up/a.txt", 3), ("/srv/up/sub/deep/b.bin", 5)]);
        assert_eq!(plan.total(), 8);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plain_names_exclude_dots_and_separators() {
        assert!(is_plain_name("report.txt"));
        assert!(is_plain_name("..hidden"));
        for name in ["", ".", "..", "a/b", "a\\b", "a\0b"] {
            assert!(!is_plain_name(name), "{name:?} should not be plain");
        }
        assert_eq!(join_remote("/", "x"), "/x");
        assert_eq!(join_remote("/srv", "x"), "/srv/x");
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
