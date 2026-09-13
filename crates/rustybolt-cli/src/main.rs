//! Headless command line driver for the rustyBolt core.
//!
//! The program exposes four commands for end users: login, launch,
//! verify, and configure.

mod auth;
mod configure;
mod consent;
mod gui;
mod httpd;
mod i18n;
mod launch;
mod platform;
mod verify;
mod web;
pub(crate) mod wifi;

use rustybolt_auth::Session;
use rustybolt_core::{ClientKind, CoreError, SessionStore};

/// The usage text of the program.
const USAGE: &str = "\
rustybolt is the fast, portable launcher for RuneLite and HDOS.

Running `rustybolt` opens the native desktop launcher window.

Usage:
  rustybolt [configure] [--browser]
  rustybolt launch <runelite|hdos> [--sub <sub>] [--character <id>]
                  [--configure] [--jar <path>]
  rustybolt login
  rustybolt verify [runelite|hdos] [--sub <sub>] [--character <id>] [--jar <path>]
  rustybolt help
  rustybolt version

A default launch opens the native desktop application window.
Use `--browser` to open the interface in your default browser.
A login writes a session into the session file.
A launch uses the first saved session, unless --sub selects another one.
`rustybolt verify` tests the configuration and displays the command line.
`rustybolt configure` opens the launcher dashboard explicitly.
";

/// The result of one command.
pub(crate) enum CliError {
    /// The core, the Java crate or a file operation failed.
    Core(CoreError),
    /// The command line is not valid, or a step of the command failed.
    Message(String),
    /// The program does not know this command.
    Unknown(String),
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            CliError::Unknown(_) => 2,
            CliError::Core(_) | CliError::Message(_) => 1,
        }
    }

    /// Writes the error to the standard error stream.
    fn report(&self) {
        eprintln!("rustybolt: {self}");
        if let CliError::Unknown(_) = self {
            eprintln!("Run `rustybolt help` for the usage.");
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Core(error) => write!(formatter, "{error}"),
            CliError::Message(text) => write!(formatter, "{text}"),
            CliError::Unknown(name) => write!(formatter, "unknown command `{name}`"),
        }
    }
}

impl From<CoreError> for CliError {
    fn from(error: CoreError) -> CliError {
        CliError::Core(error)
    }
}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> CliError {
        CliError::Core(CoreError::Io(error))
    }
}

impl From<serde_json::Error> for CliError {
    fn from(error: serde_json::Error) -> CliError {
        CliError::Core(CoreError::Json(error))
    }
}

impl From<rustybolt_auth::AuthError> for CliError {
    fn from(error: rustybolt_auth::AuthError) -> CliError {
        CliError::Core(CoreError::Auth(error))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // help and version print static text; they must not need a keychain.
    let informational = matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h" | "version" | "--version" | "-V")
    );
    let gate = if informational {
        Ok(())
    } else {
        require_keychain()
    };
    if let Err(error) = gate.and_then(|()| dispatch(&args)) {
        error.report();
        std::process::exit(i32::from(error.exit_code()));
    }
}

/// Refuses to run when the operating system has no keychain.
///
/// The Jagex session lives in the keychain only. A Linux desktop without a
/// Secret Service provider (GNOME Keyring, KDE Wallet, KeePassXC) cannot
/// hold it, so the launcher stops before it does any work.
fn require_keychain() -> Result<(), CliError> {
    rustybolt_core::keychain_available().map_err(|error| {
        CliError::Message(format!(
            "{error}\nrustyBolt stores your Jagex session in the system keychain and does not run without one.\n\
             On Linux, start a Secret Service provider such as GNOME Keyring or KDE Wallet and try again."
        ))
    })
}

/// Runs the command of the command line.
fn dispatch(args: &[String]) -> Result<(), CliError> {
    let Some(command) = args.first().map(String::as_str) else {
        return configure::run(&[]);
    };
    if command == "--browser" {
        return configure::run(args);
    }
    if command.starts_with("jagex:") {
        return configure::forward_redirect(command);
    }
    let rest = &args[1..];

    match command {
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("rustybolt {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "login" => auth::login(rest),
        "launch" => launch::run(rest),
        "verify" => verify::run(rest),
        "configure" => configure::run(rest),
        other => Err(CliError::Unknown(other.to_string())),
    }
}

/// Refuses arguments for a command that takes none.
pub(crate) fn no_arguments(args: &[String]) -> Result<(), CliError> {
    match args.first() {
        None => Ok(()),
        Some(extra) => Err(CliError::Message(format!(
            "this command takes no argument, but it got `{extra}`"
        ))),
    }
}

pub(crate) fn flag_value(args: &[String], index: usize) -> Result<String, CliError> {
    let Some(flag) = args.get(index) else {
        return Err(CliError::Message("a flag is absent".to_string()));
    };
    match args.get(index + 1) {
        Some(value) => Ok(value.clone()),
        None => Err(CliError::Message(format!("`{flag}` needs a value"))),
    }
}

/// Reads a client name from the command line.
pub(crate) fn client_kind(name: &str) -> Result<ClientKind, CliError> {
    match name {
        "runelite" => Ok(ClientKind::RuneLite),
        "hdos" => Ok(ClientKind::Hdos),
        other => Err(CliError::Message(format!(
            "`{other}` is not a client name. Use `runelite` or `hdos`."
        ))),
    }
}

/// Selects the session of one account.
///
/// A given `sub` selects that session. Without a value the function takes
/// the first session of the store.
pub(crate) fn pick_session<'a>(
    store: &'a SessionStore,
    sub: Option<&str>,
) -> Result<&'a Session, CliError> {
    match sub {
        Some(wanted) => store
            .sessions()
            .iter()
            .find(|session| session.sub == wanted)
            .ok_or_else(|| {
                CliError::Message(
                    "no saved session has this sub. Run `rustybolt login` first.".to_string(),
                )
            }),
        None => store.sessions().first().ok_or_else(|| {
            CliError::Message("no saved session exists. Run `rustybolt login` first.".to_string())
        }),
    }
}

/// Shows at most the first 8 characters of a secret value.
///
/// The function never prints a token or a session identifier in full.
pub(crate) fn short_secret(value: &str) -> String {
    let head: String = value.chars().take(8).collect();
    format!("{head}...")
}
