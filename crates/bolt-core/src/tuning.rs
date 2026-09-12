//! The user facing JVM tuning of the RuneLite client.
//!
//! The launcher keeps the tuning inside `launcher.json`. This module turns the
//! stored values into the [`bolt_jdk::Tuning`] structure that builds the real
//! JVM flags.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The garbage collector of the client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GcChoice {
    /// Let the JVM choose the collector.
    Default,
    /// The generational Z collector. This is the default of the source launcher.
    Z,
    /// The G1 collector.
    G1,
    /// The parallel collector.
    Parallel,
}

impl Default for GcChoice {
    fn default() -> Self {
        GcChoice::Z
    }
}

/// Where the icon of the installed client normally sits on macOS.
///
/// The launcher uses the first file that exists, so the Dock shows the game and
/// not the Java icon.
const DEFAULT_DOCK_ICONS: [&str; 2] = [
    "/Applications/RuneLite.app/Contents/Resources/icons.icns",
    "/Applications/RuneLite.app/Contents/Resources/runelite.icns",
];

/// The package that the launcher opens to the unnamed module.
const DEFAULT_ADD_OPENS: [&str; 7] = [
    "java.base/java.net",
    "java.base/java.io",
    "java.base/java.lang",
    "java.base/java.lang.invoke",
    "java.desktop/com.apple.eawt",
    "java.desktop/sun.awt",
    "java.desktop/java.awt.event",
];

/// The jar name that the launcher uses when the repository has no client jar.
const FALLBACK_JAR_NAME: &str = "client.jar";

/// The JVM tuning of the RuneLite client.
///
/// The default value reproduces the profile of the source launcher. The client
/// runs on JDK 24 or newer, so the default profile uses the compact object
/// headers and the AOT cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TuningConfig {
    /// Add the tuning to the command line.
    pub enabled: bool,
    /// The initial heap size, for example `2g`.
    pub heap_min: Option<String>,
    /// The largest heap size, for example `2g`.
    pub heap_max: Option<String>,
    /// The stack size of each thread, for example `2m`.
    pub stack_size: Option<String>,
    pub garbage_collector: GcChoice,
    /// Add `-XX:+UseCompactObjectHeaders`. The flag needs JDK 24 or newer.
    pub compact_object_headers: bool,
    /// Add `-XX:+UseStringDeduplication`.
    pub string_deduplication: bool,
    /// Add `--enable-native-access=ALL-UNNAMED`.
    pub native_access: bool,
    /// Add the AOT cache of the client jar. The flag needs JDK 24 or newer.
    pub aot_cache: bool,
    /// Write the garbage collector log. The launcher removes a log of a dead client.
    pub gc_log: bool,
    /// The Java packages that the launcher opens to the unnamed module.
    pub add_opens: Vec<String>,
    /// Add the Metal rendering properties. The properties apply to macOS only.
    pub java2d_metal: bool,
    /// The application name of the process. `None` leaves the name unset.
    pub application_name: Option<String>,
    /// `None` leaves the Dock icon unset.
    pub dock_icon: Option<PathBuf>,
    /// Add `-Drunelite.launcher.nojvm=true`.
    pub launcher_nojvm: bool,
    /// The JVM arguments that the launcher adds at the end.
    pub extra_jvm_args: Vec<String>,
    /// The application arguments of the client. They come after the jar.
    pub extra_app_args: Vec<String>,
}

impl Default for TuningConfig {
    fn default() -> Self {
        TuningConfig {
            enabled: true,
            heap_min: Some("2g".to_string()),
            heap_max: Some("2g".to_string()),
            stack_size: Some("2m".to_string()),
            garbage_collector: GcChoice::Z,
            compact_object_headers: true,
            string_deduplication: true,
            native_access: true,
            aot_cache: true,
            gc_log: false,
            add_opens: DEFAULT_ADD_OPENS.iter().map(|name| name.to_string()).collect(),
            java2d_metal: true,
            application_name: Some("RuneLite".to_string()),
            dock_icon: None,
            launcher_nojvm: true,
            extra_jvm_args: Vec::new(),
            extra_app_args: vec![
                "--hw-accel".to_string(),
                "METAL".to_string(),
                "--launch-mode".to_string(),
                "REFLECT".to_string(),
            ],
        }
    }
}

