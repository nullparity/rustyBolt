//! The `install` command.
//!
//! The command finds the newest release of a client and downloads it.

use std::io::Write;

use bolt_core::{Installer, Paths};

use crate::{client_kind, CliError};

/// Installs one game client.
pub(crate) fn run(args: &[String]) -> Result<(), CliError> {
    let name = args
        .first()
        .ok_or_else(|| CliError::Message("`install` needs a client name".to_string()))?;
    if args.len() > 1 {
        return Err(CliError::Message(format!(
            "`{}` is not an argument of `install`",
            args[1]
        )));
    }
    let kind = client_kind(name)?;
    let paths = Paths::resolve()?;
    let installer = Installer::new(&paths);

    println!("Looking for the newest {} release.", kind.name());
    let release = installer.latest(kind)?;
    println!("Version {}. The download starts now.", release.version);

    let mut out = std::io::stdout();
    let client = installer.install(kind, &release, &mut |done, total| {
        let text = match total {
            Some(total) if total > 0 => format!("{}%", done.saturating_mul(100) / total),
            Some(_) => "100%".to_string(),
            None => format!("{done} bytes"),
        };
        let _ = write!(out, "\r{text}    ");
        let _ = out.flush();
    })?;
    println!();
    println!(
        "Installed {} version {}.",
        client.kind.name(),
        client.version
    );
    Ok(())
}
