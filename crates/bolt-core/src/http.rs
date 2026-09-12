//! The network steps of the OAuth2 flow.

use bolt_auth::{Action, AuthConfig, Character, LoginFlow};

use crate::CoreError;

/// Runs the network requests that `bolt-auth` asks for.
pub struct HttpAuth<'a> {
    config: &'a AuthConfig,
}

impl<'a> HttpAuth<'a> {
    /// Builds the driver for this auth configuration.
    pub fn new(config: &'a AuthConfig) -> HttpAuth<'a> {
        HttpAuth { config }
    }

    /// Runs every network step until the flow needs the user again.
    ///
    /// `Navigate`, `Ignore` and `Done` come back without a request.
    pub fn advance(&self, flow: &mut LoginFlow, action: Action) -> Result<Action, CoreError> {
        let mut next = action;
        loop {
            match next {
                Action::PostForm { url, body } => {
                    let text = post(&url, "application/x-www-form-urlencoded", &body)?;
                    next = flow.on_token_response(&text)?;
                }
                Action::PostJson { url, body } => {
                    let text = post(&url, "application/json", &body)?;
                    next = Action::Done(flow.on_session_response(&text)?);
                }
                other => return Ok(other),
            }
        }
    }

    pub fn characters(&self, session_id: &str) -> Result<Vec<Character>, CoreError> {
        let (url, (name, value)) = bolt_auth::accounts_request(self.config, session_id);
        let mut response = match ureq::get(&url).header(&name, &value).call() {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(401)) => return Err(CoreError::SessionExpired),
            Err(error) => return Err(error.into()),
        };
        let text = response.body_mut().read_to_string()?;
        Ok(bolt_auth::parse_accounts(&text)?)
    }
}

fn post(url: &str, content_type: &str, body: &str) -> Result<String, CoreError> {
    let mut response = ureq::post(url)
        .header("Accept", "application/json")
        .content_type(content_type)
        .send(body)?;
    Ok(response.body_mut().read_to_string()?)
}
