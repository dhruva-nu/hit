//! JSON files under the data dir, one per project, mode 0600 (dir 0700).

use std::path::PathBuf;

use super::{StoredToken, TokenStore};
use crate::error::AuthError;

pub struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, project: &str) -> PathBuf {
        self.dir.join(format!("{project}.json"))
    }

    fn ensure_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
}

impl TokenStore for FileStore {
    fn load(&self, project: &str) -> Option<StoredToken> {
        let raw = std::fs::read_to_string(self.path(project)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    fn save(&self, project: &str, token: &StoredToken) -> Result<(), AuthError> {
        let write = || -> std::io::Result<()> {
            self.ensure_dir()?;
            let path = self.path(project);
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_vec(token)?)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
            }
            std::fs::rename(&tmp, &path)
        };
        write().map_err(|e| AuthError::Store(format!("writing token file: {e}")))
    }

    fn clear(&self, project: &str) -> Result<(), AuthError> {
        match std::fs::remove_file(self.path(project)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AuthError::Store(format!("removing token file: {e}"))),
        }
    }
}
