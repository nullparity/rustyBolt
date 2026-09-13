//! The `configure` command and native local application server.
//!
//! Spawns a lightweight local HTTP server and opens the launcher dashboard
//! in the default web browser on macOS, Linux, and Windows.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rustybolt_core::{
    Action, AuthConfig, ClientKind, Config, HttpAuth, LaunchRequest, LoginFlow, Paths,
    SessionStore, UsageStore,
};
use serde::{Deserialize, Serialize};

use tao::event_loop::EventLoopProxy;

use crate::gui::AppEvent;
use crate::httpd::{Connection, Request};
use crate::i18n::Lang;
use crate::web::{HTML_PAGE, STYLESHEET};
use crate::wifi::{WifiManager, WifiStatus};
use crate::CliError;

pub const ICON_SVG: &str = include_str!("../../../icon/rustybolt.svg");

#[derive(Serialize)]
struct JavaInfo {
    path: String,
    version: String,
    source: String,
}

#[derive(Serialize)]
struct ClientInfo {
    runelite_detected: Option<String>,
    runelite_candidates: Vec<String>,
    hdos_detected: Option<String>,
    hdos_candidates: Vec<String>,
}

#[derive(Serialize, Clone)]
struct SessionView {
    sub: String,
    display_name: String,
    suffix: String,
}

#[derive(Serialize, Clone)]
struct CharacterView {
    account_id: String,
    display_name: String,
    last_used: u64,
    use_count: u64,
}

#[derive(Serialize)]
struct ServerState {
    config: Config,
    runtimes: Vec<JavaInfo>,
    clients: ClientInfo,
    runelite_plan: Option<String>,
    sessions: Vec<SessionView>,
    active_sub: Option<String>,
    characters: Vec<CharacterView>,
    has_session: bool,
    wifi: WifiStatus,
    /// Why the keychain cannot hold a session, when it cannot. The launcher
    /// still runs; only saving a login needs the keychain.
    keychain_error: Option<String>,
}

#[derive(Serialize)]
struct LaunchResponse {
    ok: bool,
    pid: Option<u32>,
    close: bool,
    error: Option<String>,
}

#[derive(Deserialize)]
struct LaunchPayload {
    client: Option<String>,
    sub: Option<String>,
    character_id: Option<String>,
}

#[derive(Deserialize)]
struct AuthAddressPayload {
    url: Option<String>,
    address: Option<String>,
}

#[derive(Deserialize, Default)]
struct AuthStartPayload {
    /// `browser`, `window`, or absent for automatic.
    mode: Option<String>,
}

#[derive(Deserialize)]
struct RemoveSessionPayload {
    sub: String,
}

#[derive(Serialize)]
struct SaveResponse {
    ok: bool,
    plan: Option<String>,
}

