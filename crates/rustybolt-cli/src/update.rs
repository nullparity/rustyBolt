//! The release check behind the version pill of the dashboard.

use std::sync::Arc;
use std::time::Duration;

use nullparity_update::{Config as UpdateConfig, Updater};
use rustybolt_core::Config;

/// The config wins over `GITHUB_TOKEN`.
pub(crate) fn token(config: &Config) -> Option<String> {
    config
        .github_token
        .clone()
        .filter(|t| !t.trim().is_empty())
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
}

/// How often the launcher asks GitHub for a newer release.
const CHECK_INTERVAL: Duration = Duration::from_secs(3600);

/// Starts the updater with a check now and one every hour. Every request
/// passes the egress allowlist of `rustybolt-security`.
pub(crate) fn start(config: &Config) -> Arc<Updater> {
    let updater = Updater::new(
        UpdateConfig::new(
            "nullparity/rustyBolt",
            "rustybolt-cli",
            env!("CARGO_PKG_VERSION"),
        )
        .binary("rustybolt")
        .token(token(config))
        .url_check(Box::new(|url| {
            rustybolt_security::validate_url(url).map_err(|e| e.to_string())
        })),
    );
    updater.check_every(CHECK_INTERVAL);
    updater
}
