//! Loading SSH identity files, including passphrase-protected keys.
//!
//! Passphrases are cached in process memory only — never written to disk —
//! and keyed by the canonical path of the private key so every host that
//! shares a key unlocks it once per session.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use russh::keys::key::KeyPair;
use thiserror::Error;
use tokio::sync::watch;

#[derive(Default)]
struct Cache {
    /// Passphrases that decrypted their key.
    passphrases: HashMap<String, String>,
    /// Keys found encrypted. `unlock` takes no other path, so a frontend cannot
    /// use it to probe arbitrary files.
    encrypted: HashSet<String>,
}

fn cache() -> MutexGuard<'static, Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Bumped on every unlock, so a connection parked on a key can go again.
fn unlocks() -> &'static watch::Sender<()> {
    static UNLOCKS: OnceLock<watch::Sender<()>> = OnceLock::new();
    UNLOCKS.get_or_init(|| watch::channel(()).0)
}

/// Failure to load or unlock a private key file.
#[derive(Debug, Error)]
pub enum IdentityError {
    /// The key is encrypted and no usable passphrase is cached.
    #[error("SSH key {0} requires a passphrase")]
    Encrypted(String),
    /// The passphrase did not decrypt the key.
    #[error("wrong passphrase")]
    WrongPassphrase,
    /// No connection has asked for this key's passphrase.
    #[error("no passphrase was asked for SSH key {0}")]
    NotRequested(String),
    /// The file could not be read or parsed for a reason other than encryption.
    #[error("could not load SSH key {path}: {source}")]
    Load {
        path: String,
        #[source]
        source: anyhow::Error,
    },
}

/// Expand `~/` and, when the file exists, resolve it to a canonical path so
/// `~/.ssh/id_ed25519` and `/home/me/.ssh/id_ed25519` share one cache slot.
pub(crate) fn normalize_key_path(path: &str) -> String {
    let expanded = expand_tilde(path);
    std::fs::canonicalize(&expanded)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or(expanded)
}

pub(crate) fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            return path.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

/// Whether a passphrase is cached for `path` (a canonical key path, as carried
/// by [`crate::event::CoreEvent::KeyPassphraseRequired`]).
pub(crate) fn is_unlocked(path: &str) -> bool {
    cache().passphrases.contains_key(path)
}

/// Resolves once a passphrase is cached for `path`.
pub(crate) async fn unlocked(path: &str) {
    // Subscribe before checking, so an unlock in between is not missed.
    let mut rx = unlocks().subscribe();
    while !is_unlocked(path) {
        // The sender lives in a static and is never dropped.
        let _ = rx.changed().await;
    }
}

/// Decrypt `path` with `passphrase` and remember it for the rest of the process.
///
/// # Errors
/// [`IdentityError::NotRequested`] for a key no connection found encrypted,
/// [`IdentityError::WrongPassphrase`], or an unreadable file.
pub fn unlock(path: &str, passphrase: &str) -> Result<(), IdentityError> {
    let key_path = normalize_key_path(path);
    if !cache().encrypted.contains(&key_path) {
        return Err(IdentityError::NotRequested(key_path));
    }
    match russh::keys::load_secret_key(&key_path, Some(passphrase)) {
        Ok(_) => {
            cache().passphrases.insert(key_path, passphrase.to_string());
            unlocks().send_replace(());
            Ok(())
        }
        Err(russh::keys::Error::IO(e)) => Err(IdentityError::Load {
            path: key_path,
            source: e.into(),
        }),
        // The key is known to be encrypted, so any decode failure is the
        // passphrase (russh reports it as a cipher or parse error).
        Err(_) => Err(IdentityError::WrongPassphrase),
    }
}