/// Runs `rustybolt configure`.
pub(crate) fn run(args: &[String]) -> Result<(), CliError> {
    let use_browser = args.iter().any(|arg| arg == "--browser");
    for arg in args {
        if arg != "--browser" {
            return Err(CliError::Message(format!(
                "unknown flag `{arg}`. Use `--browser` to force system browser."
            )));
        }
    }

    let paths = Arc::new(Paths::resolve()?);
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}/#play");

    let guard = Guard::new(port);
    // `rustybolt jagex:...` (the Linux scheme handler) finds this instance here.
    let port_file = PortFile::write(&paths, port, &guard.token);

    let running = Arc::new(AtomicBool::new(true));
    let flow_state = Arc::new(Mutex::new(None));
    let wifi_manager = WifiManager::new(false);

    let (event_loop_opt, proxy_opt) = if !use_browser && crate::gui::has_display() {
        let (event_loop, proxy) = crate::gui::create_event_loop();
        (Some(event_loop), Some(proxy))
    } else {
        (None, None)
    };

    let s_paths = Arc::clone(&paths);
    let s_running = Arc::clone(&running);
    let s_flow_state = Arc::clone(&flow_state);
    let s_proxy = proxy_opt.clone();
    let s_wifi = wifi_manager.clone();
    let s_guard = guard.clone();

    let server_thread = thread::spawn(move || {
        for stream in listener.incoming() {
            if !s_running.load(Ordering::SeqCst) {
                break;
            }
            match stream {
                Ok(stream) => {
                    let paths = Arc::clone(&s_paths);
                    let running = Arc::clone(&s_running);
                    let flow_state = Arc::clone(&s_flow_state);
                    let proxy = s_proxy.clone();
                    let wifi = s_wifi.clone();
                    let guard = s_guard.clone();
                    thread::spawn(move || {
                        handle_connection(
                            stream,
                            &paths,
                            &running,
                            &flow_state,
                            &wifi,
                            proxy.as_ref(),
                            &guard,
                        );
                    });
                }
                Err(e) => {
                    if !s_running.load(Ordering::SeqCst) {
                        break;
                    }
                    eprintln!("rustybolt: connection error: {e}");
                }
            }
        }
    });

    if let Some(event_loop) = event_loop_opt {
        if let Err(error) = crate::gui::run_window(
            event_loop,
            &url,
            Arc::clone(&running),
            wifi_manager,
            Arc::clone(&paths),
            Arc::clone(&flow_state),
            Lang::detect(Config::load(&paths).language.as_deref()),
        ) {
            eprintln!("rustybolt: native window failed: {error}. Falling back to browser.");
            open_browser(&url);
            let _ = server_thread.join();
        }
    } else {
        println!("rustybolt: launcher server running at {url}");
        println!("Opening your default web browser. Press Ctrl+C to close.");
        open_browser(&url);
        let _ = server_thread.join();
    }

    drop(port_file);
    println!("rustybolt: launcher server stopped.");
    Ok(())
}

/// Header that carries the per-launch API token.
const TOKEN_HEADER: &str = "x-rustybolt-token";

/// What every request must prove before the launcher API answers it.
///
/// The server listens on loopback only, but any web page in the user's
/// browser can still send it a request, and DNS rebinding lets such a page
/// read the answer. `Host` must be this server, `Origin` (when a browser
/// sends one) must be this server, and every `/api/` request must carry the
/// token that only the served page and the port file know.
#[derive(Clone)]
struct Guard {
    port: u16,
    /// `127.0.0.1:<port>`
    host: String,
    /// `http://127.0.0.1:<port>`
    origin: String,
    token: String,
}

impl Guard {
    fn new(port: u16) -> Self {
        Self {
            port,
            host: format!("127.0.0.1:{port}"),
            origin: format!("http://127.0.0.1:{port}"),
            token: new_token(),
        }
    }

    /// The reason to refuse `request`, or `None` when it may proceed.
    fn refuse(&self, request: &Request) -> Option<&'static str> {
        if request.header("host") != Some(self.host.as_str()) {
            return Some("wrong Host");
        }
        if let Some(origin) = request.header("origin") {
            if origin != self.origin {
                return Some("cross-origin request");
            }
        }
        if let Some(site) = request.header("sec-fetch-site") {
            if site != "same-origin" && site != "none" {
                return Some("cross-site request");
            }
        }
        if request.path.starts_with("/api/")
            && request.header(TOKEN_HEADER) != Some(self.token.as_str())
        {
            return Some("missing or wrong API token");
        }
        None
    }
}

