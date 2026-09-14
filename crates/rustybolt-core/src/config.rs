//! The launcher configuration file.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::import::RuneLiteHome;
use crate::profile::PropertyOverrides;
use crate::session::SessionMaxAge;
use crate::tuning::TuningConfig;
use crate::Paths;

/// The default window of the recent use rule, in seconds.
pub const DEFAULT_RECENT_WINDOW: u64 = 3600;

/// The user settings of the launcher. The file is `launcher.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// If true, the launcher runs [`Config::runelite_custom_jar`] instead of the installed jar.
    pub runelite_use_custom_jar: bool,
    /// The jar file of the user, when `runelite_use_custom_jar` is true.
    pub runelite_custom_jar: Option<PathBuf>,
    /// A template for RuneLite. `%command%` expands to the default invocation.
    pub runelite_launch_command: Option<String>,
    /// A template for HDOS. `%command%` expands to the default invocation.
    pub hdos_launch_command: Option<String>,
    /// The HDOS launcher jar. `None` makes the launcher search the usual places.
    pub hdos_jar: Option<PathBuf>,
    /// The Java binary that the launcher must use.
    pub java_path: Option<PathBuf>,
    /// Java binaries to try in order, before the automatic search.
    ///
    /// The launcher takes the first entry that is an executable file. Use this
    /// list to pin a new JDK and to keep an older JDK as the fallback.
    pub java_candidates: Vec<PathBuf>,
    /// Close the launcher after it starts a game client.
    pub close_after_launch: bool,
    pub runelite_tuning: TuningConfig,
    /// The launcher makes a hard link to the Java binary with this name. The Dock
    /// and the process list then show the name instead of `java`. `None` turns the
    /// rule off.
    pub runelite_process_name: Option<String>,
    /// Forced values of the RuneLite profile properties. `None` turns the rule off.
    pub runelite_profile_overrides: Option<PropertyOverrides>,
    /// The directory of the RuneLite profiles.
    ///
    /// `None` means `~/.runelite/profiles2`.
    pub runelite_profile_dir: Option<PathBuf>,
    /// Where the launcher takes the login values from.
    pub credential_source: crate::credentials::CredentialSource,
    /// The window of the recent use rule, in seconds.
    pub usage_recent_window_secs: u64,
    /// Where the launcher keeps the RuneLite client home.
    pub runelite_home_kind: RuneLiteHome,
    /// The language of the dashboard, as a BCP 47 tag. `None` follows the
    /// operating system.
    pub language: Option<String>,
    /// Sign out of a Jagex account this long after the login. `None` keeps
    /// a login until the user signs out.
    pub session_max_age: Option<SessionMaxAge>,
    /// A GitHub token for the release check. Optional: it raises the rate
    /// limit and reaches a private repository.
    pub github_token: Option<String>,
    /// Wifi mode: the UDP keepalive to the gateway. The launcher restores
    /// it at start, so a restart after an update keeps it.
    pub wifi_mode: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            runelite_use_custom_jar: false,
            runelite_custom_jar: None,
            runelite_launch_command: None,
            hdos_launch_command: None,
            hdos_jar: None,
            java_path: None,
            java_candidates: Vec::new(),
            close_after_launch: false,
            runelite_tuning: TuningConfig::default(),
            runelite_process_name: Some("RuneLite".to_string()),
            runelite_profile_overrides: None,
            runelite_profile_dir: None,
            credential_source: crate::credentials::CredentialSource::default(),
            usage_recent_window_secs: DEFAULT_RECENT_WINDOW,
            runelite_home_kind: RuneLiteHome::default(),
            language: None,
            session_max_age: None,
            github_token: None,
            wifi_mode: false,
        }
    }
}

impl Config {
    /// Reads the config file. The function never fails.
    ///
    /// An absent file gives the default value. A malformed file gives the
    /// default value also. The load moves a stored launch argument into the
    /// typed field.
    pub fn load(paths: &Paths) -> Config {
        let mut config: Config = fs::read_to_string(paths.config_file())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        config.runelite_tuning.migrate_stored_app_args();
        config
    }

