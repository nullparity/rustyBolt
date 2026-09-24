//! The launch of a game client.

#[cfg(unix)]
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use rustybolt_jdk::{Invocation, JvmOptions};

use crate::{ClientKind, Config, CoreError, Paths};

/// Serializes the read-check-write of the pid file across threads.
///
/// The HTTP server runs each request on its own thread. Two closely timed
/// launch requests can otherwise both pass the liveness check before either
/// writes its pid.
static LAUNCH_GUARD: Mutex<()> = Mutex::new(());

#[cfg(unix)]
extern "C" {
    /// Creates a new session for the child process.
    fn setsid() -> i32;
    /// Sends a signal to a process. Signal 0 tests the process only.
    fn kill(pid: i32, sig: i32) -> i32;
}

/// The error number that `kill` gives when the process exists but the launcher
/// is not the owner. The process is alive.
#[cfg(unix)]
const EPERM: i32 = 1;

/// The login values that the game client reads from the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameCredentials {
    /// The value of `JX_SESSION_ID`.
    pub session_id: String,
    /// The value of `JX_CHARACTER_ID`.
    pub character_id: String,
    /// The launcher exports this value as `JX_DISPLAY_NAME`.
    pub display_name: String,
}

/// The arguments of one launch.
pub struct LaunchRequest<'a> {
    /// The jar file of the client.
    pub jar: &'a Path,
    /// The client.
    pub kind: ClientKind,
    /// The login values, when the user selected a character.
    pub credentials: Option<&'a GameCredentials>,
    /// The Java binary. A value here wins over the config file.
    pub java: Option<&'a Path>,
    /// A user command template. The token `%command%` is the default command.
    pub template: Option<&'a str>,
    /// Add the RuneLite `--configure` argument.
    pub configure: bool,
}

/// Starts a game client and returns the process id of the child.
///
/// The child gets a new session on unix and a new process group on Windows.
/// Standard input, output, and error streams are redirected to null so the child
/// detaches completely from the parent. The function never waits for the child.
///
/// If the same character is still starting or already running this kind of
/// client, the function returns [`CoreError::AlreadyRunning`] instead of a
/// second process. A cold JVM start can take several seconds. A launcher
/// button with no visible feedback during that time invites a second click.
/// A different character can still start its own client at the same time.
pub fn launch(paths: &Paths, request: &LaunchRequest) -> Result<u32, CoreError> {
    let plan = plan(paths, request)?;
    let pid_file = client_pid_file(paths, request.kind, &session_key(request));

    let _guard = LAUNCH_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(pid) = read_pid(&pid_file) {
        if client_process_alive(pid) {
            return Err(CoreError::AlreadyRunning(request.kind.title().to_string()));
        }
    }

    // A missing entry only costs the window its name in the top bar.
    #[cfg(target_os = "linux")]
    if request.kind == ClientKind::RuneLite {
        let _ = crate::desktop::ensure_runelite_entry(request.jar);
    }

    // The global profile overrides go into every profile file before the
    // client reads them. A failure here is reported, not fatal.
    if request.kind == ClientKind::RuneLite {
        let config = Config::load(paths);
        if let Some(overrides) = &config.runelite_profile_overrides {
            if let Err(error) = crate::apply_to_profiles(&config.profile_dir(paths), overrides) {
                eprintln!("rustybolt: profile overrides not applied: {error}");
            }
        }
    }

    let mut command = Command::new(&plan.program);
    command.args(&plan.args);
    command.current_dir(&plan.working_dir);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    for (name, value) in &plan.env {
        command.env(name, value);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if setsid() == -1 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    }

    let child = command.spawn()?;
    let pid = child.id();
    let _ = std::fs::write(&pid_file, pid.to_string());
    Ok(pid)
}

/// Where [`launch`] records the pid of the last client of this kind and
/// session.
fn client_pid_file(paths: &Paths, kind: ClientKind, session: &str) -> PathBuf {
    paths
        .cache_dir
        .join(format!("{}-{session}.pid", kind.name()))
}