/// 32 random bytes from the operating system, as hex.
fn new_token() -> String {
    use rand::TryRngCore as _;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("the operating system random generator failed");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The file that records the port and the API token of the running
/// launcher, so `rustybolt jagex:...` can reach it. Removed on drop.
struct PortFile(PathBuf);

impl PortFile {
    fn path(paths: &Paths) -> PathBuf {
        paths.runtime_dir.join("launcher.port")
    }

    fn write(paths: &Paths, port: u16, token: &str) -> Self {
        let path = Self::path(paths);
        let _ = std::fs::write(&path, format!("{port}\n{token}\n"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Self(path)
    }

    /// Reads the port and the token of a running launcher, if any.
    fn read(paths: &Paths) -> Option<(u16, String)> {
        let text = std::fs::read_to_string(Self::path(paths)).ok()?;
        let mut lines = text.lines();
        let port = lines.next()?.trim().parse().ok()?;
        let token = lines.next()?.trim().to_string();
        Some((port, token))
    }
}

impl Drop for PortFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Forwards a `jagex:` redirect URL to the running launcher.
///
/// The desktop file (Linux) and the app bundle (macOS) register the scheme,
/// so the browser starts `rustybolt jagex:code=...` for the login redirect.
pub(crate) fn forward_redirect(url: &str) -> Result<(), CliError> {
    if !rustybolt_security::is_login_redirect(url) {
        return Err(CliError::Message(format!(
            "`{url}` is not a login redirect"
        )));
    }
    let paths = Paths::resolve()?;
    let Some((port, token)) = PortFile::read(&paths) else {
        return Err(CliError::Message(
            "no running launcher found. Start rustybolt, then click Add Jagex Account.".to_string(),
        ));
    };
    let body = serde_json::json!({ "url": url }).to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| CliError::Message(format!("cannot reach the running launcher: {e}")))?;
    let request = format!(
        "POST /api/auth/redirect HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{TOKEN_HEADER}: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes())?;
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    if response.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        Err(CliError::Message(format!(
            "the launcher rejected the redirect: {}",
            response.rsplit("\r\n").next().unwrap_or("")
        )))
    }
}

fn handle_connection(
    stream: TcpStream,
    paths: &Paths,
    running: &AtomicBool,
    flow_state: &Arc<Mutex<Option<LoginFlow>>>,
    wifi: &WifiManager,
    proxy: Option<&EventLoopProxy<AppEvent>>,
    guard: &Guard,
) {
    let port = guard.port;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let mut conn = Connection::new(stream);

    while let Ok(Some(request)) = conn.next_request() {
        let keep_alive = request.keep_alive;
        if let Some(reason) = guard.refuse(&request) {
            let _ = conn.stream().write_all(
                format!(
                    "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reason}",
                    reason.len()
                )
                .as_bytes(),
            );
            break;
        }
        let req = HttpRequest {
            token: &guard.token,
            method: &request.method,
            path: &request.path,
            query: &request.query,
            body: &request.body,
            keep_alive,
            wifi,
            proxy,
        };
        let stream = conn.stream();
        let should_exit = respond(&req, stream, paths, flow_state);
        let _ = stream.flush();

        if should_exit {
            if req.path == "/api/shutdown" {
                running.store(false, Ordering::SeqCst);
                if let Some(proxy) = proxy {
                    let _ = proxy.send_event(AppEvent::Shutdown);
                }
                let _ = TcpStream::connect(("127.0.0.1", port));
                break;
            } else if let Some(proxy) = proxy {
                let _ = proxy.send_event(AppEvent::HideWindow);
            } else {
                running.store(false, Ordering::SeqCst);
                let _ = TcpStream::connect(("127.0.0.1", port));
                break;
            }
        }

        if !keep_alive {
            break;
        }
    }
    let _ = conn.stream().shutdown(std::net::Shutdown::Both);
}

struct HttpRequest<'a> {
    token: &'a str,
    method: &'a str,
    path: &'a str,
    query: &'a str,
    body: &'a [u8],
    keep_alive: bool,
    wifi: &'a WifiManager,
    proxy: Option<&'a EventLoopProxy<AppEvent>>,
}

fn security_headers() -> String {
    format!(
        "X-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: {}\r\n",
        rustybolt_security::csp_header_value()
    )
}

fn respond(
    req: &HttpRequest<'_>,
    stream: &mut TcpStream,
    paths: &Paths,
    flow_state: &Arc<Mutex<Option<LoginFlow>>>,
) -> bool {
    let conn_header = if req.keep_alive {
        "keep-alive"
    } else {
        "close"
    };
    let sec_hdrs = security_headers();
    match (req.method, req.path) {
        ("GET", "/" | "/index.html") => {
            let config = Config::load(paths);
            let lang = Lang::detect(config.language.as_deref());
            let languages: Vec<_> = Lang::ALL
                .iter()
                .map(|l| serde_json::json!({ "tag": l.tag(), "name": l.tr("lang.name") }))
                .collect();
            let state = build_state(paths, config, req.wifi.status());
            let state_json = serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
            let html = HTML_PAGE
                .replace("<!--STYLES-->", &format!("<style>{STYLESHEET}</style>"))
                .replace("<!--LOGO_SVG-->", ICON_SVG)
                .replace("<!--VERSION-->", env!("CARGO_PKG_VERSION"))
                .replace("<!--LANG-->", lang.tag())
                .replace(
                    "/*INITIAL_STATE*/",
                    &format!(
                        "window.INITIAL_STATE = {state_json}; window.RUSTYBOLT_TOKEN = {}; \
                         window.RUSTYBOLT_LANG = {}; window.RUSTYBOLT_LANGUAGES = {}; window.RUSTYBOLT_I18N = {};",
                        serde_json::to_string(req.token).unwrap_or_default(),
                        serde_json::to_string(lang.tag()).unwrap_or_default(),
                        serde_json::Value::Array(languages),
                        lang.json(),
                    ),
                );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n{sec_hdrs}\r\n{html}",
                html.len()
            );
            let _ = stream.write_all(response.as_bytes());
            false
        }
        ("GET", "/icon.svg" | "/favicon.ico") => {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{ICON_SVG}",
                ICON_SVG.len()
            );
            let _ = stream.write_all(response.as_bytes());
            false
        }
        ("GET", "/api/state" | "/api/config") => {
            let config = Config::load(paths);
            let state = build_state(paths, config, req.wifi.status());
            let json = serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                json.len()
            );
            let _ = stream.write_all(response.as_bytes());
            false
        }
        ("GET", "/api/wifi") => {
            let status = req.wifi.status();
            let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string());
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                json.len()
            );
            let _ = stream.write_all(response.as_bytes());
            false
        }
        ("POST", "/api/wifi") => {
            #[derive(Deserialize)]
            struct WifiTogglePayload {
                enabled: Option<bool>,
            }
            let payload = serde_json::from_slice::<WifiTogglePayload>(req.body).ok();
            match payload.and_then(|p| p.enabled) {
                Some(enabled) => req.wifi.set_enabled(enabled),
                None => {
                    let _ = req.wifi.toggle();
                }
            }
            let status = req.wifi.status();
            let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string());
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                json.len()
            );
            let _ = stream.write_all(response.as_bytes());
            false
        }
        ("GET", "/api/characters") => {
            let sub = req.query.split('&').find_map(|pair| {
                let mut kv = pair.splitn(2, '=');
                if kv.next() == Some("sub") {
                    kv.next()
                } else {
                    None
                }
            });
            let store = SessionStore::load(paths);
            let session = match sub {
                Some(wanted) => store.sessions().iter().find(|s| s.sub == wanted),
                None => store.sessions().first(),
            };
            let config = Config::load(paths);
            let chars = match session {
                Some(s) => fetch_characters(paths, &config, &s.session_id),
                None => Vec::new(),
            };
            let json = serde_json::to_string(&chars).unwrap_or_else(|_| "[]".to_string());
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                json.len()
            );
            let _ = stream.write_all(response.as_bytes());
            false
        }
        ("POST", "/api/auth/start") => {
            let mode = serde_json::from_slice::<AuthStartPayload>(req.body)
                .unwrap_or_default()
                .mode
                .unwrap_or_default();
            let json = match req.proxy {
                None => {
                    let url = new_flow(flow_state);
                    open_browser(&url);
                    serde_json::json!({ "ok": true, "native": false, "url": url })
                }
                Some(proxy) => {
                    let want_browser =
                        mode != "window" && crate::platform::browser_login_available();
                    let readiness = if want_browser {
                        crate::platform::readiness()
                    } else {
                        crate::platform::Readiness::PortInUse
                    };
                    match (want_browser, readiness) {
                        // Ask before opening anything: the setup is a
                        // privileged, one-time step the user must agree to.
                        (true, crate::platform::Readiness::NeedsSetup(reason)) => {
                            serde_json::json!({ "ok": true, "native": true, "setup": true, "reason": reason })
                        }
                        (true, crate::platform::Readiness::Ready) => {
                            let url = new_flow(flow_state);
                            let _ = proxy.send_event(AppEvent::OpenLoginBrowser(url));
                            serde_json::json!({ "ok": true, "native": true, "mode": "browser" })
                        }
                        (want, readiness) => {
                            let url = new_flow(flow_state);
                            let _ = proxy.send_event(AppEvent::OpenLoginWindow(url));
                            let note = if want && readiness == crate::platform::Readiness::PortInUse
                            {
                                "Port 80 is in use by another program; logging in inside rustyBolt instead."
                            } else {
                                ""
                            };
                            serde_json::json!({ "ok": true, "native": true, "mode": "window", "note": note })
                        }
                    }
                }
            }
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                json.len()
            );
            let _ = stream.write_all(response.as_bytes());
            false
        }
        ("POST", "/api/auth/setup") => {
            // Runs the native elevation prompt; blocks this connection only.
            let (status, json) = match crate::platform::install_forward() {
                Ok(()) => ("200 OK", serde_json::json!({ "ok": true })),
                Err(message) => (
                    "400 Bad Request",
                    serde_json::json!({ "ok": false, "error": message }),
                ),
            };
            let json = json.to_string();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                json.len()
            );
            let _ = stream.write_all(response.as_bytes());
            false
        }
        ("POST", "/api/auth/redirect") => {
            // A redirect URL delivered out of band (the `jagex:` scheme handler).
            let payload = serde_json::from_slice::<AuthAddressPayload>(req.body);
            let address = payload
                .ok()
                .and_then(|p| p.address.or(p.url))
                .unwrap_or_default();
            let (status, json) = if !rustybolt_security::is_login_redirect(&address) {
                (
                    "400 Bad Request",
                    serde_json::json!({ "ok": false, "error": "not a login redirect" }),
                )
            } else if let Some(proxy) = req.proxy {
                // The window finishes the flow and updates the page itself.
                let _ = proxy.send_event(AppEvent::LoginRedirect(address));
                ("200 OK", serde_json::json!({ "ok": true }))
            } else {
                match complete_login(paths, flow_state, &address) {
                    LoginOutcome::Done(_) => ("200 OK", serde_json::json!({ "ok": true })),
                    LoginOutcome::Navigate(url) => {
                        open_browser(&url);
                        ("200 OK", serde_json::json!({ "ok": true, "next": true }))
                    }
                    LoginOutcome::Error(message) => (
                        "400 Bad Request",
                        serde_json::json!({ "ok": false, "error": message }),
                    ),
                }
            };
            let json = json.to_string();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                json.len()
            );
            let _ = stream.write_all(response.as_bytes());
            false
        }
        ("POST", "/api/auth/complete") => {
            let payload = serde_json::from_slice::<AuthAddressPayload>(req.body);
            let address = payload
                .ok()
                .and_then(|p| p.address.or(p.url))
                .unwrap_or_default();
            let (success, result_json) = match complete_login(paths, flow_state, &address) {
                LoginOutcome::Done(json) => (true, json),
                LoginOutcome::Navigate(url) => {
                    open_browser(&url);
                    let json = serde_json::json!({
                        "ok": false,
                        "next": true,
                        "url": url,
                        "error": "One more step: approve the consent page that opened in your browser, then paste the address it redirects to (it starts with http://localhost/).",
                    });
                    (false, json.to_string())
                }
                LoginOutcome::Error(message) => (
                    false,
                    serde_json::json!({ "ok": false, "error": message }).to_string(),
                ),
            };

            let status = if success { "200 OK" } else { "400 Bad Request" };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{result_json}",
                result_json.len()
            );
            let _ = stream.write_all(response.as_bytes());
            false
        }
        ("POST", "/api/auth/remove") => {
            if let Ok(payload) = serde_json::from_slice::<RemoveSessionPayload>(req.body) {
                let mut store = SessionStore::load(paths);
                store.remove(&payload.sub);
                let _ = store.save();
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: 11\r\nConnection: {conn_header}\r\n\r\n{{\"ok\":true}}"
                    )
                    .as_bytes(),
                );
            } else {
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: {conn_header}\r\n\r\n"
                    )
                    .as_bytes(),
                );
            }
            false
        }
        ("POST" | "PUT", "/api/save" | "/api/config") => {
            if let Ok(config) = serde_json::from_slice::<Config>(req.body) {
                let _ = config.save(paths);
                let plan = compute_preview(paths, &config, ClientKind::RuneLite);
                let res = SaveResponse { ok: true, plan };
                let json =
                    serde_json::to_string(&res).unwrap_or_else(|_| "{\"ok\":true}".to_string());
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                    json.len()
                );
                let _ = stream.write_all(response.as_bytes());
            } else {
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: {conn_header}\r\n\r\n"
                    )
                    .as_bytes(),
                );
            }
            false
        }
        ("POST", "/api/shutdown") => {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
            );
            true
        }
        ("POST", "/api/launch") => {
            let payload =
                serde_json::from_slice::<LaunchPayload>(req.body).unwrap_or(LaunchPayload {
                    client: None,
                    sub: None,
                    character_id: None,
                });
            let kind = match payload.client.as_deref() {
                Some("hdos") => ClientKind::Hdos,
                _ => ClientKind::RuneLite,
            };
            let config = Config::load(paths);
            match crate::launch::launch_client(
                paths,
                &config,
                kind,
                payload.sub.as_deref(),
                payload.character_id.as_deref(),
            ) {
                Ok(pid) => {
                    let should_close = config.close_after_launch;
                    let res = LaunchResponse {
                        ok: true,
                        pid: Some(pid),
                        close: should_close,
                        error: None,
                    };
                    let json =
                        serde_json::to_string(&res).unwrap_or_else(|_| "{\"ok\":true}".to_string());
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                        json.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    should_close
                }
                Err(error) => {
                    let res = LaunchResponse {
                        ok: false,
                        pid: None,
                        close: false,
                        error: Some(error.to_string()),
                    };
                    let json = serde_json::to_string(&res)
                        .unwrap_or_else(|_| "{\"ok\":false}".to_string());
                    let response = format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                        json.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    false
                }
            }
        }
        ("POST", "/api/launch/runelite" | "/api/launch/hdos") => {
            let kind = if req.path.ends_with("hdos") {
                ClientKind::Hdos
            } else {
                ClientKind::RuneLite
            };
            let config = Config::load(paths);
            match crate::launch::launch_client(paths, &config, kind, None, None) {
                Ok(pid) => {
                    let should_close = config.close_after_launch;
                    let res = LaunchResponse {
                        ok: true,
                        pid: Some(pid),
                        close: should_close,
                        error: None,
                    };
                    let json =
                        serde_json::to_string(&res).unwrap_or_else(|_| "{\"ok\":true}".to_string());
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                        json.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    should_close
                }
                Err(error) => {
                    let res = LaunchResponse {
                        ok: false,
                        pid: None,
                        close: false,
                        error: Some(error.to_string()),
                    };
                    let json = serde_json::to_string(&res)
                        .unwrap_or_else(|_| "{\"ok\":false}".to_string());
                    let response = format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
                        json.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    false
                }
            }
        }
        _ => {
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: {conn_header}\r\n\r\n"
                )
                .as_bytes(),
            );
            false
        }
    }
}

