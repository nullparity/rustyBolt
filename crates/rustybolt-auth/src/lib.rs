//! Jagex OAuth2 login flow with PKCE, without input or output.
//!
//! The crate holds protocol logic only. The caller does every network request and
//! every navigation. `LoginFlow` accepts the result of each step and returns the
//! next action. A native shell, a web view and a command line tool can all drive
//! the same flow.
//!
//! The original Bolt launcher puts this logic inside a CEF window class. That design ties the protocol
//! to one user interface toolkit. This crate keeps the two apart.

use base64::Engine as _;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// The bytes that a URL query keeps as they are: the RFC 3986 unreserved set.
const QUERY_ESCAPES: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Default origin of the account service.
pub const ACCOUNT_ORIGIN: &str = "https://account.jagex.com";
/// Default origin of the game session service.
pub const AUTH_ORIGIN: &str = "https://auth.jagex.com";
pub const CLIENT_ID: &str = "com_jagex_auth_desktop_launcher";
/// Client identifier of the consent step.
pub const CONSENT_CLIENT_ID: &str = "1fddee4e-b100-4f4e-b2b0-097f9088f9d2";
/// Redirect target of the first step.
pub const REDIRECT_URL: &str = "https://secure.runescape.com/m=weblogin/launcher-redirect";
/// Scopes of the first step.
pub const DEFAULT_SCOPES: &str = "openid offline gamesso.token.create user.profile.read \
user.entitlement.read user.game.read user.sku.read user.voucher.redeem";

const REDIRECT_HOST: &str = "secure.runescape.com";
const REDIRECT_PATH: &str = "/m=weblogin/launcher-redirect";
const CONSENT_HOST: &str = "localhost";
const VERIFIER_LEN: usize = 43;
const STATE_LEN: usize = 12;

/// Characters of the code verifier. The set is unreserved, so no encode is necessary.
const VERIFIER_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._";
/// Characters of the state value and the nonce value.
const STATE_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Endpoints and identifiers of the login flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthConfig {
    /// Origin of the authorization and token endpoints.
    pub account_origin: String,
    /// Origin of the game session endpoints.
    pub auth_origin: String,
    /// Client identifier of the first step.
    pub client_id: String,
    pub consent_client_id: String,
    /// Redirect target of the first step.
    pub redirect_url: String,
    /// Space separated scope list of the first step.
    pub scopes: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            account_origin: ACCOUNT_ORIGIN.to_string(),
            auth_origin: AUTH_ORIGIN.to_string(),
            client_id: CLIENT_ID.to_string(),
            consent_client_id: CONSENT_CLIENT_ID.to_string(),
            redirect_url: REDIRECT_URL.to_string(),
            scopes: DEFAULT_SCOPES.to_string(),
        }
    }
}

/// A code verifier and its S256 challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    /// Secret value. The client keeps it until the token request.
    pub verifier: String,
    /// Base64url of the SHA-256 of the verifier, without padding.
    pub challenge: String,
}

impl Pkce {
    /// Makes a pair from 43 random characters.
    ///
    /// The random source is the operating system generator. The original Bolt launcher uses `std::rand`,
    /// which an attacker can predict.
    pub fn generate() -> Self {
        Self::from_verifier(&random_string(VERIFIER_LEN, VERIFIER_CHARS))
    }

    /// Makes a pair from a known verifier.
    pub fn from_verifier(verifier: &str) -> Self {
        let digest = Sha256::digest(verifier.as_bytes());
        Self {
            verifier: verifier.to_string(),
            challenge: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest),
        }
    }
}

/// Step of the login flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The shell must load the authorization URL.
    Authorize,
    /// The shell must send the token request.
    Token,
    /// The shell must load the consent URL.
    Consent,
    /// The shell must send the game session request.
    Session,
    /// The flow is complete.
    Done,
}

/// Next step for the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// The URL is not part of the flow. Let the shell continue.
    Ignore,
    /// Send a POST request with content type `application/x-www-form-urlencoded`.
    PostForm { url: String, body: String },
    /// Load this URL in the same view.
    Navigate { url: String },
    /// Send a POST request with content type `application/json`.
    PostJson { url: String, body: String },
    /// The flow is complete.
    Done(Session),
}

