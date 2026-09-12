//! Paths, configuration, session storage, client install and client launch.
//!
//! This crate holds the logic that Bolt keeps inside the launcher window:
//! the platform directories, the launcher config, the saved sessions, the
//! RuneLite and HDOS installers, and the child process launch.
//! The crate uses `bolt-auth` for the OAuth2 flow and `bolt-jdk` for Java.

mod import;
mod config;
mod credentials;
mod file;
mod http;
mod install;
mod launch;
mod paths;
mod profile;
mod session;
mod tuning;
mod usage;

pub use import::{import_apply, import_plan, ImportEntry, ImportPlan, RuneLiteHome, system_runelite_dir};
pub use bolt_auth::{Action, AuthConfig, Character, LoginFlow, Session};
pub use config::{Config, DEFAULT_RECENT_WINDOW};
pub use credentials::{CommandCredentials, CredentialFormat, CredentialSource};
pub use http::HttpAuth;
pub use install::{ClientKind, Installer, InstalledClient, Release};
pub use launch::{
    GameCredentials, LaunchPlan, LaunchRequest, client_invocation, client_options, launch, plan,
    tuned_client_options,
};
pub use paths::Paths;
pub use profile::{PropertyOverrides, apply_to_profiles};
pub use session::SessionStore;
pub use tuning::{GcChoice, TuningConfig};
pub use usage::UsageStore;

use thiserror::Error;

/// Errors that the core crate returns.
#[derive(Debug, Error)]
pub enum CoreError {
    /// A file or directory operation failed.
    #[error("input or output error: {0}")]
    Io(#[from] std::io::Error),
    /// A network request failed, or the server sent an unexpected answer.
    #[error("network error: {0}")]
    Http(String),
    /// An external command failed, or its output does not have the wanted shape.
    #[error("credential command error: {0}")]
    Command(String),
    /// A JSON document is not valid, or it does not have the wanted shape.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// The OAuth2 flow rejected a step.
    #[error("authentication error: {0}")]
    Auth(#[from] bolt_auth::AuthError),
    #[error("no Java runtime of version 11 or newer exists")]
    NoJava,
    /// The downloaded file does not have the digest that the release gives.
    #[error("sha256 mismatch: expected {expected}, got {actual}")]
    DigestMismatch {
        expected: String,
        actual: String,
    },
    /// The server rejected the session. The user must log in again.
    #[error("the session expired")]
    SessionExpired,
    #[error("the client is not installed")]
    NotInstalled,
    /// The user launch template is not valid.
    #[error("launch template error: {0}")]
    Template(#[from] bolt_jdk::TemplateError),
}

impl From<ureq::Error> for CoreError {
    fn from(error: ureq::Error) -> Self {
        CoreError::Http(error.to_string())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A temporary directory. The directory removes itself on drop.
    pub(crate) struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        pub(crate) fn new(label: &str) -> TempDir {
            let count = COUNTER.fetch_add(1, Ordering::Relaxed);
            let name = format!("bolt-core-{}-{}-{}", label, std::process::id(), count);
            let path = std::env::temp_dir().join(name);
            std::fs::create_dir_all(&path).expect("cannot create the temporary directory");
            TempDir { path }
        }

        pub(crate) fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// Builds `Paths` with the four directories below one root.
    pub(crate) fn paths(root: &Path) -> crate::Paths {
        crate::Paths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            runtime_dir: root.join("run"),
        }
    }
}