fn fetch_characters(paths: &Paths, config: &Config, session_id: &str) -> Vec<CharacterView> {
    let auth_config = AuthConfig::default();
    let http = HttpAuth::new(&auth_config);
    let Ok(chars) = http.characters(session_id) else {
        return Vec::new();
    };
    let usage = UsageStore::load(paths);
    let ordered = usage.order(&chars, config.usage_recent_window_secs, |c| &c.account_id);
    ordered
        .into_iter()
        .map(|c| {
            let u = usage.usage(&c.account_id);
            CharacterView {
                account_id: c.account_id.clone(),
                display_name: c.display_name.clone(),
                last_used: u.last_used,
                use_count: u.count,
            }
        })
        .collect()
}

/// Starts a fresh login flow and returns its authorize URL.
fn new_flow(flow_state: &Arc<Mutex<Option<LoginFlow>>>) -> String {
    let flow = LoginFlow::new(AuthConfig::default());
    let url = flow.authorize_url();
    if let Ok(mut lock) = flow_state.lock() {
        *lock = Some(flow);
    }
    url
}

/// Result of feeding one redirect address to the active login flow.
#[derive(Debug)]
pub(crate) enum LoginOutcome {
    /// The login finished. Holds the JSON the page expects (`session`, `characters`).
    Done(String),
    /// The provider needs the consent page at this URL.
    Navigate(String),
    /// The flow failed or is gone. Holds a message for the user.
    Error(String),
}

