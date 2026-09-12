# rustyBolt cross-crate contract

This file is the authority for crate boundaries. An implementation crate MUST keep
the public names and shapes below. Add items freely. Do not rename or remove.

## Why this split

Bolt is one CEF process. `Browser::LoginWindow` is at the same time the window, the
HTTP request interceptor and the OAuth client. Java discovery and process launch sit
inside two platform files of the same window class. Every port to a new OS touches
all three concerns.

rustyBolt splits them:

| crate | owns | knows about |
| --- | --- | --- |
| `bolt-auth` | Jagex OAuth2 PKCE state machine | nothing, no I/O |
| `bolt-jdk` | Java discovery and JVM argv | file system only |
| `bolt-core` | paths, config, sessions, install, launch | `bolt-auth`, `bolt-jdk`, HTTP |
| `bolt-cli` | headless driver | `bolt-core` |
| `bolt-macos` | AppKit and WKWebView shell | `bolt-core` |

A new OS shell implements UI only. It never repeats protocol or launch logic.

## bolt-auth

Sans-I/O. The caller does every network request. The flow returns the next action.

```rust
pub struct AuthConfig {
    pub account_origin: String,   // https://account.jagex.com
    pub auth_origin: String,      // https://auth.jagex.com
    pub client_id: String,        // com_jagex_auth_desktop_launcher
    pub consent_client_id: String,// 1fddee4e-b100-4f4e-b2b0-097f9088f9d2
    pub redirect_url: String,     // https://secure.runescape.com/m=weblogin/launcher-redirect
    pub scopes: String,
}
impl Default for AuthConfig { /* production values */ }

pub struct Pkce { pub verifier: String, pub challenge: String }
impl Pkce {
    pub fn generate() -> Pkce;              // 43 chars, OS CSPRNG
    pub fn from_verifier(v: &str) -> Pkce;  // S256 + base64url, no padding
}

pub struct LoginFlow { /* state1 state2 nonce pkce sub stage */ }
impl LoginFlow {
    pub fn new(config: AuthConfig) -> LoginFlow;
    pub fn authorize_url(&self) -> String;
    pub fn stage(&self) -> Stage;
    /// Feed every URL the shell navigates to. Returns Action::Ignore for unrelated URLs.
    pub fn on_navigation(&mut self, url: &str) -> Result<Action, AuthError>;
    /// Feed the body of the request that Action::PostForm asked for.
    pub fn on_token_response(&mut self, body: &str) -> Result<Action, AuthError>;
    /// Feed the body of the request that Action::PostJson asked for.
    pub fn on_session_response(&mut self, body: &str) -> Result<Session, AuthError>;
}

pub enum Stage { Authorize, Token, Consent, Session, Done }

pub enum Action {
    Ignore,
    PostForm { url: String, body: String },  // content-type x-www-form-urlencoded
    Navigate { url: String },                // consent step, load in the same view
    PostJson { url: String, body: String },  // content-type application/json
    Done(Session),
}

pub struct Session {
    pub session_id: String,
    pub display_name: String,
    pub suffix: String,
    pub sub: String,
}

/// GET this url with header ("Authorization", value) to list characters.
pub fn accounts_request(config: &AuthConfig, session_id: &str) -> (String, (String, String));
pub fn parse_accounts(body: &str) -> Result<Vec<Character>, AuthError>;
pub struct Character { pub account_id: String, pub display_name: String }

pub enum AuthError { StateMismatch, NonceMismatch, SubMismatch, MissingField(&'static str),
                     BadJwt, Json(String), Provider { error: String, description: Option<String> },
                     WrongStage }
```

Rules:
- The generator is `rand::rngs::OsRng`. Bolt uses `std::rand`, which is not safe here.
- NEVER print a token. Bolt prints the `id_token` to stdout.
- `on_navigation` matches the code redirect by host and path, and the consent redirect
  by `localhost` host with the parameters in the fragment.

## bolt-jdk