/// Result of a complete login.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Session {
    /// Game session identifier. The launch step passes it to the client.
    pub session_id: String,
    /// Name of the account, without the suffix.
    pub display_name: String,
    /// Part of the name after the last number sign.
    pub suffix: String,
    /// Stable account identifier from the `sub` claim.
    pub sub: String,
}

/// One character of an account.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Character {
    /// Identifier that the launch step passes as `JX_CHARACTER_ID`.
    pub account_id: String,
    /// The launch step exports this value as `JX_DISPLAY_NAME`.
    pub display_name: String,
}

/// Fault of the login flow.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthError {
    /// The `state` value of a redirect does not match the value of this flow.
    #[error("state mismatch")]
    StateMismatch,
    /// The `nonce` claim does not match the value of this flow.
    #[error("nonce mismatch")]
    NonceMismatch,
    /// The `sub` claim of the second token does not match the first token.
    #[error("subject mismatch")]
    SubMismatch,
    /// A response does not carry a necessary field.
    #[error("missing field: {0}")]
    MissingField(&'static str),
    /// A token is not a JSON web token with a readable payload.
    #[error("bad token")]
    BadJwt,
    /// A response body is not readable JSON.
    #[error("bad json: {0}")]
    Json(String),
    /// The provider refused the request.
    #[error("provider error: {error}")]
    Provider {
        /// Value of the `error` field.
        error: String,
        /// Value of the `error_description` field.
        description: Option<String>,
    },
    /// The caller used a handler that does not belong to the current stage.
    #[error("wrong stage")]
    WrongStage,
}

/// State machine of the login flow.
#[derive(Debug, Clone)]
pub struct LoginFlow {
    config: AuthConfig,
    state1: String,
    state2: String,
    nonce: String,
    pkce: Pkce,
    stage: Stage,
    sub: Option<String>,
    display_name: String,
    suffix: String,
    id_token: Option<String>,
}

impl LoginFlow {
    /// Makes a flow with new random values.
    pub fn new(config: AuthConfig) -> Self {
        Self::with_secrets(
            config,
            Pkce::generate(),
            random_string(STATE_LEN, STATE_CHARS),
            random_string(STATE_LEN, STATE_CHARS),
            random_string(STATE_LEN, STATE_CHARS),
        )
    }

    /// Makes a flow with given secrets. Tests use this function.
    pub fn with_secrets(
        config: AuthConfig,
        pkce: Pkce,
        state1: String,
        state2: String,
        nonce: String,
    ) -> Self {
        Self {
            config,
            state1,
            state2,
            nonce,
            pkce,
            stage: Stage::Authorize,
            sub: None,
            display_name: String::new(),
            suffix: String::new(),
            id_token: None,
        }
    }

    /// Returns the current step.
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Returns the configuration of this flow.
    pub fn config(&self) -> &AuthConfig {
        &self.config
    }

    /// Returns the URL that starts the login.
    pub fn authorize_url(&self) -> String {
        format!(
            "{}/oauth2/auth?auth_method=&login_type=&flow=launcher&response_type=code\
&client_id={}&code_challenge_method=S256&prompt=login&scope={}&redirect_uri={}\
&code_challenge={}&state={}",
            self.config.account_origin,
            encode(&self.config.client_id),
            encode(&self.config.scopes),
            encode(&self.config.redirect_url),
            encode(&self.pkce.challenge),
            self.state1,
        )
    }

    /// Accepts every URL that the shell loads.
    ///
    /// The function returns `Action::Ignore` for a URL that is not part of the flow.
    pub fn on_navigation(&mut self, url: &str) -> Result<Action, AuthError> {
        // The launcher-redirect page bounces to `jagex:code=..,state=..,intent=..`
        // when the login ran in an external browser.
        if let Some(intent) = url.trim().strip_prefix("jagex:") {
            let parsed = Url::from_query(&intent.replace(',', "&"));
            return self.on_code_redirect(&parsed);
        }
        let parsed = match Url::parse(url) {
            Some(parsed) => parsed,
            None => return Ok(Action::Ignore),
        };

        if parsed.host == REDIRECT_HOST && parsed.path == REDIRECT_PATH {
            return self.on_code_redirect(&parsed);
        }
        if parsed.host == CONSENT_HOST && (parsed.path == "/" || parsed.path.is_empty()) {
            return self.on_consent_redirect(&parsed);
        }
        Ok(Action::Ignore)
    }

