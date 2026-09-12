//! The login values from a saved session or an external secret manager.

use std::collections::HashMap;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::{CoreError, GameCredentials};

/// The key of the session value.
const SESSION_ID: &str = "JX_SESSION_ID";
/// The key of the character value.
const CHARACTER_ID: &str = "JX_CHARACTER_ID";
/// The key of the display name value.
const DISPLAY_NAME: &str = "JX_DISPLAY_NAME";
/// The item token of a command argument.
const ITEM_TOKEN: &str = "{item}";
/// The three labels of the 1Password field argument.
const ONEPASSWORD_FIELDS: &str = "label=JX_CHARACTER_ID,JX_SESSION_ID,JX_DISPLAY_NAME";

/// The shape of the output of the credential command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialFormat {
    /// A JSON array of `{"label": .., "value": ..}` objects. The 1Password shape.
    OnePassword,
    /// A JSON object with the keys `JX_SESSION_ID`, `JX_CHARACTER_ID` and `JX_DISPLAY_NAME`.
    Json,
    /// `KEY=VALUE` lines.
    EnvLines,
}

/// The command that gives the login values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandCredentials {
    /// The program. The value is "op" for 1Password.
    pub program: String,
    /// The arguments. Every `{item}` becomes the item name.
    pub args: Vec<String>,
    /// The shape of the output.
    pub format: CredentialFormat,
}

/// Where the launcher takes the login values from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialSource {
    /// The saved session file.
    Session,
    /// An external command.
    Command(CommandCredentials),
}

impl Default for CredentialSource {
    fn default() -> CredentialSource {
        CredentialSource::Session
    }
}

impl CommandCredentials {
    /// The preset of the 1Password command line tool.
    ///
    /// It runs one `op item get` call, so the user answers one prompt only.
    pub fn one_password(vault: &str) -> CommandCredentials {
        CommandCredentials {
            program: "op".to_string(),
            args: vec![
                "item".to_string(),
                "get".to_string(),
                ITEM_TOKEN.to_string(),
                "--vault".to_string(),
                vault.to_string(),
                "--fields".to_string(),
                ONEPASSWORD_FIELDS.to_string(),
                "--reveal".to_string(),
                "--format".to_string(),
                "json".to_string(),
            ],
            format: CredentialFormat::OnePassword,
        }
    }

    /// Runs the command for one item and reads the three login values.
    ///
    /// A non zero exit status gives the trimmed standard error text.
    /// The function never prints the output.
    pub fn fetch(&self, item: &str) -> Result<GameCredentials, CoreError> {
        let args: Vec<String> = self
            .args
            .iter()
            .map(|argument| argument.replace(ITEM_TOKEN, item))
            .collect();
        let output = Command::new(&self.program)
            .args(&args)
            .output()
            .map_err(|error| {
                CoreError::Command(format!("cannot run the command {}: {}", self.program, error))
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CoreError::Command(stderr.trim().to_string()));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_credentials(self.format, &stdout)
    }
}

/// Reads the three login values out of command output. Public for the tests.
pub fn parse_credentials(
    format: CredentialFormat,
    text: &str,
) -> Result<GameCredentials, CoreError> {
    let values = match format {
        CredentialFormat::OnePassword => labelled_values(text)?,
        CredentialFormat::Json => json_values(text)?,
        CredentialFormat::EnvLines => line_values(text),
    };
    build(&values)
}

/// Reads the `label` and `value` pairs of the 1Password JSON array.
fn labelled_values(text: &str) -> Result<HashMap<String, String>, CoreError> {
    #[derive(Deserialize)]
    struct LabelledField {
        #[serde(default)]
        label: String,
        #[serde(default)]
        value: Option<String>,
    }

    let fields: Vec<LabelledField> = serde_json::from_str(text)?;
    let mut values = HashMap::new();
    for field in fields {
        if field.label.is_empty() {
            continue;
        }
        if let Some(value) = field.value {
            if !value.is_empty() {
                values.insert(field.label, value);
            }
        }
    }
    Ok(values)
}

/// Reads the three keys of a JSON object.
fn json_values(text: &str) -> Result<HashMap<String, String>, CoreError> {
    let document: serde_json::Value = serde_json::from_str(text)?;
    let mut values = HashMap::new();
    if let serde_json::Value::Object(map) = document {
        for key in [SESSION_ID, CHARACTER_ID, DISPLAY_NAME] {
            if let Some(serde_json::Value::String(value)) = map.get(key) {
                if !value.is_empty() {
                    values.insert(key.to_string(), value.clone());
                }
            }
        }
    }
    Ok(values)
}

/// Reads the `KEY=VALUE` lines. One pair of surrounding quotation marks goes away.
fn line_values(text: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.trim().to_string(), unquote(value.trim()).to_string());
        }
    }
    values
}

