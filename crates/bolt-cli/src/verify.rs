//! The `verify` command.
//!
//! The command runs every step of a launch except the last one. It prints the
//! jar, the session, the Java runtime and the command line, so a user can see
//! what a launch does before it starts a client.

use std::path::PathBuf;

use bolt_core::{ClientKind, Config, LaunchRequest, Paths};

use crate::launch::{resolve_credentials, resolve_jar, Options};
use crate::{client_kind, flag_value, CliError};

/// Runs `rustybolt verify [runelite|hdos] [flags]`.
pub(crate) fn run(args: &[String]) -> Result<(), CliError> {
    let options = parse(args)?;
    let paths = Paths::resolve()?;
    let config = Config::load(&paths);

    let jar = resolve_jar(&config, &options)?;
    println!("Client: {}", jar.display());

    let credentials = resolve_credentials(&paths, &config, &options)?
        .ok_or_else(|| CliError::Message("no saved session. Run `rustybolt login`.".to_string()))?;
    println!("Character: {}", credentials.display_name);

    let template = match options.kind {
        ClientKind::RuneLite => config.runelite_launch_command.as_deref(),
        ClientKind::Hdos => config.hdos_launch_command.as_deref(),
    };
    let request = LaunchRequest {
        jar: &jar,
        kind: options.kind,
        credentials: Some(&credentials),
        java: None,
        template,
        configure: false,
    };
    let plan = bolt_core::plan(&paths, &request)?;

    let version = bolt_jdk::probe(&plan.program)
        .and_then(|runtime| runtime.version)
        .map(|version| version.raw)
        .unwrap_or_else(|| "unknown version".to_string());
    println!("Java: {} ({version})", plan.program.display());

    println!("Command line:");
    let mut parts = vec![plan.program.to_string_lossy().into_owned()];
    parts.extend(plan.args.iter().cloned());
    println!("  {}", parts.join(" "));
    println!(
        "Every step passed. `rustybolt launch {}` starts the client.",
        options.kind.name()
    );
    Ok(())
}

/// Reads the client name and the flags. The client is RuneLite by default.
fn parse(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options {
        kind: ClientKind::RuneLite,
        sub: None,
        character: None,
        configure: false,
        jar: None,
        dry_run: true,
        show_env: false,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--sub" => {
                options.sub = Some(flag_value(args, index)?);
                index += 2;
            }
            "--character" => {
                options.character = Some(flag_value(args, index)?);
                index += 2;
            }
            "--jar" => {
                options.jar = Some(PathBuf::from(flag_value(args, index)?));
                index += 2;
            }
            name if index == 0 && !name.starts_with("--") => {
                options.kind = client_kind(name)?;
                index += 1;
            }
            other => {
                return Err(CliError::Message(format!(
                    "`{other}` is not an argument of `verify`"
                )))
            }
        }
    }
    Ok(options)
}
