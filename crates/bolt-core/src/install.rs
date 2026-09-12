//! The download and the install of the game clients.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{CoreError, Paths};

/// The URL of the RuneLite release list.
const RUNELITE_RELEASES_URL: &str = "https://api.github.com/repos/runelite/launcher/releases";
const HDOS_GETDOWN_URL: &str = "https://cdn.hdos.dev/client/getdown.txt";
/// The file name of the RuneLite asset.
const RUNELITE_JAR: &str = "runelite.jar";
/// The user agent that the GitHub API needs.
const USER_AGENT: &str = concat!("rustybolt/", env!("CARGO_PKG_VERSION"));

/// The game client that the launcher can install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    /// The RuneLite client.
    RuneLite,
    /// The HDOS client.
    Hdos,
}

impl ClientKind {
    /// The file stem of the jar file and the version file.
    pub fn name(self) -> &'static str {
        match self {
            ClientKind::RuneLite => "runelite",
            ClientKind::Hdos => "hdos",
        }
    }
}

/// A client that is on the disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledClient {
    /// The client.
    pub kind: ClientKind,
    /// The jar file.
    pub jar: PathBuf,
    /// The version text that the install wrote.
    pub version: String,
}

/// One release of a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The version of the release. For RuneLite this is the GitHub asset id.
    pub version: String,
    /// The URL of the jar file.
    pub url: String,
    /// The size of the jar file in bytes, when the server gives it.
    pub size: Option<u64>,
    /// The sha256 digest of the jar file in lower case hex, when the server gives it.
    pub sha256: Option<String>,
}

/// Finds and installs the game clients.
pub struct Installer<'a> {
    paths: &'a Paths,
}

impl<'a> Installer<'a> {
    pub fn new(paths: &'a Paths) -> Installer<'a> {
        Installer { paths }
    }

    /// Reads the installed client. `None` means that a file is absent.
    pub fn installed(&self, kind: ClientKind) -> Option<InstalledClient> {
        let jar = self.jar_path(kind);
        if !jar.is_file() {
            return None;
        }
        let version = fs::read_to_string(self.version_path(kind)).ok()?;
        Some(InstalledClient {
            kind,
            jar,
            version: version.trim().to_string(),
        })
    }

    pub fn latest(&self, kind: ClientKind) -> Result<Release, CoreError> {
        match kind {
            ClientKind::RuneLite => self.latest_runelite(),
            ClientKind::Hdos => self.latest_hdos(),
        }
    }

    /// Downloads one release into the client directory.
    ///
    /// The download goes to a temporary file. The function checks the digest
    /// when the release gives one, and it renames the file into place after
    /// the check.
    pub fn install(
        &self,
        kind: ClientKind,
        release: &Release,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<InstalledClient, CoreError> {
        let jar = self.jar_path(kind);
        let part = self.part_path(kind);
        fs::create_dir_all(self.paths.client_dir())?;

        if let Err(error) = self.download(release, &part, progress) {
            let _ = fs::remove_file(&part);
            return Err(error);
        }
        if let Err(error) = crate::file::set_mode(&part, 0o755) {
            let _ = fs::remove_file(&part);
            return Err(error.into());
        }
        fs::rename(&part, &jar)?;
        fs::write(self.version_path(kind), &release.version)?;

        Ok(InstalledClient {
            kind,
            jar,
            version: release.version.clone(),
        })
    }

    fn jar_path(&self, kind: ClientKind) -> PathBuf {
        self.paths.client_dir().join(format!("{}.jar", kind.name()))
    }

    fn version_path(&self, kind: ClientKind) -> PathBuf {
        self.paths.client_dir().join(format!("{}.version", kind.name()))
    }

    fn part_path(&self, kind: ClientKind) -> PathBuf {
        self.paths.client_dir().join(format!("{}.jar.part", kind.name()))
    }

    fn latest_runelite(&self) -> Result<Release, CoreError> {
        let body = ureq::get(RUNELITE_RELEASES_URL)
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", USER_AGENT)
            .call()?
            .body_mut()
            .read_to_string()?;
        let releases: Vec<Value> = serde_json::from_str(&body)?;
        let release = releases
            .first()
            .ok_or_else(|| CoreError::Http("the RuneLite release list is empty".to_string()))?;
        let assets = release
            .get("assets")
            .and_then(Value::as_array)
            .ok_or_else(|| CoreError::Http("the newest RuneLite release has no assets".to_string()))?;
        let asset = assets
            .iter()
            .find(|asset| {
                asset
                    .get("name")
                    .and_then(Value::as_str)
                    .map(|name| name.eq_ignore_ascii_case(RUNELITE_JAR))
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                CoreError::Http(format!("the newest RuneLite release has no {RUNELITE_JAR}"))
            })?;

        let version = match asset.get("id") {
            Some(Value::Number(number)) => number.to_string(),
            Some(Value::String(text)) => text.clone(),
            _ => {
                return Err(CoreError::Http(
                    "the RuneLite asset has no id".to_string(),
                ));
            }
        };
        let url = asset
            .get("browser_download_url")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::Http("the RuneLite asset has no url".to_string()))?
            .to_string();

        Ok(Release {
            version,
            url,
            size: asset.get("size").and_then(Value::as_u64),
            sha256: asset_digest(asset),
        })
    }

    fn latest_hdos(&self) -> Result<Release, CoreError> {
        let body = ureq::get(HDOS_GETDOWN_URL)
            .call()?
            .body_mut()
            .read_to_string()?;
        let version = hdos_version(&body).ok_or_else(|| {
            CoreError::Http("the HDOS getdown config has no launcher version".to_string())
        })?;
        Ok(Release {
            url: hdos_jar_url(&version),
            version,
            size: None,
            sha256: None,
        })
    }

    fn download(
        &self,
        release: &Release,
        part: &Path,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<(), CoreError> {
        let response = ureq::get(&release.url).call()?;
        let total = release.size.or_else(|| response.body().content_length());
        let mut reader = response.into_body().into_reader();
        let mut file = fs::File::create(part)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        let mut written: u64 = 0;

        progress(0, total);
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count])?;
            hasher.update(&buffer[..count]);
            written += count as u64;
            progress(written, total);
        }
        file.flush()?;
        drop(file);

