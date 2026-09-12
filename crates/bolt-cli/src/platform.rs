//! Operating system integration for the browser login: the `jagex:` URL
//! scheme, and the ability to receive `http://localhost` on port 80.

use std::net::TcpListener;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
use std::process::Command;

/// Port that the operating system forwards loopback port 80 to, when the
/// launcher cannot bind port 80 itself (macOS).
pub(crate) const FORWARDED_PORT: u16 = 21080;

/// The pf anchor file that `macos/install-login-redirect.sh` writes.
#[cfg(target_os = "macos")]
const ANCHOR_FILE: &str = "/etc/pf.anchors/net.runelite.rustybolt";

/// Directory the macOS forward daemon watches. A file named `login-active`
/// inside it turns the port 80 forward on; its absence turns it off.
#[cfg(target_os = "macos")]
const FLAG_DIR: &str = "/Users/Shared/rustyBolt";

/// Whether the consent redirect can be received right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Readiness {
    /// Port 80 (or the forward to [`FORWARDED_PORT`]) is ours to use.
    Ready,
    /// One-time privileged setup makes it work. Holds a user-facing reason.
    NeedsSetup(String),
    /// Another program holds the port; the browser flow cannot work now.
    PortInUse,
}

/// Reports whether the login can run in the system browser.
///
/// That needs the `jagex:` URL scheme to route back to this process.
/// `RUSTYBOLT_LOGIN=window|browser` overrides the detection.
pub(crate) fn browser_login_available() -> bool {
    match std::env::var("RUSTYBOLT_LOGIN").as_deref() {
        Ok("browser") => return true,
        Ok("window") => return false,
        _ => {}
    }
    #[cfg(target_os = "macos")]
    {
        // Launch Services only knows the scheme of an app bundle.
        std::env::current_exe()
            .map(|exe| exe.to_string_lossy().contains("/Contents/MacOS/"))
            .unwrap_or(false)
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-mime")
            .args(["query", "default", "x-scheme-handler/jagex"])
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).contains("rustybolt"))
            .unwrap_or(false)
    }
    #[cfg(target_os = "windows")]
    {
        // The per-user registry keys are written by `claim_jagex_scheme`.
        true
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        false
    }
}

/// Makes this app the default handler of the `jagex:` scheme.
///
/// The official Jagex Launcher claims the same scheme, so the system must be
/// told which app gets the login redirect before each browser login.
pub(crate) fn claim_jagex_scheme() {
    #[cfg(target_os = "macos")]
    {
        use core_foundation::base::TCFType;
        use core_foundation::string::{CFString, CFStringRef};

        #[link(name = "CoreServices", kind = "framework")]
        extern "C" {
            fn LSSetDefaultHandlerForURLScheme(scheme: CFStringRef, bundle_id: CFStringRef) -> i32;
        }

        let scheme = CFString::new("jagex");
        let bundle_id = CFString::new("net.runelite.rustybolt");
        // SAFETY: both arguments are valid CFStrings that outlive the call.
        let status = unsafe {
            LSSetDefaultHandlerForURLScheme(
                scheme.as_concrete_TypeRef(),
                bundle_id.as_concrete_TypeRef(),
            )
        };
        if status != 0 {
            eprintln!("rustybolt: could not claim the jagex: URL scheme (status {status})");
        }
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-mime")
            .args(["default", "rustybolt.desktop", "x-scheme-handler/jagex"])
            .status();
    }
    #[cfg(target_os = "windows")]
    {
        // HKCU\Software\Classes wins over the machine-wide registration of
        // the official launcher, and needs no elevation.
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let command = format!("\"{}\" \"%1\"", exe.display());
        let key = r"HKCU\Software\Classes\jagex";
        let steps: [Vec<&str>; 3] = [
            vec!["add", key, "/ve", "/d", "URL:Jagex Protocol", "/f"],
            vec!["add", key, "/v", "URL Protocol", "/d", "", "/f"],
            vec![
                "add",
                r"HKCU\Software\Classes\jagex\shell\open\command",
                "/ve",
                "/d",
                &command,
                "/f",
            ],
        ];
        for args in steps {
            let _ = Command::new("reg").args(args).status();
        }
    }
}

/// Checks whether the consent redirect on `http://localhost` can reach us.
pub(crate) fn readiness() -> Readiness {
    match TcpListener::bind("127.0.0.1:80") {
        Ok(_) => return Readiness::Ready,
        Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => return Readiness::PortInUse,
        Err(_) => {}
    }
    // Port 80 is privileged here.
    if forward_installed() {
        return match TcpListener::bind(("127.0.0.1", FORWARDED_PORT)) {
            Ok(_) => Readiness::Ready,
            Err(_) => Readiness::PortInUse,
        };
    }
    Readiness::NeedsSetup(setup_reason().to_string())
}

/// Reports whether the one-time setup of this platform is present.
pub(crate) fn forward_installed() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::path::Path::new(ANCHOR_FILE).exists()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// What the setup dialog tells the user the privileged step does.
fn setup_reason() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macOS reserves port 80. rustyBolt installs a small forwarding rule that \
sends localhost port 80 to rustyBolt, only while a login is in progress."
    }
    #[cfg(target_os = "linux")]
    {
        "Linux reserves port 80. rustyBolt grants itself permission to listen on \
low ports (cap_net_bind_service). Nothing else changes."
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        "Port 80 is not available to rustyBolt on this system."
    }
}

/// Runs the one-time privileged setup through the native elevation prompt.
pub(crate) fn install_forward() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let script = bundled_resource("install-login-redirect.sh")
            .ok_or("the setup script is missing from the app bundle")?;
        let shell = format!(
            "/bin/sh {} install",
            quote_for_applescript(&script.to_string_lossy())
        );
        let status = Command::new("osascript")
            .args([
                "-e",
                &format!("do shell script \"{shell}\" with administrator privileges"),
            ])
            .status()
            .map_err(|e| format!("cannot run osascript: {e}"))?;
        if !status.success() {
            return Err("setup was cancelled or failed".to_string());
        }
        if !forward_installed() {
            return Err("setup finished but the forwarding rule is missing".to_string());
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let status = Command::new("pkexec")
            .arg("setcap")
            .arg("cap_net_bind_service=+ep")
            .arg(&exe)
            .status()
            .map_err(|e| format!("cannot run pkexec: {e}"))?;
        if !status.success() {
            return Err("setup was cancelled or failed".to_string());
        }
        // The capability applies to new processes only.
        Err("setup done. Restart rustyBolt to finish logging in through the browser.".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err("no setup is available on this system".to_string())
    }
}

/// Turns the macOS port 80 forward on or off. A no-op elsewhere.
pub(crate) fn set_login_active(active: bool) {
    #[cfg(target_os = "macos")]
    {
        let flag = PathBuf::from(FLAG_DIR).join("login-active");
        if active {
            let _ = std::fs::create_dir_all(FLAG_DIR);
            let _ = std::fs::write(&flag, b"");
        } else {
            let _ = std::fs::remove_file(&flag);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = active;
    }
}

/// A file from the app bundle's `Resources` directory (macOS).
#[cfg(target_os = "macos")]
fn bundled_resource(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let resources = exe.parent()?.parent()?.join("Resources").join(name);
    resources.exists().then_some(resources)
}

#[cfg(target_os = "macos")]
fn quote_for_applescript(path: &str) -> String {
    // The path lands inside an AppleScript string that becomes a shell
    // command: shell-quote it, then escape for the AppleScript literal.
    let shell = format!("'{}'", path.replace('\'', "'\\''"));
    shell.replace('\\', "\\\\").replace('"', "\\\"")
}
