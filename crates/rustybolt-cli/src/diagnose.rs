//! The `diagnose` command and the shared bundle builder.

use std::path::{Path, PathBuf};

use rustybolt_core::diagnose::{collect, write_zip, Accounts, Redactor};
use rustybolt_core::{AuthConfig, Config, HttpAuth, Paths, SessionStore};

use crate::CliError;

/// Writes the bundle into `dir` and returns its path. The character counts
/// come from the account service; an account that does not answer counts as
/// unknown.
pub(crate) fn write_bundle(paths: &Paths, dir: &Path) -> std::io::Result<PathBuf> {
    let config = Config::load(paths);
    let store = SessionStore::load_active(paths, &config);
    let auth = AuthConfig::default();
    let http = HttpAuth::new(&auth);

    let mut secrets = Vec::new();
    let mut counts = Vec::new();
    for session in store.sessions() {
        secrets.push((session.session_id.clone(), "<session>"));
        secrets.push((session.sub.clone(), "<account>"));
        secrets.push((session.display_name.clone(), "<account>"));
        let characters = http.characters(&session.session_id).ok();
        if let Some(characters) = &characters {
            for character in characters {
                secrets.push((character.display_name.clone(), "<character>"));
                secrets.push((character.account_id.clone(), "<character>"));
            }
        }
        counts.push(characters.map(|c| c.len()));
    }
    let redactor = Redactor::new(secrets);
    let accounts = Accounts {
        character_counts: counts,
    };
    let files = collect(
        paths,
        &config,
        &accounts,
        &redactor,
        env!("CARGO_PKG_VERSION"),
    );

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("rustybolt-diagnostics-{stamp}.zip"));
    write_zip(&path, &files)?;
    Ok(path)
}

/// Runs `rustybolt diagnose [directory]`. The default directory is the
/// current one.
pub(crate) fn run(args: &[String]) -> Result<(), CliError> {
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let paths = Paths::resolve()?;
    let path = write_bundle(&paths, &dir)?;
    println!("{}", path.display());
    println!("The bundle holds no account, character or user names, ids, addresses or emails. Attach it to a bug report.");
    Ok(())
}