        if let Some(expected) = &release.sha256 {
            check_digest(expected, &hasher.finalize())?;
        }
        Ok(())
    }
}

/// Compares the digest of the downloaded bytes with the digest of the release.
fn check_digest(expected: &str, digest: &[u8]) -> Result<(), CoreError> {
    let actual = to_hex(digest);
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(CoreError::DigestMismatch {
            expected: expected.to_string(),
            actual,
        })
    }
}
/// Reads the `digest` field of a GitHub asset. The form is `sha256:<hex>`.
fn asset_digest(asset: &Value) -> Option<String> {
    asset
        .get("digest")
        .and_then(Value::as_str)
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .map(|hex| hex.to_ascii_lowercase())
}

/// Reads the `launcher.version` line of the HDOS getdown config.
///
/// The line matches `^launcher.version *= *(.*)$`.
fn hdos_version(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix("launcher.version")?;
        let rest = rest.trim_start_matches(' ').strip_prefix('=')?;
        let value = rest.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

fn hdos_jar_url(version: &str) -> String {
    format!("https://cdn.hdos.dev/launcher/v{version}/hdos-launcher.jar")
}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TempDir, paths};

    const GETDOWN: &str = "\
java.main = 17
launcher.version = 2.14.3
launcher.url = https://cdn.hdos.dev/launcher/
";

    #[test]
    fn hdos_version_reads_the_launcher_line() {
        assert_eq!(hdos_version(GETDOWN), Some("2.14.3".to_string()));
    }

    #[test]
    fn hdos_version_handles_spacing_and_decoy_lines() {
        let text = "launcher.version=1.0\r\nlauncher.version_extra = 9\n";
        assert_eq!(hdos_version(text), Some("1.0".to_string()));
        assert_eq!(hdos_version("java.main = 17\n"), None);
        assert_eq!(hdos_version("launcher.version = \n"), None);
    }

    #[test]
    fn hdos_release_url_uses_the_version() {
        assert_eq!(
            hdos_jar_url("2.14.3"),
            "https://cdn.hdos.dev/launcher/v2.14.3/hdos-launcher.jar"
        );
    }

    #[test]
    fn installed_needs_both_files() {
        let temp = TempDir::new("install-state");
        let paths = paths(temp.path());
        let installer = Installer::new(&paths);
        fs::create_dir_all(paths.client_dir()).unwrap();

        assert_eq!(installer.installed(ClientKind::RuneLite), None);

        fs::write(paths.client_dir().join("runelite.jar"), b"jar").unwrap();
        assert_eq!(installer.installed(ClientKind::RuneLite), None);

        fs::write(paths.client_dir().join("runelite.version"), "12345\n").unwrap();
        let installed = installer.installed(ClientKind::RuneLite).unwrap();
        assert_eq!(installed.kind, ClientKind::RuneLite);
        assert_eq!(installed.jar, paths.client_dir().join("runelite.jar"));
        assert_eq!(installed.version, "12345");

        assert_eq!(installer.installed(ClientKind::Hdos), None);
    }

    #[test]
    fn check_digest_compares_the_hex_text() {
        let hello = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        let digest = Sha256::digest(b"hello");
        check_digest(hello, &digest).unwrap();
        check_digest(&hello.to_ascii_uppercase(), &digest).unwrap();

        let error = check_digest("00".repeat(32).as_str(), &digest).unwrap_err();
        match error {
            CoreError::DigestMismatch { expected, actual } => {
                assert_eq!(expected, "00".repeat(32));
                assert_eq!(actual, hello);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn a_failed_download_keeps_the_old_jar() {
        let temp = TempDir::new("install-failed");
        let paths = paths(temp.path());
        let installer = Installer::new(&paths);
        fs::create_dir_all(paths.client_dir()).unwrap();
        fs::write(paths.client_dir().join("runelite.jar"), b"old").unwrap();
        fs::write(paths.client_dir().join("runelite.version"), "1").unwrap();

        let release = Release {
            version: "2".to_string(),
            url: "not-a-url".to_string(),
            size: Some(3),
            sha256: Some("00".repeat(32)),
        };
        let mut calls = 0;
        let error = installer
            .install(ClientKind::RuneLite, &release, &mut |_, _| calls += 1)
            .unwrap_err();

        assert!(matches!(error, CoreError::Http(_)));
        assert_eq!(calls, 0);
        assert_eq!(
            fs::read_to_string(paths.client_dir().join("runelite.jar")).unwrap(),
            "old"
        );
        assert_eq!(installer.installed(ClientKind::RuneLite).unwrap().version, "1");
        assert!(!paths.client_dir().join("runelite.jar.part").exists());
    }

    #[test]
    fn to_hex_writes_lower_case() {
        assert_eq!(to_hex(&[0x00, 0x0f, 0xff]), "000fff");
    }
}
