//! The `home` command.
//!
//! The launcher gives the client its own home, so a launch never writes into the
//! home of another launcher. These commands show that home and change it.

use std::path::PathBuf;

use bolt_core::{system_runelite_dir, Config, Paths, RuneLiteHome};

use crate::CliError;

/// Runs `rustybolt home` and `rustybolt home use`.
pub(crate) fn run(args: &[String]) -> Result<(), CliError> {
    match args.first().map(String::as_str) {
        None => show(),
        Some("use") => use_home(&args[1..]),
        Some(other) => Err(CliError::Usage(format!(
            "`{other}` is not a part of `home`. Use `home` or `home use <isolated|system|<path>>`."
        ))),
    }
}

fn show() -> Result<(), CliError> {
    let paths = Paths::resolve()?;
    let config = Config::load(&paths);

    println!("Home kind:         {}", config.runelite_home_kind);
    println!(
        "RuneLite home:     {}",
        config.runelite_home(&paths).join(".runelite").display()
    );
    println!("Profile directory: {}", config.profile_dir(&paths).display());
    match system_runelite_dir() {
        Some(dir) if dir.is_dir() => {
            println!("System home:       {} (present)", dir.display());
            println!("Run `rustybolt import` to copy it into the launcher home.");
        }
        Some(dir) => println!("System home:       {} (absent)", dir.display()),
        None => println!("System home:       unknown, because HOME is absent"),
    }
    Ok(())
}

fn use_home(args: &[String]) -> Result<(), CliError> {
    let Some(value) = args.first() else {
        return Err(CliError::Usage(
            "`home use` needs `isolated`, `system` or a directory path.".to_string(),
        ));
    };

    let kind = match value.as_str() {
        "isolated" | "private" => RuneLiteHome::Isolated,
        "system" => RuneLiteHome::System,
        path => RuneLiteHome::Custom(PathBuf::from(path)),
    };

    let paths = Paths::resolve()?;
    let mut config = Config::load(&paths);
    config.runelite_home_kind = kind;
    config.save(&paths)?;

    println!("Home kind:     {}", config.runelite_home_kind);
    println!(
        "RuneLite home: {}",
        config.runelite_home(&paths).join(".runelite").display()
    );
    Ok(())
}