    fn on_code_redirect(&mut self, parsed: &Url) -> Result<Action, AuthError> {
        if self.stage != Stage::Authorize {
            return Err(AuthError::WrongStage);
        }
        provider_error(&parsed.query)?;

        let code = parsed.raw_query("code")?;
        let state = get(&parsed.query, "state")?;
        if state != self.state1 {
            return Err(AuthError::StateMismatch);
        }

        // The code arrives encoded. The original Bolt launcher forwards the raw value, so no second
        // encode takes place here.
        let body = format!(
            "grant_type=authorization_code&client_id={}&code={}&code_verifier={}&redirect_uri={}",
            encode(&self.config.client_id),
            code,
            self.pkce.verifier,
            encode(&self.config.redirect_url),
        );
        self.stage = Stage::Token;
        Ok(Action::PostForm {
            url: format!("{}/oauth2/token", self.config.account_origin),
            body,
        })
    }

    fn on_consent_redirect(&mut self, parsed: &Url) -> Result<Action, AuthError> {
        if self.stage != Stage::Consent {
            return Err(AuthError::WrongStage);
        }
        provider_error(&parsed.fragment)?;

        // The provider puts the values in the fragment, not in the query.
        get(&parsed.fragment, "code")?;
        let state = get(&parsed.fragment, "state")?;
        let id_token = get(&parsed.fragment, "id_token")?;
        if state != self.state2 {
            return Err(AuthError::StateMismatch);
        }

        let payload = decode_jwt_payload(&id_token)?;
        let nonce = claim_str(&payload, "nonce")?;
        let sub = claim_str(&payload, "sub")?;
        let nickname = claim_str(&payload, "nickname")?;
        if payload.get("auth_time").and_then(|v| v.as_i64()).is_none() {
            return Err(AuthError::MissingField("auth_time"));
        }
        if nonce != self.nonce {
            return Err(AuthError::NonceMismatch);
        }
        if self.sub.as_deref() != Some(sub.as_str()) {
            return Err(AuthError::SubMismatch);
        }

        let (display_name, suffix) = split_nickname(&nickname);
        self.display_name = display_name;
        self.suffix = suffix;
        self.stage = Stage::Session;
        Ok(Action::PostJson {
            url: format!("{}/game-session/v1/sessions", self.config.auth_origin),
            body: serde_json::json!({ "idToken": id_token }).to_string(),
        })
    }

    /// Accepts the body of the request that `Action::PostForm` asked for.
    pub fn on_token_response(&mut self, body: &str) -> Result<Action, AuthError> {
        if self.stage != Stage::Token {
            return Err(AuthError::WrongStage);
        }
        let json = parse_json(body)?;
        provider_error_json(&json)?;

        let id_token = field_str(&json, "id_token")?;
        field_str(&json, "access_token")?;
        field_str(&json, "refresh_token")?;
        if json.get("expires_in").and_then(|v| v.as_i64()).is_none() {
            return Err(AuthError::MissingField("expires_in"));
        }

        let payload = decode_jwt_payload(&id_token)?;
        self.sub = Some(claim_str(&payload, "sub")?);
        self.id_token = Some(id_token.clone());
        self.stage = Stage::Consent;

        Ok(Action::Navigate {
            url: format!(
                "{}/oauth2/auth?prompt=consent&redirect_uri=http%3A%2F%2Flocalhost\
&response_type=id_token+code&client_id={}&scope=openid+offline&id_token_hint={}\
&state={}&nonce={}",
                self.config.account_origin,
                encode(&self.config.consent_client_id),
                encode(&id_token),
                self.state2,
                self.nonce,
            ),
        })
    }