/// Advances the active login flow with a redirect address.
///
/// On `Done` the session is saved to the store. On `Navigate` the flow stays
/// active for the next address. On an error the flow is dropped.
pub(crate) fn complete_login(
    paths: &Paths,
    flow_state: &Arc<Mutex<Option<LoginFlow>>>,
    address: &str,
) -> LoginOutcome {
    let Some(mut flow) = flow_state.lock().ok().and_then(|mut g| g.take()) else {
        return LoginOutcome::Error(
            "No login flow active. Click Add Jagex Account first.".to_string(),
        );
    };
    let auth_config = AuthConfig::default();
    let http = HttpAuth::new(&auth_config);
    let action_res = match flow.on_navigation(address) {
        Ok(action) => http.advance(&mut flow, action).map_err(|e| e.to_string()),
        Err(err) => Err(err.to_string()),
    };
    match action_res {
        Ok(Action::Done(session)) => {
            let mut store = SessionStore::load(paths);
            store.upsert(session.clone());
            if let Err(error) = store.save() {
                return LoginOutcome::Error(format!(
                    "Logged in, but the session could not be saved: {error}"
                ));
            }
            let config = Config::load(paths);
            let chars = fetch_characters(paths, &config, &session.session_id);
            let view = SessionView {
                sub: session.sub.clone(),
                display_name: session.display_name.clone(),
                suffix: session.suffix.clone(),
            };
            #[derive(Serialize)]
            struct LoginSuccess {
                ok: bool,
                session: SessionView,
                characters: Vec<CharacterView>,
            }
            let json = serde_json::to_string(&LoginSuccess {
                ok: true,
                session: view,
                characters: chars,
            })
            .unwrap_or_else(|_| "{\"ok\":true}".to_string());
            LoginOutcome::Done(json)
        }
        Ok(Action::Navigate { url }) => {
            if let Ok(mut lock) = flow_state.lock() {
                *lock = Some(flow);
            }
            LoginOutcome::Navigate(url)
        }
        Ok(_) => {
            if let Ok(mut lock) = flow_state.lock() {
                *lock = Some(flow);
            }
            LoginOutcome::Error(
                "That address is not part of the login flow. Paste the full URL from the browser address bar.".to_string(),
            )
        }
        Err(err) => LoginOutcome::Error(err),
    }
}

