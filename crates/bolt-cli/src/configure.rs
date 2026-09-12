//! The `configure` command.
//!
//! The command opens the settings window of the native application.

use crate::{no_arguments, CliError};

/// Runs `rustybolt configure`.
pub(crate) fn run(args: &[String]) -> Result<(), CliError> {
    no_arguments(args)?;
    open_settings()
}

#[cfg(target_os = "macos")]
fn open_settings() -> Result<(), CliError> {
    use std::process::Command;

    // The installed application first. `open` fails when no such application exists.
    let status = Command::new("open")
        .args(["-a", "rustyBolt", "--args", "--advanced"])
        .status();
    if matches!(status, Ok(status) if status.success()) {
        return Ok(());
    }

    // A build from source keeps the application binary next to this one.
    let sibling = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("rustybolt-macos")));
    if let Some(binary) = sibling.filter(|path| path.is_file()) {
        Command::new(binary).arg("--advanced").spawn()?;
        return Ok(());
    }

    Err(CliError::Message(
        "rustyBolt.app is not installed. Move it to Applications, then run `rustybolt configure` again."
            .to_string(),
    ))
}

#[cfg(not(target_os = "macos"))]
fn open_settings() -> Result<(), CliError> {
    Err(CliError::Message(
        "the settings window exists on macOS only in this release".to_string(),
    ))
}
