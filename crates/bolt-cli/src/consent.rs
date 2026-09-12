//! Catches the OAuth consent redirect (`http://localhost/#...`) from the
//! system browser.
//!
//! Jagex registers `http://localhost` with no port, so the listener must own
//! port 80. The tokens arrive in the fragment, which the browser never sends,
//! so the served page forwards `location.hash` with a POST.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tao::event_loop::EventLoopProxy;

use crate::gui::AppEvent;

/// How long the browser gets to finish the consent step.
const TIMEOUT: Duration = Duration::from_secs(300);

const PAGE: &str = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>rustyBolt</title>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self';">
<style>body{font-family:system-ui,sans-serif;background:#15161a;color:#e8e6e1;display:flex;align-items:center;justify-content:center;height:100vh;margin:0}p{font-size:1.1rem}</style>
</head><body><p id="msg">Finishing login&hellip;</p><script>
(async()=>{const m=document.getElementById('msg');try{
const r=await fetch('/callback',{method:'POST',body:location.hash});
m.textContent=r.ok?'Logged in. You can close this tab and return to rustyBolt.':'rustyBolt rejected the login. Return to the launcher.';
}catch(e){m.textContent='Could not reach rustyBolt. Return to the launcher.';}
history.replaceState(null,'',location.pathname);})();
</script></body></html>"##;

/// Owns the port 80 listener for the length of one login.
pub(crate) struct ConsentListener {
    stop: Arc<AtomicBool>,
}

impl Drop for ConsentListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Give port 80 back to the system the moment the flow ends.
        crate::platform::set_login_active(false);
    }
}

/// Starts listening on port 80 for the consent redirect.
///
/// The redirect URL is sent to the event loop as `LoginRedirect`. Returns
/// `None` when port 80 is not available.
pub(crate) fn start(proxy: EventLoopProxy<AppEvent>) -> Option<ConsentListener> {
    // Browsers resolve `localhost` to ::1 or 127.0.0.1; bind whichever work.
    let bind = |port: u16| -> Vec<TcpListener> {
        [format!("127.0.0.1:{port}"), format!("[::1]:{port}")]
            .iter()
            .filter_map(|addr| TcpListener::bind(addr).ok())
            .collect()
    };
    let mut listeners = bind(80);
    if listeners.is_empty() && crate::platform::forward_installed() {
        listeners = bind(crate::platform::FORWARDED_PORT);
        if !listeners.is_empty() {
            crate::platform::set_login_active(true);
        }
    }
    if listeners.is_empty() {
        return None;
    }
    for listener in &listeners {
        let _ = listener.set_nonblocking(true);
    }

    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    std::thread::spawn(move || {
        let deadline = Instant::now() + TIMEOUT;
        while !thread_stop.load(Ordering::SeqCst) && Instant::now() < deadline {
            let mut idle = true;
            for listener in &listeners {
                if let Ok((stream, _)) = listener.accept() {
                    idle = false;
                    if handle(stream, &proxy) {
                        thread_stop.store(true, Ordering::SeqCst);
                    }
                }
            }
            if idle {
                std::thread::sleep(Duration::from_millis(30));
            }
        }
    });

    Some(ConsentListener { stop })
}

/// Serves one request. Returns `true` once the redirect was delivered.
fn handle(mut stream: TcpStream, proxy: &EventLoopProxy<AppEvent>) -> bool {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&buf[..n]);
                if let Some(header_end) = find(&raw, b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&raw[..header_end]).to_string();
                    let length = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if raw.len() >= header_end + 4 + length {
                        break;
                    }
                }
                if raw.len() > 64 * 1024 {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let Some(header_end) = find(&raw, b"\r\n\r\n") else {
        return false;
    };
    let head = String::from_utf8_lossy(&raw[..header_end]).to_string();
    let body = String::from_utf8_lossy(&raw[header_end + 4..]).to_string();
    let mut parts = head.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    match (method, path) {
        ("GET", "/") => {
            respond(&mut stream, "200 OK", "text/html; charset=utf-8", PAGE);
            false
        }
        ("POST", "/callback") => {
            let url = format!("http://localhost/{}", body.trim());
            if bolt_security::is_login_redirect(&url) && body.trim_start().starts_with('#') {
                let _ = proxy.send_event(AppEvent::LoginRedirect(url));
                respond(&mut stream, "200 OK", "text/plain", "ok");
                true
            } else {
                respond(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain",
                    "not a login redirect",
                );
                false
            }
        }
        _ => {
            respond(&mut stream, "404 Not Found", "text/plain", "");
            false
        }
    }
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
