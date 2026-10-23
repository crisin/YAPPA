//! The session token is the one secret on the client. It goes into the OS credential store
//! (Windows Credential Manager, macOS Keychain). Where that is not available — a locked-down
//! account, a Linux box without a secret service — it falls back to a file in the profile,
//! readable only by its user on Unix, with a warning in the log. A missing store must never
//! stop someone from talking (platform rule).

use crate::profile::{Profile, TOKEN_FALLBACK_FILE};
use std::collections::BTreeMap;
use std::sync::Mutex;

const SERVICE: &str = "yappa";

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, secret: &str);
    fn delete(&self, key: &str);
}

/// Keyring if it works on this machine (probed once), the file otherwise.
pub fn open(profile: &Profile) -> Box<dyn SecretStore> {
    if std::env::var_os("YAPPA_NO_KEYRING").is_none() && KeyringStore::works(profile) {
        Box::new(KeyringStore {
            prefix: profile.name().to_string(),
        })
    } else {
        tracing::warn!("no OS credential store — keeping the session token in a file");
        Box::new(FileStore::new(profile.clone()))
    }
}

struct KeyringStore {
    /// The profile name, so two profiles keep separate logins.
    prefix: String,
}

impl KeyringStore {
    fn entry(&self, key: &str) -> Option<keyring::Entry> {
        keyring::Entry::new(SERVICE, &format!("{}:{key}", self.prefix)).ok()
    }

    /// Reads a probe entry instead of writing one (realtime review C1): "no entry" means the
    /// store answers, anything else means it does not.
    fn works(profile: &Profile) -> bool {
        let Ok(entry) = keyring::Entry::new(SERVICE, &format!("{}:probe", profile.name())) else {
            return false;
        };
        matches!(entry.get_password(), Ok(_) | Err(keyring::Error::NoEntry))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Option<String> {
        self.entry(key)?.get_password().ok()
    }

    fn set(&self, key: &str, secret: &str) {
        if let Some(Err(error)) = self.entry(key).map(|e| e.set_password(secret)) {
            tracing::warn!(%error, "could not store the session token");
        }
    }

    fn delete(&self, key: &str) {
        if let Some(entry) = self.entry(key) {
            let _ = entry.delete_credential();
        }
    }
}

/// The fallback: one JSON map in the profile folder.
pub struct FileStore {
    profile: Profile,
    lock: Mutex<()>,
}

impl FileStore {
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            lock: Mutex::new(()),
        }
    }

    fn read(&self) -> BTreeMap<String, String> {
        self.profile
            .read_json(TOKEN_FALLBACK_FILE)
            .unwrap_or_default()
    }

    fn write(&self, map: &BTreeMap<String, String>) {
        self.profile.write_json(TOKEN_FALLBACK_FILE, map);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                self.profile.path(TOKEN_FALLBACK_FILE),
                std::fs::Permissions::from_mode(0o600),
            );
        }
    }
}

impl SecretStore for FileStore {
    fn get(&self, key: &str) -> Option<String> {
        let _guard = self.lock.lock().expect("lock");
        self.read().get(key).cloned()
    }

    fn set(&self, key: &str, secret: &str) {
        let _guard = self.lock.lock().expect("lock");
        let mut map = self.read();
        map.insert(key.into(), secret.into());
        self.write(&map);
    }

    fn delete(&self, key: &str) {
        let _guard = self.lock.lock().expect("lock");
        let mut map = self.read();
        if map.remove(key).is_some() {
            self.write(&map);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_store_keeps_and_forgets_secrets() {
        let dir = std::env::temp_dir().join(format!("yappa-secrets-{}", std::process::id()));
        let store = FileStore::new(Profile::new(&dir, Some("t")));
        assert_eq!(store.get("a"), None);
        store.set("a", "geheim");
        store.set("b", "auch");
        assert_eq!(store.get("a").as_deref(), Some("geheim"));
        store.delete("a");
        assert_eq!(store.get("a"), None);
        assert_eq!(store.get("b").as_deref(), Some("auch"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