/// Names the session of a launch request, for [`client_pid_file`].
///
/// The launcher tracks one client per character, so a second character can
/// start its own client while the first one is still running. The character
/// id comes from the Jagex API, so the function keeps only characters that
/// are safe in a file name.
fn session_key(request: &LaunchRequest) -> String {
    let raw = request
        .credentials
        .map(|credentials| credentials.character_id.as_str())
        .unwrap_or("default");
    let safe: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if safe.is_empty() {
        "default".to_string()
    } else {
        safe
    }
}

/// Reads the pid that a previous call to [`launch`] recorded.
fn read_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Reports whether the process of `pid` still runs.
///
/// On Windows, the function always returns `false`. A stale pid file never
/// blocks a launch there. [`launch`] still records the pid on every
/// platform.
#[cfg(unix)]
fn client_process_alive(pid: u32) -> bool {
    pid_alive(pid as i32)
}

#[cfg(not(unix))]
fn client_process_alive(_pid: u32) -> bool {
    false
}

/// Everything that the launcher needs to start one client process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    /// The program of the child process.
    pub program: PathBuf,
    /// The arguments of the child process. The program is not in this list.
    pub args: Vec<String>,
    /// The working directory of the child process.
    pub working_dir: PathBuf,
    /// The environment values that the launcher adds.
    pub env: Vec<(String, String)>,
}

/// [`launch`] runs this plan. A caller that shows the command line to the user
/// must use this function, so the shown command and the started command agree.
pub fn plan(paths: &Paths, request: &LaunchRequest) -> Result<LaunchPlan, CoreError> {
    if !request.jar.exists() {
        return Err(CoreError::NotInstalled);
    }

    let config = Config::load(paths);
    let java = resolve_java(request.java, &config)?;
    // The client home follows the config, so a launch never writes to another
    // launcher home by accident.
    let home = config.runelite_home(paths);
    std::fs::create_dir_all(&home)?;
    let data_dir = home.as_path();
    let feature = java_feature(&java);

    let log_dir = paths.cache_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;
    prune_gc_logs(&log_dir);

    // The Dock and the process list show the file name of the executable. A hard
    // link with the client name gives the process its own identity. The link MUST
    // sit inside the `bin` directory of the runtime, because a Java binary finds
    // its own runtime through its path. A link somewhere else cannot start.
    let java = match (&config.runelite_process_name, request.kind) {
        (Some(name), ClientKind::RuneLite) => brand(&java, name),
        _ => java,
    };

    let options = tuned_client_options(
        request.kind,
        request.jar,
        data_dir,
        request.configure,
        &config,
        &Host {
            feature,
            log_dir: &log_dir,
            total_memory: crate::memory::total_bytes(),
        },
    );
    let default = options.invocation(&java);
    let invocation = match request.template {
        Some(template) => rustybolt_jdk::apply_template(template, &default)?,
        None => default,
    };

    let mut env = Vec::new();
    if let Some(credentials) = request.credentials {
        env.push(("JX_SESSION_ID".to_string(), credentials.session_id.clone()));
        env.push((
            "JX_CHARACTER_ID".to_string(),
            credentials.character_id.clone(),
        ));
        env.push((
            "JX_DISPLAY_NAME".to_string(),
            credentials.display_name.clone(),
        ));
    }
    #[cfg(unix)]
    env.push(("HOME".to_string(), data_dir.to_string_lossy().into_owned()));

    Ok(LaunchPlan {
        program: invocation.program,
        args: invocation.args,
        working_dir: data_dir.to_path_buf(),
        env,
    })
}

/// Selects the Java binary: the request, then the config file, then the pinned
/// candidates, then the system.
fn resolve_java(explicit: Option<&Path>, config: &Config) -> Result<PathBuf, CoreError> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    if let Some(path) = &config.java_path {
        return Ok(path.clone());
    }
    for candidate in &config.java_candidates {
        if is_executable(candidate) {
            return Ok(candidate.clone());
        }
    }
    rustybolt_jdk::select(11)
        .map(|runtime| runtime.path)
        .ok_or(CoreError::NoJava)
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

/// Finds the feature number of the Java binary. A failed probe gives 11.
fn java_feature(java: &Path) -> u32 {
    rustybolt_jdk::probe(java)
        .and_then(|runtime| runtime.version)
        .map(|version| version.feature)
        .unwrap_or(11)
}