    /// Accepts the body of the request that `Action::PostJson` asked for.
    pub fn on_session_response(&mut self, body: &str) -> Result<Session, AuthError> {
        if self.stage != Stage::Session {
            return Err(AuthError::WrongStage);
        }
        let json = parse_json(body)?;
        provider_error_json(&json)?;

        let session_id = field_str(&json, "sessionId")?;
        let sub = self.sub.clone().ok_or(AuthError::MissingField("sub"))?;
        self.stage = Stage::Done;
        Ok(Session {
            session_id,
            display_name: self.display_name.clone(),
            suffix: self.suffix.clone(),
            sub,
        })
    }
}

/// Returns the URL and the authorization header of the character list request.
pub fn accounts_request(config: &AuthConfig, session_id: &str) -> (String, (String, String)) {
    (
        format!("{}/game-session/v1/accounts", config.auth_origin),
        ("Authorization".to_string(), format!("Bearer {session_id}")),
    )
}

/// Reads the body of the character list response.
pub fn parse_accounts(body: &str) -> Result<Vec<Character>, AuthError> {
    let json = parse_json(body)?;
    let list = json.as_array().ok_or(AuthError::MissingField("accounts"))?;
    let mut out = Vec::with_capacity(list.len());
    for item in list {
        out.push(Character {
            account_id: field_str(item, "accountId")?,
            display_name: field_str(item, "displayName")?,
        });
    }
    Ok(out)
}

/// Splits a nickname at the last number sign.
fn split_nickname(nickname: &str) -> (String, String) {
    match nickname.rfind('#') {
        Some(pos) => (nickname[..pos].to_string(), nickname[pos + 1..].to_string()),
        None => (nickname.to_string(), String::new()),
    }
}

fn parse_json(body: &str) -> Result<serde_json::Value, AuthError> {
    serde_json::from_str(body).map_err(|e| AuthError::Json(e.to_string()))
}

fn field_str(json: &serde_json::Value, name: &'static str) -> Result<String, AuthError> {
    json.get(name)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or(AuthError::MissingField(name))
}

fn claim_str(payload: &serde_json::Value, name: &'static str) -> Result<String, AuthError> {
    field_str(payload, name)
}

fn provider_error_json(json: &serde_json::Value) -> Result<(), AuthError> {
    match json.get("error").and_then(|v| v.as_str()) {
        Some(error) => Err(AuthError::Provider {
            error: error.to_string(),
            description: json
                .get("error_description")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        }),
        None => Ok(()),
    }
}

fn provider_error(values: &HashMap<String, String>) -> Result<(), AuthError> {
    match values.get("error") {
        Some(error) => Err(AuthError::Provider {
            error: error.clone(),
            description: values.get("error_description").cloned(),
        }),
        None => Ok(()),
    }
}

/// Reads the payload of a JSON web token. The signature is not checked.
///
/// The provider sends the token over TLS, and the session request checks it again.
fn decode_jwt_payload(token: &str) -> Result<serde_json::Value, AuthError> {
    let mut parts = token.split('.');
    let (_header, payload) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(_)) => (h, p),
        _ => return Err(AuthError::BadJwt),
    };
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| AuthError::BadJwt)?;
    let text = String::from_utf8(bytes).map_err(|_| AuthError::BadJwt)?;
    serde_json::from_str(&text).map_err(|_| AuthError::BadJwt)
}

/// The function reads bytes from the operating system generator and rejects a byte
/// that would make the result uneven.
fn random_string(len: usize, alphabet: &[u8]) -> String {
    use rand::TryRngCore as _;

    let size = alphabet.len() as u8;
    let limit = u8::MAX - (u8::MAX % size) - (size - 1);
    let mut out = String::with_capacity(len);
    let mut buffer = [0u8; 64];
    let mut index = buffer.len();
    while out.len() < len {
        if index == buffer.len() {
            rand::rngs::OsRng
                .try_fill_bytes(&mut buffer)
                .expect("operating system entropy");
            index = 0;
        }
        let byte = buffer[index];
        index += 1;
        if byte <= limit {
            out.push(alphabet[(byte % size) as usize] as char);
        }
    }
    out
}

