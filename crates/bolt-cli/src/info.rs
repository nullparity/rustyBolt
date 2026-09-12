//! The `paths`, `config`, `sessions` and `usage` commands.
//!
//! The commands show the state of the core. Only the `usage` command reads
//! the use records of the launcher.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use bolt_core::{Config, Paths, SessionStore, UsageStore};

use crate::{no_arguments, short_secret, CliError};

/// The file name of the use records below the config directory.
const USAGE_FILE: &str = "usage.json";

/// The number of seconds in one minute. A shorter age shows as `just now`.
const MINUTE_SECS: u64 = 60;
/// The number of seconds in one hour.
const HOUR_SECS: u64 = 3600;
/// The number of seconds in one day.
const DAY_SECS: u64 = 86400;

/// Prints the four directories and the two files.
pub(crate) fn paths(args: &[String]) -> Result<(), CliError> {
    no_arguments(args)?;
    let paths = Paths::resolve()?;
    println!("config_dir:       {}", paths.config_dir.display());
    println!("data_dir:         {}", paths.data_dir.display());
    println!("cache_dir:        {}", paths.cache_dir.display());
    println!("runtime_dir:      {}", paths.runtime_dir.display());
    println!("config_file:      {}", paths.config_file().display());
    println!("credentials_file: {}", paths.credentials_file().display());
    Ok(())
}

/// Prints the config file as pretty JSON.
pub(crate) fn config(args: &[String]) -> Result<(), CliError> {
    no_arguments(args)?;
    let paths = Paths::resolve()?;
    let config = Config::load(&paths);
    println!("{}", serde_json::to_string_pretty(&config)?);
    Ok(())
}

/// Prints the use records of the recent use rule.
///
/// The function shows at most the first 8 characters of every key.
pub(crate) fn usage(args: &[String]) -> Result<(), CliError> {
    no_arguments(args)?;
    let paths = Paths::resolve()?;
    let store = UsageStore::load(&paths);
    let keys = stored_keys(&paths.config_dir.join(USAGE_FILE));
    if keys.is_empty() {
        println!("No use record exists.");
        return Ok(());
    }

    let window = Config::load(&paths).usage_recent_window_secs;
    for key in store.order(&keys, window, |key| key.as_str()) {
        let record = store.usage(key);
        println!(
            "{}  {}",
            short_secret(key),
            describe(record.count, record.last_used)
        );
    }
    Ok(())
}

/// Reads the key names of the use file.
///
/// An absent or malformed file gives no key. Only the names are read here,
/// because the store holds the values.
fn stored_keys(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(serde_json::Value::Object(map)) => map.into_iter().map(|(key, _)| key).collect(),
        _ => Vec::new(),
    }
}

/// Describes one use record in words.
fn describe(count: u64, last_used: u64) -> String {
    let uses = if count == 1 {
        "1 use".to_string()
    } else {
        format!("{count} uses")
    };
    match age(last_used) {
        Some(age) => format!("{uses}, {age}"),
        None => format!("{uses}, never used"),
    }
}

/// The age of one use time in words. Zero means that the key was never used.
fn age(last_used: u64) -> Option<String> {
    if last_used == 0 {
        return None;
    }
    let seconds = now_secs().saturating_sub(last_used);
    let text = if seconds < MINUTE_SECS {
        "just now".to_string()
    } else if seconds < HOUR_SECS {
        unit(seconds / MINUTE_SECS, "minute")
    } else if seconds < DAY_SECS {
        unit(seconds / HOUR_SECS, "hour")
    } else {
        unit(seconds / DAY_SECS, "day")
    };
    Some(text)
}

/// Builds `1 minute ago` or `3 minutes ago`.
fn unit(value: u64, name: &str) -> String {
    if value == 1 {
        format!("1 {name} ago")
    } else {
        format!("{value} {name}s ago")
    }
}

/// The present time in seconds since the epoch. A clock before the epoch gives zero.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Prints the saved sessions.
pub(crate) fn sessions(args: &[String]) -> Result<(), CliError> {
    no_arguments(args)?;
    let paths = Paths::resolve()?;
    let store = SessionStore::load(&paths);
    if store.sessions().is_empty() {
        println!("No saved session.");
        return Ok(());
    }
    for session in store.sessions() {
        println!(
            "{}#{}  sub={}  session={}",
            session.display_name,
            session.suffix,
            session.sub,
            short_secret(&session.session_id)
        );
    }
    Ok(())
}