/// Gives the Java binary the name of the client.
///
/// The link goes into the `bin` directory of the runtime itself. A Java binary
/// reads its own path to find its runtime, so a link in another directory cannot
/// start. The function returns the original path when the runtime directory
/// rejects the link, which happens when another user owns it.
fn brand(java: &Path, name: &str) -> PathBuf {
    let Some(home) = rustybolt_jdk::probe(java).and_then(|runtime| runtime.home) else {
        return java.to_path_buf();
    };
    let bin = home.join("bin");
    let real = bin.join(if cfg!(windows) { "javaw.exe" } else { "java" });
    if !real.is_file() {
        return java.to_path_buf();
    }
    rustybolt_jdk::branded_java(&real, &bin, name)
}

/// Removes every `gc-<pid>.log` of a dead client and returns the count.
///
/// The gc log of a dead client is stale. The pid in the file name names the
/// client that wrote it.
#[cfg(unix)]
fn prune_gc_logs(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = gc_log_pid(&name.to_string_lossy()) else {
            continue;
        };
        if pid_alive(pid) {
            continue;
        }
        if std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Removes every `gc-<pid>.log` of a dead client and returns the count.
#[cfg(windows)]
fn prune_gc_logs(_dir: &Path) -> usize {
    0
}

/// Reads the process id from a `gc-<pid>.log` file name.
#[cfg(unix)]
fn gc_log_pid(name: &str) -> Option<i32> {
    let stem = name.strip_prefix("gc-")?.strip_suffix(".log")?;
    stem.parse().ok()
}

/// Reports if the process still runs. Signal 0 does not send a signal.
#[cfg(unix)]
fn pid_alive(pid: i32) -> bool {
    if unsafe { kill(pid, 0) } == 0 {
        return true;
    }
    io::Error::last_os_error().raw_os_error() == Some(EPERM)
}

/// Builds the JVM options of one client.
pub fn client_options(
    kind: ClientKind,
    jar: &Path,
    data_dir: &Path,
    configure: bool,
) -> JvmOptions {
    let home = data_dir.to_string_lossy().into_owned();
    match kind {
        ClientKind::RuneLite => {
            let mut app_args = vec![format!("-J-Duser.home={home}")];
            if configure {
                app_args.push("--configure".to_string());
            }
            JvmOptions {
                system_properties: vec![("user.home".to_string(), home)],
                jvm_args: Vec::new(),
                jar: jar.to_path_buf(),
                app_args,
            }
        }
        ClientKind::Hdos => JvmOptions {
            system_properties: vec![
                ("user.home".to_string(), home.clone()),
                ("app.user.home".to_string(), home),
            ],
            jvm_args: Vec::new(),
            jar: jar.to_path_buf(),
            app_args: Vec::new(),
        },
    }
}

/// Builds the JVM options of one client with the stored tuning.
///
/// The tuning applies to the RuneLite client only, and only when the user turns
/// it on. Every other case gives the plain options. `log_dir` holds the gc log
/// and the AOT cache.
/// What the tuning needs to know about the machine and the runtime.
pub struct Host<'a> {
    /// The Java feature number the client runs on.
    pub feature: u32,
    /// Where the GC log and the AOT cache go.
    pub log_dir: &'a Path,
    /// The physical memory in bytes; `None` leaves the heap as stored.
    pub total_memory: Option<u64>,
}

pub fn tuned_client_options(
    kind: ClientKind,
    jar: &Path,
    data_dir: &Path,
    configure: bool,
    config: &Config,
    host: &Host<'_>,
) -> JvmOptions {
    let mut options = client_options(kind, jar, data_dir, configure);
    let Host {
        feature,
        log_dir,
        total_memory,
    } = *host;
    let tuning_config = &config.runelite_tuning;
    if kind != ClientKind::RuneLite || !tuning_config.enabled {
        return options;
    }

    let tuning = tuning_config.to_tuning(log_dir, client_repository().as_deref(), total_memory);
    options
        .system_properties
        .extend(tuning_config.system_properties());
    options.jvm_args.extend(tuning.flags(feature));
    options.jvm_args.extend(tuning_config.dock_args());
    options.app_args.extend(tuning_config.app_args());
    options
}

