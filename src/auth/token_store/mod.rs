//! Token persistence. `FileStore` (0600 files) is always available; the
//! keyring backend is feature-gated and falls back to files on any error so
//! headless Linux without a Secret Service never crashes.

mod file;
#[cfg(feature = "keyring")]
mod keyring;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::TokenStoreKind;
use crate::error::AuthError;

pub use file::FileStore;
#[cfg(feature = "keyring")]
pub use keyring::KeyringStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Unix seconds; `None` for opaque tokens with unknown expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_unix: Option<u64>,
    #[serde(default = "default_token_type")]
    pub token_type: String,
}

fn default_token_type() -> String {
    "Bearer".to_string()
}

impl StoredToken {
    /// Fresh if there's no known expiry, or expiry is more than `margin_secs` away.
    pub fn is_fresh(&self, margin_secs: u64) -> bool {
        match self.expires_at_unix {
            None => true,
            Some(exp) => now_unix() + margin_secs < exp,
        }
    }
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub trait TokenStore: Send + Sync {
    fn load(&self, project: &str) -> Option<StoredToken>;
    fn save(&self, project: &str, token: &StoredToken) -> Result<(), AuthError>;
    fn clear(&self, project: &str) -> Result<(), AuthError>;
}

pub fn new_token_store(
    kind: TokenStoreKind,
    token_dir: PathBuf,
) -> Result<Box<dyn TokenStore>, AuthError> {
    match kind {
        TokenStoreKind::File => Ok(Box::new(FileStore::new(token_dir))),
        TokenStoreKind::Keyring => keyring_store().ok_or_else(|| {
            AuthError::Store(
                "token_store = \"keyring\" but this build lacks the 'keyring' feature".into(),
            )
        }),
        TokenStoreKind::Auto => Ok(keyring_store().unwrap_or(Box::new(FileStore::new(token_dir)))),
    }
}

#[cfg(feature = "keyring")]
fn keyring_store() -> Option<Box<dyn TokenStore>> {
    Some(Box::new(KeyringStore))
}

#[cfg(not(feature = "keyring"))]
fn keyring_store() -> Option<Box<dyn TokenStore>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_store_round_trip_and_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path().join("tokens"));
        let token = StoredToken {
            access_token: "abc".into(),
            refresh_token: None,
            expires_at_unix: Some(now_unix() + 3600),
            token_type: "Bearer".into(),
        };
        store.save("demo", &token).unwrap();
        let loaded = store.load("demo").unwrap();
        assert_eq!(loaded.access_token, "abc");
        assert!(loaded.is_fresh(60));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join("tokens/demo.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        store.clear("demo").unwrap();
        assert!(store.load("demo").is_none());
        store.clear("demo").unwrap(); // idempotent
    }

    #[test]
    fn freshness_margin() {
        let token = StoredToken {
            access_token: "t".into(),
            refresh_token: None,
            expires_at_unix: Some(now_unix() + 30),
            token_type: "Bearer".into(),
        };
        assert!(token.is_fresh(0));
        assert!(!token.is_fresh(60));
    }
}
