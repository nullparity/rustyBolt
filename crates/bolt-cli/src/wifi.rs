//! Wi-Fi keepalive manager for game tick latency stability.

use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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

pub(crate) fn parse_ping_latency(line: &str) -> Option<f64> {
    if let Some(pos) = line.find("time=") {
        let remainder = &line[pos + 5..];
        let num_str: String = remainder
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if let Ok(val) = num_str.parse::<f64>() {
            return Some(val);
        }
    }
    if let Some(pos) = line.find("time<") {
        let remainder = &line[pos + 5..];
        let num_str: String = remainder
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if let Ok(val) = num_str.parse::<f64>() {
            return Some(val);
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
        st.gateway = Some(gw_str.clone());
        st.error = None;
    }

    let mut streaming_child: Option<Child> = None;

    #[cfg(target_os = "macos")]
    {
        if let Ok(child) = Command::new("ping")
            .args(["-i", "0.1", &gw_str])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            streaming_child = Some(child);
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(child) = Command::new("ping")
            .args(["-i", "0.1", &gw_str])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            streaming_child = Some(child);
        }
    }

    if let Some(mut child) = streaming_child {
        if let Some(stdout) = child.stdout.take() {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            while enabled.load(Ordering::SeqCst) {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Some(lat) = parse_ping_latency(&line) {
                            if let Ok(mut st) = status.lock() {
                                st.latency_ms = Some(lat);
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    } else {
        while enabled.load(Ordering::SeqCst) {
            let start = Instant::now();
            let mut success = false;

            #[cfg(windows)]
            {
                let res = Command::new("ping")
                    .args(["-n", "1", "-w", "80", &gw_str])
                    .output();
                if let Ok(out) = res {
                    let text = String::from_utf8_lossy(&out.stdout);
                    if let Some(lat) = parse_ping_latency(&text) {
                        if let Ok(mut st) = status.lock() {
                            st.latency_ms = Some(lat);
                        }
                        success = true;
                    }
                }
            }

            #[cfg(not(windows))]
            {
                let res = Command::new("ping")
                    .args(["-c", "1", "-W", "1", &gw_str])
                    .output();
                if let Ok(out) = res {
                    let text = String::from_utf8_lossy(&out.stdout);
                    if let Some(lat) = parse_ping_latency(&text) {
                        if let Ok(mut st) = status.lock() {
                            st.latency_ms = Some(lat);
                        }
                        success = true;
                    }
                }
            }

            if !success {
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                if let Ok(mut st) = status.lock() {
                    st.latency_ms = Some(elapsed);
                }
            }

            let elapsed = start.elapsed();
            if elapsed < Duration::from_millis(100) {
                thread::sleep(Duration::from_millis(100) - elapsed);
            }
        }
    }

    if let Ok(mut st) = status.lock() {
        st.enabled = false;
        st.latency_ms = None;
    }
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
    fn test_parse_ping_latency() {
        let line = "64 bytes from 192.168.1.1: icmp_seq=1 ttl=64 time=3.576 ms";
        assert_eq!(parse_ping_latency(line), Some(3.576));
        let win_line = "Reply from 192.168.1.1: bytes=32 time<1ms TTL=64";
        assert_eq!(parse_ping_latency(win_line), Some(1.0));
    }

    #[test]
    fn test_wifi_manager_lifecycle() {
        let manager = WifiManager::new(false);
        assert!(!manager.is_enabled());

        manager.set_enabled(true);
        assert!(manager.is_enabled());

        thread::sleep(Duration::from_millis(400));
        let status = manager.status();
        assert!(status.enabled);
        assert!(status.gateway.is_some());

        manager.set_enabled(false);
        assert!(!manager.is_enabled());
    }
}
