//! OS keychain via the `keyring` crate. Any backend error degrades to a
//! logged failure (load -> None) so callers can fall back to re-login.

use super::{StoredToken, TokenStore};
use crate::error::AuthError;

pub struct KeyringStore;

impl KeyringStore {
    fn entry(project: &str) -> Result<keyring::Entry, AuthError> {
        keyring::Entry::new("hitpoint", project)
            .map_err(|e| AuthError::Store(format!("keyring: {e}")))
    }
}

impl TokenStore for KeyringStore {
    fn load(&self, project: &str) -> Option<StoredToken> {
        let entry = Self::entry(project).ok()?;
        let raw = entry.get_password().ok()?;
        serde_json::from_str(&raw).ok()
    }

    fn save(&self, project: &str, token: &StoredToken) -> Result<(), AuthError> {
        let entry = Self::entry(project)?;
        let raw = serde_json::to_string(token)
            .map_err(|e| AuthError::Store(format!("serializing token: {e}")))?;
        entry
            .set_password(&raw)
            .map_err(|e| AuthError::Store(format!("keyring: {e}")))
    }

    fn clear(&self, project: &str) -> Result<(), AuthError> {
        let entry = Self::entry(project)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AuthError::Store(format!("keyring: {e}"))),
        }
    }
}
