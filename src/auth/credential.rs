//! Resolving configured credential references (literal, env, keyring, prompt)
//! to their concrete values at login time.

use super::Interactor;
use crate::error::AuthError;

/// Resolve a configured credential reference to its value.
pub(crate) fn resolve_credential(
    cred: &crate::config::CredentialRef,
    label: &str,
    interactor: &dyn Interactor,
    secret: bool,
) -> Result<String, AuthError> {
    use crate::config::CredentialRef;
    match cred {
        CredentialRef::Value { value } => Ok(value.clone()),
        CredentialRef::Env { env } => std::env::var(env)
            .map_err(|_| AuthError::Credential(format!("environment variable {env} is not set"))),
        CredentialRef::Keyring { keyring } => keyring_lookup(keyring),
        CredentialRef::Prompt { prompt } => {
            if !prompt {
                return Err(AuthError::Credential(format!(
                    "{label}: prompt = false makes the credential unreachable"
                )));
            }
            if secret {
                interactor.prompt_secret(label)
            } else {
                interactor.prompt_line(label)
            }
        }
    }
}

#[cfg(feature = "keyring")]
fn keyring_lookup(entry_name: &str) -> Result<String, AuthError> {
    let entry = keyring::Entry::new("hitpoint", entry_name)
        .map_err(|e| AuthError::Credential(format!("keyring: {e}")))?;
    entry
        .get_password()
        .map_err(|e| AuthError::Credential(format!("keyring entry '{entry_name}': {e}")))
}

#[cfg(not(feature = "keyring"))]
fn keyring_lookup(entry_name: &str) -> Result<String, AuthError> {
    Err(AuthError::Credential(format!(
        "credential '{entry_name}' uses the keyring, but this build lacks the 'keyring' feature"
    )))
}