/// Removes one pair of surrounding quotation marks.
fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &value[1..value.len() - 1];
        }
    }
    value
}

/// Builds the login values. An absent or empty value is an error.
fn build(values: &HashMap<String, String>) -> Result<GameCredentials, CoreError> {
    Ok(GameCredentials {
        session_id: value_of(values, SESSION_ID)?,
        character_id: value_of(values, CHARACTER_ID)?,
        display_name: value_of(values, DISPLAY_NAME)?,
    })
}

/// Reads one value, or names the missing field.
fn value_of(values: &HashMap<String, String>, key: &str) -> Result<String, CoreError> {
    values
        .get(key)
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| CoreError::Command(format!("the credential output has no value for {key}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_password_gives_the_exact_argument_list() {
        let credentials = CommandCredentials::one_password("My Vault");
        assert_eq!(credentials.program, "op");
        assert_eq!(credentials.format, CredentialFormat::OnePassword);
        assert_eq!(
            credentials.args,
            vec![
                "item",
                "get",
                "{item}",
                "--vault",
                "My Vault",
                "--fields",
                "label=JX_CHARACTER_ID,JX_SESSION_ID,JX_DISPLAY_NAME",
                "--reveal",
                "--format",
                "json",
            ]
        );
    }

    #[test]
    fn parse_credentials_reads_the_one_password_array() {
        let text = r#"[
            {"label": "JX_CHARACTER_ID", "value": "char"},
            {"label": "JX_SESSION_ID", "value": "sess"},
            {"label": "JX_DISPLAY_NAME", "value": "Player"}
        ]"#;
        let values = parse_credentials(CredentialFormat::OnePassword, text).unwrap();
        assert_eq!(values.character_id, "char");
        assert_eq!(values.session_id, "sess");
        assert_eq!(values.display_name, "Player");
    }

    #[test]
    fn parse_credentials_reads_the_json_object() {
        let text = r#"{"JX_SESSION_ID": "sess", "JX_CHARACTER_ID": "char", "JX_DISPLAY_NAME": "Player"}"#;
        let values = parse_credentials(CredentialFormat::Json, text).unwrap();
        assert_eq!(values.session_id, "sess");
        assert_eq!(values.character_id, "char");
        assert_eq!(values.display_name, "Player");
    }

    #[test]
    fn parse_credentials_reads_the_env_lines_and_removes_quotes() {
        let text = "JX_SESSION_ID=\"sess\"\nJX_CHARACTER_ID=char\nJX_DISPLAY_NAME=\"Player Name\"\n";
        let values = parse_credentials(CredentialFormat::EnvLines, text).unwrap();
        assert_eq!(values.session_id, "sess");
        assert_eq!(values.character_id, "char");
        assert_eq!(values.display_name, "Player Name");
    }

    #[test]
    fn parse_credentials_names_the_missing_field() {
        let text = r#"{"JX_CHARACTER_ID": "char", "JX_DISPLAY_NAME": "Player"}"#;
        let error = parse_credentials(CredentialFormat::Json, text).unwrap_err();
        let message = error.to_string();
        assert!(message.contains(SESSION_ID), "unexpected message: {message}");
    }

    #[cfg(unix)]
    #[test]
    fn fetch_reads_the_standard_output_of_the_command() {
        let credentials = CommandCredentials {
            program: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf 'JX_SESSION_ID=sess\\nJX_CHARACTER_ID=char\\nJX_DISPLAY_NAME=Player\\n'"
                    .to_string(),
            ],
            format: CredentialFormat::EnvLines,
        };
        let values = credentials.fetch("item").unwrap();
        assert_eq!(values.session_id, "sess");
        assert_eq!(values.character_id, "char");
        assert_eq!(values.display_name, "Player");
    }

    #[cfg(unix)]
    #[test]
    fn fetch_reports_the_trimmed_standard_error_of_a_failed_command() {
        let credentials = CommandCredentials {
            program: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf 'the item is absent\\n' >&2; exit 3".to_string(),
            ],
            format: CredentialFormat::Json,
        };
        let error = credentials.fetch("item").unwrap_err();
        assert_eq!(error.to_string(), "credential command error: the item is absent");
    }
}
