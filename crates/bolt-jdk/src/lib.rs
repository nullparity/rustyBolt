//! Java runtime discovery and Java process argument building.
//!
//! Bolt probes `JAVA_HOME` and then `PATH`. This crate does the same, and it also
//! reads the standard install locations of each system. The crate touches the file
//! system only. It does not start a game and it does not know the user interface.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

pub mod tuning;
pub use tuning::*;

/// Largest number of arguments of a user template. Bolt uses the same limit.
const MAX_ARG_COUNT: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaVersion {
    pub feature: u32,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Explicit,
    JavaHome,
    Path,
    SystemLocation,
}

#[derive(Debug, Clone)]
pub struct JavaRuntime {
    pub path: PathBuf,
    pub home: Option<PathBuf>,
    pub version: Option<JavaVersion>,
    pub source: Source,
}

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("empty template")]
    Empty,
    #[error("unbalanced quote in template")]
    UnbalancedQuote,
    #[error("too many arguments (max {})", max)]
    TooManyArguments { max: usize },
}

/// Reads the version and the real home of one Java binary.
///
/// The call asks for the settings as well as the version. The settings give
/// `java.home`, which names the runtime that a shim binary such as
/// `/usr/bin/java` points at. The discovery uses that value to drop a repeat.
pub fn probe(path: &Path) -> Option<JavaRuntime> {
    let output = Command::new(path)
        .arg("-XshowSettings:properties")
        .arg("-version")
        .output()
        .ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let version = parse_version(&text)?;
    let home = property(&text, "java.home")
        .map(PathBuf::from)
        .or_else(|| determine_home(path));
    Some(JavaRuntime {
        path: path.to_path_buf(),
        home,
        version: Some(version),
        source: Source::Explicit,
    })
}

/// Reads one `name = value` line of the settings output.
fn property(text: &str, name: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(name) else {
            continue;
        };
        let rest = rest.trim_start();
        if let Some(value) = rest.strip_prefix('=') {
            return Some(value.trim().to_string());
        }
    }
    None
}

/// The function accepts `openjdk version "21.0.10"` and `openjdk 17.0.9 2023-10-17`.
fn parse_version(full: &str) -> Option<JavaVersion> {
    for line in full.lines() {
        if let Some(start) = line.find("version \"") {
            let rest = &line[start + 9..];
            if let Some(end) = rest.find('"') {
                if let Some(version) = parse_version_string(&rest[..end]) {
                    return Some(version);
                }
            }
        }
    }
    // Some builds print the version without quotation marks.
    for token in full.split_whitespace() {
        if token.starts_with(|c: char| c.is_ascii_digit()) {
            if let Some(version) = parse_version_string(token) {
                return Some(version);
            }
        }
    }
    None
}

/// Reads a version string and finds its feature number.
///
/// `1.8.0_402` gives 8. `21.0.10` gives 21. `17` gives 17.
fn parse_version_string(version_str: &str) -> Option<JavaVersion> {
    let mut parts = version_str.split('.');
    let first = parts.next()?;
    let major: u32 = first
        .trim_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .ok()?;

    let feature = if major == 1 {
        // The old scheme puts the feature number in the second position.
        let second = parts.next()?;
        second
            .trim_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()?
    } else {
        major
    };

    Some(JavaVersion {
        feature,
        raw: version_str.to_string(),
    })
}

fn determine_home(executable_path: &Path) -> Option<PathBuf> {
    let bin_path = executable_path
        .parent()?
        .canonicalize()
        .ok()?;
    let home = bin_path.parent()?;
    if home.join("lib").exists() || home.join("conf").exists() || home.join("jre").exists() {
        Some(home.to_path_buf())
    } else {
        None
    }
}

