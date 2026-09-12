//! The `configure` command.
//!
//! Spawns a lightweight local HTTP server and opens the configuration dashboard
//! in the default web browser on macOS, Linux, and Windows.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use bolt_core::{ClientKind, Config, LaunchRequest, Paths};
use serde::Serialize;

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

#[derive(Serialize)]
struct ServerState {
    config: Config,
    runtimes: Vec<JavaInfo>,
    clients: ClientInfo,
    runelite_plan: Option<String>,
}

#[derive(Serialize)]
struct SaveResponse {
    ok: bool,
    plan: Option<String>,
}

/// Runs `rustybolt configure`.
pub(crate) fn run(args: &[String]) -> Result<(), CliError> {
    no_arguments(args)?;

    let paths = Paths::resolve()?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}/");

    println!("rustybolt: configuration server running at {url}");
    println!("Opening your default web browser. Press Ctrl+C to close.");

    open_browser(&url);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

                if let Some(should_exit) = handle_client(&mut stream, &paths) {
                    if should_exit {
                        println!("rustybolt: configuration server stopped.");
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("rustybolt: connection error: {e}");
            }
        }
    }

    Ok(())
}

fn handle_client(stream: &mut TcpStream, paths: &Paths) -> Option<bool> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).ok()? == 0 {
        return None;
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;

    let mut content_length = 0usize;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).ok()? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(stripped) = lower.strip_prefix("content-length:") {
            content_length = stripped.trim().parse().unwrap_or(0);
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        let _ = reader.read_exact(&mut body);
    }

    match (method, path) {
        ("GET", "/" | "/index.html") => {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                HTML_PAGE.len(),
                HTML_PAGE
            );
            let _ = stream.write_all(response.as_bytes());
            Some(false)
        }
        ("GET", "/icon.svg" | "/favicon.ico") => {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                ICON_SVG.len(),
                ICON_SVG
            );
            let _ = stream.write_all(response.as_bytes());
            Some(false)
        }
        ("GET", "/api/state") => {
            let config = Config::load(paths);
            let state = build_state(paths, config);
            let json = serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                json.len(),
                json
            );
            let _ = stream.write_all(response.as_bytes());
            Some(false)
        }
        ("POST", "/api/save") => {
            if let Ok(config) = serde_json::from_slice::<Config>(&body) {
                let _ = config.save(paths);
                let plan = compute_preview(paths, &config, ClientKind::RuneLite);
                let res = SaveResponse { ok: true, plan };
                let json =
                    serde_json::to_string(&res).unwrap_or_else(|_| "{\"ok\":true}".to_string());
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    json.len(),
                    json
                );
                let _ = stream.write_all(response.as_bytes());
            } else {
                let _ = stream.write_all(
                    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
            Some(false)
        }
        ("POST", "/api/shutdown") => {
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}");
            Some(true)
        }
        _ => {
            let _ = stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            Some(false)
        }
    }
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

    ServerState {
        config,
        runtimes,
        clients,
        runelite_plan,
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