/// Finds the RuneLite jar cache of the user, normally `~/.runelite/repository2`.
fn client_repository() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".runelite").join("repository2"))
}

/// Builds the default command line of one client.
///
/// The `program` field is the Java binary. The `args` field holds the
/// arguments only, so it never repeats the binary.
pub fn client_invocation(
    kind: ClientKind,
    jar: &Path,
    data_dir: &Path,
    java: &Path,
    configure: bool,
) -> Invocation {
    client_options(kind, jar, data_dir, configure).invocation(java)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{paths, TempDir};

    fn arguments(kind: ClientKind, configure: bool) -> Vec<String> {
        let java = Path::new("/usr/bin/java");
        let invocation = client_invocation(
            kind,
            Path::new("/data/runelite.jar"),
            Path::new("/home/ada/.local/share/rustybolt"),
            java,
            configure,
        );
        assert_eq!(invocation.program, java);
        invocation.args
    }

    #[test]
    fn runelite_arguments_hold_the_home_property_and_the_jar() {
        assert_eq!(
            arguments(ClientKind::RuneLite, false),
            vec![
                "-Duser.home=/home/ada/.local/share/rustybolt",
                "-jar",
                "/data/runelite.jar",
                "-J-Duser.home=/home/ada/.local/share/rustybolt",
            ]
        );
    }

    #[test]
    fn runelite_configure_appends_the_flag() {
        assert_eq!(
            arguments(ClientKind::RuneLite, true),
            vec![
                "-Duser.home=/home/ada/.local/share/rustybolt",
                "-jar",
                "/data/runelite.jar",
                "-J-Duser.home=/home/ada/.local/share/rustybolt",
                "--configure",
            ]
        );
    }

    #[test]
    fn hdos_arguments_hold_both_home_properties() {
        assert_eq!(
            arguments(ClientKind::Hdos, false),
            vec![
                "-Duser.home=/home/ada/.local/share/rustybolt",
                "-Dapp.user.home=/home/ada/.local/share/rustybolt",
                "-jar",
                "/data/runelite.jar",
            ]
        );
    }

    #[test]
    fn hdos_ignores_the_configure_flag() {
        assert_eq!(
            arguments(ClientKind::Hdos, true),
            arguments(ClientKind::Hdos, false)
        );
    }

    #[test]
    fn launch_refuses_an_absent_jar() {
        let temp = TempDir::new("launch-missing");
        let paths = paths(temp.path());
        let request = LaunchRequest {
            jar: Path::new("/does/not/exist/runelite.jar"),
            kind: ClientKind::RuneLite,
            credentials: None,
            java: Some(Path::new("/usr/bin/java")),
            template: None,
            configure: false,
        };
        assert!(matches!(
            launch(&paths, &request),
            Err(CoreError::NotInstalled)
        ));
    }

    #[test]
    fn resolve_java_prefers_the_request_then_the_config() {
        let config = Config {
            java_path: Some(PathBuf::from("/config/java")),
            ..Default::default()
        };
        assert_eq!(
            resolve_java(Some(Path::new("/request/java")), &config).unwrap(),
            PathBuf::from("/request/java")
        );
        assert_eq!(
            resolve_java(None, &config).unwrap(),
            PathBuf::from("/config/java")
        );
    }

    #[test]
    fn resolve_java_takes_the_first_executable_candidate() {
        let temp = TempDir::new("resolve-java-candidates");
        let missing = temp.path().join("missing-java");
        let plain = temp.path().join("plain-java");
        std::fs::write(&plain, b"not executable").unwrap();
        let good = temp.path().join("good-java");
        std::fs::write(&good, b"executable").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&good, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        // Windows has no execute bit, so every file counts as executable.
        let expected = if cfg!(windows) {
            plain.clone()
        } else {
            good.clone()
        };
        let config = Config {
            java_candidates: vec![missing, plain, good],
            ..Default::default()
        };
        assert_eq!(resolve_java(None, &config).unwrap(), expected);
        assert_eq!(java_feature(&temp.path().join("absent")), 11);
    }

    fn tuned_arguments(
        kind: ClientKind,
        config: &Config,
        feature: u32,
        log_dir: &Path,
    ) -> Vec<String> {
        tuned_client_options(
            kind,
            Path::new("/data/runelite.jar"),
            Path::new("/home/ada/.local/share/rustybolt"),
            false,
            config,
            &Host {
                feature,
                log_dir,
                total_memory: None,
            },
        )
        .invocation(Path::new("/usr/bin/java"))
        .args
    }

    #[test]
    fn runelite_tuning_at_feature_25_holds_the_modern_flags() {
        let temp = TempDir::new("tuned-runelite-25");
        let log_dir = temp.path().join("logs");
        std::fs::create_dir_all(&log_dir).unwrap();

        let args = tuned_arguments(ClientKind::RuneLite, &Config::default(), 25, &log_dir);
        for flag in [
            "-Xms2g",
            "-Xmx2g",
            "-Xss2m",
            "-XX:MaxDirectMemorySize=512m",
            "-XX:MaxMetaspaceSize=1g",
            "-XX:ReservedCodeCacheSize=240m",
            "-XX:NativeMemoryTracking=summary",
            "-XX:+UseZGC",
            "-XX:+UseCompactObjectHeaders",
            "--enable-native-access=ALL-UNNAMED",
            "-XX:+UseStringDeduplication",
        ] {
            assert!(args.contains(&flag.to_string()), "missing {flag}");
        }
        assert!(!args.iter().any(|arg| arg.contains("ZGenerational")));
        for package in [
            "java.base/java.net",
            "java.base/java.io",
            "java.base/java.lang",
            "java.base/java.lang.invoke",
            "java.desktop/sun.awt",
            "java.desktop/java.awt.event",
        ] {
            assert!(args.contains(&format!("--add-opens={package}=ALL-UNNAMED")));
        }
        // The macOS-only pieces stay off other platforms.
        let mac = cfg!(target_os = "macos");
        assert_eq!(
            args.contains(&"--add-opens=java.desktop/com.apple.eawt=ALL-UNNAMED".to_string()),
            mac
        );
        assert!(args.contains(&"-Duser.home=/home/ada/.local/share/rustybolt".to_string()));
        assert_eq!(
            args.contains(&"-Dsun.java2d.metal=true".to_string()),
            cfg!(target_os = "macos")
        );
        assert_eq!(
            args.contains(&"-Dapple.awt.application.name=RuneLite".to_string()),
            mac
        );
        assert!(args.contains(&"-Drunelite.launcher.nojvm=true".to_string()));
        assert!(args.contains(&"-jar".to_string()));
        assert!(args.contains(&"/data/runelite.jar".to_string()));
        #[cfg(target_os = "macos")]
        assert!(args.contains(&"-Xdock:name=RuneLite".to_string()));
        if mac {
            assert!(args
                .ends_with(&["--hw-accel", "METAL", "--launch-mode", "REFLECT"].map(String::from)));
        } else {
            assert!(args.ends_with(&["--launch-mode", "REFLECT"].map(String::from)));
            assert!(!args.contains(&"--hw-accel".to_string()));
        }
    }

    #[test]
    fn runelite_tuning_below_feature_24_holds_the_older_flags() {
        let temp = TempDir::new("tuned-runelite-21");
        let log_dir = temp.path().join("logs");
        std::fs::create_dir_all(&log_dir).unwrap();

        let args = tuned_arguments(ClientKind::RuneLite, &Config::default(), 21, &log_dir);
        assert!(args.contains(&"-XX:+ZGenerational".to_string()));
        assert!(args.contains(&"-XX:+UseZGC".to_string()));
        assert!(!args.contains(&"-XX:+UseCompactObjectHeaders".to_string()));
        assert!(!args.contains(&"-XX:+UseStringDeduplication".to_string()));
        assert!(!args.iter().any(|arg| arg.starts_with("-XX:AOTCache")));
    }

    #[test]
    fn hdos_ignores_the_tuning() {
        let temp = TempDir::new("tuned-hdos");
        let log_dir = temp.path().join("logs");
        std::fs::create_dir_all(&log_dir).unwrap();

        assert_eq!(
            tuned_arguments(ClientKind::Hdos, &Config::default(), 25, &log_dir),
            arguments(ClientKind::Hdos, false)
        );
    }

    #[test]
    fn a_disabled_tuning_keeps_the_plain_arguments() {
        let temp = TempDir::new("tuned-disabled");
        let log_dir = temp.path().join("logs");
        std::fs::create_dir_all(&log_dir).unwrap();
        let mut config = Config::default();
        config.runelite_tuning.enabled = false;

        assert_eq!(
            tuned_arguments(ClientKind::RuneLite, &config, 25, &log_dir),
            arguments(ClientKind::RuneLite, false)
        );
    }

    #[cfg(unix)]
    #[test]
    fn prune_gc_logs_removes_the_log_of_a_dead_client() {
        let temp = TempDir::new("prune-gc-logs");
        let dir = temp.path();
        let alive = dir.join(format!("gc-{}.log", std::process::id()));
        let dead = dir.join("gc-999999.log");
        let other = dir.join("client.txt");
        std::fs::write(&alive, b"log").unwrap();
        std::fs::write(&dead, b"log").unwrap();
        std::fs::write(&other, b"keep").unwrap();

        assert_eq!(prune_gc_logs(dir), 1);
        assert!(alive.exists());
        assert!(!dead.exists());
        assert!(other.exists());
    }

    #[test]
    fn launch_spawns_detached_child() {
        let temp = TempDir::new("launch-detached");
        let paths = paths(temp.path());
        let jar = temp.path().join("client.jar");
        std::fs::write(&jar, b"fake jar").unwrap();

        #[cfg(unix)]
        let java = PathBuf::from("/bin/sh");
        #[cfg(windows)]
        let java = PathBuf::from("cmd.exe");

        let request = LaunchRequest {
            jar: &jar,
            kind: ClientKind::RuneLite,
            credentials: None,
            java: Some(&java),
            template: Some(if cfg!(windows) {
                "cmd.exe /c exit 0"
            } else {
                "/bin/sh -c 'exit 0'"
            }),
            configure: false,
        };

        let pid = launch(&paths, &request).expect("launch should succeed");
        assert!(pid > 0);
    }

    #[cfg(unix)]
    #[test]
    fn launch_refuses_a_second_launch_of_the_same_character() {
        let temp = TempDir::new("launch-duplicate");
        let paths = paths(temp.path());
        let jar = temp.path().join("client.jar");
        std::fs::write(&jar, b"fake jar").unwrap();
        let java = PathBuf::from("/bin/sh");

        let request = LaunchRequest {
            jar: &jar,
            kind: ClientKind::RuneLite,
            credentials: None,
            java: Some(&java),
            template: Some("/bin/sh -c 'sleep 5'"),
            configure: false,
        };

        let first = launch(&paths, &request).expect("the first launch should succeed");
        let second = launch(&paths, &request);
        unsafe {
            kill(first as i32, 9);
        }

        assert!(matches!(second, Err(CoreError::AlreadyRunning(_))));
    }

    #[cfg(unix)]
    #[test]
    fn launch_allows_a_different_character_at_the_same_time() {
        let temp = TempDir::new("launch-multi-account");
        let paths = paths(temp.path());
        let jar = temp.path().join("client.jar");
        std::fs::write(&jar, b"fake jar").unwrap();
        let java = PathBuf::from("/bin/sh");

        let request_of = |character_id: &str| GameCredentials {
            session_id: "session".to_string(),
            character_id: character_id.to_string(),
            display_name: "Ada".to_string(),
        };
        let first_credentials = request_of("111");
        let second_credentials = request_of("222");

        let first = LaunchRequest {
            jar: &jar,
            kind: ClientKind::RuneLite,
            credentials: Some(&first_credentials),
            java: Some(&java),
            template: Some("/bin/sh -c 'sleep 5'"),
            configure: false,
        };
        let second = LaunchRequest {
            jar: &jar,
            kind: ClientKind::RuneLite,
            credentials: Some(&second_credentials),
            java: Some(&java),
            template: Some("/bin/sh -c 'sleep 5'"),
            configure: false,
        };

        let first_pid = launch(&paths, &first).expect("the first character should launch");
        let second_pid = launch(&paths, &second).expect("a different character should launch");
        unsafe {
            kill(first_pid as i32, 9);
            kill(second_pid as i32, 9);
        }

        assert_ne!(first_pid, second_pid);
    }
}
