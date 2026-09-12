//! The `java` command group.
//!
//! The commands show the runtimes that `bolt-jdk` finds, and they pin the
//! runtime of the config.

use std::path::{Path, PathBuf};

use bolt_core::{Config, Paths};
use bolt_jdk::{JavaRuntime, Source};

use crate::{no_arguments, CliError};

/// Feature number of the first Java release with the tuned JVM flags.
const TUNED_FEATURE: u32 = 24;

/// The options that need feature 24 or newer.
const TUNED_OPTIONS: &str = "compact headers, string dedup, native access, AOT cache";

/// Runs `rustybolt java <subcommand>`.
pub(crate) fn run(args: &[String]) -> Result<(), CliError> {
    match args.first().map(String::as_str) {
        Some("list") => {
            no_arguments(&args[1..])?;
            list()
        }
        Some("select") => {
            let min = minimum(&args[1..])?;
            select(min)
        }
        Some("use") => use_runtime(&args[1..]),
        Some(other) => Err(CliError::Message(format!(
            "`java {other}` is not a subcommand. Use `java list`, `java select` or `java use`."
        ))),
        None => Err(CliError::Message(
            "`java` needs a subcommand. Use `java list`, `java select` or `java use`.".to_string(),
        )),
    }
}

/// Reads the `--min` value. The default is feature 11.
fn minimum(args: &[String]) -> Result<u32, CliError> {
    let mut min = 11;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--min" => {
                let value = crate::flag_value(args, index)?;
                min = value
                    .parse::<u32>()
                    .map_err(|_| CliError::Message(format!("`{value}` is not a whole number")))?;
                index += 2;
            }
            other => {
                return Err(CliError::Message(format!(
                    "`{other}` is not an argument of `java select`"
                )))
            }
        }
    }
    Ok(min)
}

/// Prints every runtime of this system.
///
/// A `*` marks the runtime that the launcher chooses now. The last column names
/// the options that the runtime supports.
fn list() -> Result<(), CliError> {
    let paths = Paths::resolve()?;
    let config = Config::load(&paths);
    let chosen = current_java(&config);

    let runtimes = bolt_jdk::discover();
    if runtimes.is_empty() {
        println!("Found no Java runtime.");
        return Ok(());
    }
    for runtime in &runtimes {
        let marker = match &chosen {
            Some(path) if same_path(path, &runtime.path) => "*",
            _ => " ",
        };
        println!(
            "{marker} {}  {}  {}  {}",
            source_name(&runtime.source),
            version_name(runtime),
            runtime.path.display(),
            option_support(runtime)
        );
    }

    if let Some(path) = &chosen {
        if !runtimes
            .iter()
            .any(|runtime| same_path(path, &runtime.path))
        {
            println!(
                "The launcher uses {}, and this list does not hold that file.",
                path.display()
            );
        }
    }
    Ok(())
}

/// Prints the first runtime that meets the minimum feature number.
fn select(min: u32) -> Result<(), CliError> {
    match bolt_jdk::select(min) {
        Some(runtime) => {
            println!(
                "{}  {}  {}",
                source_name(&runtime.source),
                version_name(&runtime),
                runtime.path.display()
            );
            Ok(())
        }
        None => Err(CliError::Message(format!(
            "no Java runtime of feature {min} or newer exists"
        ))),
    }
}

/// Pins the Java binary of the config, or clears the pin.
///
/// The function writes the config only after a good probe, so a bad path
/// leaves the config as it was.
fn use_runtime(args: &[String]) -> Result<(), CliError> {
    let value = args
        .first()
        .ok_or_else(|| CliError::Message("`java use` needs a path or `auto`".to_string()))?;
    no_arguments(&args[1..])?;

    let paths = Paths::resolve()?;
    let mut config = Config::load(&paths);

    if value == "auto" {
        config.java_path = None;
        config.save(&paths)?;
        println!("The launcher now chooses the Java runtime itself.");
        return Ok(());
    }

    let path = PathBuf::from(value);
    let runtime = bolt_jdk::probe(&path).ok_or_else(|| {
        CliError::Message(format!(
            "`{value}` is not a Java binary that reports a version"
        ))
    })?;
    let feature = runtime
        .version
        .as_ref()
        .map(|version| version.feature)
        .unwrap_or(0);
    config.java_path = Some(runtime.path.clone());
    config.save(&paths)?;
    println!(
        "The launcher now uses feature {feature} at {}.",
        runtime.path.display()
    );
    Ok(())
}

/// The Java binary that the launcher chooses now.
///
/// The order is the path of the config, then the first executable candidate,
/// then the automatic search. The rule matches the launcher core.
pub(crate) fn current_java(config: &Config) -> Option<PathBuf> {
    if let Some(path) = &config.java_path {
        return Some(path.clone());
    }
    for candidate in &config.java_candidates {
        if is_executable(candidate) {
            return Some(candidate.clone());
        }
    }
    bolt_jdk::select(11).map(|runtime| runtime.path)
}

/// The feature number of the Java binary that the launcher chooses now.
///
/// A failed probe gives feature 11, which matches the launcher core.
pub(crate) fn current_feature(config: &Config) -> u32 {
    current_java(config)
        .and_then(|path| bolt_jdk::probe(&path))
        .and_then(|runtime| runtime.version)
        .map(|version| version.feature)
        .unwrap_or(11)
}

/// Reports if two paths name the same file.
fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Reports if the path is an executable file.
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// Reports if the path is an executable file.
#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn source_name(source: &Source) -> &'static str {
    match source {
        Source::Explicit => "explicit",
        Source::JavaHome => "java_home",
        Source::Path => "path",
        Source::SystemLocation => "system",
    }
}

fn version_name(runtime: &JavaRuntime) -> String {
    match &runtime.version {
        Some(version) => version.raw.clone(),
        None => "unknown".to_string(),
    }
}

/// The options that a runtime supports.
///
/// The four tuned options need feature 24 or newer. An older runtime gets a
/// dash, so the user sees that the options do nothing there.
fn option_support(runtime: &JavaRuntime) -> &'static str {
    match &runtime.version {
        Some(version) if version.feature >= TUNED_FEATURE => TUNED_OPTIONS,
        _ => "—",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bolt_jdk::JavaVersion;

    /// Builds a runtime with one feature number.
    fn runtime(feature: Option<u32>) -> JavaRuntime {
        JavaRuntime {
            path: PathBuf::from("/usr/bin/java"),
            home: None,
            version: feature.map(|feature| JavaVersion {
                feature,
                raw: feature.to_string(),
            }),
            source: Source::Path,
        }
    }

    #[test]
    fn the_option_note_follows_the_feature_gate() {
        assert_eq!(option_support(&runtime(Some(23))), "—");
        assert_eq!(option_support(&runtime(Some(24))), TUNED_OPTIONS);
        assert_eq!(option_support(&runtime(Some(25))), TUNED_OPTIONS);
        assert_eq!(option_support(&runtime(None)), "—");
    }
}