/// Load a private key, using a cached passphrase when the file is encrypted.
///
/// # Errors
/// [`IdentityError::Encrypted`] when the key needs a passphrase that has not
/// been unlocked yet; [`IdentityError::Load`] for I/O or parse failures.
pub(crate) fn load_key_pair(path: &str) -> Result<KeyPair, IdentityError> {
    let key_path = normalize_key_path(path);
    // Plain first: a key whose passphrase was removed on disk must not be fed
    // a cached one.
    match russh::keys::load_secret_key(&key_path, None) {
        Ok(key) => return Ok(key),
        Err(russh::keys::Error::KeyIsEncrypted) => {}
        Err(e) => {
            return Err(IdentityError::Load {
                path: key_path,
                source: e.into(),
            })
        }
    }

    let passphrase = {
        let mut cache = cache();
        cache.encrypted.insert(key_path.clone());
        cache.passphrases.get(&key_path).cloned()
    };
    let Some(passphrase) = passphrase else {
        return Err(IdentityError::Encrypted(key_path));
    };
    russh::keys::load_secret_key(&key_path, Some(&passphrase)).map_err(|_| {
        // Re-encrypted since it was unlocked: ask again.
        cache().passphrases.remove(&key_path);
        IdentityError::Encrypted(key_path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;

    fn ssh_keygen(args: &[&str]) {
        let status = Command::new("ssh-keygen")
            .args(args)
            .status()
            .expect("these tests need ssh-keygen on PATH");
        assert!(status.success(), "ssh-keygen {args:?} failed: {status}");
    }

    fn write_key(dir: &Path, passphrase: &str) -> String {
        let path: PathBuf = dir.join("id_ed25519");
        let path = path.to_str().expect("utf-8 path").to_string();
        ssh_keygen(&["-q", "-t", "ed25519", "-f", &path, "-N", passphrase]);
        path
    }

    #[test]
    fn an_unencrypted_key_loads_without_a_passphrase() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_key(dir.path(), "");
        load_key_pair(&path).expect("unencrypted key should load");
    }

    #[test]
    fn an_encrypted_key_reports_that_a_passphrase_is_required() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_key(dir.path(), "correct horse");
        match load_key_pair(&path) {
            Err(IdentityError::Encrypted(reported)) => {
                assert_eq!(reported, normalize_key_path(&path))
            }
            other => panic!("expected Encrypted, got {other:?}"),
        }
    }

    #[test]
    fn unlocking_caches_the_passphrase_for_later_loads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_key(dir.path(), "correct horse");
        assert!(load_key_pair(&path).is_err());

        unlock(&path, "correct horse").expect("unlock");
        assert!(is_unlocked(&normalize_key_path(&path)));
        load_key_pair(&path).expect("cached passphrase should decrypt the key");
    }

    #[test]
    fn a_wrong_passphrase_is_rejected_and_not_cached() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_key(dir.path(), "correct horse");
        assert!(load_key_pair(&path).is_err());

        assert!(matches!(
            unlock(&path, "wrong"),
            Err(IdentityError::WrongPassphrase)
        ));
        assert!(matches!(
            load_key_pair(&path),
            Err(IdentityError::Encrypted(_))
        ));
    }

    #[test]
    fn only_a_key_found_encrypted_can_be_unlocked() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_key(dir.path(), "correct horse");
        assert!(matches!(
            unlock(&path, "correct horse"),
            Err(IdentityError::NotRequested(_))
        ));
    }

    #[test]
    fn a_key_changed_on_disk_after_unlocking_is_read_as_it_is_now() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_key(dir.path(), "correct horse");
        assert!(load_key_pair(&path).is_err());
        unlock(&path, "correct horse").expect("unlock");

        ssh_keygen(&["-q", "-p", "-f", &path, "-P", "correct horse", "-N", "new"]);
        assert!(
            matches!(load_key_pair(&path), Err(IdentityError::Encrypted(_))),
            "a stale passphrase must lead to a new prompt"
        );
        assert!(!is_unlocked(&normalize_key_path(&path)));

        ssh_keygen(&["-q", "-p", "-f", &path, "-P", "new", "-N", ""]);
        load_key_pair(&path).expect("a key without a passphrase loads as is");
    }

    #[tokio::test]
    async fn a_waiter_wakes_when_its_key_is_unlocked() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_key(dir.path(), "correct horse");
        assert!(load_key_pair(&path).is_err());
        let key = normalize_key_path(&path);

        let waiter = tokio::spawn(async move { unlocked(&key).await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!waiter.is_finished());

        unlock(&path, "correct horse").expect("unlock");
        tokio::time::timeout(Duration::from_secs(5), waiter)
            .await
            .expect("the waiter wakes")
            .expect("the waiter ran");
    }
}
