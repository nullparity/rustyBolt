//! The physical memory of the machine, and the heap that fits in it.
//!
//! A fixed 2 GB heap took a 2 GB virtual machine down: the kernel killed
//! the desktop session to make room for the client.

#[cfg(target_os = "macos")]
use std::process::Command;

/// The physical memory in bytes, or `None` when the platform gives no answer.
pub fn total_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        let line = text.lines().find(|line| line.starts_with("MemTotal:"))?;
        let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kib * 1024)
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            dwMemoryLoad: 0,
            ullTotalPhys: 0,
            ullAvailPhys: 0,
            ullTotalPageFile: 0,
            ullAvailPageFile: 0,
            ullTotalVirtual: 0,
            ullAvailVirtual: 0,
            ullAvailExtendedVirtual: 0,
        };
        // SAFETY: `status` is a correctly sized, writable MEMORYSTATUSEX.
        let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
        (ok != 0).then_some(status.ullTotalPhys)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        None
    }
}

/// Reads a JVM size such as `2g`, `512m` or `1048576k` into bytes.
pub fn parse_jvm_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let (digits, unit) = text.split_at(
        text.trim_end_matches(|c: char| c.is_ascii_alphabetic())
            .len(),
    );
    let value: u64 = digits.parse().ok()?;
    let factor = match unit.to_ascii_lowercase().as_str() {
        "" => 1,
        "k" => 1 << 10,
        "m" => 1 << 20,
        "g" => 1 << 30,
        _ => return None,
    };
    Some(value * factor)
}

/// The largest heap that leaves the rest of the machine alone: half of the
/// physical memory, rounded down to 256 MB, never below 512 MB. Written as a
/// JVM size in megabytes.
pub fn heap_cap(total_bytes: u64) -> String {
    const STEP: u64 = 256 << 20;
    const FLOOR: u64 = 512 << 20;
    let half = (total_bytes / 2 / STEP) * STEP;
    format!("{}m", half.max(FLOOR) >> 20)
}

/// Applies [`heap_cap`] to a configured heap. A maximum above the cap comes
/// down to the cap, and a minimum above the new maximum goes away so the JVM
/// starts small.
pub fn fit_heap(
    heap_min: Option<String>,
    heap_max: Option<String>,
    total_bytes: Option<u64>,
) -> (Option<String>, Option<String>) {
    let Some(total) = total_bytes else {
        return (heap_min, heap_max);
    };
    let cap = heap_cap(total);
    let cap_bytes = parse_jvm_size(&cap).unwrap_or(u64::MAX);
    let over = |size: &Option<String>| {
        size.as_deref()
            .and_then(parse_jvm_size)
            .is_some_and(|bytes| bytes > cap_bytes)
    };
    let heap_max = if over(&heap_max) {
        Some(cap.clone())
    } else {
        heap_max
    };
    let heap_min = if over(&heap_min) { None } else { heap_min };
    (heap_min, heap_max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_parse_with_jvm_units() {
        assert_eq!(parse_jvm_size("2g"), Some(2 << 30));
        assert_eq!(parse_jvm_size("512M"), Some(512 << 20));
        assert_eq!(parse_jvm_size("1024k"), Some(1 << 20));
        assert_eq!(parse_jvm_size("x"), None);
    }

    #[test]
    fn the_cap_is_half_the_machine_with_a_floor() {
        assert_eq!(heap_cap(2 << 30), "1024m");
        assert_eq!(heap_cap(4 << 30), "2048m");
        assert_eq!(heap_cap(1 << 30), "512m");
        assert_eq!(heap_cap(3 << 30), "1536m");
    }

    #[test]
    fn a_heap_that_fits_stays_and_one_that_does_not_comes_down() {
        let two_g = || (Some("2g".to_string()), Some("2g".to_string()));
        let (min, max) = fit_heap(two_g().0, two_g().1, Some(16 << 30));
        assert_eq!((min.as_deref(), max.as_deref()), (Some("2g"), Some("2g")));
        let (min, max) = fit_heap(two_g().0, two_g().1, Some(2 << 30));
        assert_eq!((min.as_deref(), max.as_deref()), (None, Some("1024m")));
        let (min, max) = fit_heap(two_g().0, two_g().1, None);
        assert_eq!((min.as_deref(), max.as_deref()), (Some("2g"), Some("2g")));
    }

    #[test]
    fn this_machine_reports_its_memory() {
        assert!(total_bytes().is_some_and(|bytes| bytes > 256 << 20));
    }
}
