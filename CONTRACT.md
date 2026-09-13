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
| `rustybolt-auth` | Jagex OAuth2 PKCE state machine | nothing, no I/O |
| `rustybolt-jdk` | Java discovery and JVM argv | file system only |
| `rustybolt-security` | egress allowlist and Content Security Policy | nothing, no I/O |
| `rustybolt-core` | paths, config, sessions, client lookup, launch | `rustybolt-auth`, `rustybolt-jdk`, `rustybolt-security`, HTTP |
| `rustybolt-cli` | portable CLI driver and configuration UI | `rustybolt-core`, `rustybolt-jdk`, `rustybolt-security` |

The launcher driver is portable across macOS, Linux, and Windows. It never repeats protocol or launch logic.

## rustybolt-auth

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

## rustybolt-jdk

```rust
pub struct JavaVersion { pub feature: u32, pub raw: String }
pub enum Source { Explicit, JavaHome, Path, SystemLocation, ClientBundle }
pub struct JavaRuntime { pub path: PathBuf, pub home: Option<PathBuf>,
                         pub version: Option<JavaVersion>, pub source: Source,
                         pub headless: bool }  // Linux: no lib/libawt_xawt.so; select() skips it

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
`/usr/lib/jvm` and `/opt` on Linux, `Program Files` JDK vendors on Windows),
then the runtime that the RuneLite installer ships (`%LOCALAPPDATA%\RuneLite\jre`,
`/Applications/RuneLite.app/Contents/Resources/jre`, `~/.local/share/RuneLite/jre`).
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
- `Gc::Z` gives `-XX:+UseG1GC` below feature 15, where ZGC is experimental; adds
  `-XX:+ZGenerational` on features 21 to 23; feature 24 and newer are generational only.
- Compact object headers, native access, string deduplication and the AOT cache need
  feature 24 or newer.

`branded_java` makes a hard link to the Java binary with a chosen name. The Dock and
the process list show the file name of an unbundled executable, so the link gives the
client its own identity. The function returns the original path when the link fails.

## rustybolt-core

```rust
pub struct Paths { pub config_dir: PathBuf, pub data_dir: PathBuf,
                   pub cache_dir: PathBuf, pub runtime_dir: PathBuf }
impl Paths {
    pub fn resolve() -> io::Result<Paths>;   // creates the directories
    pub fn config_file(&self) -> PathBuf;    // launcher.json
    pub fn credentials_file(&self) -> PathBuf; // creds.json of older versions; load() moves it into the keychain
}

pub struct Config { /* serde, see src; keeps Bolt key names where they still apply */ }
impl Config { pub fn load(p: &Paths) -> Config; pub fn save(&self, p: &Paths) -> io::Result<()>; }

// One keychain entry (service `rustybolt`, user `sessions`) holds every session as JSON:
// macOS Keychain, Windows Credential Manager, Linux Secret Service.
pub trait Vault: Send { fn read(&self) -> Result<Option<String>, KeychainError>; fn write(&self, s: &str) -> Result<(), KeychainError>; }
pub struct KeychainError(pub String);
pub fn keychain_available() -> Result<(), KeychainError>; // the dashboard shows the Err; only saving a login needs Ok

pub struct SessionStore { /* Vec<Session> plus Box<dyn Vault> */ }
impl SessionStore {
    pub fn load(p: &Paths) -> SessionStore;           // keychain; imports and removes creds.json
    pub fn with_vault(v: Box<dyn Vault>) -> SessionStore;
    pub fn save(&self) -> io::Result<()>;
    pub fn upsert(&mut self, s: Session);
    pub fn remove(&mut self, sub: &str);
    pub fn sessions(&self) -> &[Session];
}

pub enum ClientKind { RuneLite, Hdos }
impl ClientKind { pub fn name(self) -> &'static str; pub fn title(self) -> &'static str; pub fn wiki_url(self) -> &'static str; }

// The user installs the client with the installer of that project. The launcher
// only looks for the jar. A path from the config wins, even when the file is absent.
pub fn client_candidates(kind: ClientKind) -> Vec<PathBuf>;           // every place the launcher looks
pub fn locate_client(kind: ClientKind, config: &Config) -> Option<PathBuf>;

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

// The serde form of the JVM tuning. It maps to `rustybolt_jdk::Tuning`.
pub enum GcChoice { Default, Z, G1, Parallel }
pub struct TuningConfig { /* heap, stack, gc, gates, add_opens, dock, app args */ }
impl TuningConfig {
    pub fn to_tuning(&self, log_dir: &Path, client_repository: Option<&Path>) -> rustybolt_jdk::Tuning;
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
impl PropertyOverrides { pub fn gpu_preset() -> PropertyOverrides;  // a starting point for the editor, never applied by itself
                         pub fn apply_to_text(&self, text: &str) -> String; }
pub fn apply_to_profiles(dir: &Path, overrides: &PropertyOverrides) -> Result<usize, CoreError>;  // launch() runs it for RuneLite when Config::runelite_profile_overrides is Some

pub struct HttpAuth<'a> { /* drives rustybolt-auth over ureq */ }

pub mod memory {  // the heap that fits the machine
    pub fn total_bytes() -> Option<u64>;
    pub fn heap_cap(total_bytes: u64) -> String;             // half, rounded to 256 MB, never below 512 MB
    pub fn fit_heap(min: Option<String>, max: Option<String>, total: Option<u64>) -> (Option<String>, Option<String>);
}
pub mod desktop {  // Linux: a hidden desktop entry that names the RuneLite window for GNOME
    pub fn ensure_runelite_entry(jar: &Path) -> io::Result<()>;
}
pub mod diagnose {  // a bug-report bundle with the personal parts replaced
    pub struct Accounts { pub character_counts: Vec<Option<usize>> }
    pub struct Redactor;  // new(secrets) learns names and ids; redact() also masks home, user, hashes, profiles, IPs, emails
    pub fn collect(p: &Paths, c: &Config, a: &Accounts, r: &Redactor, version: &str) -> Vec<(String, Vec<u8>)>;
    pub fn write_zip(path: &Path, files: &[(String, Vec<u8>)]) -> io::Result<()>;  // stored entries
}
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

## rustybolt-security

Validates every network destination and generates Content Security Policy rules.

```rust
pub const ALLOWED_JAGEX_HOST: &str;
pub const ALLOWED_AUTH_HOST: &str;
pub const ALLOWED_REDIRECT_HOST: &str;
pub const CSP_VALUE: &str;
pub const CSP_META_TAG: &str;

pub enum SecurityError {
    DisallowedHost(String),
    InsecureScheme(String),
    DisallowedPort(u16),
    InvalidUserInfo,
    MalformedUrl(String),
}

pub fn is_allowed_host(host: &str) -> bool;
pub fn is_allowed_navigation(url: &str) -> bool;
pub fn is_allowed_external_url(url: &str) -> bool;
pub fn csp_header_value() -> &'static str;
pub fn csp_meta_tag() -> &'static str;
pub fn validate_url(raw_url: &str) -> Result<(), SecurityError>;
```

## Verification

`cargo test --workspace` and `cargo run -p rustybolt-cli -- verify`.
