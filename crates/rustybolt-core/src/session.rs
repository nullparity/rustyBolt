//! The saved login sessions, held in the keychain of the operating system.
//!
//! macOS uses the Keychain, Windows uses the Credential Manager and Linux
//! uses the Secret Service (GNOME Keyring, KDE Wallet, KeePassXC). One entry
//! holds every session as a JSON array. A session file from an older version
//! moves into the keychain on the first load.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use keyring::Entry;
use rustybolt_auth::Session;
use serde::{Deserialize, Serialize};

use crate::{Config, Paths};

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

/// The unit of a [`SessionMaxAge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgeUnit {
    Hours,
    Days,
    Weeks,
    /// Thirty days.
    Months,
}

impl AgeUnit {
    fn seconds(self) -> u64 {
        match self {
            AgeUnit::Hours => 3600,
            AgeUnit::Days => 86_400,
            AgeUnit::Weeks => 7 * 86_400,
            AgeUnit::Months => 30 * 86_400,
        }
    }
}

/// How long a saved login stays before the launcher signs it out.
///
/// The user picks a count and a unit, as in "3 days". The pair keeps the
/// dashboard free of duration syntax and lets each language order the words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMaxAge {
    pub count: u32,
    pub unit: AgeUnit,
}

impl SessionMaxAge {
    /// The age in seconds.
    pub fn seconds(self) -> u64 {
        u64::from(self.count) * self.unit.seconds()
    }
}

/// One vault entry: the session and when the user signed in.
#[derive(Serialize, Deserialize)]
struct Held {
    #[serde(flatten)]
    session: Session,
    /// Unix seconds. Absent in entries that an older version wrote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    signed_in_at: Option<u64>,
}

/// The current Unix time in seconds.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The sessions of the user, and the vault that holds them.
pub struct SessionStore {
    vault: Box<dyn Vault>,
    sessions: Vec<Session>,
    /// When the user signed in, by `sub`. A session without an entry came
    /// from an older version.
    signed_in_at: HashMap<String, u64>,
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

    /// Reads the sessions and signs out the ones older than
    /// `config.session_max_age`. A sign-out goes into the keychain at once.
    pub fn load_active(paths: &Paths, config: &Config) -> SessionStore {
        let mut store = SessionStore::load(paths);
        if store.expire(config.session_max_age, now()) {
            let _ = store.save();
        }
        store
    }

    /// Reads the sessions from this vault.
    pub fn with_vault(vault: Box<dyn Vault>) -> SessionStore {
        let held = vault
            .read()
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str::<Vec<Held>>(&text).ok())
            .unwrap_or_default();
        let mut store = SessionStore {
            vault,
            sessions: Vec::with_capacity(held.len()),
            signed_in_at: HashMap::new(),
        };
        for entry in held {
            if let Some(at) = entry.signed_in_at {
                store.signed_in_at.insert(entry.session.sub.clone(), at);
            }
            store.sessions.push(entry.session);
        }
        store
    }

    /// Writes the sessions as JSON into the vault.
    pub fn save(&self) -> io::Result<()> {
        let held: Vec<Held> = self
            .sessions
            .iter()
            .map(|session| Held {
                session: session.clone(),
                signed_in_at: self.signed_in_at.get(&session.sub).copied(),
            })
            .collect();
        let text = serde_json::to_string(&held)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        self.vault.write(&text)?;
        Ok(())
    }

    /// Adds a session, or replaces the session with the same `sub` value.
    /// The sign-in time is now.
    pub fn upsert(&mut self, session: Session) {
        self.upsert_at(session, now());
    }

    /// [`SessionStore::upsert`] with an explicit sign-in time, in Unix seconds.
    pub fn upsert_at(&mut self, session: Session, signed_in_at: u64) {
        self.signed_in_at.insert(session.sub.clone(), signed_in_at);
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
        self.signed_in_at.remove(sub);
        self.sessions.retain(|held| held.sub != sub);
    }

    /// Removes every session that the user signed into more than `max_age`
    /// before `now`. `None` keeps every session. Returns true when the
    /// store changed and needs a save.
    ///
    /// A session from an older version has no sign-in time. The call stamps
    /// it with `now`, so it lasts one more `max_age` and the vault gets the
    /// stamp on the next save.
    pub fn expire(&mut self, max_age: Option<SessionMaxAge>, now: u64) -> bool {
        let Some(max_age) = max_age else {
            return false;
        };
        let mut changed = false;
        for session in &self.sessions {
            if !self.signed_in_at.contains_key(&session.sub) {
                self.signed_in_at.insert(session.sub.clone(), now);
                changed = true;
            }
        }
        let limit = max_age.seconds();
        let signed_in_at = &self.signed_in_at;
        let before = self.sessions.len();
        self.sessions
            .retain(|held| now.saturating_sub(signed_in_at[&held.sub]) < limit);
        if self.sessions.len() != before {
            self.signed_in_at
                .retain(|sub, _| self.sessions.iter().any(|held| &held.sub == sub));
            changed = true;
        }
        changed
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
    fn expire_signs_out_the_old_sessions() {
        let vault = Memory::default();
        let mut store = SessionStore::with_vault(Box::new(vault.clone()));
        store.upsert_at(session("old", "Ada"), 1_000);
        store.upsert_at(session("new", "Bea"), 5_000);
        let max_age = Some(SessionMaxAge {
            count: 1,
            unit: AgeUnit::Hours,
        });

        assert!(!store.expire(max_age, 4_599));
        assert_eq!(subs(&store), vec!["old".to_string(), "new".to_string()]);
        assert!(store.expire(max_age, 4_600));
        assert_eq!(subs(&store), vec!["new".to_string()]);
        assert!(!store.expire(None, u64::MAX));

        store.save().unwrap();
        let loaded = SessionStore::with_vault(Box::new(vault));
        assert_eq!(loaded.signed_in_at.get("new"), Some(&5_000));
    }

    #[test]
    fn expire_stamps_a_session_from_an_older_version() {
        let vault = Memory::default();
        vault
            .write(r#"[{"session_id":"id-a","display_name":"Ada","suffix":"1","sub":"a"}]"#)
            .unwrap();
        let mut store = SessionStore::with_vault(Box::new(vault));
        let max_age = Some(SessionMaxAge {
            count: 2,
            unit: AgeUnit::Days,
        });

        assert!(store.expire(max_age, 100));
        assert_eq!(subs(&store), vec!["a".to_string()]);
        assert_eq!(store.signed_in_at.get("a"), Some(&100));
        assert!(!store.expire(max_age, 100 + 2 * 86_400 - 1));
        assert!(store.expire(max_age, 100 + 2 * 86_400));
        assert!(store.sessions().is_empty());
    }

    #[test]
    fn the_age_units_give_seconds() {
        let age = |count, unit| SessionMaxAge { count, unit }.seconds();
        assert_eq!(age(3, AgeUnit::Hours), 3 * 3600);
        assert_eq!(age(3, AgeUnit::Days), 3 * 86_400);
        assert_eq!(age(2, AgeUnit::Weeks), 14 * 86_400);
        assert_eq!(age(19, AgeUnit::Months), 19 * 30 * 86_400);
        let json = serde_json::to_string(&SessionMaxAge {
            count: 3,
            unit: AgeUnit::Days,
        })
        .unwrap();
        assert_eq!(json, r#"{"count":3,"unit":"days"}"#);
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