#[cfg(unix)]
fn read_path_env() -> Vec<String> {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[cfg(windows)]
fn read_path_env() -> Vec<String> {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn canonicalize_path(p: &Path) -> Option<PathBuf> {
    p.canonicalize().ok()
}

#[cfg(target_os = "macos")]
fn system_location_candidates() -> Vec<PathBuf> {
    let mut candidates = java_home_tool_locations();

    // The system tool reports a runtime that a package installed. It misses a
    // runtime that the user unpacked, so these directories are read as well.
    let mut roots = vec![
        PathBuf::from("/Library/Java/JavaVirtualMachines"),
        PathBuf::from("/opt"),
        PathBuf::from("/opt/homebrew/opt"),
        PathBuf::from("/usr/local/opt"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join(".jdks"));
        roots.push(home.join("Library/Java/JavaVirtualMachines"));
    }

    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // A runtime sits at the directory itself, below `Contents/Home`, or
            // below a `jdk` link such as the one of a local install.
            candidates.push(path.clone());
            candidates.push(path.join("Contents/Home"));
            candidates.push(path.join("jdk"));
            candidates.push(path.join("libexec/openjdk.jdk/Contents/Home"));
        }
    }
    candidates
}

/// Asks the system tool for every installed runtime.
///
/// The tool prints a property list. A small scanner reads the home paths from it.
#[cfg(target_os = "macos")]
fn java_home_tool_locations() -> Vec<PathBuf> {
    let output = match Command::new("/usr/libexec/java_home").arg("-X").output() {
        Ok(output) if output.status.success() => output,
        _ => return vec![],
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut homes = vec![];
    let mut rest = text.as_ref();
    while let Some(pos) = rest.find("<key>JVMHomePath</key>") {
        rest = &rest[pos + "<key>JVMHomePath</key>".len()..];
        let start = match rest.find("<string>") {
            Some(start) => start + "<string>".len(),
            None => break,
        };
        let end = match rest[start..].find("</string>") {
            Some(end) => start + end,
            None => break,
        };
        homes.push(PathBuf::from(rest[start..end].trim()));
        rest = &rest[end..];
    }
    homes
}

#[cfg(all(unix, not(target_os = "macos")))]
fn system_location_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![];
    for base in ["/usr/lib/jvm", "/usr/java", "/opt/java"] {
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                candidates.push(entry.path());
            }
        }
    }
    candidates
}

#[cfg(windows)]
fn system_location_candidates() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Ok(value) = std::env::var(variable) {
            roots.push(PathBuf::from(value));
        }
    }
    if let Ok(value) = std::env::var("LOCALAPPDATA") {
        roots.push(PathBuf::from(value).join("Programs"));
    }
    if roots.is_empty() {
        roots.push(PathBuf::from(r"C:\Program Files"));
    }

    const VENDORS: [&str; 7] = [
        "Java",
        "Eclipse Adoptium",
        "Eclipse Foundation",
        "Amazon Corretto",
        "Microsoft",
        "Zulu",
        "BellSoft",
    ];

    // A vendor directory holds one directory for each installed runtime, so the
    // search reads the children as well as the vendor directory itself.
    let mut candidates = Vec::new();
    for root in &roots {
        for vendor in VENDORS {
            let vendor_dir = root.join(vendor);
            let Ok(entries) = std::fs::read_dir(&vendor_dir) else {
                continue;
            };
            candidates.push(vendor_dir);
            for entry in entries.flatten() {
                candidates.push(entry.path());
            }
        }
    }
    candidates
}

#[cfg(unix)]
fn system_location_executable_name() -> &'static str {
    "java"
}

#[cfg(windows)]
fn system_location_executable_name() -> &'static str {
    "javaw.exe"
}

/// Turns a list of runtime home directories into executable paths.
///
/// The function keeps a path only when the file is present.
fn find_java_in_candidates(candidates: &[PathBuf], exec_name: &str) -> Vec<PathBuf> {
    let mut found = vec![];
    for home in candidates {
        for relative in [PathBuf::from("bin").join(exec_name), PathBuf::from(exec_name)] {
            let path = home.join(relative);
            if path.is_file() {
                found.push(path);
                break;
            }
        }
    }
    found
}

