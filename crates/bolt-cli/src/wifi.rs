//! Wi-Fi keepalive manager for game tick latency stability.

use std::io::{BufRead, BufReader};
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use pinger::{get_pinger, run_ping, PingOptions, PingResult};

/// UDP discard port; nothing listens, but the frame still wakes the radio.
const KEEPALIVE_PORT: u16 = 9;
const KEEPALIVE_INTERVAL: Duration = Duration::from_millis(10);
const LATENCY_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WifiStatus {
    pub enabled: bool,
    pub gateway: Option<String>,
    pub latency_ms: Option<f64>,
    pub error: Option<String>,
}

pub(crate) fn detect_gateway() -> Option<IpAddr> {
    if let Ok(gw) = default_net::get_default_gateway() {
        return Some(gw.ip_addr);
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("route")
            .args(["-n", "get", "default"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let trimmed = line.trim();
                if let Some(ip_str) = trimmed.strip_prefix("gateway:") {
                    if let Ok(ip) = ip_str.trim().parse::<IpAddr>() {
                        return Some(ip);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = Command::new("ip")
            .args(["route", "show", "default"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = text.split_whitespace().collect();
            if let Some(pos) = parts.iter().position(|&p| p == "via") {
                if let Some(ip_str) = parts.get(pos + 1) {
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        return Some(ip);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = Command::new("route").args(["print", "0.0.0.0"]).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 && parts[0] == "0.0.0.0" && parts[1] == "0.0.0.0" {
                    if let Ok(ip) = parts[2].parse::<IpAddr>() {
                        return Some(ip);
                    }
                }
            }
        }
    }

    None
}

#[derive(Clone)]
pub(crate) struct WifiManager {
    enabled: Arc<AtomicBool>,
    status: Arc<Mutex<WifiStatus>>,
}

impl WifiManager {
    pub(crate) fn new(initial_enabled: bool) -> Self {
        let manager = Self {
            enabled: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new(WifiStatus {
                enabled: false,
                gateway: None,
                latency_ms: None,
                error: None,
            })),
        };

        if initial_enabled {
            manager.set_enabled(true);
        }

        manager
    }

    pub(crate) fn status(&self) -> WifiStatus {
        self.status.lock().unwrap().clone()
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub(crate) fn toggle(&self) -> bool {
        let next = !self.is_enabled();
        self.set_enabled(next);
        next
    }

    pub(crate) fn set_enabled(&self, enable: bool) {
        if enable {
            if self
                .enabled
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                let enabled_flag = Arc::clone(&self.enabled);
                let status_arc = Arc::clone(&self.status);
                thread::spawn(move || {
                    run_wifi_worker(enabled_flag, status_arc);
                });
            }
        } else if self
            .enabled
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            if let Ok(mut st) = self.status.lock() {
                st.enabled = false;
                st.latency_ms = None;
            }
        }
    }
}

fn run_wifi_worker(enabled: Arc<AtomicBool>, status: Arc<Mutex<WifiStatus>>) {
    let gw_ip = match detect_gateway() {
        Some(ip) => ip,
        None => {
            if let Ok(mut st) = status.lock() {
                st.enabled = false;
                st.error = Some("No default gateway detected".to_string());
            }
            enabled.store(false, Ordering::SeqCst);
            return;
        }
    };

    let gw_str = gw_ip.to_string();
    if let Ok(mut st) = status.lock() {
        st.enabled = true;
        st.gateway = Some(gw_str);
        st.error = None;
    }

    // Send a tiny UDP datagram to the gateway's discard port every 10ms.
    // The payload is irrelevant; the point is to keep the radio out of
    // 802.11 power-save so game ticks do not see wake-up jitter.
    // Unlike ICMP this needs no privileges and no external binary.
    let bind_addr: SocketAddr = match gw_ip {
        IpAddr::V4(_) => "0.0.0.0:0".parse().unwrap(),
        IpAddr::V6(_) => "[::]:0".parse().unwrap(),
    };
    let sock = match UdpSocket::bind(bind_addr) {
        Ok(s) => s,
        Err(e) => {
            if let Ok(mut st) = status.lock() {
                st.enabled = false;
                st.error = Some(format!("UDP bind failed: {e}"));
            }
            enabled.store(false, Ordering::SeqCst);
            return;
        }
    };
    let target = SocketAddr::new(gw_ip, KEEPALIVE_PORT);

    // Separate slow ICMP probe for the latency readout in the UI. Uses the
    // system ping binary via `pinger` (locale-pinned, cross-platform parsing).
    {
        let enabled = Arc::clone(&enabled);
        let status = Arc::clone(&status);
        let gw_str = gw_ip.to_string();
        thread::spawn(move || run_latency_probe(gw_str, enabled, status));
    }

    while enabled.load(Ordering::SeqCst) {
        let start = Instant::now();
        // Errors (e.g. ICMP port-unreachable surfacing on the socket) are
        // expected and harmless; the frame still went over the air.
        let _ = sock.send_to(&[0u8], target);
        let elapsed = start.elapsed();
        if elapsed < KEEPALIVE_INTERVAL {
            thread::sleep(KEEPALIVE_INTERVAL - elapsed);
        }
    }

    if let Ok(mut st) = status.lock() {
        st.enabled = false;
        st.latency_ms = None;
    }
}

fn run_latency_probe(target: String, enabled: Arc<AtomicBool>, status: Arc<Mutex<WifiStatus>>) {
    // Drive the ping child ourselves rather than via `pinger::ping` so we can
    // kill it on toggle-off; pinger's own reader would block in
    // `wait_with_output` forever on an interval ping that never exits.
    let opts = PingOptions::new(target, LATENCY_INTERVAL, None);
    let pinger = match get_pinger(opts) {
        Ok(p) => p,
        Err(e) => {
            if let Ok(mut st) = status.lock() {
                st.error = Some(format!("latency probe unavailable: {e}"));
            }
            return;
        }
    };
    let (cmd, args) = pinger.ping_args();
    let mut child = match run_ping(cmd, args) {
        Ok(c) => c,
        Err(e) => {
            if let Ok(mut st) = status.lock() {
                st.error = Some(format!("latency probe unavailable: {e}"));
            }
            return;
        }
    };
    let parse = pinger.parse_fn();

    let (tx, rx) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Some(r) = parse(line) {
                    if tx.send(r).is_err() {
                        break;
                    }
                }
            }
        });
    }

    while enabled.load(Ordering::SeqCst) {
        // Poll so we notice `enabled` flipping without waiting a full interval.
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(PingResult::Pong(rtt, _)) => {
                if let Ok(mut st) = status.lock() {
                    st.latency_ms = Some(rtt.as_secs_f64() * 1000.0);
                }
            }
            Ok(PingResult::Timeout(_)) => {
                if let Ok(mut st) = status.lock() {
                    st.latency_ms = None;
                }
            }
            Ok(PingResult::Unknown(_)) | Err(RecvTimeoutError::Timeout) => {}
            Ok(PingResult::PingExited(..)) | Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_gateway() {
        let gw = detect_gateway();
        assert!(gw.is_some());
    }

    #[test]
    fn test_wifi_manager_lifecycle() {
        let manager = WifiManager::new(false);
        assert!(!manager.is_enabled());

        manager.set_enabled(true);
        assert!(manager.is_enabled());

        // First ICMP reply lands well within a couple of seconds on a LAN.
        let deadline = Instant::now() + Duration::from_secs(3);
        while manager.status().latency_ms.is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        let status = manager.status();
        assert!(status.enabled);
        assert!(status.gateway.is_some());
        assert!(
            status.latency_ms.is_some(),
            "no latency sample: {:?}",
            status.error
        );

        manager.set_enabled(false);
        assert!(!manager.is_enabled());
    }
}
