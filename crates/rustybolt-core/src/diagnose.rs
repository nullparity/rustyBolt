//! A diagnostics bundle for bug reports.
//!
//! One zip holds a report and the tails of the client logs. Nothing in it
//! names the user: account and character names, session and account ids,
//! account hashes, the user name, the home directory, addresses and emails
//! are replaced before anything is written. The keychain entry is never
//! read for this; only whether the keychain answers goes in.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::{Config, Paths};

/// Lines kept from the end of each log.
const LOG_TAIL_LINES: usize = 400;

/// What the caller knows that the module does not: how many characters each
/// saved account has, in account order. `None` when the lookup failed.
pub struct Accounts {
    pub character_counts: Vec<Option<usize>>,
}

/// Replaces the personal parts of a text.
pub struct Redactor {
    /// Exact strings and what replaces them, longest first.
    exact: Vec<(String, &'static str)>,
}

impl Redactor {
    /// Learns the secrets from the saved sessions, the home directory and the
    /// user name. `secrets` holds `(value, label)` pairs such as
    /// `(session_id, "<session>")`.
    pub fn new(secrets: Vec<(String, &'static str)>) -> Self {
        let mut exact: Vec<(String, &'static str)> = secrets
            .into_iter()
            .filter(|(value, _)| value.len() >= 3)
            .collect();
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            let home = PathBuf::from(home);
            if let Some(name) = home.file_name().and_then(|n| n.to_str()) {
                exact.push((name.to_string(), "<user>"));
            }
            exact.push((home.to_string_lossy().into_owned(), "~"));
            exact.push((home.to_string_lossy().replace('\\', "/"), "~"));
        }
        for var in ["USER", "USERNAME", "LOGNAME"] {
            if let Ok(name) = std::env::var(var) {
                exact.push((name, "<user>"));
            }
        }
        exact.retain(|(value, _)| value.len() >= 3);
        exact.sort_by_key(|(value, _)| std::cmp::Reverse(value.len()));
        exact.dedup();
        Self { exact }
    }

    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (value, label) in &self.exact {
            out = out.replace(value.as_str(), label);
        }
        out = replace_pattern(&out, "account hash ", "<hash>", |c| {
            c == '-' || c.is_ascii_digit()
        });
        // RuneLite profile names are the user's own words.
        out = replace_pattern(&out, "Profile '", "<profile>", |c| c != '\'');
        out = replace_ipv4(&out);
        out = replace_emails(&out);
        out
    }
}

/// Replaces the run of `accept` characters after every `prefix`.
fn replace_pattern(text: &str, prefix: &str, label: &str, accept: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(prefix) {
        let after = at + prefix.len();
        out.push_str(&rest[..after]);
        let run = rest[after..].chars().take_while(|&c| accept(c)).count();
        let run_bytes: usize = rest[after..].chars().take(run).map(char::len_utf8).sum();
        out.push_str(if run > 0 { label } else { "" });
        rest = &rest[after + run_bytes..];
    }
    out.push_str(rest);
    out
}

fn replace_ipv4(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, token) in text.split(' ').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let core = token.trim_end_matches(|c: char| !c.is_ascii_digit());
        let tail = &token[core.len()..];
        let is_ip = core.split('.').count() == 4
            && core.split('.').all(|part| {
                !part.is_empty() && part.len() <= 3 && part.chars().all(|c| c.is_ascii_digit())
            });
        if is_ip {
            out.push_str("<ip>");
            out.push_str(tail);
        } else {
            out.push_str(token);
        }
    }
    out
}

fn replace_emails(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, token) in text.split(' ').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let looks_like_email = token.contains('@')
            && token
                .rsplit('@')
                .next()
                .is_some_and(|domain| domain.contains('.'));
        out.push_str(if looks_like_email { "<email>" } else { token });
    }
    out
}

