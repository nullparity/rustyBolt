//! The release check behind the version pill of the dashboard.
//!
//! The check runs once in the background when the launcher starts, and
//! again when the page asks. The page shows the result, and the install
//! swaps the binary and restarts the launcher.

use std::sync::{Arc, Mutex};
use std::thread;

use rustybolt_core::Config;
use rustybolt_update::{InstallKind, Release, Updater};
use serde::Serialize;

/// What the page needs to draw the version pill.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct UpdateState {
    pub current: &'static str,
    /// False until the first check answers.
    pub checked: bool,
    pub release: Option<Release>,
    pub error: Option<String>,
    pub install: InstallKind,
}

impl UpdateState {
    fn new() -> UpdateState {
        UpdateState {
            current: env!("CARGO_PKG_VERSION"),
            checked: false,
            release: None,
            error: None,
            install: rustybolt_update::install_kind(),
        }
    }
}

pub(crate) type Shared = Arc<Mutex<UpdateState>>;

/// The config wins over `GITHUB_TOKEN`.
fn token(config: &Config) -> Option<String> {
    config
        .github_token
        .clone()
        .filter(|t| !t.trim().is_empty())
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
}

/// Runs the check now and stores the answer.
pub(crate) fn check(state: &Shared, config: &Config) {
    let result = Updater::new(token(config)).newer_than(env!("CARGO_PKG_VERSION"));
    let mut held = state.lock().unwrap_or_else(|e| e.into_inner());
    held.checked = true;
    match result {
        Ok(release) => {
            held.release = release;
            held.error = None;
        }
        Err(error) => held.error = Some(error.to_string()),
    }
}

/// Starts the state with one check in the background.
pub(crate) fn start(config: &Config) -> Shared {
    let state = Arc::new(Mutex::new(UpdateState::new()));
    let config = config.clone();
    let thread_state = Arc::clone(&state);
    thread::spawn(move || check(&thread_state, &config));
    state
}

/// Installs the release the last check found. Returns the path of the new
/// binary, so the caller can restart it.
pub(crate) fn install(state: &Shared, config: &Config) -> Result<std::path::PathBuf, String> {
    let (release, install) = {
        let held = state.lock().unwrap_or_else(|e| e.into_inner());
        (held.release.clone(), held.install.clone())
    };
    let Some(release) = release else {
        return Err("no update is known; check again first".to_string());
    };
    if let InstallKind::Managed { reason } = install {
        return Err(format!(
            "this launcher was installed as {reason}; take the new version from {}",
            release.page_url
        ));
    }
    Updater::new(token(config))
        .install(&release)
        .map_err(|error| error.to_string())
}