```rust
pub struct JavaVersion { pub feature: u32, pub raw: String }
pub enum Source { Explicit, JavaHome, Path, SystemLocation }
pub struct JavaRuntime { pub path: PathBuf, pub home: Option<PathBuf>,
                         pub version: Option<JavaVersion>, pub source: Source }

pub fn discover() -> Vec<JavaRuntime>;             // ordered by Source, deduplicated by real path
pub fn select(min_feature: u32) -> Option<JavaRuntime>;
pub fn probe(path: &Path) -> Option<JavaRuntime>;  // runs `java -version`

pub struct JvmOptions {
    pub system_properties: Vec<(String, String)>,
    pub jvm_args: Vec<String>,
    pub jar: PathBuf,
    pub app_args: Vec<String>,
}
pub struct Invocation { pub program: PathBuf, pub args: Vec<String> }
impl JvmOptions { pub fn invocation(&self, java: &Path) -> Invocation; }

/// Replaces the `%command%` token of a user template with the default invocation.
pub fn apply_template(template: &str, default: &Invocation) -> Result<Invocation, TemplateError>;
```

Discovery order per OS: `JAVA_HOME`, then `PATH`, then system locations
(`/usr/libexec/java_home` and `/Library/Java/JavaVirtualMachines` on macOS,
`/usr/lib/jvm` and `/opt` on Linux, `Program Files` JDK vendors on Windows).
The Windows executable name is `javaw.exe`, other systems use `java`.

### The tuning layer

```rust
pub enum Gc { Default, Z, G1, Parallel }
pub enum AotMode { Load, Record }
pub struct AotCache { pub path: PathBuf, pub mode: AotMode }
pub struct Tuning {
    pub heap_min: Option<String>, pub heap_max: Option<String>, pub stack_size: Option<String>,
    pub gc: Gc, pub compact_object_headers: bool, pub string_deduplication: bool,
    pub native_access: bool, pub add_opens: Vec<String>, pub aot_cache: Option<AotCache>,
    pub gc_log: Option<PathBuf>, pub extra: Vec<String>,
}
impl Tuning { pub fn flags(&self, feature: u32) -> Vec<String>; }

pub fn newest_jar(dir: &Path, prefix: &str) -> Option<PathBuf>;
pub fn aot_cache_for(cache_dir: &Path, client_jar: &Path) -> AotCache;
pub fn prune_aot_caches(cache_dir: &Path, keep: &Path) -> io::Result<usize>;
pub fn branded_java(java: &Path, dir: &Path, name: &str) -> PathBuf;
```

`Tuning` is pure. It never finds the feature number by itself; the caller gives it.
That rule keeps the flag gates testable with no JDK on the machine.

Gate rules:
- `Gc::Z` adds `-XX:+ZGenerational` only below feature 24. Feature 24 and newer are
  generational only and print `Ignoring option ZGenerational`.
- Compact object headers, native access, string deduplication and the AOT cache need
  feature 24 or newer.

`branded_java` makes a hard link to the Java binary with a chosen name. The Dock and
the process list show the file name of an unbundled executable, so the link gives the
client its own identity. The function returns the original path when the link fails.

## bolt-core