fn discover_from_java_home() -> Vec<JavaRuntime> {
    let java_home = match std::env::var("JAVA_HOME") {
        Ok(value) if !value.is_empty() => value,
        _ => return vec![],
    };
    let home = PathBuf::from(java_home);
    let exec_name = system_location_executable_name();
    find_java_in_candidates(&[home], exec_name)
        .into_iter()
        .filter_map(|path| probe_with_source(&path, Source::JavaHome))
        .collect()
}

/// Finds every runtime that the `PATH` variable makes reachable.
fn discover_from_path() -> Vec<JavaRuntime> {
    let exec_name = system_location_executable_name();
    let mut found = vec![];
    for entry in read_path_env() {
        let path = PathBuf::from(entry).join(exec_name);
        if path.is_file() {
            if let Some(runtime) = probe_with_source(&path, Source::Path) {
                found.push(runtime);
            }
        }
    }
    found
}

/// Finds every runtime in the standard locations of this system.
fn discover_from_system_location() -> Vec<JavaRuntime> {
    let exec_name = system_location_executable_name();
    let candidates = system_location_candidates();
    find_java_in_candidates(&candidates, exec_name)
        .into_iter()
        .filter_map(|path| probe_with_source(&path, Source::SystemLocation))
        .collect()
}

fn probe_with_source(path: &Path, source: Source) -> Option<JavaRuntime> {
    let mut runtime = probe(path)?;
    runtime.source = source;
    Some(runtime)
}

/// Returns every runtime of this system.
///
/// The order follows the source: `JAVA_HOME` first, then `PATH`, then the
/// standard locations. Two entries that lead to the same runtime become one.
/// A shim such as `/usr/bin/java` therefore does not repeat its target.
pub fn discover() -> Vec<JavaRuntime> {
    let mut runtimes = vec![];
    runtimes.extend(discover_from_java_home());
    runtimes.extend(discover_from_path());
    runtimes.extend(discover_from_system_location());

    let mut unique: Vec<JavaRuntime> = vec![];
    let mut seen = HashSet::new();
    for mut runtime in runtimes {
        let real = canonicalize_path(&runtime.path).unwrap_or_else(|| runtime.path.clone());
        // `java.home` names the runtime itself. Two paths that report the same
        // home are the same runtime, so only the first one stays.
        let key = runtime
            .home
            .as_ref()
            .and_then(|home| canonicalize_path(home))
            .unwrap_or_else(|| real.clone());
        if seen.insert(key) {
            runtime.path = real;
            unique.push(runtime);
        }
    }
    unique
}

/// Returns the first runtime that meets the minimum feature number.
///
/// A runtime with a known version comes first. A runtime with no known version is
/// the last resort.
pub fn select(min_feature: u32) -> Option<JavaRuntime> {
    let runtimes = discover();
    runtimes
        .iter()
        .find(|runtime| {
            runtime
                .version
                .as_ref()
                .is_some_and(|version| version.feature >= min_feature)
        })
        .or_else(|| runtimes.iter().find(|runtime| runtime.version.is_none()))
        .cloned()
}