impl TuningConfig {
    /// Builds the JVM tuning. `log_dir` holds the gc log and the AOT cache.
    /// `client_repository` is the RuneLite jar cache, normally `~/.runelite/repository2`.
    pub fn to_tuning(&self, log_dir: &Path, client_repository: Option<&Path>) -> bolt_jdk::Tuning {
        let aot_cache = self.take_aot_cache(log_dir, client_repository);
        bolt_jdk::Tuning {
            heap_min: self.heap_min.clone(),
            heap_max: self.heap_max.clone(),
            stack_size: self.stack_size.clone(),
            gc: self.collector(),
            compact_object_headers: self.compact_object_headers,
            string_deduplication: self.string_deduplication,
            native_access: self.native_access,
            add_opens: self.add_opens.clone(),
            aot_cache,
            gc_log: if self.gc_log {
                Some(log_dir.to_path_buf())
            } else {
                None
            },
            extra: self.extra_jvm_args.clone(),
        }
    }

    /// Builds the AOT cache of the newest client jar and removes a stale cache.
    ///
    /// A repository without a client jar keeps the plain name `client.jar` as the
    /// key, so the launcher writes the cache of the next install.
    fn take_aot_cache(
        &self,
        log_dir: &Path,
        client_repository: Option<&Path>,
    ) -> Option<bolt_jdk::AotCache> {
        if !self.aot_cache {
            return None;
        }
        let jar = client_repository
            .and_then(|dir| bolt_jdk::newest_jar(dir, "client-"))
            .unwrap_or_else(|| PathBuf::from(FALLBACK_JAR_NAME));
        let cache = bolt_jdk::aot_cache_for(log_dir, &jar);
        // A stale cache is not a failure. The launch continues.
        let _ = bolt_jdk::prune_aot_caches(log_dir, &cache.path);
        Some(cache)
    }

    /// The system properties that this tuning adds, as key and value pairs.
    pub fn system_properties(&self) -> Vec<(String, String)> {
        let mut properties = Vec::new();
        if self.java2d_metal && cfg!(target_os = "macos") {
            properties.push(("sun.java2d.metal".to_string(), "true".to_string()));
            properties.push((
                "apple.awt.application.appearance".to_string(),
                "system".to_string(),
            ));
        }
        if let Some(name) = &self.application_name {
            properties.push(("apple.awt.application.name".to_string(), name.clone()));
        }
        if self.launcher_nojvm {
            properties.push(("runelite.launcher.nojvm".to_string(), "true".to_string()));
        }
        properties
    }

