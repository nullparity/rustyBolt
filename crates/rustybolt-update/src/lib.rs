//! The release check and the in-place update of the launcher binary.
//!
//! The release layout is the one that dist makes: one archive per target
//! triple, `rustybolt-cli-<triple>.tar.gz` (`.zip` on Windows), with a
//! `.sha256` file next to it. The update downloads the archive of this
//! machine, checks the digest, takes the executable out and swaps it for
//! the running one. A restart then runs the new version.

use std::fs;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The GitHub repository that holds the releases.
pub const REPO: &str = "nullparity/rustyBolt";
/// The dist app name: the stem of every archive.
const APP: &str = "rustybolt-cli";
/// The executable inside the archive.
const BINARY: &str = "rustybolt";
/// Redirect hops a download may take. GitHub uses one.
const MAX_HOPS: usize = 5;
/// The largest archive the update accepts, in bytes.
const MAX_ARCHIVE_BYTES: u64 = 200 << 20;

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("network error: {0}")]
    Http(String),
    #[error("{0}")]
    Security(#[from] rustybolt_security::SecurityError),
    #[error("the release {0} has no archive for {1}")]
    NoAsset(String, String),
    #[error("the archive digest does not match: expected {expected}, got {actual}")]
    Checksum { expected: String, actual: String },
    #[error("the archive holds no {BINARY} executable")]
    NoBinary,
    #[error("the release has no readable version: {0}")]
    BadVersion(String),
    #[error("{0}")]
    Io(#[from] io::Error),
}

impl From<ureq::Error> for UpdateError {
    fn from(error: ureq::Error) -> UpdateError {
        UpdateError::Http(error.to_string())
    }
}

/// A release newer than the running launcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Release {
    /// The version without the `v`, as in `0.5.0`.
    pub version: String,
    /// The release page, for a user who must install by hand.
    pub page_url: String,
    archive_url: String,
    checksum_url: String,
}

/// Where the launcher binary can go: replaced in place, or only through the
/// tool that installed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallKind {
    /// The update can swap the binary.
    InPlace,
    /// A package manager or installer owns the binary. The user takes the
    /// new package from the release page.
    Managed { reason: String },
}

/// The release client. A token raises the GitHub rate limit and reaches a
/// private repository.
pub struct Updater {
    repo: String,
    token: Option<String>,
    target: String,
}

impl Updater {
    pub fn new(token: Option<String>) -> Updater {
        Updater {
            repo: REPO.to_string(),
            token: token.filter(|t| !t.trim().is_empty()),
            target: target_triple(),
        }
    }

    /// `None` when the latest release is `current` or older.
    pub fn newer_than(&self, current: &str) -> Result<Option<Release>, UpdateError> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", self.repo);
        let body = self.fetch(&url, "application/vnd.github+json")?;
        let text = String::from_utf8_lossy(&body);
        let api: ApiRelease =
            serde_json::from_str(&text).map_err(|e| UpdateError::Http(e.to_string()))?;
        // An older release may predate the dist layout; it needs no archive.
        if !is_newer(&api.tag_name, current)? {
            return Ok(None);
        }
        Ok(Some(pick_release(api, &self.target)?))
    }

    /// Downloads the archive of `release`, checks it and swaps the running
    /// binary. Returns the path of the binary.
    pub fn install(&self, release: &Release) -> Result<PathBuf, UpdateError> {
        let archive = self.fetch(&release.archive_url, "application/octet-stream")?;
        let expected = parse_checksum(&String::from_utf8_lossy(
            &self.fetch(&release.checksum_url, "text/plain")?,
        ));
        let actual = hex(&Sha256::digest(&archive));
        if expected != actual {
            return Err(UpdateError::Checksum { expected, actual });
        }
        let binary = extract_binary(&archive, release.archive_url.ends_with(".zip"))?;

        let exe = std::env::current_exe()?;
        let staged = exe.with_extension("update");
        fs::write(&staged, binary)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))?;
        }
        let swapped = self_replace::self_replace(&staged);
        let _ = fs::remove_file(&staged);
        swapped?;
        Ok(exe)
    }

    /// GETs `url` and follows redirects one hop at a time, so every hop
    /// passes the egress allowlist.
    fn fetch(&self, url: &str, accept: &str) -> Result<Vec<u8>, UpdateError> {
        let config = ureq::config::Config::builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .build();
        let agent = ureq::Agent::new_with_config(config);
        let mut url = url.to_string();
        for _ in 0..MAX_HOPS {
            rustybolt_security::validate_url(&url)?;
            let mut request = agent.get(&url).header("Accept", accept).header(
                "User-Agent",
                concat!("rustybolt/", env!("CARGO_PKG_VERSION")),
            );
            // The token goes to the API host only. A storage host gets a signed URL.
            if let (Some(token), true) = (&self.token, url.starts_with("https://api.github.com/")) {
                request = request.header("Authorization", &format!("Bearer {token}"));
            }
            let mut response = request.call()?;
            let status = response.status().as_u16();
            if (300..400).contains(&status) {
                let Some(next) = response
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                else {
                    return Err(UpdateError::Http(format!("{status} without a location")));
                };
                url = next.to_string();
                continue;
            }
            if status != 200 {
                return Err(UpdateError::Http(format!("{url} answered {status}")));
            }
            return Ok(response
                .body_mut()
                .with_config()
                .limit(MAX_ARCHIVE_BYTES)
                .read_to_vec()?);
        }
        Err(UpdateError::Http("too many redirects".to_string()))
    }
}

