//! The saved login sessions.

use std::fs;
use std::io;
use std::path::PathBuf;

use bolt_auth::Session;

use crate::Paths;
use crate::file::write_private;

/// The sessions of the user, and the file that holds them.
pub struct SessionStore {
    path: PathBuf,
    sessions: Vec<Session>,
}

impl SessionStore {
    /// Reads the session file. An absent or malformed file gives no sessions.
    pub fn load(paths: &Paths) -> SessionStore {
        let path = paths.credentials_file();
        let sessions = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<Session>>(&text).ok())
            .unwrap_or_default();
        SessionStore { path, sessions }
    }

    /// Writes the sessions as JSON.
    ///
    /// The write goes through a temporary file, and the mode is 0600 on unix.
    pub fn save(&self) -> io::Result<()> {
        let text = serde_json::to_vec_pretty(&self.sessions)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_private(&self.path, &text, 0o600)
    }

    /// Adds a session, or replaces the session with the same `sub` value.
    pub fn upsert(&mut self, session: Session) {
        match self.sessions.iter_mut().find(|held| held.sub == session.sub) {
            Some(held) => *held = session,
            None => self.sessions.push(session),
        }
    }

    /// Removes the session with this `sub` value. An absent value is not an error.
    pub fn remove(&mut self, sub: &str) {
        self.sessions.retain(|held| held.sub != sub);
    }

    /// The sessions, in file order.
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TempDir, paths};

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
        let temp = TempDir::new("sessions-upsert");
        let mut store = SessionStore::load(&paths(temp.path()));
        store.upsert(session("a", "Ada"));
        store.upsert(session("b", "Bea"));
        store.upsert(session("a", "Ada Two"));

        assert_eq!(subs(&store), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(store.sessions()[0].display_name, "Ada Two");
        assert_eq!(store.sessions()[1].display_name, "Bea");
    }

    #[test]
    fn remove_drops_only_the_wanted_sub() {
        let temp = TempDir::new("sessions-remove");
        let mut store = SessionStore::load(&paths(temp.path()));
        store.upsert(session("a", "Ada"));
        store.upsert(session("b", "Bea"));
        store.remove("a");

        assert_eq!(subs(&store), vec!["b".to_string()]);
        store.remove("missing");
        assert_eq!(subs(&store), vec!["b".to_string()]);
    }

    #[test]
    fn save_and_load_keeps_the_sessions() {
        let temp = TempDir::new("sessions");
        let paths = paths(temp.path());
        let mut store = SessionStore::load(&paths);
        store.upsert(session("a", "Ada"));
        store.save().unwrap();

        let loaded = SessionStore::load(&paths);
        assert_eq!(subs(&loaded), vec!["a".to_string()]);
        assert_eq!(loaded.sessions()[0].session_id, "id-a");
        assert_eq!(loaded.sessions()[0].suffix, "0001");
        assert_eq!(loaded.sessions()[0].display_name, "Ada");
    }

    #[test]
    fn an_absent_or_malformed_file_gives_no_sessions() {
        let temp = TempDir::new("sessions-bad");
        let paths = paths(temp.path());
        assert!(SessionStore::load(&paths).sessions().is_empty());

        fs::create_dir_all(&paths.config_dir).unwrap();
        fs::write(paths.credentials_file(), "not json").unwrap();
        assert!(SessionStore::load(&paths).sessions().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn save_uses_mode_0600() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new("sessions-mode");
        let paths = paths(temp.path());
        let mut store = SessionStore::load(&paths);
        store.upsert(session("a", "Ada"));
        store.save().unwrap();

        let mode = fs::metadata(paths.credentials_file())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