#[derive(Debug, Clone)]
pub struct JvmOptions {
    pub system_properties: Vec<(String, String)>,
    pub jvm_args: Vec<String>,
    pub jar: PathBuf,
    pub app_args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Invocation {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl JvmOptions {
    /// Builds the program and the arguments of a Java process.
    ///
    /// The order is: every system property, then the JVM arguments, then `-jar`,
    /// then the jar path, then the application arguments. The program does not
    /// appear in `args`.
    pub fn invocation(&self, java: &Path) -> Invocation {
        let mut args = Vec::new();
        for (key, value) in &self.system_properties {
            args.push(format!("-D{key}={value}"));
        }
        args.extend(self.jvm_args.iter().cloned());
        args.push("-jar".to_string());
        args.push(self.jar.to_string_lossy().into_owned());
        args.extend(self.app_args.iter().cloned());
        Invocation {
            program: java.to_path_buf(),
            args,
        }
    }
}

/// The token `%command%` becomes the program and every argument of the default
/// invocation, at the position of the token. A template without that token runs
/// alone, and the default invocation is not used.
pub fn apply_template(template: &str, default: &Invocation) -> Result<Invocation, TemplateError> {
    if template.trim().is_empty() {
        return Err(TemplateError::Empty);
    }

    let mut parts: Vec<String> = vec![];
    for token in tokenize(template)? {
        if token == "%command%" {
            parts.push(default.program.to_string_lossy().into_owned());
            parts.extend(default.args.iter().cloned());
        } else {
            parts.push(token);
        }
    }
    if parts.len() > MAX_ARG_COUNT {
        return Err(TemplateError::TooManyArguments { max: MAX_ARG_COUNT });
    }
    if parts.is_empty() {
        return Err(TemplateError::Empty);
    }

    let program = PathBuf::from(parts.remove(0));
    Ok(Invocation {
        program,
        args: parts,
    })
}

fn tokenize(template: &str) -> Result<Vec<String>, TemplateError> {
    let mut tokens = vec![];
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;

    for c in template.chars() {
        match c {
        '\'' => {
            if in_double {
                current.push(c);
            } else {
                if in_single {
                    in_single = false;
                } else {
                    in_single = true;
                }
            }
        }
        '"' => {
            if in_single {
                current.push(c);
            } else {
                if in_double {
                    in_double = false;
                } else {
                    in_double = true;
                }
            }
        }
        ' ' => {
            if in_single || in_double {
                current.push(c);
            } else if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        }
        _ => {
            current.push(c);
        }
        }
    }

    if in_single || in_double {
        return Err(TemplateError::UnbalancedQuote);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_runtime(path: &str, version: &str, _feature: u32) -> JavaRuntime {
        let version = parse_version_string(version).unwrap();
        let path = PathBuf::from(path);
        let home = determine_home(&path);
        JavaRuntime {
            path,
            home,
            version: Some(version),
            source: Source::Explicit,
        }
    }

    #[test]
    fn test_parse_version_openjdk() {
        let version = parse_version_string("21.0.10").unwrap();
        assert_eq!(version.feature, 21);
        assert_eq!(version.raw, "21.0.10");
    }

    #[test]
    fn test_parse_version_jdk8() {
        let version = parse_version_string("1.8.0_402").unwrap();
        assert_eq!(version.feature, 8);
        assert_eq!(version.raw, "1.8.0_402");
    }

    #[test]
    fn test_parse_version_jdk17() {
        let version = parse_version_string("17.0.9").unwrap();
        assert_eq!(version.feature, 17);
        assert_eq!(version.raw, "17.0.9");
    }

    #[test]
    fn test_parse_version_jdk19() {
        let version = parse_version_string("19").unwrap();
        assert_eq!(version.feature, 19);
        assert_eq!(version.raw, "19");
    }

    #[test]
    fn test_jvm_options_invocation_order() {
        let jvm_opts = JvmOptions {
            system_properties: vec![
                ("key1".to_string(), "value1".to_string()),
                ("key2".to_string(), "value2".to_string()),
            ],
            jvm_args: vec!["-Xmx2g".to_string()],
            jar: PathBuf::from("/path/to/jar.jar"),
            app_args: vec!["--config".to_string(), "my.cfg".to_string()],
        };

        let java = PathBuf::from("/usr/bin/java");
        let invocation = jvm_opts.invocation(&java);

        // The program does not repeat in the argument list.
        let expected = vec![
            "-Dkey1=value1",
            "-Dkey2=value2",
            "-Xmx2g",
            "-jar",
            "/path/to/jar.jar",
            "--config",
            "my.cfg",
        ];

        assert_eq!(invocation.program, java);
        assert_eq!(invocation.args, expected);
    }

    #[test]
    fn test_apply_template_with_command_start() {
        let default = Invocation {
            program: PathBuf::from("/usr/bin/java"),
            args: vec![
                "-Dkey=value".to_string(),
                "-jar".to_string(),
                "app.jar".to_string(),
            ],
        };

        let template = "%command% -jar my.jar";
        let result = apply_template(template, &default).unwrap();

        assert_eq!(result.program, PathBuf::from("/usr/bin/java"));
        assert_eq!(result.args, vec!["-Dkey=value", "-jar", "app.jar", "-jar", "my.jar"]);
    }

    #[test]
    fn test_apply_template_with_command_middle() {
        let default = Invocation {
            program: PathBuf::from("/usr/bin/java"),
            args: vec!["--config".to_string(), "my.cfg".to_string()],
        };

        // The template program runs the default invocation as its arguments.
        let template = "echo %command%";
        let result = apply_template(template, &default).unwrap();

        assert_eq!(result.program, PathBuf::from("echo"));
        assert_eq!(result.args, vec!["/usr/bin/java", "--config", "my.cfg"]);
    }

    #[test]
    fn test_apply_template_with_quotes() {
        let default = Invocation {
            program: PathBuf::from("/path with space/java"),
            args: vec!["-jar".to_string(), "my.jar".to_string()],
        };

        let template = "\"%command%\" -jar app.jar";
        let result = apply_template(template, &default).unwrap();

        assert_eq!(result.program, PathBuf::from("/path with space/java"));
        assert_eq!(
            result.args,
            vec!["-jar", "my.jar", "-jar", "app.jar"]
        );
    }

    #[test]
    fn test_apply_template_unbalanced_quote() {
        let default = Invocation {
            program: PathBuf::from("/usr/bin/java"),
            args: vec![],
        };

        let template = "echo \"unbalanced";
        let result = apply_template(template, &default);

        assert!(matches!(result, Err(TemplateError::UnbalancedQuote)));
    }

    #[test]
    fn test_apply_template_empty() {
        let default = Invocation {
            program: PathBuf::from("/usr/bin/java"),
            args: vec![],
        };

        let result = apply_template("", &default);

        assert!(matches!(result, Err(TemplateError::Empty)));
    }

    #[test]
    fn test_apply_template_no_command() {
        let default = Invocation {
            program: PathBuf::from("/usr/bin/java"),
            args: vec!["arg1".to_string(), "arg2".to_string()],
        };

        let template = "custom command arg1 arg2";
        let result = apply_template(template, &default).unwrap();

        assert_eq!(result.program, PathBuf::from("custom"));
        assert_eq!(result.args, vec!["command", "arg1", "arg2"]);
    }

    #[test]
    fn test_apply_template_too_many_arguments() {
        let default = Invocation {
            program: PathBuf::from("/usr/bin/java"),
            args: vec![],
        };

        let mut template = String::from("%command% ");
        for _ in 0..257 {
            template.push_str("arg ");
        }

        let result = apply_template(&template, &default);

        assert!(matches!(result, Err(TemplateError::TooManyArguments { .. })));
    }

    #[test]
    fn test_discover_returns_only_existing_paths() {
        let runtimes = discover();
        for runtime in &runtimes {
            assert!(runtime.path.exists(), "Path {} does not exist", runtime.path.display());
        }
    }

    #[test]
    fn test_select_prefers_known_version() {
        let mut runtimes = discover();
        runtimes.sort_by_key(|r| r.version.as_ref().map(|v| v.feature).unwrap_or(0));

        if !runtimes.is_empty() {
            let first = runtimes[0].clone();
            let selected = select(first.version.as_ref().map(|v| v.feature).unwrap_or(0));

            if let Some(selected) = selected {
                assert_eq!(selected.path, first.path);
            }
        }
    }
}