/// The files of the bundle, in order: `(name, content)`.
pub fn collect(
    paths: &Paths,
    config: &Config,
    accounts: &Accounts,
    redactor: &Redactor,
    version: &str,
) -> Vec<(String, Vec<u8>)> {
    let mut report = String::new();
    let line = |report: &mut String, text: String| {
        report.push_str(&text);
        report.push('\n');
    };
    line(&mut report, format!("rustybolt {version}"));
    line(
        &mut report,
        format!("os: {} {}", std::env::consts::OS, std::env::consts::ARCH),
    );
    line(
        &mut report,
        format!(
            "memory: {}",
            crate::memory::total_bytes()
                .map(|b| format!("{} MB", b >> 20))
                .unwrap_or_else(|| "unknown".to_string())
        ),
    );
    line(
        &mut report,
        format!(
            "keychain: {}",
            crate::keychain_available()
                .map(|()| "available".to_string())
                .unwrap_or_else(|e| e.to_string())
        ),
    );
    line(
        &mut report,
        format!("accounts: {}", accounts.character_counts.len()),
    );
    for (i, count) in accounts.character_counts.iter().enumerate() {
        line(
            &mut report,
            format!(
                "  account {}: {} characters",
                i + 1,
                count
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ),
        );
    }
    line(&mut report, String::new());
    line(&mut report, "runtimes:".to_string());
    for rt in rustybolt_jdk::discover() {
        line(
            &mut report,
            format!(
                "  {} [{:?}{}] {}",
                rt.version
                    .as_ref()
                    .map(|v| v.raw.clone())
                    .unwrap_or_else(|| "?".to_string()),
                rt.source,
                if rt.headless { ", headless" } else { "" },
                rt.path.display()
            ),
        );
    }
    line(&mut report, String::new());
    for kind in [crate::ClientKind::RuneLite, crate::ClientKind::Hdos] {
        line(
            &mut report,
            format!(
                "{}: {}",
                kind.name(),
                crate::locate_client(kind, config)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "not found".to_string())
            ),
        );
    }
    line(&mut report, String::new());
    line(&mut report, "config:".to_string());
    // A credential command may name a secret-manager item; keep its kind only.
    let mut config_json = serde_json::to_value(config).unwrap_or_default();
    if let Some(source) = config_json.get_mut("credential_source") {
        let kind = match &*source {
            serde_json::Value::String(kind) => kind.clone(),
            serde_json::Value::Object(map) => map.keys().cloned().collect::<Vec<_>>().join(","),
            other => other.to_string(),
        };
        *source = serde_json::Value::String(format!("<{kind}>"));
    }
    line(
        &mut report,
        serde_json::to_string_pretty(&config_json).unwrap_or_else(|_| "unreadable".to_string()),
    );

    let mut files = vec![(
        "report.txt".to_string(),
        redactor.redact(&report).into_bytes(),
    )];
    let logs = config.runelite_home(paths).join(".runelite").join("logs");
    for name in ["launcher.log", "client.log"] {
        if let Ok(text) = fs::read_to_string(logs.join(name)) {
            let tail: Vec<&str> = text.lines().rev().take(LOG_TAIL_LINES).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            files.push((
                format!("runelite-{name}"),
                redactor.redact(&tail.join("\n")).into_bytes(),
            ));
        }
    }
    files
}

/// Writes `files` as a zip with stored entries. Enough for a few text files.
pub fn write_zip(path: &Path, files: &[(String, Vec<u8>)]) -> io::Result<()> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let mut crc = flate2::Crc::new();
        crc.update(data);
        let crc = crc.sum();
        let offset = out.len() as u32;
        let header = |v: &mut Vec<u8>| {
            v.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // version, flags, method, time, date
            v.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(name.len() as u16).to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
        };
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        header(&mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 20, 0]);
        header(&mut central);
        central.extend_from_slice(&[0; 10]); // comment length, disk, internal and external attributes
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let central_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    fs::File::create(path)?.write_all(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_redactor_replaces_names_ids_paths_hashes_addresses_and_emails() {
        let r = Redactor::new(vec![
            ("Tpain195".to_string(), "<account>"),
            ("BbVXZMd8R5Z6vYMdpo7lr".to_string(), "<account>"),
            ("abc123session".to_string(), "<session>"),
        ]);
        let home = std::env::var("HOME").unwrap_or_default();
        let text = format!(
            "Logged in as Tpain195 sub=BbVXZMd8R5Z6vYMdpo7lr JX_SESSION_ID=abc123session \
             home={home}/.runelite gateway 192.168.1.1: mail a@b.co account hash -498416345 done"
        );
        let out = r.redact(&text);
        assert!(
            !out.contains("Tpain195") && !out.contains("BbVXZ") && !out.contains("abc123session")
        );
        assert!(out.contains("home=~/.runelite"), "{out}");
        assert!(out.contains("gateway <ip>:"), "{out}");
        assert!(out.contains("mail <email>"), "{out}");
        assert!(out.contains("account hash <hash> done"), "{out}");
        assert_eq!(
            r.redact("Profile 'afk combat' (sync"),
            "Profile '<profile>' (sync"
        );
        assert!(out.contains("<account> sub=<account>"), "{out}");
    }

    #[test]
    fn a_zip_round_trips_through_the_icon_reader() {
        let dir = crate::test_support::TempDir::new("diag-zip");
        let path = dir.path().join("d.zip");
        write_zip(
            &path,
            &[
                ("report.txt".to_string(), b"hello".to_vec()),
                ("runelite-client.log".to_string(), b"line\n".to_vec()),
            ],
        )
        .unwrap();
        // The icon reader in `desktop` parses the same central directory.
        assert_eq!(
            crate::desktop::read_zip_entry_for_test(&path, "report.txt")
                .unwrap()
                .as_deref(),
            Some(&b"hello"[..])
        );
        assert_eq!(
            crate::desktop::read_zip_entry_for_test(&path, "runelite-client.log")
                .unwrap()
                .as_deref(),
            Some(&b"line\n"[..])
        );
    }
}
