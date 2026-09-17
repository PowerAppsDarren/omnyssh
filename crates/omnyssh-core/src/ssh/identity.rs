//! Loading SSH identity files, including passphrase-protected keys.
//!
//! Passphrases are cached in process memory only — never written to disk —
//! and keyed by the canonical path of the private key so every host that
//! shares a key unlocks it once per session.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

use russh::keys::key::KeyPair;
use thiserror::Error;

static CACHE: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();

fn cache() -> &'static RwLock<HashMap<String, String>> {
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Failure to load or unlock a private key file.
#[derive(Debug, Error)]
pub enum IdentityError {
    /// The key is encrypted and no usable passphrase is cached.
    #[error("SSH key {0} requires a passphrase")]
    Encrypted(String),
    /// A passphrase was supplied (or cached) but did not decrypt the key.
    #[error("wrong passphrase for SSH key {0}")]
    WrongPassphrase(String),
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

fn cached_passphrase(path: &str) -> Option<String> {
    cache()
        .read()
        .expect("identity passphrase cache poisoned")
        .get(path)
        .cloned()
}

fn remember(path: &str, passphrase: &str) {
    cache()
        .write()
        .expect("identity passphrase cache poisoned")
        .insert(path.to_string(), passphrase.to_string());
}

fn forget(path: &str) {
    cache()
        .write()
        .expect("identity passphrase cache poisoned")
        .remove(path);
}

/// Decrypt `path` with `passphrase` and remember it for the rest of the process.
///
/// # Errors
/// Wrong passphrase, unreadable file, or a key that is not encrypted but
/// still fails to parse.
pub fn unlock(path: &str, passphrase: &str) -> Result<(), IdentityError> {
    let key_path = normalize_key_path(path);
    match russh::keys::load_secret_key(&key_path, Some(passphrase)) {
        Ok(_) => {
            remember(&key_path, passphrase);
            Ok(())
        }
        Err(russh::keys::Error::KeyIsEncrypted) => Err(IdentityError::WrongPassphrase(key_path)),
        Err(e) => Err(IdentityError::Load {
            path: key_path,
            source: e.into(),
        }),
    }
}

/// Load a private key, using a cached passphrase when the file is encrypted.
///
/// # Errors
/// [`IdentityError::Encrypted`] when the key needs a passphrase that has not
/// been unlocked yet; other variants for I/O, parse, or decrypt failures.
pub fn load_key_pair(path: &str) -> Result<KeyPair, IdentityError> {
    let key_path = normalize_key_path(path);
    let passphrase = cached_passphrase(&key_path);
    match russh::keys::load_secret_key(Path::new(&key_path), passphrase.as_deref()) {
        Ok(key) => Ok(key),
        Err(russh::keys::Error::KeyIsEncrypted) if passphrase.is_none() => {
            Err(IdentityError::Encrypted(key_path))
        }
        Err(russh::keys::Error::KeyIsEncrypted) => {
            forget(&key_path);
            Err(IdentityError::WrongPassphrase(key_path))
        }
        Err(e) => Err(IdentityError::Load {
            path: key_path,
            source: e.into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    fn ssh_keygen_available() -> bool {
        Command::new("ssh-keygen").output().is_ok()
    }

    fn write_key(dir: &Path, passphrase: &str) -> PathBuf {
        let path = dir.join("id_ed25519");
        let status = Command::new("ssh-keygen")
            .args([
                "-t",
                "ed25519",
                "-f",
                path.to_str().expect("utf-8 path"),
                "-N",
                passphrase,
                "-q",
            ])
            .status()
            .expect("run ssh-keygen");
        assert!(status.success(), "ssh-keygen failed: {status}");
        path
    }

    #[test]
    fn an_unencrypted_key_loads_without_a_passphrase() {
        if !ssh_keygen_available() {
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_key(dir.path(), "");
        load_key_pair(path.to_str().expect("utf-8")).expect("unencrypted key should load");
    }

    #[test]
    fn an_encrypted_key_reports_that_a_passphrase_is_required() {
        if !ssh_keygen_available() {
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_key(dir.path(), "correct horse");
        let path_str = path.to_str().expect("utf-8");
        forget(&normalize_key_path(path_str));
        match load_key_pair(path_str) {
            Err(IdentityError::Encrypted(_)) => {}
            other => panic!("expected Encrypted, got {other:?}"),
        }
    }

    #[test]
    fn unlocking_caches_the_passphrase_for_later_loads() {
        if !ssh_keygen_available() {
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_key(dir.path(), "correct horse");
        let path_str = path.to_str().expect("utf-8");
        forget(&normalize_key_path(path_str));

        unlock(path_str, "correct horse").expect("unlock");
        load_key_pair(path_str).expect("cached passphrase should decrypt the key");
        forget(&normalize_key_path(path_str));
    }

    #[test]
    fn a_wrong_passphrase_is_rejected_and_not_cached() {
        if !ssh_keygen_available() {
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_key(dir.path(), "correct horse");
        let path_str = path.to_str().expect("utf-8");
        forget(&normalize_key_path(path_str));

        match unlock(path_str, "wrong") {
            Err(IdentityError::WrongPassphrase(_)) | Err(IdentityError::Load { .. }) => {}
            other => panic!("expected wrong-passphrase error, got {other:?}"),
        }
        match load_key_pair(path_str) {
            Err(IdentityError::Encrypted(_)) => {}
            other => panic!("wrong passphrase must not be cached, got {other:?}"),
        }
    }
}