```rust
pub struct Paths { pub config_dir: PathBuf, pub data_dir: PathBuf,
                   pub cache_dir: PathBuf, pub runtime_dir: PathBuf }
impl Paths {
    pub fn resolve() -> io::Result<Paths>;   // creates the directories
    pub fn config_file(&self) -> PathBuf;    // launcher.json
    pub fn credentials_file(&self) -> PathBuf; // creds.json, mode 0600 on unix
    pub fn client_dir(&self) -> PathBuf;
}

pub struct Config { /* serde, see src; keeps Bolt key names where they still apply */ }
impl Config { pub fn load(p: &Paths) -> Config; pub fn save(&self, p: &Paths) -> io::Result<()>; }

pub struct SessionStore { /* Vec<Session> plus file path */ }
impl SessionStore {
    pub fn load(p: &Paths) -> SessionStore;
    pub fn save(&self) -> io::Result<()>;
    pub fn upsert(&mut self, s: Session);
    pub fn remove(&mut self, sub: &str);
    pub fn sessions(&self) -> &[Session];
}

pub enum ClientKind { RuneLite, Hdos }
pub struct InstalledClient { pub kind: ClientKind, pub jar: PathBuf, pub version: String }

pub struct Installer<'a> { /* paths */ }
impl<'a> Installer<'a> {
    pub fn installed(&self, kind: ClientKind) -> Option<InstalledClient>;
    pub fn latest(&self, kind: ClientKind) -> Result<Release, CoreError>;
    pub fn install(&self, kind: ClientKind, rel: &Release,
                   progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<InstalledClient, CoreError>;
}
pub struct Release { pub version: String, pub url: String, pub size: Option<u64>, pub sha256: Option<String> }

pub struct GameCredentials { pub session_id: String, pub character_id: String, pub display_name: String }
pub struct LaunchRequest<'a> {
    pub jar: &'a Path,
    pub kind: ClientKind,
    pub credentials: Option<&'a GameCredentials>,
    pub java: Option<&'a Path>,
    pub template: Option<&'a str>,
    pub configure: bool,
}
pub fn launch(paths: &Paths, req: &LaunchRequest) -> Result<u32, CoreError>;  // detached child pid

/// Everything that the launcher needs to start one client process.
pub struct LaunchPlan { pub program: PathBuf, pub args: Vec<String>,
                        pub working_dir: PathBuf, pub env: Vec<(String, String)> }
/// `launch` runs exactly this plan. A shell that shows the command line MUST use it,
/// so the shown command and the started command never differ.
pub fn plan(paths: &Paths, req: &LaunchRequest) -> Result<LaunchPlan, CoreError>;

// The serde form of the JVM tuning. It maps to `bolt_jdk::Tuning`.
pub enum GcChoice { Default, Z, G1, Parallel }
pub struct TuningConfig { /* heap, stack, gc, gates, add_opens, dock, app args */ }
impl TuningConfig {
    pub fn to_tuning(&self, log_dir: &Path, client_repository: Option<&Path>) -> bolt_jdk::Tuning;
    pub fn system_properties(&self) -> Vec<(String, String)>;
    pub fn dock_args(&self) -> Vec<String>;
}
pub fn tuned_client_options(kind: ClientKind, jar: &Path, data_dir: &Path, configure: bool,
                            config: &Config, feature: u32, log_dir: &Path) -> JvmOptions;

// Which character the user wants next.
pub struct Usage { pub count: u64, pub last_used: u64 }
pub struct UsageStore { /* usage.json of the config directory */ }
impl UsageStore {
    pub fn load(p: &Paths) -> UsageStore;
    pub fn save(&self) -> io::Result<()>;
    pub fn mark_used(&mut self, key: &str);
    pub fn usage(&self, key: &str) -> Usage;
    pub fn order<'a, T>(&self, items: &'a [T], window: u64, key: impl Fn(&T) -> &str) -> Vec<&'a T>;
}

// Login values from a secret manager instead of a saved session.
pub enum CredentialFormat { OnePassword, Json, EnvLines }
pub struct CommandCredentials { pub program: String, pub args: Vec<String>, pub format: CredentialFormat }
pub enum CredentialSource { Session, Command(CommandCredentials) }
impl CommandCredentials {
    pub fn one_password(vault: &str) -> CommandCredentials;
    pub fn fetch(&self, item: &str) -> Result<GameCredentials, CoreError>;
}

// Forced RuneLite profile properties.
pub struct PropertyOverrides { pub strip_prefixes: Vec<String>, pub force: Vec<(String, String)> }
impl PropertyOverrides { pub fn gpu_defaults() -> PropertyOverrides;
                         pub fn apply_to_text(&self, text: &str) -> String; }
pub fn apply_to_profiles(dir: &Path, overrides: &PropertyOverrides) -> Result<usize, CoreError>;

pub struct HttpAuth<'a> { /* drives bolt-auth over ureq */ }
impl<'a> HttpAuth<'a> {
    pub fn new(config: &'a AuthConfig) -> HttpAuth<'a>;
    /// Runs every network step that the flow asks for until the next UI step.
    pub fn advance(&self, flow: &mut LoginFlow, action: Action) -> Result<Action, CoreError>;
    pub fn characters(&self, session_id: &str) -> Result<Vec<Character>, CoreError>;
}
```

Launch rules taken from Bolt, with the errors fixed:
- Environment of the child: `JX_SESSION_ID`, `JX_CHARACTER_ID`, `JX_DISPLAY_NAME`,
  plus `HOME` set to the data directory on unix.
- Working directory is the data directory.
- The child detaches. On unix it calls `setsid`.
- RuneLite argv: `-Duser.home=<data>` `-jar <jar>` `-J-Duser.home=<data>` `[--configure]`.
- HDOS argv: `-Duser.home=<data>` `-Dapp.user.home=<data>` `-jar <jar>`.
- A download writes to a temporary file, checks the digest when the release gives one,
  then renames. Bolt truncates the live file instead.

## Verification

`cargo test --workspace` and `cargo run -p bolt-cli -- java list`.
