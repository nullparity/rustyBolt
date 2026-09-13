//! A desktop entry for the RuneLite window on Linux. The module builds
//! everywhere so its tests run everywhere; only Linux calls it.
//!
//! Java sets the window class `net-runelite-launcher-Launcher`, and GNOME
//! shows that string in the top bar unless a desktop entry claims it. The
//! launcher writes one into the user's applications directory before it
//! starts the client, with the icon taken from the client jar.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const WM_CLASS: &str = "net-runelite-launcher-Launcher";
const ICON_NAME: &str = "rustybolt-runelite";
const ICON_IN_JAR: &str = "net/runelite/launcher/runelite_128.png";

/// The user's data directory, `$XDG_DATA_HOME` or `~/.local/share`.
fn data_home() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_DATA_HOME") {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
}

/// Writes the desktop entry and the icon, unless both are current.
pub fn ensure_runelite_entry(jar: &Path) -> io::Result<()> {
    let Some(data) = data_home() else {
        return Ok(());
    };
    let icon = data
        .join("icons/hicolor/128x128/apps")
        .join(format!("{ICON_NAME}.png"));
    if !icon.is_file() {
        if let Some(png) = read_zip_entry(jar, ICON_IN_JAR)? {
            fs::create_dir_all(icon.parent().unwrap())?;
            fs::write(&icon, png)?;
        }
    }
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=RuneLite\nComment=RuneLite, started by rustyBolt\n\
         Exec=rustybolt launch runelite\nIcon={ICON_NAME}\nTerminal=false\nCategories=Game;\n\
         NoDisplay=true\nStartupWMClass={WM_CLASS}\n"
    );
    let path = data
        .join("applications")
        .join(format!("{ICON_NAME}.desktop"));
    if fs::read_to_string(&path).ok().as_deref() != Some(entry.as_str()) {
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, entry)?;
    }
    Ok(())
}

/// Reads one file out of a zip archive. Stored and deflated entries only,
/// which is all a jar holds. `None` when the archive has no such file.
fn read_zip_entry(archive: &Path, wanted: &str) -> io::Result<Option<Vec<u8>>> {
    let bytes = fs::read(archive)?;
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
    let u32_at = |at: usize| {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
    };

    // The end of central directory record sits in the last 64 KB.
    let tail_start = bytes.len().saturating_sub(65_557);
    let Some(eocd) = (tail_start..bytes.len().saturating_sub(21))
        .rev()
        .find(|&at| bytes[at..at + 4] == [0x50, 0x4b, 0x05, 0x06])
    else {
        return Ok(None);
    };
    let entries = u16_at(eocd + 10);
    let mut at = u32_at(eocd + 16);

    for _ in 0..entries {
        if at + 46 > bytes.len() || bytes[at..at + 4] != [0x50, 0x4b, 0x01, 0x02] {
            return Ok(None);
        }
        let method = u16_at(at + 10);
        let compressed = u32_at(at + 20);
        let name_len = u16_at(at + 28);
        let extra_len = u16_at(at + 30);
        let comment_len = u16_at(at + 32);
        let local = u32_at(at + 42);
        let name = &bytes[at + 46..at + 46 + name_len];
        at += 46 + name_len + extra_len + comment_len;
        if name != wanted.as_bytes() {
            continue;
        }
        // The local header repeats the name and extra fields with its own lengths.
        let data = local + 30 + u16_at(local + 26) + u16_at(local + 28);
        let raw = &bytes[data..data + compressed];
        return match method {
            0 => Ok(Some(raw.to_vec())),
            8 => {
                let mut out = Vec::new();
                flate2::read::DeflateDecoder::new(raw).read_to_end(&mut out)?;
                Ok(Some(out))
            }
            _ => Ok(None),
        };
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A zip with one deflated file, built by hand so the test has no writer dependency.
    fn zip_with(name: &str, content: &[u8]) -> Vec<u8> {
        let mut deflated = Vec::new();
        flate2::write::DeflateEncoder::new(&mut deflated, flate2::Compression::default())
            .write_all(content)
            .unwrap();
        let mut z = Vec::new();
        let local_at = 0u32;
        z.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 8, 0, 0, 0, 0, 0]);
        z.extend_from_slice(&[0; 4]); // crc, unchecked here
        z.extend_from_slice(&(deflated.len() as u32).to_le_bytes());
        z.extend_from_slice(&(content.len() as u32).to_le_bytes());
        z.extend_from_slice(&(name.len() as u16).to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());
        z.extend_from_slice(name.as_bytes());
        z.extend_from_slice(&deflated);
        let cd_at = z.len() as u32;
        z.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 8, 0, 0, 0, 0, 0]);
        z.extend_from_slice(&[0; 4]);
        z.extend_from_slice(&(deflated.len() as u32).to_le_bytes());
        z.extend_from_slice(&(content.len() as u32).to_le_bytes());
        z.extend_from_slice(&(name.len() as u16).to_le_bytes());
        z.extend_from_slice(&[0; 2 + 2 + 2 + 2 + 4]);
        z.extend_from_slice(&local_at.to_le_bytes());
        z.extend_from_slice(name.as_bytes());
        let cd_len = z.len() as u32 - cd_at;
        z.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 1, 0, 1, 0]);
        z.extend_from_slice(&cd_len.to_le_bytes());
        z.extend_from_slice(&cd_at.to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());
        z
    }

    #[test]
    fn reads_a_deflated_entry_and_misses_an_absent_one() {
        let dir = crate::test_support::TempDir::new("zip-entry");
        let jar = dir.path().join("a.jar");
        fs::write(&jar, zip_with("net/x/icon.png", b"PNG-BYTES")).unwrap();
        assert_eq!(
            read_zip_entry(&jar, "net/x/icon.png").unwrap().as_deref(),
            Some(&b"PNG-BYTES"[..])
        );
        assert_eq!(read_zip_entry(&jar, "nope").unwrap(), None);
    }
}