fn build_state(paths: &Paths, config: Config, wifi: WifiStatus) -> ServerState {
    let runtimes = rustybolt_jdk::discover()
        .into_iter()
        .map(|rt| JavaInfo {
            path: rt.path.display().to_string(),
            version: rt
                .version
                .map(|v| format!("Feature {} ({})", v.feature, v.raw))
                .unwrap_or_else(|| "Unknown".to_string()),
            source: match rt.source {
                rustybolt_jdk::Source::Explicit => "Explicit",
                rustybolt_jdk::Source::JavaHome => "JAVA_HOME",
                rustybolt_jdk::Source::Path => "PATH",
                rustybolt_jdk::Source::SystemLocation => "System",
                rustybolt_jdk::Source::ClientBundle => "Bundled with RuneLite",
            }
            .to_string(),
        })
        .collect();

    let clients = ClientInfo {
        runelite_detected: rustybolt_core::locate_client(ClientKind::RuneLite, &config)
            .map(|p| p.display().to_string()),
        runelite_candidates: rustybolt_core::client_candidates(ClientKind::RuneLite)
            .into_iter()
            .map(|p| p.display().to_string())
            .collect(),
        hdos_detected: rustybolt_core::locate_client(ClientKind::Hdos, &config)
            .map(|p| p.display().to_string()),
        hdos_candidates: rustybolt_core::client_candidates(ClientKind::Hdos)
            .into_iter()
            .map(|p| p.display().to_string())
            .collect(),
    };

    let runelite_plan = compute_preview(paths, &config, ClientKind::RuneLite);
    let store = SessionStore::load(paths);
    let sessions: Vec<SessionView> = store
        .sessions()
        .iter()
        .map(|s| SessionView {
            sub: s.sub.clone(),
            display_name: s.display_name.clone(),
            suffix: s.suffix.clone(),
        })
        .collect();

    let active_session = store.sessions().first();
    let active_sub = active_session.map(|s| s.sub.clone());
    let characters = if let Some(session) = active_session {
        fetch_characters(paths, &config, &session.session_id)
    } else {
        Vec::new()
    };
    let has_session = !sessions.is_empty();

    ServerState {
        config,
        runtimes,
        clients,
        runelite_plan,
        sessions,
        active_sub,
        characters,
        has_session,
        wifi,
        keychain_error: rustybolt_core::keychain_available()
            .err()
            .map(|error| error.to_string()),
    }
}

