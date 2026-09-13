//! The saved login sessions, held in the keychain of the operating system.
//!
//! macOS uses the Keychain, Windows uses the Credential Manager and Linux
//! uses the Secret Service (GNOME Keyring, KDE Wallet, KeePassXC). One entry
//! holds every session as a JSON array. A session file from an older version
//! moves into the keychain on the first load.

use std::fs;
use std::io;

use keyring::Entry;
use rustybolt_auth::Session;

use crate::Paths;

/// The keychain service name of the entry.
const SERVICE: &str = "rustybolt";
/// The keychain user name of the entry.
const ACCOUNT: &str = "sessions";

/// A place that holds one secret string.
pub trait Vault: Send {
    /// The secret, or `None` when the vault holds no entry.
    fn read(&self) -> Result<Option<String>, KeychainError>;
    /// Replaces the secret.
    fn write(&self, secret: &str) -> Result<(), KeychainError>;
}

/// The keychain of the operating system is missing or refuses access.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no keychain is available to hold the Jagex session: {0}")]
pub struct KeychainError(pub String);

impl From<KeychainError> for io::Error {
    fn from(error: KeychainError) -> io::Error {
        io::Error::new(io::ErrorKind::PermissionDenied, error.to_string())
    }
}

/// The keychain entry of the operating system.
struct Keychain;

impl Keychain {
    fn entry() -> Result<Entry, KeychainError> {
        Entry::new(SERVICE, ACCOUNT).map_err(|error| KeychainError(error.to_string()))
    }
}

impl Vault for Keychain {
    fn read(&self) -> Result<Option<String>, KeychainError> {
        match Self::entry()?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(KeychainError(error.to_string())),
        }
    }

    fn write(&self, secret: &str) -> Result<(), KeychainError> {
        Self::entry()?
            .set_password(secret)
            .map_err(|error| KeychainError(error.to_string()))
    }
}

/// Checks that the keychain of the operating system answers.
///
/// The launcher refuses to run without one, because the Jagex session
/// must not sit in a plain file. On Linux this needs a Secret Service
/// provider on the session bus, such as GNOME Keyring or KDE Wallet.
pub fn keychain_available() -> Result<(), KeychainError> {
    Keychain.read().map(|_| ())
}

/// The sessions of the user, and the vault that holds them.
pub struct SessionStore {
    vault: Box<dyn Vault>,
    sessions: Vec<Session>,
}

impl SessionStore {
    /// Reads the sessions from the keychain. An absent or malformed entry
    /// gives no sessions. A session file from an older version moves into
    /// the keychain, and the file goes away.
    pub fn load(paths: &Paths) -> SessionStore {
        let mut store = SessionStore::with_vault(Box::new(Keychain));
        let legacy = paths.credentials_file();
        if store.sessions.is_empty() {
            if let Some(sessions) = fs::read_to_string(&legacy)
                .ok()
                .and_then(|text| serde_json::from_str::<Vec<Session>>(&text).ok())
            {
                store.sessions = sessions;
                if store.save().is_ok() {
                    let _ = fs::remove_file(&legacy);
                }
            }
        }
        store
    }

    /// Reads the sessions from this vault.
    pub fn with_vault(vault: Box<dyn Vault>) -> SessionStore {
        let sessions = vault
            .read()
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str::<Vec<Session>>(&text).ok())
            .unwrap_or_default();
        SessionStore { vault, sessions }
    }

    /// Writes the sessions as JSON into the vault.
    pub fn save(&self) -> io::Result<()> {
        let text = serde_json::to_string(&self.sessions)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        self.vault.write(&text)?;
        Ok(())
    }

    /// Adds a session, or replaces the session with the same `sub` value.
    pub fn upsert(&mut self, session: Session) {
        match self
            .sessions
            .iter_mut()
            .find(|held| held.sub == session.sub)
        {
            Some(held) => *held = session,
            None => self.sessions.push(session),
        }
    }

    /// Removes the session with this `sub` value. An absent value is not an error.
    pub fn remove(&mut self, sub: &str) {
        self.sessions.retain(|held| held.sub != sub);
    }

    /// The sessions, in vault order.
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// A vault in memory. Clones share the same secret.
    #[derive(Clone, Default)]
    struct Memory(Arc<Mutex<Option<String>>>);

    impl Vault for Memory {
        fn read(&self) -> Result<Option<String>, KeychainError> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn write(&self, secret: &str) -> Result<(), KeychainError> {
            *self.0.lock().unwrap() = Some(secret.to_string());
            Ok(())
        }
    }

    /// A vault that always fails, like a desktop without a Secret Service.
    struct Absent;

    impl Vault for Absent {
        fn read(&self) -> Result<Option<String>, KeychainError> {
            Err(KeychainError("no secret service".into()))
        }
        fn write(&self, _: &str) -> Result<(), KeychainError> {
            Err(KeychainError("no secret service".into()))
        }
    }

    fn session(sub: &str, name: &str) -> Session {
        Session {
            session_id: format!("id-{sub}"),
            display_name: name.to_string(),
            suffix: "0001".to_string(),
            sub: sub.to_string(),
        }
    }

    fn subs(store: &SessionStore) -> Vec<String> {
        store
            .sessions()
            .iter()
            .map(|held| held.sub.clone())
            .collect()
    }

    #[test]
    fn upsert_replaces_a_session_with_the_same_sub() {
        let mut store = SessionStore::with_vault(Box::new(Memory::default()));
        store.upsert(session("a", "Ada"));
        store.upsert(session("b", "Bea"));
        store.upsert(session("a", "Ada Two"));

        assert_eq!(subs(&store), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(store.sessions()[0].display_name, "Ada Two");
        assert_eq!(store.sessions()[1].display_name, "Bea");
    }

    #[test]
    fn remove_drops_only_the_wanted_sub() {
        let mut store = SessionStore::with_vault(Box::new(Memory::default()));
        store.upsert(session("a", "Ada"));
        store.upsert(session("b", "Bea"));
        store.remove("a");

        assert_eq!(subs(&store), vec!["b".to_string()]);
        store.remove("missing");
        assert_eq!(subs(&store), vec!["b".to_string()]);
    }

    #[test]
    fn save_and_load_keeps_the_sessions() {
        let vault = Memory::default();
        let mut store = SessionStore::with_vault(Box::new(vault.clone()));
        store.upsert(session("a", "Ada"));
        store.save().unwrap();

        let loaded = SessionStore::with_vault(Box::new(vault));
        assert_eq!(subs(&loaded), vec!["a".to_string()]);
        assert_eq!(loaded.sessions()[0].session_id, "id-a");
        assert_eq!(loaded.sessions()[0].suffix, "0001");
        assert_eq!(loaded.sessions()[0].display_name, "Ada");
    }

    #[test]
    fn an_absent_or_malformed_entry_gives_no_sessions() {
        assert!(SessionStore::with_vault(Box::new(Memory::default()))
            .sessions()
            .is_empty());

        let vault = Memory::default();
        vault.write("not json").unwrap();
        assert!(SessionStore::with_vault(Box::new(vault))
            .sessions()
            .is_empty());
    }

    #[test]
    fn save_fails_without_a_keychain() {
        let mut store = SessionStore::with_vault(Box::new(Absent));
        assert!(store.sessions().is_empty());
        store.upsert(session("a", "Ada"));
        let error = store.save().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("no keychain"));
    }
}
