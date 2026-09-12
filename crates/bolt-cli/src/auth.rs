//! The `accounts` and `login` commands.
//!
//! The login needs a browser. The user opens the authorization URL and
//! pastes the address bar of every redirect back into this program.

use std::io::Write;

use bolt_core::{Action, AuthConfig, Config, HttpAuth, LoginFlow, Paths, SessionStore, UsageStore};

use crate::{flag_value, no_arguments, pick_session, CliError};

/// Lists the characters of one saved session.
pub(crate) fn accounts(args: &[String]) -> Result<(), CliError> {
    let mut sub: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--sub" => {
                sub = Some(flag_value(args, index)?);
                index += 2;
            }
            other => {
                return Err(CliError::Message(format!(
                    "`{other}` is not an argument of `accounts`"
                )))
            }
        }
    }

    let paths = Paths::resolve()?;
    let store = SessionStore::load(&paths);
    let session = pick_session(&store, sub.as_deref())?;

    let auth = AuthConfig::default();
    let http = HttpAuth::new(&auth);
    let characters = http
        .characters(&session.session_id)
        .map_err(|error| match error {
            bolt_core::CoreError::SessionExpired => CliError::Message(
                "the saved session expired. Run `rustybolt login` again.".to_string(),
            ),
            other => CliError::Core(other),
        })?;

    if characters.is_empty() {
        println!("This account has no character.");
        return Ok(());
    }
    // A character that the user opened inside the recent window comes first.
    let usage = UsageStore::load(&paths);
    let window = Config::load(&paths).usage_recent_window_secs;
    for character in usage.order(&characters, window, |character| {
        character.account_id.as_str()
    }) {
        println!("{} ({})", character.display_name, character.account_id);
    }
    Ok(())
}

/// Drives the login flow with the user.
pub(crate) fn login(args: &[String]) -> Result<(), CliError> {
    no_arguments(args)?;
    let paths = Paths::resolve()?;
    let config = AuthConfig::default();
    let mut flow = LoginFlow::new(config.clone());
    let http = HttpAuth::new(&config);

    println!("Open this URL in a browser:");
    println!("{}", flow.authorize_url());
    println!("Log in there. The browser then leaves for the redirect page.");
    println!("Copy the whole address bar of that page. Paste it here.");

    loop {
        let address = read_address()?;
        let action = flow.on_navigation(&address)?;
        let action = http.advance(&mut flow, action)?;
        match action {
            Action::Ignore => {
                println!("That address is not part of the flow. Paste the next address.");
            }
            Action::Navigate { url } => {
                println!("The provider needs one more page. Open this URL:");
                println!("{url}");
                println!("Copy the address bar of the page after that redirect. Paste it here.");
            }
            Action::Done(session) => {
                let name = session.display_name.clone();
                let mut store = SessionStore::load(&paths);
                store.upsert(session);
                store.save()?;
                println!("Saved the session of {name}.");
                return Ok(());
            }
            Action::PostForm { .. } | Action::PostJson { .. } => {
                return Err(CliError::Message(
                    "the flow asked for a request after the request ran".to_string(),
                ))
            }
        }
    }
}

/// Reads one address from the standard input.
fn read_address() -> Result<String, CliError> {
    print!("Address: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    let count = std::io::stdin().read_line(&mut line)?;
    if count == 0 {
        return Err(CliError::Message("the input closed".to_string()));
    }
    Ok(line.trim().to_string())
}
