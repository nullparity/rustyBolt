//! Where the game clients sit on the disk.
//!
//! The user installs RuneLite or HDOS with the installer of that project.
//! The launcher looks in the places that those installers use, and it takes a
//! path from the config first.

use std::path::PathBuf;

use crate::Config;

/// The game client that the launcher can start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    /// The RuneLite client.
    RuneLite,
    /// The HDOS client.
    Hdos,
}

impl ClientKind {
    /// The lower case name of the command line.
    pub fn name(self) -> &'static str {
        match self {
            ClientKind::RuneLite => "runelite",
            ClientKind::Hdos => "hdos",
        }
    }

    /// The name that the project uses.
    pub fn title(self) -> &'static str {
        match self {
            ClientKind::RuneLite => "RuneLite",
            ClientKind::Hdos => "HDOS",
        }
    }

    /// The wiki page that explains the client and links its installer.
    pub fn wiki_url(self) -> &'static str {
        match self {
            ClientKind::RuneLite => "https://oldschool.runescape.wiki/w/RuneLite",
            ClientKind::Hdos => "https://oldschool.runescape.wiki/w/HDOS",
        }
    }

    /// The jar file from the config, if the user named one.
    fn configured_jar(self, config: &Config) -> Option<PathBuf> {
        match self {
            ClientKind::RuneLite if config.runelite_use_custom_jar => {
                config.runelite_custom_jar.clone()
            }
            ClientKind::RuneLite => None,
            ClientKind::Hdos => config.hdos_jar.clone(),
        }
    }
}

/// The places where the installer of each client puts the jar.
///
/// The list holds every candidate, so an error message can show the user
/// where the launcher looked. Use [`locate`] to find the first file that exists.
pub fn candidates(kind: ClientKind) -> Vec<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let mut paths = Vec::new();
    match kind {
        ClientKind::RuneLite => {
            if cfg!(target_os = "macos") {
                paths.push(PathBuf::from(
                    "/Applications/RuneLite.app/Contents/Resources/RuneLite.jar",
                ));
                if let Some(home) = &home {
                    paths.push(
                        home.join("Applications/RuneLite.app/Contents/Resources/RuneLite.jar"),
                    );
                }
            }
            if cfg!(windows) {
                if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                    paths.push(PathBuf::from(local).join("RuneLite").join("RuneLite.jar"));
                }
            }
            if let Some(home) = &home {
                paths.push(home.join(".local/share/RuneLite/RuneLite.jar"));
                paths.push(home.join("RuneLite.jar"));
                paths.push(home.join("Downloads/RuneLite.jar"));
            }
        }
        ClientKind::Hdos => {
            if let Some(home) = &home {
                paths.push(home.join("hdos-launcher.jar"));
                paths.push(home.join("Downloads/hdos-launcher.jar"));
            }
        }
    }
    paths
}

/// Finds the jar of one client.
///
/// A path from the config wins, even when the file is absent, so the user sees
/// a clear error for a wrong path instead of a silent fallback. Without a
/// config path the function takes the first candidate that is a file.
pub fn locate(kind: ClientKind, config: &Config) -> Option<PathBuf> {
    if let Some(jar) = kind.configured_jar(config) {
        return Some(jar);
    }
    candidates(kind).into_iter().find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configured_jar_wins_even_when_absent() {
        let config = Config {
            runelite_use_custom_jar: true,
            runelite_custom_jar: Some(PathBuf::from("/nowhere/RuneLite.jar")),
            hdos_jar: Some(PathBuf::from("/nowhere/hdos.jar")),
            ..Default::default()
        };
        assert_eq!(
            locate(ClientKind::RuneLite, &config),
            Some(PathBuf::from("/nowhere/RuneLite.jar"))
        );
        assert_eq!(
            locate(ClientKind::Hdos, &config),
            Some(PathBuf::from("/nowhere/hdos.jar"))
        );
    }

    #[test]
    fn an_unused_custom_jar_is_ignored() {
        let config = Config {
            runelite_use_custom_jar: false,
            runelite_custom_jar: Some(PathBuf::from("/nowhere/RuneLite.jar")),
            ..Default::default()
        };
        assert_ne!(
            locate(ClientKind::RuneLite, &config),
            Some(PathBuf::from("/nowhere/RuneLite.jar"))
        );
    }

    #[test]
    fn every_candidate_ends_in_a_jar() {
        for kind in [ClientKind::RuneLite, ClientKind::Hdos] {
            let list = candidates(kind);
            assert!(!list.is_empty());
            for path in list {
                assert_eq!(path.extension().and_then(|e| e.to_str()), Some("jar"));
            }
        }
    }
}
