//! The `login` command.
//!
//! The login needs a browser. The user opens the authorization URL and
//! pastes the address bar of every redirect back into this program.

use std::io::Write;

use bolt_core::{Action, AuthConfig, HttpAuth, LoginFlow, Paths, SessionStore};

use crate::{no_arguments, CliError};

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