    /// The plain JVM arguments that are not `-D` properties, such as the dock flags.
    pub fn dock_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if cfg!(target_os = "macos") {
            if let Some(name) = &self.application_name {
                args.push(format!("-Xdock:name={name}"));
            }
            if let Some(icon) = self.dock_icon() {
                args.push(format!("-Xdock:icon={}", icon.to_string_lossy()));
            }
        }
        args
    }

    /// The config wins. Without a value the function looks for the icon of the
    /// installed client. A Java process with no icon shows the Java icon, which
    /// tells the user nothing about the game.
    pub fn dock_icon(&self) -> Option<PathBuf> {
        if let Some(icon) = &self.dock_icon {
            return Some(icon.clone());
        }
        DEFAULT_DOCK_ICONS
            .iter()
            .map(PathBuf::from)
            .find(|path| path.is_file())
    }

    /// Maps the stored collector to the collector of the JVM builder.
    fn collector(&self) -> bolt_jdk::Gc {
        match self.garbage_collector {
            GcChoice::Default => bolt_jdk::Gc::Default,
            GcChoice::Z => bolt_jdk::Gc::Z,
            GcChoice::G1 => bolt_jdk::Gc::G1,
            GcChoice::Parallel => bolt_jdk::Gc::Parallel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn default_reproduces_the_source_launcher() {
        let config = TuningConfig::default();
        assert!(config.enabled);
        assert_eq!(config.heap_min.as_deref(), Some("2g"));
        assert_eq!(config.heap_max.as_deref(), Some("2g"));
        assert_eq!(config.stack_size.as_deref(), Some("2m"));
        assert_eq!(config.garbage_collector, GcChoice::Z);
        assert!(config.compact_object_headers);
        assert!(config.string_deduplication);
        assert!(config.native_access);
        assert!(config.aot_cache);
        assert!(!config.gc_log);
        assert_eq!(
            config.add_opens,
            vec![
                "java.base/java.net",
                "java.base/java.io",
                "java.base/java.lang",
                "java.base/java.lang.invoke",
                "java.desktop/com.apple.eawt",
                "java.desktop/sun.awt",
                "java.desktop/java.awt.event",
            ]
        );
        assert!(config.java2d_metal);
        assert_eq!(config.application_name.as_deref(), Some("RuneLite"));
        assert_eq!(config.dock_icon, None);
        assert!(config.launcher_nojvm);
        assert!(config.extra_jvm_args.is_empty());
        assert_eq!(
            config.extra_app_args,
            vec!["--hw-accel", "METAL", "--launch-mode", "REFLECT"]
        );
    }

    #[test]
    fn default_serializes_with_lowercase_collector() {
        let json = serde_json::to_string(&TuningConfig::default()).unwrap();
        assert!(json.contains("\"garbage_collector\":\"z\""));
        let back: TuningConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, TuningConfig::default());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn system_properties_hold_the_macos_identity() {
        let config = TuningConfig::default();
        assert_eq!(
            config.system_properties(),
            vec![
                ("sun.java2d.metal".to_string(), "true".to_string()),
                (
                    "apple.awt.application.appearance".to_string(),
                    "system".to_string()
                ),
                (
                    "apple.awt.application.name".to_string(),
                    "RuneLite".to_string()
                ),
                ("runelite.launcher.nojvm".to_string(), "true".to_string()),
            ]
        );
        // The icon of the installed client is not on every machine, so the test
        // names one. The fallback has its own test below.
        let mut with_icon = config.clone();
        with_icon.dock_icon = Some(PathBuf::from("/tmp/example.icns"));
        assert_eq!(
            with_icon.dock_args(),
            vec![
                "-Xdock:name=RuneLite".to_string(),
                "-Xdock:icon=/tmp/example.icns".to_string(),
            ]
        );
    }

    #[test]
    fn the_dock_icon_falls_back_to_the_installed_client() {
        let config = TuningConfig::default();
        assert_eq!(config.dock_icon, None);
        // Without a value the launcher looks for the icon of the client. A Java
        // process with no icon shows the Java icon instead of the game.
        match config.dock_icon() {
            Some(path) => assert!(path.is_file()),
            None => assert!(DEFAULT_DOCK_ICONS
                .iter()
                .all(|candidate| !Path::new(candidate).is_file())),
        }
    }

    #[test]
    fn aot_cache_records_an_empty_repository() {
        let temp = TempDir::new("tuning-empty-repository");
        let log_dir = temp.path().join("logs");
        std::fs::create_dir_all(&log_dir).unwrap();
        let repository = temp.path().join("repository2");
        std::fs::create_dir_all(&repository).unwrap();

        let tuning = TuningConfig::default().to_tuning(&log_dir, Some(&repository));
        let cache = tuning.aot_cache.expect("the default asks for an AOT cache");
        assert!(matches!(cache.mode, bolt_jdk::AotMode::Record));
        assert_eq!(cache.path, log_dir.join("client.aot"));
    }

    #[test]
    fn aot_cache_loads_an_existing_file() {
        let temp = TempDir::new("tuning-existing-aot");
        let log_dir = temp.path().join("logs");
        std::fs::create_dir_all(&log_dir).unwrap();
        let repository = temp.path().join("repository2");
        std::fs::create_dir_all(&repository).unwrap();
        std::fs::write(repository.join("client-1.10.0.jar"), b"jar").unwrap();
        std::fs::write(log_dir.join("client-1.10.0.aot"), b"cache").unwrap();
        std::fs::write(log_dir.join("client-1.9.0.aot"), b"stale").unwrap();

        let tuning = TuningConfig::default().to_tuning(&log_dir, Some(&repository));
        let cache = tuning.aot_cache.expect("the default asks for an AOT cache");
        assert!(matches!(cache.mode, bolt_jdk::AotMode::Load));
        assert_eq!(cache.path, log_dir.join("client-1.10.0.aot"));
        assert!(!log_dir.join("client-1.9.0.aot").exists());
    }

    #[test]
    fn to_tuning_copies_the_stored_values() {
        let temp = TempDir::new("tuning-values");
        let log_dir = temp.path().join("logs");
        let mut config = TuningConfig::default();
        config.garbage_collector = GcChoice::Parallel;
        config.heap_max = Some("4g".to_string());
        config.gc_log = true;
        config.extra_jvm_args = vec!["-XX:+AlwaysPreTouch".to_string()];

        let tuning = config.to_tuning(&log_dir, None);
        assert!(matches!(tuning.gc, bolt_jdk::Gc::Parallel));
        assert_eq!(tuning.heap_max.as_deref(), Some("4g"));
        assert_eq!(tuning.gc_log, Some(log_dir));
        assert_eq!(tuning.extra, vec!["-XX:+AlwaysPreTouch".to_string()]);
    }
}