fn compute_preview(paths: &Paths, config: &Config, kind: ClientKind) -> Option<String> {
    let jar = rustybolt_core::locate_client(kind, config).or_else(|| {
        if kind == ClientKind::RuneLite {
            Some(PathBuf::from(
                "/Applications/RuneLite.app/Contents/Resources/RuneLite.jar",
            ))
        } else {
            None
        }
    })?;

    let template = match kind {
        ClientKind::RuneLite => config.runelite_launch_command.as_deref(),
        ClientKind::Hdos => config.hdos_launch_command.as_deref(),
    };

    let req = LaunchRequest {
        jar: &jar,
        kind,
        credentials: None,
        java: config.java_path.as_deref(),
        template,
        configure: false,
    };

    let plan = rustybolt_core::plan(paths, &req).ok()?;
    let mut parts = vec![plan.program.to_string_lossy().into_owned()];
    parts.extend(plan.args);
    Some(parts.join(" "))
}

pub(crate) fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();

    // Not `cmd /C start`: cmd splits the command at every `&`, so the
    // query string of the authorize URL would be cut after the first pair.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt as _;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let wide = |s: &str| -> Vec<u16> {
            std::ffi::OsStr::new(s)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        };
        let verb = wide("open");
        let target = wide(url);
        // SAFETY: both strings are NUL-terminated and outlive the call.
        unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            );
        }
    }
}