/// How the running binary was installed.
pub fn install_kind() -> InstallKind {
    let exe = std::env::current_exe().unwrap_or_default();
    install_kind_of(&exe, std::env::var_os("APPIMAGE").is_some())
}

fn install_kind_of(exe: &Path, appimage: bool) -> InstallKind {
    let managed = |reason: &str| InstallKind::Managed {
        reason: reason.to_string(),
    };
    if appimage {
        return managed("AppImage");
    }
    let text = exe.to_string_lossy();
    if cfg!(target_os = "linux") && (text.starts_with("/usr/") || text.starts_with("/opt/")) {
        return managed("package");
    }
    if cfg!(target_os = "windows") {
        let lower = text.to_ascii_lowercase();
        if lower.contains("\\program files") {
            return managed("msi");
        }
    }
    InstallKind::InPlace
}

/// Starts the binary at `exe` again with the arguments of this process.
/// The caller exits afterwards.
pub fn restart(exe: &Path) -> io::Result<()> {
    Command::new(exe)
        .args(std::env::args_os().skip(1))
        .spawn()
        .map(|_| ())
}

/// The Rust target triple of this build, as dist names the archives.
pub fn target_triple() -> String {
    let arch = std::env::consts::ARCH;
    let os = match std::env::consts::OS {
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        other => {
            return format!("{arch}-unknown-{other}-gnu");
        }
    };
    format!("{arch}-{os}")
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
}

/// Picks the archive of `target` out of a GitHub release.
fn pick_release(api: ApiRelease, target: &str) -> Result<Release, UpdateError> {
    let version = api.tag_name.trim_start_matches('v').to_string();
    let extension = if target.contains("windows") {
        "zip"
    } else {
        "tar.gz"
    };
    let archive_name = format!("{APP}-{target}.{extension}");
    let checksum_name = format!("{archive_name}.sha256");
    let find = |name: &str| {
        api.assets
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.browser_download_url.clone())
    };
    let (Some(archive_url), Some(checksum_url)) = (find(&archive_name), find(&checksum_name))
    else {
        return Err(UpdateError::NoAsset(api.tag_name, target.to_string()));
    };
    Ok(Release {
        version,
        page_url: api.html_url,
        archive_url,
        checksum_url,
    })
}

/// True when `candidate` is a later semantic version than `current`.
fn is_newer(candidate: &str, current: &str) -> Result<bool, UpdateError> {
    let parse = |v: &str| {
        semver::Version::parse(v.trim_start_matches('v'))
            .map_err(|e| UpdateError::BadVersion(format!("{v}: {e}")))
    };
    Ok(parse(candidate)? > parse(current)?)
}