    /// Writes the config file as pretty JSON.
    pub fn save(&self, paths: &Paths) -> io::Result<()> {
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::create_dir_all(&paths.config_dir)?;
        fs::write(paths.config_file(), text)
    }

    /// The directory of the RuneLite profile properties.
    ///
    /// The directory follows the chosen home, so `profile apply` never touches a
    /// file that the launched client does not read.
    pub fn profile_dir(&self, paths: &Paths) -> PathBuf {
        if let Some(dir) = &self.runelite_profile_dir {
            return dir.clone();
        }
        self.runelite_home(paths).join(".runelite/profiles2")
    }

    /// The value of `-Duser.home` for the client.
    ///
    /// The client makes its `.runelite` directory below this path.
    pub fn runelite_home(&self, paths: &Paths) -> PathBuf {
        match &self.runelite_home_kind {
            RuneLiteHome::Isolated => paths.data_dir.clone(),
            RuneLiteHome::System => std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| paths.data_dir.clone()),
            RuneLiteHome::Custom(path) => path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{paths, TempDir};

    fn sample() -> Config {
        Config {
            runelite_use_custom_jar: true,
            runelite_custom_jar: Some(PathBuf::from("/tmp/custom.jar")),
            runelite_launch_command: Some("time %command%".to_string()),
            java_path: Some(PathBuf::from("/usr/bin/java")),
            close_after_launch: true,
            ..Config::default()
        }
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = TempDir::new("config");
        let paths = paths(dir.path());
        let config = sample();
        config.save(&paths).expect("cannot save the config");
        assert_eq!(Config::load(&paths), config);
    }

    #[test]
    fn the_default_config_round_trips() {
        let dir = TempDir::new("config-default");
        let paths = paths(dir.path());
        let config = Config::default();
        config.save(&paths).expect("cannot save the config");
        assert_eq!(Config::load(&paths), config);
    }

    #[test]
    fn load_uses_the_default_for_an_absent_file() {
        let dir = TempDir::new("config-absent");
        assert_eq!(Config::load(&paths(dir.path())), Config::default());
    }

    #[test]
    fn load_uses_the_default_for_a_malformed_file() {
        let dir = TempDir::new("config-bad");
        let paths = paths(dir.path());
        fs::create_dir_all(&paths.config_dir).expect("cannot make the directory");
        fs::write(paths.config_file(), "{ not json").expect("cannot write the file");
        assert_eq!(Config::load(&paths), Config::default());
    }

    #[test]
    fn load_moves_a_stored_app_argument_into_the_typed_field() {
        let dir = TempDir::new("config-legacy-args");
        let paths = paths(dir.path());
        fs::create_dir_all(&paths.config_dir).expect("cannot make the directory");
        fs::write(
            paths.config_file(),
            r#"{"runelite_tuning":{"extra_app_args":["--hw-accel","OFF","--launch-mode","LAUNCHER"]}}"#,
        )
        .expect("cannot write the file");

        let tuning = Config::load(&paths).runelite_tuning;

        assert_eq!(tuning.hw_accel, crate::tuning::HwAccel::Off);
        assert_eq!(tuning.launch_mode, crate::tuning::LaunchMode::Launcher);
        assert!(tuning.extra_app_args.is_empty());
    }

    #[test]
    fn an_old_file_keeps_the_new_defaults() {
        let dir = TempDir::new("config-old");
        let paths = paths(dir.path());
        fs::create_dir_all(&paths.config_dir).expect("cannot make the directory");
        fs::write(paths.config_file(), r#"{"close_after_launch":true}"#)
            .expect("cannot write the file");
        let config = Config::load(&paths);
        assert!(config.close_after_launch);
        // A file that a older version wrote still gets every new default.
        assert_eq!(config.runelite_tuning, TuningConfig::default());
        assert_eq!(config.usage_recent_window_secs, DEFAULT_RECENT_WINDOW);
    }
}