/// Encodes every byte that a URL query does not accept.
fn encode(value: &str) -> String {
    utf8_percent_encode(value, QUERY_ESCAPES).to_string()
}

/// The parts of a URL that the flow reads. Query and fragment values are decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Url {
    host: String,
    path: String,
    raw_query: String,
    query: HashMap<String, String>,
    fragment: HashMap<String, String>,
}

impl Url {
    /// Reads a URL. The function returns `None` for input that has no host.
    fn parse(input: &str) -> Option<Self> {
        let parsed = url::Url::parse(input.trim()).ok()?;
        let host = parsed.host_str()?;
        let raw_query = parsed.query().unwrap_or("").to_string();
        Some(Self {
            host: host.to_ascii_lowercase(),
            path: parsed.path().to_string(),
            query: parse_pairs(&raw_query),
            fragment: parse_pairs(parsed.fragment().unwrap_or("")),
            raw_query,
        })
    }

    /// A URL that is only a query string, such as the `jagex:` intent.
    fn from_query(raw_query: &str) -> Self {
        Self {
            host: String::new(),
            path: String::new(),
            query: parse_pairs(raw_query),
            fragment: HashMap::new(),
            raw_query: raw_query.to_string(),
        }
    }

    /// Returns a query value in its raw, still encoded form.
    fn raw_query(&self, name: &'static str) -> Result<String, AuthError> {
        self.raw_query
            .split('&')
            .filter_map(|pair| pair.split_once('=').or(Some((pair, ""))))
            .find(|(key, _)| decode(key) == name)
            .map(|(_, value)| value.to_string())
            .ok_or(AuthError::MissingField(name))
    }
}

/// Returns a decoded value from a query or fragment map.
fn get(values: &HashMap<String, String>, name: &'static str) -> Result<String, AuthError> {
    values
        .get(name)
        .cloned()
        .ok_or(AuthError::MissingField(name))
}

/// Decodes percent escapes and the plus sign. Bad escapes stay as they are.
fn decode(value: &str) -> String {
    percent_decode_str(&value.replace('+', " "))
        .decode_utf8_lossy()
        .into_owned()
}

