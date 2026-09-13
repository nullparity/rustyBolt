//! The platform directories of the launcher.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The directory name below the platform base directory.
const DIR_NAME: &str = "rustybolt";
/// The config file name.
const CONFIG_FILE: &str = "launcher.json";
/// The saved session file name.
const CREDENTIALS_FILE: &str = "creds.json";

/// The directories that the launcher uses.
///
/// Every path is absolute after [`Paths::resolve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// The directory of the launcher config and the saved sessions.
    pub config_dir: PathBuf,
    /// The directory of the game clients and the game data.
    pub data_dir: PathBuf,
    /// The directory of temporary downloads.
    pub cache_dir: PathBuf,
    /// The directory of the runtime files, for example the lock file.
    pub runtime_dir: PathBuf,
}

impl Paths {
    /// Builds the platform directories and creates every one of them.
    pub fn resolve() -> io::Result<Paths> {
        let dirs = dirs_for(Platform::current(), &Environment::from_process())?;
        create_dirs(&dirs)?;
        Ok(Paths {
            config_dir: dirs.config,
            data_dir: dirs.data,
            cache_dir: dirs.cache,
            runtime_dir: dirs.runtime,
        })
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join(CONFIG_FILE)
    }

    /// The session file of versions before the keychain. `SessionStore::load`
    /// moves it into the keychain and removes it.
    pub fn credentials_file(&self) -> PathBuf {
        self.config_dir.join(CREDENTIALS_FILE)
    }
}

/// The operating system rules that select the base directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Platform {
    Linux,
    MacOs,
    Windows,
}

impl Platform {
    pub(crate) fn current() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

/// The environment variables that select the base directories.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Environment {
    pub(crate) home: Option<PathBuf>,
    pub(crate) xdg_config_home: Option<PathBuf>,
    pub(crate) xdg_data_home: Option<PathBuf>,
    pub(crate) xdg_cache_home: Option<PathBuf>,
    pub(crate) xdg_runtime_dir: Option<PathBuf>,
    pub(crate) appdata: Option<PathBuf>,
    pub(crate) localappdata: Option<PathBuf>,
}

impl Environment {
    pub(crate) fn from_process() -> Environment {
        Environment {
            home: read_var("HOME"),
            xdg_config_home: read_var("XDG_CONFIG_HOME"),
            xdg_data_home: read_var("XDG_DATA_HOME"),
            xdg_cache_home: read_var("XDG_CACHE_HOME"),
            xdg_runtime_dir: read_var("XDG_RUNTIME_DIR"),
            appdata: read_var("APPDATA"),
            localappdata: read_var("LOCALAPPDATA"),
        }
    }
}

fn read_var(name: &str) -> Option<PathBuf> {
    match std::env::var_os(name) {
        Some(value) if !value.is_empty() => Some(PathBuf::from(value)),
        _ => None,
    }
}

/// The four base directories, before the program name is appended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dirs {
    pub(crate) config: PathBuf,
    pub(crate) data: PathBuf,
    pub(crate) cache: PathBuf,
    pub(crate) runtime: PathBuf,
}

/// Selects the base directories for one platform and one environment.
///
/// The function is pure. Tests call it for every platform on any host.
pub(crate) fn dirs_for(platform: Platform, env: &Environment) -> io::Result<Dirs> {
    match platform {
        Platform::Linux => linux_dirs(env),
        Platform::MacOs => macos_dirs(env),
        Platform::Windows => windows_dirs(env),
    }
}

fn linux_dirs(env: &Environment) -> io::Result<Dirs> {
    let home = home_dir(env)?;
    let config_fallback = home.join(".config");
    let data_fallback = home.join(".local").join("share");
    let cache_fallback = home.join(".cache");
    let runtime_fallback = PathBuf::from("/tmp");
    Ok(Dirs {
        config: select(env.xdg_config_home.as_deref(), &config_fallback).join(DIR_NAME),
        data: select(env.xdg_data_home.as_deref(), &data_fallback).join(DIR_NAME),
        cache: select(env.xdg_cache_home.as_deref(), &cache_fallback).join(DIR_NAME),
        runtime: select(env.xdg_runtime_dir.as_deref(), &runtime_fallback).join(DIR_NAME),
    })
}

fn macos_dirs(env: &Environment) -> io::Result<Dirs> {
    let home = home_dir(env)?;
    let support = home
        .join("Library")
        .join("Application Support")
        .join(DIR_NAME);
    let caches = home.join("Library").join("Caches").join(DIR_NAME);
    Ok(Dirs {
        config: support.clone(),
        data: support,
        cache: caches.clone(),
        runtime: caches,
    })
}

fn windows_dirs(env: &Environment) -> io::Result<Dirs> {
    let roaming = env
        .appdata
        .as_deref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA is not set"))?;
    let local = env
        .localappdata
        .as_deref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set"))?;
    Ok(Dirs {
        config: roaming.join(DIR_NAME),
        data: roaming.join(DIR_NAME),
        cache: local.join(DIR_NAME),
        runtime: local.join(DIR_NAME),
    })
}

