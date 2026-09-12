//! Headless command line driver for the rustyBolt core.
//!
//! The program shows every core function with no window toolkit.
//! It holds the command line only. The core holds the logic.

mod auth;
mod home;
mod import;
mod info;
mod install;
mod java;
mod launch;
mod tuning;

use bolt_auth::Session;
use bolt_core::{ClientKind, CoreError, SessionStore};

/// The usage text of the program.
const USAGE: &str = "\
rustybolt is the headless driver of the rustyBolt core.

Usage:
  rustybolt java list
  rustybolt java select [--min <n>]
  rustybolt java use <path|auto>
  rustybolt paths
  rustybolt config
  rustybolt sessions
  rustybolt accounts [--sub <sub>]
  rustybolt login
  rustybolt install <runelite|hdos>
  rustybolt launch <runelite|hdos> [--sub <sub>] [--character <id>]
                  [--configure] [--jar <path>] [--dry-run] [--show-env]
  rustybolt usage
  rustybolt tuning [on|off]
  rustybolt tuning set <key> <value>
  rustybolt tuning reset
  rustybolt tuning flags [--feature <n>]
  rustybolt profile apply [--dry-run]
  rustybolt home
  rustybolt home use <isolated|system|<path>>
  rustybolt import [--overwrite] [--secrets] [--dry-run]
  rustybolt help

The default minimum Java feature is 11.
A login writes a session into the session file.
A launch uses the first saved session, unless --sub selects another one.
A launch dry run shows the environment names only. `--show-env` shows the
first 8 characters of every value.

The launcher gives the client its own RuneLite home, so a launch never writes
into `~/.runelite`. `home` shows that directory and `home use` changes it.
`import` copies the real `~/.runelite` into the launcher home. It only reads
the real home. It leaves the login files alone, unless `--secrets` asks for
them, and it keeps a file that the target already holds, unless `--overwrite`
asks to replace it.

`java use auto` clears the chosen Java binary.
`tuning set` takes these keys:
  heap_min, heap_max, stack_size, garbage_collector,
  compact_object_headers, string_deduplication, native_access, aot_cache,
  gc_log, java2d_metal, launcher_nojvm, application_name, process_name,
  extra_jvm_args, extra_app_args
A setting key takes `on`, `off`, `true` or `false`.
`garbage_collector` takes `default`, `z`, `g1` or `parallel`.
An optional text key takes `none` to clear it.
`tuning flags` uses the feature number of the chosen Java runtime, unless
`--feature` gives another one. It names every option that the feature gate
removes. The four tuned options need feature 24 or newer.
";

/// The result of one command.
pub(crate) enum CliError {
    /// The core, the Java crate or a file operation failed.
    Core(CoreError),
    /// The command line is not valid, or a step of the command failed.
    Message(String),
    /// The program does not know this command.
    Unknown(String),
    /// The command line is not valid, and the message holds the valid values.
    Usage(String),
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            CliError::Unknown(_) | CliError::Usage(_) => 2,
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
            CliError::Message(text) | CliError::Usage(text) => write!(formatter, "{text}"),
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

impl From<bolt_auth::AuthError> for CliError {
    fn from(error: bolt_auth::AuthError) -> CliError {
        CliError::Core(CoreError::Auth(error))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = dispatch(&args) {
        error.report();
        std::process::exit(i32::from(error.exit_code()));
    }
}

/// Runs the command of the command line.
fn dispatch(args: &[String]) -> Result<(), CliError> {
    let Some(command) = args.first().map(String::as_str) else {
        print!("{USAGE}");
        return Ok(());
    };
    let rest = &args[1..];

    match command {
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(())
        }
        "java" => java::run(rest),
        "paths" => info::paths(rest),
        "config" => info::config(rest),
        "sessions" => info::sessions(rest),
        "accounts" => auth::accounts(rest),
        "login" => auth::login(rest),
        "install" => install::run(rest),
        "launch" => launch::run(rest),
        "usage" => info::usage(rest),
        "tuning" => tuning::run(rest),
        "profile" => tuning::profile(rest),
        "home" => home::run(rest),
        "import" => import::run(rest),
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
                CliError::Message("no saved session has this sub. Run `rustybolt login` first.".to_string())
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
