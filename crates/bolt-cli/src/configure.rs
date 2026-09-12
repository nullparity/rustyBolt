//! The `configure` command and native local application server.
//!
//! Spawns a lightweight local HTTP server and opens the launcher dashboard
//! in the default web browser on macOS, Linux, and Windows.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use bolt_core::{
    Action, AuthConfig, ClientKind, Config, HttpAuth, LaunchRequest, LoginFlow, Paths,
    SessionStore, UsageStore,
};
use serde::{Deserialize, Serialize};

use crate::web::HTML_PAGE;
use crate::{no_arguments, CliError};

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
    no_arguments(args)?;

    let paths = Arc::new(Paths::resolve()?);
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}/");

    println!("rustybolt: launcher server running at {url}");
    println!("Opening your default web browser. Press Ctrl+C to close.");

    open_browser(&url);

    let running = Arc::new(AtomicBool::new(true));
    let flow_state = Arc::new(Mutex::new(None));

    for stream in listener.incoming() {
        if !running.load(Ordering::SeqCst) {
            break;
        }
        match stream {
            Ok(stream) => {
                let paths = Arc::clone(&paths);
                let running = Arc::clone(&running);
                let flow_state = Arc::clone(&flow_state);
                thread::spawn(move || {
                    handle_connection(stream, &paths, &running, &flow_state, port);
                });
            }
            Err(e) => {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                eprintln!("rustybolt: connection error: {e}");
            }
        }
    }

    println!("rustybolt: launcher server stopped.");
    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    paths: &Paths,
    running: &AtomicBool,
    flow_state: &Arc<Mutex<Option<LoginFlow>>>,
    port: u16,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let mut reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(_) => return,
    };

    loop {
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
            break;
        }

        let mut parts = request_line.split_whitespace();
        let Some(method) = parts.next() else { break };
        let Some(raw_path) = parts.next() else { break };
        let mut path_parts = raw_path.splitn(2, '?');
        let path = path_parts.next().unwrap_or(raw_path);
        let query = path_parts.next().unwrap_or("");

        let mut content_length = 0usize;
        let mut keep_alive = false;
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" || line == "\n" {
                break;
            }
            let lower = line.to_ascii_lowercase();
            if let Some(stripped) = lower.strip_prefix("content-length:") {
                content_length = stripped.trim().parse().unwrap_or(0);
            } else if lower.starts_with("connection:") && lower.contains("keep-alive") {
                keep_alive = true;
            }
        }

        let mut body = vec![0u8; content_length];
        if content_length > 0 && reader.read_exact(&mut body).is_err() {
            break;
        }

        let req = HttpRequest {
            method,
            path,
            query,
            body: &body,
            keep_alive,
        };
        let should_exit = respond(&req, &mut stream, paths, flow_state);
        let _ = stream.flush();

        if should_exit {
            running.store(false, Ordering::SeqCst);
            let _ = TcpStream::connect(("127.0.0.1", port));
            break;
        }

        if !keep_alive {
            break;
        }
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

struct HttpRequest<'a> {
    method: &'a str,
    path: &'a str,
    query: &'a str,
    body: &'a [u8],
    keep_alive: bool,
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
    match (req.method, req.path) {
        ("GET", "/" | "/index.html") => {
            let config = Config::load(paths);
            let state = build_state(paths, config);
            let state_json = serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
            let html = HTML_PAGE.replace("<!--LOGO_SVG-->", ICON_SVG).replace(
                "/*INITIAL_STATE*/",
                &format!("window.INITIAL_STATE = {state_json};"),
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{html}",
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
            let state = build_state(paths, config);
            let json = serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
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
            let config = AuthConfig::default();
            let flow = LoginFlow::new(config);
            let url = flow.authorize_url();
            if let Ok(mut lock) = flow_state.lock() {
                *lock = Some(flow);
            }
            let json = format!("{{\"ok\":true,\"url\":\"{url}\"}}");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: {conn_header}\r\n\r\n{json}",
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
            let mut flow_opt = flow_state.lock().ok().and_then(|mut g| g.take());
            let (success, result_json) = if let Some(mut flow) = flow_opt.take() {
                let auth_config = AuthConfig::default();
                let http = HttpAuth::new(&auth_config);
                let action_res = match flow.on_navigation(&address) {
                    Ok(action) => http.advance(&mut flow, action).map_err(|e| e.to_string()),
                    Err(err) => Err(err.to_string()),
                };
                match action_res {
                    Ok(Action::Done(session)) => {
                        let mut store = SessionStore::load(paths);
                        store.upsert(session.clone());
                        let _ = store.save();
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
                        (true, json)
                    }
                    Ok(_) => {
                        if let Ok(mut lock) = flow_state.lock() {
                            *lock = Some(flow);
                        }
                        (
                            false,
                            "{\"ok\":false,\"error\":\"More verification steps needed. Please paste final redirect URL.\"}".to_string(),
                        )
                    }
                    Err(err) => (false, format!("{{\"ok\":false,\"error\":\"{err}\"}}")),
                }
            } else {
                (
                    false,
                    "{\"ok\":false,\"error\":\"No login flow active. Click Start Login first.\"}"
                        .to_string(),
                )
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

fn build_state(paths: &Paths, config: Config) -> ServerState {
    let runtimes = bolt_jdk::discover()
        .into_iter()
        .map(|rt| JavaInfo {
            path: rt.path.display().to_string(),
            version: rt
                .version
                .map(|v| format!("Feature {} ({})", v.feature, v.raw))
                .unwrap_or_else(|| "Unknown".to_string()),
            source: match rt.source {
                bolt_jdk::Source::Explicit => "Explicit",
                bolt_jdk::Source::JavaHome => "JAVA_HOME",
                bolt_jdk::Source::Path => "PATH",
                bolt_jdk::Source::SystemLocation => "System",
            }
            .to_string(),
        })
        .collect();

    let clients = ClientInfo {
        runelite_detected: bolt_core::locate_client(ClientKind::RuneLite, &config)
            .map(|p| p.display().to_string()),
        runelite_candidates: bolt_core::client_candidates(ClientKind::RuneLite)
            .into_iter()
            .map(|p| p.display().to_string())
            .collect(),
        hdos_detected: bolt_core::locate_client(ClientKind::Hdos, &config)
            .map(|p| p.display().to_string()),
        hdos_candidates: bolt_core::client_candidates(ClientKind::Hdos)
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
    }
}

fn compute_preview(paths: &Paths, config: &Config, kind: ClientKind) -> Option<String> {
    let jar = bolt_core::locate_client(kind, config).or_else(|| {
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

    let plan = bolt_core::plan(paths, &req).ok()?;
    let mut parts = vec![plan.program.to_string_lossy().into_owned()];
    parts.extend(plan.args);
    Some(parts.join(" "))
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();

    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
}