fn home_dir(env: &Environment) -> io::Result<&Path> {
    env.home
        .as_deref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))
}

/// Creates the four directories, with the parents.
fn create_dirs(dirs: &Dirs) -> io::Result<()> {
    for dir in [&dirs.config, &dirs.data, &dirs.cache, &dirs.runtime] {
        fs::create_dir_all(dir)?;
    }
    Ok(())
}

fn select<'a>(value: Option<&'a Path>, fallback: &'a Path) -> &'a Path {
    value.unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_with_home(home: &str) -> Environment {
        Environment {
            home: Some(PathBuf::from(home)),
            ..Environment::default()
        }
    }

    #[test]
    fn linux_uses_the_xdg_variables() {
        let env = Environment {
            home: Some(PathBuf::from("/home/ada")),
            xdg_config_home: Some(PathBuf::from("/xdg/config")),
            xdg_data_home: Some(PathBuf::from("/xdg/data")),
            xdg_cache_home: Some(PathBuf::from("/xdg/cache")),
            xdg_runtime_dir: Some(PathBuf::from("/xdg/run")),
            ..Environment::default()
        };
        let dirs = dirs_for(Platform::Linux, &env).unwrap();
        assert_eq!(dirs.config, PathBuf::from("/xdg/config/rustybolt"));
        assert_eq!(dirs.data, PathBuf::from("/xdg/data/rustybolt"));
        assert_eq!(dirs.cache, PathBuf::from("/xdg/cache/rustybolt"));
        assert_eq!(dirs.runtime, PathBuf::from("/xdg/run/rustybolt"));
    }

    #[test]
    fn linux_falls_back_to_the_home_directory() {
        let dirs = dirs_for(Platform::Linux, &env_with_home("/home/ada")).unwrap();
        assert_eq!(dirs.config, PathBuf::from("/home/ada/.config/rustybolt"));
        assert_eq!(dirs.data, PathBuf::from("/home/ada/.local/share/rustybolt"));
        assert_eq!(dirs.cache, PathBuf::from("/home/ada/.cache/rustybolt"));
        assert_eq!(dirs.runtime, PathBuf::from("/tmp/rustybolt"));
    }

    #[test]
    fn linux_needs_home() {
        let error = dirs_for(Platform::Linux, &Environment::default()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn macos_puts_config_and_data_together() {
        let dirs = dirs_for(Platform::MacOs, &env_with_home("/Users/ada")).unwrap();
        let support = PathBuf::from("/Users/ada/Library/Application Support/rustybolt");
        let caches = PathBuf::from("/Users/ada/Library/Caches/rustybolt");
        assert_eq!(dirs.config, support);
        assert_eq!(dirs.data, support);
        assert_eq!(dirs.cache, caches);
        assert_eq!(dirs.runtime, caches);
    }

    #[test]
    fn macos_needs_home() {
        assert!(dirs_for(Platform::MacOs, &Environment::default()).is_err());
    }

    #[test]
    fn windows_splits_roaming_and_local() {
        let env = Environment {
            appdata: Some(PathBuf::from("C:\\Users\\ada\\AppData\\Roaming")),
            localappdata: Some(PathBuf::from("C:\\Users\\ada\\AppData\\Local")),
            ..Environment::default()
        };
        let dirs = dirs_for(Platform::Windows, &env).unwrap();
        assert_eq!(dirs.config, env.appdata.clone().unwrap().join("rustybolt"));
        assert_eq!(dirs.data, dirs.config);
        assert_eq!(
            dirs.cache,
            env.localappdata.clone().unwrap().join("rustybolt")
        );
        assert_eq!(dirs.runtime, dirs.cache);
    }

    #[test]
    fn windows_needs_the_appdata_variables() {
        assert!(dirs_for(Platform::Windows, &Environment::default()).is_err());
    }

    #[test]
    fn create_dirs_makes_all_four_directories() {
        let temp = crate::test_support::TempDir::new("paths");
        let root = temp.path().join("nested");
        let dirs = Dirs {
            config: root.join("config"),
            data: root.join("data"),
            cache: root.join("cache"),
            runtime: root.join("run"),
        };
        create_dirs(&dirs).unwrap();
        for dir in [&dirs.config, &dirs.data, &dirs.cache, &dirs.runtime] {
            assert!(dir.is_dir(), "{} is not a directory", dir.display());
        }
    }

    #[test]
    fn file_accessors_extend_the_directories() {
        let paths = crate::test_support::paths(Path::new("/base"));
        assert_eq!(
            paths.config_file(),
            PathBuf::from("/base/config/launcher.json")
        );
        assert_eq!(
            paths.credentials_file(),
            PathBuf::from("/base/config/creds.json")
        );
    }
}