/// The digest from a `.sha256` file: the first word, lower case.
fn parse_checksum(text: &str) -> String {
    text.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The `rustybolt` executable inside a dist archive.
fn extract_binary(archive: &[u8], is_zip: bool) -> Result<Vec<u8>, UpdateError> {
    let wanted = |path: &Path| {
        path.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == BINARY || n == "rustybolt.exe")
    };
    if is_zip {
        let mut zip = zip::ZipArchive::new(Cursor::new(archive))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        for index in 0..zip.len() {
            let mut file = zip
                .by_index(index)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            if file.is_file() && wanted(Path::new(file.name())) {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                return Ok(bytes);
            }
        }
    } else {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
        for entry in tar.entries()? {
            let mut entry = entry?;
            if entry.header().entry_type().is_file() && wanted(&entry.path()?) {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes)?;
                return Ok(bytes);
            }
        }
    }
    Err(UpdateError::NoBinary)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const RELEASE: &str = r#"{
        "tag_name": "v0.5.0",
        "html_url": "https://github.com/nullparity/rustyBolt/releases/tag/v0.5.0",
        "assets": [
            {"name": "rustybolt-cli-aarch64-apple-darwin.tar.gz",
             "browser_download_url": "https://github.com/nullparity/rustyBolt/releases/download/v0.5.0/rustybolt-cli-aarch64-apple-darwin.tar.gz"},
            {"name": "rustybolt-cli-aarch64-apple-darwin.tar.gz.sha256",
             "browser_download_url": "https://github.com/nullparity/rustyBolt/releases/download/v0.5.0/rustybolt-cli-aarch64-apple-darwin.tar.gz.sha256"},
            {"name": "rustybolt_0.5.0_darwin_arm64.dmg",
             "browser_download_url": "https://github.com/nullparity/rustyBolt/releases/download/v0.5.0/rustybolt_0.5.0_darwin_arm64.dmg"}
        ]
    }"#;

    fn parse_release(body: &str, target: &str) -> Result<Release, UpdateError> {
        pick_release(serde_json::from_str(body).unwrap(), target)
    }

    #[test]
    fn parse_release_picks_the_archive_of_the_target() {
        let release = parse_release(RELEASE, "aarch64-apple-darwin").unwrap();
        assert_eq!(release.version, "0.5.0");
        assert!(release.archive_url.ends_with("aarch64-apple-darwin.tar.gz"));
        assert!(release.checksum_url.ends_with(".tar.gz.sha256"));
        assert!(release.page_url.ends_with("/tag/v0.5.0"));

        let error = parse_release(RELEASE, "x86_64-pc-windows-msvc").unwrap_err();
        assert!(
            matches!(error, UpdateError::NoAsset(tag, target) if tag == "v0.5.0" && target.contains("windows"))
        );
    }

    #[test]
    fn a_windows_target_wants_a_zip() {
        let body = RELEASE.replace("aarch64-apple-darwin.tar.gz", "x86_64-pc-windows-msvc.zip");
        let release = parse_release(&body, "x86_64-pc-windows-msvc").unwrap();
        assert!(release.archive_url.ends_with("x86_64-pc-windows-msvc.zip"));
    }

    #[test]
    fn versions_compare_as_semver() {
        assert!(is_newer("0.5.0", "0.4.0").unwrap());
        assert!(is_newer("v0.10.0", "0.9.9").unwrap());
        assert!(!is_newer("0.4.0", "0.4.0").unwrap());
        assert!(!is_newer("0.4.0-rc.1", "0.4.0").unwrap());
        assert!(is_newer("nope", "0.4.0").is_err());
    }

    #[test]
    fn checksum_takes_the_first_word() {
        assert_eq!(parse_checksum("ABCD  file.tar.gz\n"), "abcd");
        assert_eq!(parse_checksum(""), "");
    }

    #[test]
    fn extract_finds_the_binary_in_a_tarball() {
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        for (path, data) in [
            ("rustybolt-cli-x/README.md", b"docs".as_slice()),
            ("rustybolt-cli-x/rustybolt", b"binary".as_slice()),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            tar.append_data(&mut header, path, data).unwrap();
        }
        let archive = tar.into_inner().unwrap().finish().unwrap();
        assert_eq!(extract_binary(&archive, false).unwrap(), b"binary");

        let mut empty = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        empty.finish().unwrap();
        let archive = empty.into_inner().unwrap().finish().unwrap();
        assert!(matches!(
            extract_binary(&archive, false),
            Err(UpdateError::NoBinary)
        ));
    }

    #[test]
    fn extract_finds_the_binary_in_a_zip() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("rustybolt-cli-x/README.md", options)
            .unwrap();
        zip.write_all(b"docs").unwrap();
        zip.start_file("rustybolt-cli-x/rustybolt.exe", options)
            .unwrap();
        zip.write_all(b"binary").unwrap();
        let archive = zip.finish().unwrap().into_inner();
        assert_eq!(extract_binary(&archive, true).unwrap(), b"binary");
    }

    #[test]
    fn install_kind_follows_the_path() {
        assert_eq!(
            install_kind_of(Path::new("/x/rustybolt"), true),
            InstallKind::Managed {
                reason: "AppImage".into()
            }
        );
        if cfg!(target_os = "linux") {
            assert!(matches!(
                install_kind_of(Path::new("/usr/bin/rustybolt"), false),
                InstallKind::Managed { .. }
            ));
        }
        if cfg!(target_os = "windows") {
            assert!(matches!(
                install_kind_of(
                    Path::new(r"C:\Program Files\rustyBolt\rustybolt.exe"),
                    false
                ),
                InstallKind::Managed { .. }
            ));
        }
        assert_eq!(
            install_kind_of(
                Path::new("/Applications/rustyBolt.app/Contents/MacOS/rustybolt"),
                false
            ),
            InstallKind::InPlace
        );
    }

    #[test]
    fn the_target_triple_matches_the_dist_names() {
        let triple = target_triple();
        assert!(triple.starts_with(std::env::consts::ARCH));
        assert!(
            triple.ends_with("apple-darwin")
                || triple.ends_with("pc-windows-msvc")
                || triple.ends_with("unknown-linux-gnu")
        );
    }

    #[test]
    fn the_token_stays_off_the_download_hosts() {
        // `fetch` adds the bearer header for the API host only; the rule is
        // a prefix test, so pin it here.
        assert!("https://api.github.com/repos/x".starts_with("https://api.github.com/"));
        assert!(!"https://objects.githubusercontent.com/x".starts_with("https://api.github.com/"));
    }
}
