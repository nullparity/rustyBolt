//! Forced RuneLite profile properties.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::CoreError;

/// The file extension of a profile file.
const PROFILE_EXTENSION: &str = "properties";
/// The extension of the backup copy.
const BACKUP_EXTENSION: &str = "bak";

/// The property prefixes that the GPU example removes.
const GPU_PREFIXES: [&str; 2] = ["gpu.", "region-locker-gpu."];

/// The forced pairs of the GPU example, in order: what works well for the GPU
/// plugin on a modern macOS laptop. An example of the form for other systems,
/// not a recommendation there and never a default.
const GPU_FORCED: [(&str, &str); 15] = [
    ("gpu.expandedMapLoadingChunks", "0"),
    ("gpu.vsyncMode", "OFF"),
    ("gpu.colorBlindIntensity", "100"),
    ("gpu.hideUnrelatedMaps", "true"),
    ("gpu.colorBlindMode", "NONE"),
    ("gpu.antiAliasingMode", "DISABLED"),
    ("gpu.drawDistance", "25"),
    ("gpu.fpsTarget", "60"),
    ("gpu.brightTextures", "true"),
    ("gpu.anisotropicFilteringLevel", "0"),
    ("gpu.uiScalingMode", "CATMULL_ROM"),
    ("gpu.smoothBanding", "true"),
    ("gpu.fogDepth", "3"),
    ("gpu.removeVertexSnapping", "false"),
    ("gpu.unlockFps", "true"),
];

/// The forced values of the RuneLite profile properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PropertyOverrides {
    /// Every property line that starts with one of these prefixes goes away.
    pub strip_prefixes: Vec<String>,
    /// These key and value pairs replace the removed lines.
    pub force: Vec<(String, String)>,
}

impl PropertyOverrides {
    /// The GPU example. The launcher applies nothing unless the user turns
    /// overrides on; this is one starting point for the editor.
    pub fn gpu_example() -> PropertyOverrides {
        PropertyOverrides {
            strip_prefixes: GPU_PREFIXES
                .iter()
                .map(|prefix| (*prefix).to_string())
                .collect(),
            force: GPU_FORCED
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect(),
        }
    }

    /// Applies the rules to the text of one properties file.
    ///
    /// The function keeps every other line and its order, then appends the
    /// forced lines.
    pub fn apply_to_text(&self, text: &str) -> String {
        let mut output = String::new();
        for line in text.lines() {
            if self.removes(line) {
                continue;
            }
            output.push_str(line);
            output.push('\n');
        }
        for (key, value) in &self.force {
            output.push_str(key);
            output.push('=');
            output.push_str(value);
            output.push('\n');
        }
        output
    }

    /// Tells if a line starts with one of the strip prefixes.
    fn removes(&self, line: &str) -> bool {
        self.strip_prefixes
            .iter()
            .any(|prefix| line.starts_with(prefix.as_str()))
    }
}

/// Applies the rules to every `*.properties` file of a directory.
///
/// The function returns the changed count. It writes a `.bak` copy before the
/// first change of each file. An absent directory changes no file.
pub fn apply_to_profiles(dir: &Path, overrides: &PropertyOverrides) -> Result<usize, CoreError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let mut changed = 0;
    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some(PROFILE_EXTENSION) {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let updated = overrides.apply_to_text(&text);
        if updated == text {
            continue;
        }
        fs::copy(&path, backup_path(&path))?;
        fs::write(&path, &updated)?;
        changed += 1;
    }
    Ok(changed)
}

/// Builds the backup path of one profile file.
fn backup_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "profile".to_string());
    path.with_file_name(format!("{}.{}", name, BACKUP_EXTENSION))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn gpu_example_gives_the_exact_forced_pairs() {
        let overrides = PropertyOverrides::gpu_example();
        assert_eq!(overrides.strip_prefixes, vec!["gpu.", "region-locker-gpu."]);
        let expected: Vec<(String, String)> = GPU_FORCED
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        assert_eq!(overrides.force, expected);
        assert_eq!(overrides.force.len(), 15);
    }

    #[test]
    fn apply_to_text_removes_both_prefixes_keeps_other_lines_and_appends_the_forced_pairs() {
        let overrides = PropertyOverrides::gpu_example();
        let text = "gpu.vsyncMode=ON\nregion-locker-gpu.drawDistance=10\nother=1\nanother=2\n";
        let output = overrides.apply_to_text(text);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(&lines[0..2], &["other=1", "another=2"]);
        assert!(!output.contains("gpu.vsyncMode=ON"));
        assert!(!output.contains("region-locker-gpu.drawDistance=10"));
        assert_eq!(lines.len(), 2 + overrides.force.len());
        for (key, value) in &overrides.force {
            let wanted = format!("{}={}", key, value);
            assert!(
                output.lines().any(|line| line == wanted),
                "the output has no line {wanted}"
            );
        }
    }

    #[test]
    fn apply_to_profiles_writes_a_backup_and_reports_the_changed_count() {
        let temp = TempDir::new("profile-files");
        let dir = temp.path().join("profiles2");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.properties");
        let original = "gpu.fpsTarget=30\nother.value=1\n";
        fs::write(&file, original).unwrap();
        let overrides = PropertyOverrides::gpu_example();

        assert_eq!(apply_to_profiles(&dir, &overrides).unwrap(), 1);
        assert_eq!(fs::read_to_string(backup_path(&file)).unwrap(), original);
        let updated = fs::read_to_string(&file).unwrap();
        assert!(updated.starts_with("other.value=1\n"));
        assert!(updated.contains("gpu.fpsTarget=60\n"));
        assert!(!updated.contains("gpu.fpsTarget=30"));

        assert_eq!(apply_to_profiles(&dir, &overrides).unwrap(), 0);
    }

    #[test]
    fn apply_to_profiles_ignores_an_absent_directory_and_other_files() {
        let temp = TempDir::new("profile-absent");
        let overrides = PropertyOverrides::gpu_example();
        let absent = temp.path().join("missing");
        assert_eq!(apply_to_profiles(&absent, &overrides).unwrap(), 0);

        let dir = temp.path().join("profiles2");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("notes.txt"), "gpu.fpsTarget=30\n").unwrap();
        assert_eq!(apply_to_profiles(&dir, &overrides).unwrap(), 0);
    }
}
