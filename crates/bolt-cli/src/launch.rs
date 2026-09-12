//! The `launch` command.
//!
//! The command resolves the jar and the login values, and then starts the
//! client. With `--dry-run` it prints the command line only.

use std::path::PathBuf;

use bolt_core::{
    AuthConfig, Character, ClientKind, Config, CredentialSource, GameCredentials, HttpAuth,
    Installer, LaunchRequest, Paths, SessionStore, UsageStore,
};

use crate::{client_kind, flag_value, pick_session, short_secret, CliError};

/// The arguments of one launch.
struct Options {
    kind: ClientKind,
    sub: Option<String>,
    character: Option<String>,
    configure: bool,
    jar: Option<PathBuf>,
    dry_run: bool,
    show_env: bool,
}

/// Runs `rustybolt launch <client> [flags]`.
pub(crate) fn run(args: &[String]) -> Result<(), CliError> {
    let options = parse(args)?;
    let paths = Paths::resolve()?;
    let config = Config::load(&paths);

    let jar = resolve_jar(&paths, &config, &options)?;
    let credentials = resolve_credentials(&paths, &config, &options)?;
    let template = match options.kind {
        ClientKind::RuneLite => config.runelite_launch_command.as_deref(),
        ClientKind::Hdos => config.hdos_launch_command.as_deref(),
    };

    let request = LaunchRequest {
        jar: &jar,
        kind: options.kind,
        credentials: credentials.as_ref(),
        java: None,
        template,
        configure: options.configure,
    };

    if options.dry_run {
        // The plan is the command that `launch` runs, so the two never differ.
        let plan = bolt_core::plan(&paths, &request)?;
        println!("program: {}", plan.program.display());
        for argument in &plan.args {
            println!("{argument}");
        }
        for (name, value) in &plan.env {
            if options.show_env {
                // The value may be a login secret, so only its head is shown.
                println!("env: {name}={}", short_secret(value));
            } else {
                println!("env: {name}");
            }
        }
        return Ok(());
    }

    let pid = bolt_core::launch(&paths, &request)?;
    println!("Started the client. The process id is {pid}.");
    if let Some(credentials) = &credentials {
        record_use(&paths, &credentials.character_id);
    }
    Ok(())
}

/// Records the use of one character for the order rule.
///
/// A failure to save the record gives a warning. It never stops the launch.
fn record_use(paths: &Paths, character_id: &str) {
    let mut usage = UsageStore::load(paths);
    usage.mark_used(character_id);
    if let Err(error) = usage.save() {
        eprintln!("rustybolt: cannot save the use record: {error}");
    }
}

/// Reads the client name and the flags.
fn parse(args: &[String]) -> Result<Options, CliError> {
    let name = args
        .first()
        .ok_or_else(|| CliError::Message("`launch` needs a client name".to_string()))?;
    let kind = client_kind(name)?;

    let mut options = Options {
        kind,
        sub: None,
        character: None,
        configure: false,
        jar: None,
        dry_run: false,
        show_env: false,
    };

    let mut index = 1;
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
            "--configure" => {
                options.configure = true;
                index += 1;
            }
            "--dry-run" => {
                options.dry_run = true;
                index += 1;
            }
            "--show-env" => {
                options.show_env = true;
                index += 1;
            }
            other => {
                return Err(CliError::Message(format!(
                    "`{other}` is not an argument of `launch`"
                )))
            }
        }
    }
    Ok(options)
}

/// Selects the jar file: the flag, then the installed client, then the config.
fn resolve_jar(paths: &Paths, config: &Config, options: &Options) -> Result<PathBuf, CliError> {
    let jar = if let Some(jar) = &options.jar {
        jar.clone()
    } else if let Some(client) = Installer::new(paths).installed(options.kind) {
        client.jar
    } else if options.kind == ClientKind::RuneLite && config.runelite_use_custom_jar {
        match &config.runelite_custom_jar {
            Some(jar) => jar.clone(),
            None => {
                return Err(CliError::Message(
                    "the config selects a custom jar, but it holds no jar path".to_string(),
                ))
            }
        }
    } else {
        return Err(CliError::Message(format!(
            "the {} client is not installed. Run `rustybolt install {}`.",
            options.kind.name(),
            options.kind.name()
        )));
    };

    if !jar.exists() {
        return Err(CliError::Message(format!(
            "the jar file does not exist: {}",
            jar.display()
        )));
    }
    Ok(jar)
}

/// Selects the login values: the session and one character of it.
///
/// The characters follow the use order, so the character of the last launch
/// comes first. A credential command gives the values of one item instead.
fn resolve_credentials(
    paths: &Paths,
    config: &Config,
    options: &Options,
) -> Result<Option<GameCredentials>, CliError> {
    let store = SessionStore::load(paths);
    let session = match pick_session(&store, options.sub.as_deref()) {
        Ok(session) => session,
        Err(error) => {
            if options.sub.is_none() && options.character.is_none() {
                eprintln!("rustybolt: no saved session. The client starts without login values.");
                return Ok(None);
            }
            return Err(error);
        }
    };

    let config_http = AuthConfig::default();
    let http = HttpAuth::new(&config_http);
    let characters = http.characters(&session.session_id).map_err(|error| match error {
        bolt_core::CoreError::SessionExpired => CliError::Message(
            "the saved session expired. Run `rustybolt login` again.".to_string(),
        ),
        other => CliError::Core(other),
    })?;

    let usage = UsageStore::load(paths);
    let ordered = usage.order(&characters, config.usage_recent_window_secs, |character| {
        character.account_id.as_str()
    });

    if let CredentialSource::Command(command) = &config.credential_source {
        // The flag names the item of the command. Without the flag the item is
        // the display name of the first character of the use order.
        let item = match &options.character {
            Some(wanted) => wanted.clone(),
            None => pick_character(&characters, &ordered, options)?
                .display_name
                .clone(),
        };
        return Ok(Some(command.fetch(&item)?));
    }

    let character = pick_character(&characters, &ordered, options)?;
    Ok(Some(GameCredentials {
        session_id: session.session_id.clone(),
        character_id: character.account_id.clone(),
        display_name: character.display_name.clone(),
    }))
}

/// Selects one character: the flag, then the first character of the use order.
fn pick_character<'a>(
    characters: &'a [Character],
    ordered: &[&'a Character],
    options: &Options,
) -> Result<&'a Character, CliError> {
    match &options.character {
        Some(wanted) => characters
            .iter()
            .find(|character| &character.account_id == wanted)
            .ok_or_else(|| {
                CliError::Message("this account has no character with that id".to_string())
            }),
        None => ordered
            .first()
            .copied()
            .ok_or_else(|| CliError::Message("this account has no character".to_string())),
    }
}