/// Reads `key=value&key=value` into a map of decoded values.
fn parse_pairs(input: &str) -> HashMap<String, String> {
    form_urlencoded::parse(input.as_bytes())
        .filter(|(key, _)| !key.is_empty())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes an unsigned token with the given payload.
    fn jwt(payload: serde_json::Value) -> String {
        let body = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string());
        format!("eyJhbGciOiJub25lIn0.{body}.sig")
    }

    fn flow() -> LoginFlow {
        LoginFlow::with_secrets(
            AuthConfig::default(),
            Pkce::from_verifier("verifier000000000000000000000000000000000000"),
            "STATEONE0001".to_string(),
            "STATETWO0002".to_string(),
            "NONCE0000001".to_string(),
        )
    }

    #[test]
    fn challenge_matches_rfc_7636_vector() {
        // Vector of RFC 7636 appendix B.
        let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(
            pkce.challenge,
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_verifier_has_the_required_shape() {
        let pkce = Pkce::generate();
        assert_eq!(pkce.verifier.len(), VERIFIER_LEN);
        assert!(pkce.verifier.bytes().all(|b| VERIFIER_CHARS.contains(&b)));
        assert_ne!(pkce.verifier, Pkce::generate().verifier);
    }

    #[test]
    fn authorize_url_carries_the_challenge_and_the_state() {
        let flow = flow();
        let url = flow.authorize_url();
        assert!(url.starts_with("https://account.jagex.com/oauth2/auth?"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains(&format!("code_challenge={}", encode(&flow.pkce.challenge))));
        assert!(url.contains("state=STATEONE0001"));
        assert!(url.contains(
            "redirect_uri=https%3A%2F%2Fsecure.runescape.com%2Fm%3Dweblogin%2Flauncher-redirect"
        ));
        assert!(!url.contains(&flow.pkce.verifier));
    }

    #[test]
    fn code_redirect_makes_the_token_request() {
        let mut flow = flow();
        let action = flow
            .on_navigation(
                "https://secure.runescape.com/m=weblogin/launcher-redirect?code=abc%2Fdef&state=STATEONE0001",
            )
            .unwrap();
        match action {
            Action::PostForm { url, body } => {
                assert_eq!(url, "https://account.jagex.com/oauth2/token");
                // The raw value passes through without a second encode.
                assert!(body.contains("code=abc%2Fdef"));
                assert!(body.contains("code_verifier=verifier000000000000000000000000000000000000"));
            }
            other => panic!("unexpected action: {other:?}"),
        }
        assert_eq!(flow.stage(), Stage::Token);
    }

    #[test]
    fn jagex_intent_makes_the_token_request() {
        let mut flow = flow();
        let action = flow
            .on_navigation("jagex:code=abc%2Fdef,state=STATEONE0001,intent=social_auth")
            .unwrap();
        match action {
            Action::PostForm { url, body } => {
                assert_eq!(url, "https://account.jagex.com/oauth2/token");
                assert!(body.contains("code=abc%2Fdef"));
            }
            other => panic!("unexpected action: {other:?}"),
        }
        assert_eq!(flow.stage(), Stage::Token);

        let mut flow = self::flow();
        let error = flow
            .on_navigation("jagex:code=abc,state=OTHER,intent=social_auth")
            .unwrap_err();
        assert_eq!(error, AuthError::StateMismatch);
    }

    #[test]
    fn code_redirect_rejects_a_wrong_state() {
        let mut flow = flow();
        let error = flow
            .on_navigation(
                "https://secure.runescape.com/m=weblogin/launcher-redirect?code=abc&state=OTHER",
            )
            .unwrap_err();
        assert_eq!(error, AuthError::StateMismatch);
        assert_eq!(flow.stage(), Stage::Authorize);
    }

    #[test]
    fn code_redirect_reports_a_provider_error() {
        let mut flow = flow();
        let error = flow
            .on_navigation(
                "https://secure.runescape.com/m=weblogin/launcher-redirect?error=access_denied&error_description=user%20said%20no",
            )
            .unwrap_err();
        match error {
            AuthError::Provider { error, description } => {
                assert_eq!(error, "access_denied");
                assert_eq!(description.as_deref(), Some("user said no"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn unrelated_urls_are_ignored() {
        let mut flow = flow();
        assert_eq!(
            flow.on_navigation("https://example.com/x").unwrap(),
            Action::Ignore
        );
        assert_eq!(flow.on_navigation("not a url").unwrap(), Action::Ignore);
        assert_eq!(flow.stage(), Stage::Authorize);
    }

    #[test]
    fn consent_redirect_needs_the_matching_nonce_and_subject() {
        let mut flow = flow();
        flow.stage = Stage::Consent;
        flow.sub = Some("account-1".to_string());

        let wrong_nonce = jwt(serde_json::json!({
            "nonce": "OTHERNONCE00", "sub": "account-1", "nickname": "Zezima#42", "auth_time": 1
        }));
        let error = flow
            .on_navigation(&format!(
                "http://localhost/#code=c&state=STATETWO0002&id_token={wrong_nonce}"
            ))
            .unwrap_err();
        assert_eq!(error, AuthError::NonceMismatch);

        let wrong_sub = jwt(serde_json::json!({
            "nonce": "NONCE0000001", "sub": "account-2", "nickname": "Zezima#42", "auth_time": 1
        }));
        let error = flow
            .on_navigation(&format!(
                "http://localhost/#code=c&state=STATETWO0002&id_token={wrong_sub}"
            ))
            .unwrap_err();
        assert_eq!(error, AuthError::SubMismatch);
    }

    #[test]
    fn nickname_splits_at_the_last_number_sign() {
        assert_eq!(split_nickname("Zezima#42"), ("Zezima".into(), "42".into()));
        assert_eq!(split_nickname("a#b#c"), ("a#b".into(), "c".into()));
        assert_eq!(split_nickname("NoSuffix"), ("NoSuffix".into(), "".into()));
    }

    #[test]
    fn handlers_reject_a_wrong_stage() {
        let mut flow = flow();
        assert_eq!(
            flow.on_token_response("{}").unwrap_err(),
            AuthError::WrongStage
        );
        assert_eq!(
            flow.on_session_response("{}").unwrap_err(),
            AuthError::WrongStage
        );
    }

    #[test]
    fn token_response_needs_every_field() {
        let mut flow = flow();
        flow.stage = Stage::Token;
        let body = serde_json::json!({
            "id_token": jwt(serde_json::json!({"sub": "account-1"})),
            "access_token": "a",
            "refresh_token": "r"
        })
        .to_string();
        assert_eq!(
            flow.on_token_response(&body).unwrap_err(),
            AuthError::MissingField("expires_in")
        );
    }

    #[test]
    fn full_flow_reaches_a_session() {
        let mut flow = flow();

        let action = flow
            .on_navigation(
                "https://secure.runescape.com/m=weblogin/launcher-redirect?code=CODE1&state=STATEONE0001",
            )
            .unwrap();
        assert!(matches!(action, Action::PostForm { .. }));

        let id_token = jwt(serde_json::json!({"sub": "account-1"}));
        let token_body = serde_json::json!({
            "id_token": id_token, "access_token": "a", "refresh_token": "r", "expires_in": 3600
        })
        .to_string();
        let action = flow.on_token_response(&token_body).unwrap();
        let consent_url = match action {
            Action::Navigate { url } => url,
            other => panic!("unexpected action: {other:?}"),
        };
        assert!(consent_url.contains("prompt=consent"));
        assert!(consent_url.contains("client_id=1fddee4e-b100-4f4e-b2b0-097f9088f9d2"));
        assert!(consent_url.contains("state=STATETWO0002"));
        assert!(consent_url.contains("nonce=NONCE0000001"));
        assert_eq!(flow.stage(), Stage::Consent);

        let second = jwt(serde_json::json!({
            "nonce": "NONCE0000001", "sub": "account-1", "nickname": "Zezima#42", "auth_time": 100
        }));
        let action = flow
            .on_navigation(&format!(
                "http://localhost/#code=CODE2&state=STATETWO0002&id_token={second}"
            ))
            .unwrap();
        match action {
            Action::PostJson { url, body } => {
                assert_eq!(url, "https://auth.jagex.com/game-session/v1/sessions");
                assert_eq!(body, format!("{{\"idToken\":\"{second}\"}}"));
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let session = flow
            .on_session_response(r#"{"sessionId":"session-9"}"#)
            .unwrap();
        assert_eq!(
            session,
            Session {
                session_id: "session-9".to_string(),
                display_name: "Zezima".to_string(),
                suffix: "42".to_string(),
                sub: "account-1".to_string(),
            }
        );
        assert_eq!(flow.stage(), Stage::Done);
    }

    #[test]
    fn accounts_request_uses_a_bearer_header() {
        let (url, header) = accounts_request(&AuthConfig::default(), "session-9");
        assert_eq!(url, "https://auth.jagex.com/game-session/v1/accounts");
        assert_eq!(
            header,
            ("Authorization".to_string(), "Bearer session-9".to_string())
        );
    }

    #[test]
    fn accounts_body_reads_every_character() {
        let list = parse_accounts(
            r#"[{"accountId":"a1","displayName":"One"},{"accountId":"a2","displayName":"Two"}]"#,
        )
        .unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].account_id, "a2");
        assert_eq!(
            parse_accounts("{}").unwrap_err(),
            AuthError::MissingField("accounts")
        );
    }

    #[test]
    fn url_parser_reads_host_path_query_and_fragment() {
        let url = Url::parse("https://Host.Example:8443/a/b?x=1&y=2#z=3").unwrap();
        assert_eq!(url.host, "host.example");
        assert_eq!(url.path, "/a/b");
        assert_eq!(url.query.get("x").map(String::as_str), Some("1"));
        assert_eq!(url.fragment.get("z").map(String::as_str), Some("3"));
        assert!(Url::parse("https://").is_none());
    }

    #[test]
    fn decoder_reads_escapes_and_keeps_bad_input() {
        assert_eq!(decode("a%20b+c"), "a b c");
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
    }
}
