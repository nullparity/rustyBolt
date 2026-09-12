//! JVM tuning flags and the file system helpers for a tuned launch.
//!
//! The layer is pure. It takes a Java feature number and it returns a flag list.
//! The caller supplies the feature number. This module never reads a Java version,
//! and it never guesses.
//!
//! Two flag groups depend on the feature number. `-XX:+ZGenerational` is valid
//! below feature 24 only. The compact object headers, the native access, the
//! string deduplication, and the AOT cache need feature 24 or newer.

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Feature number of the first Java release with the generational-only ZGC.
const GENERATIONAL_ONLY_ZGC: u32 = 24;

/// Garbage collector selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Gc {
    /// No collector flag. The JVM picks the default.
    #[default]
    Default,
    /// The Z garbage collector.
    Z,
    /// The G1 garbage collector.
    G1,
    /// The parallel garbage collector.
    Parallel,
}

/// AOT cache direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AotMode {
    /// Read an existing cache. The flag is `-XX:AOTCache=<path>`.
    Load,
    /// Write a cache on exit. The flag is `-XX:AOTCacheOutput=<path>`.
    Record,
}

/// An AOT cache file and its direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AotCache {
    pub path: PathBuf,
    pub mode: AotMode,
}

/// The complete JVM tuning of one launch.
///
/// `Default` is the empty tuning. Every optional field is off, and `gc` is
/// `Gc::Default`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tuning {
    /// The initial heap size. The value `2g` gives `-Xms2g`.
    pub heap_min: Option<String>,
    /// The maximum heap size. The value `2g` gives `-Xmx2g`.
    pub heap_max: Option<String>,
    /// The thread stack size. The value `2m` gives `-Xss2m`.
    pub stack_size: Option<String>,
    /// The garbage collector.
    pub gc: Gc,
    /// Ask for compact object headers. Feature 24 and newer only.
    pub compact_object_headers: bool,
    /// Ask for string deduplication. Feature 24 and newer only.
    pub string_deduplication: bool,
    /// Give the unnamed module native access. Feature 24 and newer only.
    pub native_access: bool,
    /// Open a named module package. Each name gives one `--add-opens` flag.
    pub add_opens: Vec<String>,
    /// The AOT cache, when the launch uses one. Feature 24 and newer only.
    pub aot_cache: Option<AotCache>,
    /// The directory for the GC log. The log stays off when this is `None`.
    pub gc_log: Option<PathBuf>,
    /// Extra flags. The function adds them last.
    pub extra: Vec<String>,
}

impl Tuning {
    /// Builds the JVM flag list for one Java feature number.
    ///
    /// The order is fixed: heap minimum, heap maximum, GC flags, stack size,
    /// feature gated flags, added opens, AOT cache, GC log, extras.
    pub fn flags(&self, feature: u32) -> Vec<String> {
        let mut flags = Vec::new();

        if let Some(min) = &self.heap_min {
            flags.push(format!("-Xms{min}"));
        }
        if let Some(max) = &self.heap_max {
            flags.push(format!("-Xmx{max}"));
        }

        match self.gc {
            Gc::Default => {}
            Gc::Z => {
                flags.push("-XX:+UseZGC".to_string());
                if feature < GENERATIONAL_ONLY_ZGC {
                    flags.push("-XX:+ZGenerational".to_string());
                }
            }
            Gc::G1 => flags.push("-XX:+UseG1GC".to_string()),
            Gc::Parallel => flags.push("-XX:+UseParallelGC".to_string()),
        }

        if let Some(stack) = &self.stack_size {
            flags.push(format!("-Xss{stack}"));
        }

        if self.compact_object_headers && feature >= GENERATIONAL_ONLY_ZGC {
            flags.push("-XX:+UseCompactObjectHeaders".to_string());
        }
        if self.native_access && feature >= GENERATIONAL_ONLY_ZGC {
            flags.push("--enable-native-access=ALL-UNNAMED".to_string());
        }
        if self.string_deduplication && feature >= GENERATIONAL_ONLY_ZGC {
            flags.push("-XX:+UseStringDeduplication".to_string());
        }

        for package in &self.add_opens {
            flags.push(format!("--add-opens={package}=ALL-UNNAMED"));
        }

        if feature >= GENERATIONAL_ONLY_ZGC {
            if let Some(cache) = &self.aot_cache {
                let path = cache.path.display();
                match cache.mode {
                    AotMode::Load => flags.push(format!("-XX:AOTCache={path}")),
                    AotMode::Record => flags.push(format!("-XX:AOTCacheOutput={path}")),
                }
            }
        }

        if let Some(dir) = &self.gc_log {
            let file = dir.join("gc-%p.log");
            flags.push(format!(
                "-Xlog:gc*:file={}:time,uptime:filecount=3,filesize=10m",
                file.display()
            ));
        }

        flags.extend(self.extra.iter().cloned());
        flags
    }
}

/// The function sorts by modified time, newest first. A file that does not match
/// the prefix or the `.jar` suffix is ignored. It returns `None` when the
/// directory holds no match.
pub fn newest_jar(dir: &Path, prefix: &str) -> Option<PathBuf> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name,
            None => continue,
        };
        if !name.starts_with(prefix) || !name.ends_with(".jar") {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let modified = match entry.metadata().and_then(|meta| meta.modified()) {
            Ok(modified) => modified,
            Err(_) => continue,
        };
        match &newest {
            Some((best, _)) if *best >= modified => {}
            _ => newest = Some((modified, path)),
        }
    }
    newest.map(|(_, path)| path)
}

/// The file name is the jar stem plus `.aot`. The mode is `Load` when the file
/// exists, and `Record` when it does not.
pub fn aot_cache_for(cache_dir: &Path, client_jar: &Path) -> AotCache {
    let stem = client_jar
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("client");
    let path = cache_dir.join(format!("{stem}.aot"));
    let mode = if path.exists() {
        AotMode::Load
    } else {
        AotMode::Record
    };
    AotCache { path, mode }
}

/// Removes every `*.aot` of a directory that is not `keep`.
///
/// The function returns the removed count.
pub fn prune_aot_caches(cache_dir: &Path, keep: &Path) -> io::Result<usize> {
    let keep = std::fs::canonicalize(keep).unwrap_or_else(|_| keep.to_path_buf());
    let mut removed = 0;
    for entry in std::fs::read_dir(cache_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("aot") {
            continue;
        }
        let resolved = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if resolved == keep {
            continue;
        }
        std::fs::remove_file(&path)?;
        removed += 1;
    }
    Ok(removed)
}

/// Makes a hard link to `java` named `name` inside `dir`, and returns the link path.
///
/// The Dock and the process list show the file name of an unbundled executable.
/// The function replaces a link that points at another file, and it keeps a link
/// that is already correct. It returns the original path when the link fails.
pub fn branded_java(java: &Path, dir: &Path, name: &str) -> PathBuf {
    let link = dir.join(name);
    if same_file(&link, java) {
        return link;
    }
    if std::fs::symlink_metadata(&link).is_ok() && std::fs::remove_file(&link).is_err() {
        return java.to_path_buf();
    }
    match std::fs::hard_link(java, &link) {
        Ok(()) => link,
        Err(_) => java.to_path_buf(),
    }
}

/// Reports whether two paths name the same file.
///
/// The function compares the canonical paths first. It compares the device and
/// the inode second, because two hard links keep separate canonical paths.
fn same_file(a: &Path, b: &Path) -> bool {
    if let (Ok(canonical_a), Ok(canonical_b)) = (std::fs::canonicalize(a), std::fs::canonicalize(b))
    {
        if canonical_a == canonical_b {
            return true;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(meta_a), Ok(meta_b)) = (std::fs::metadata(a), std::fs::metadata(b)) {
            return meta_a.dev() == meta_b.dev() && meta_a.ino() == meta_b.ino();
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temporary directory that removes itself after the test.
    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "bolt-jdk-tuning-{label}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn source_tuning() -> Tuning {
        Tuning {
            heap_min: Some("2g".to_string()),
            heap_max: Some("2g".to_string()),
            stack_size: Some("2m".to_string()),
            gc: Gc::Z,
            compact_object_headers: true,
            string_deduplication: true,
            native_access: true,
            add_opens: vec![
                "java.base/java.net".to_string(),
                "java.base/java.io".to_string(),
                "java.base/java.lang".to_string(),
                "java.base/java.lang.invoke".to_string(),
                "java.desktop/com.apple.eawt".to_string(),
                "java.desktop/sun.awt".to_string(),
                "java.desktop/java.awt.event".to_string(),
            ],
            aot_cache: Some(AotCache {
                path: PathBuf::from("/tmp/bolt/client-1.0.aot"),
                mode: AotMode::Load,
            }),
            gc_log: Some(PathBuf::from("/tmp/bolt/logs")),
            extra: vec![
                "-Dsun.java2d.metal=true".to_string(),
                "-Drunelite.launcher.nojvm=true".to_string(),
            ],
        }
    }

    fn write_file(path: &Path, contents: &str) {
        std::fs::write(path, contents).unwrap();
    }

    fn set_modified(path: &Path, seconds: u64) {
        let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
            .unwrap();
    }

    #[test]
    fn test_flags_full_order_feature_25() {
        let expected = vec![
            "-Xms2g",
            "-Xmx2g",
            "-XX:+UseZGC",
            "-Xss2m",
            "-XX:+UseCompactObjectHeaders",
            "--enable-native-access=ALL-UNNAMED",
            "-XX:+UseStringDeduplication",
            "--add-opens=java.base/java.net=ALL-UNNAMED",
            "--add-opens=java.base/java.io=ALL-UNNAMED",
            "--add-opens=java.base/java.lang=ALL-UNNAMED",
            "--add-opens=java.base/java.lang.invoke=ALL-UNNAMED",
            "--add-opens=java.desktop/com.apple.eawt=ALL-UNNAMED",
            "--add-opens=java.desktop/sun.awt=ALL-UNNAMED",
            "--add-opens=java.desktop/java.awt.event=ALL-UNNAMED",
            "-XX:AOTCache=/tmp/bolt/client-1.0.aot",
            "-Xlog:gc*:file=/tmp/bolt/logs/gc-%p.log:time,uptime:filecount=3,filesize=10m",
            "-Dsun.java2d.metal=true",
            "-Drunelite.launcher.nojvm=true",
        ];
        assert_eq!(source_tuning().flags(25), expected);
    }

    #[test]
    fn test_flags_full_order_feature_21() {
        let expected = vec![
            "-Xms2g",
            "-Xmx2g",
            "-XX:+UseZGC",
            "-XX:+ZGenerational",
            "-Xss2m",
            "--add-opens=java.base/java.net=ALL-UNNAMED",
            "--add-opens=java.base/java.io=ALL-UNNAMED",
            "--add-opens=java.base/java.lang=ALL-UNNAMED",
            "--add-opens=java.base/java.lang.invoke=ALL-UNNAMED",
            "--add-opens=java.desktop/com.apple.eawt=ALL-UNNAMED",
            "--add-opens=java.desktop/sun.awt=ALL-UNNAMED",
            "--add-opens=java.desktop/java.awt.event=ALL-UNNAMED",
            "-Xlog:gc*:file=/tmp/bolt/logs/gc-%p.log:time,uptime:filecount=3,filesize=10m",
            "-Dsun.java2d.metal=true",
            "-Drunelite.launcher.nojvm=true",
        ];
        assert_eq!(source_tuning().flags(21), expected);
    }

    #[test]
    fn test_feature_gates_absent_below_24() {
        let flags = source_tuning().flags(21);
        assert!(flags.contains(&"-XX:+ZGenerational".to_string()));
        assert!(!flags
            .iter()
            .any(|f| f.starts_with("-XX:+UseCompactObjectHeaders")));
        assert!(!flags
            .iter()
            .any(|f| f.starts_with("--enable-native-access")));
        assert!(!flags
            .iter()
            .any(|f| f.starts_with("-XX:+UseStringDeduplication")));
        assert!(!flags.iter().any(|f| f.starts_with("-XX:AOTCache")));
    }

    #[test]
    fn test_feature_gates_present_at_24_and_above() {
        for feature in [24, 25] {
            let flags = source_tuning().flags(feature);
            assert!(!flags.contains(&"-XX:+ZGenerational".to_string()));
            assert!(flags.contains(&"-XX:+UseCompactObjectHeaders".to_string()));
            assert!(flags.contains(&"--enable-native-access=ALL-UNNAMED".to_string()));
            assert!(flags.contains(&"-XX:+UseStringDeduplication".to_string()));
            assert!(flags.contains(&"-XX:AOTCache=/tmp/bolt/client-1.0.aot".to_string()));
        }
    }

    #[test]
    fn test_gc_choice_flags() {
        let mut tuning = Tuning::default();
        assert_eq!(tuning.flags(25), Vec::<String>::new());

        tuning.gc = Gc::G1;
        assert_eq!(tuning.flags(25), vec!["-XX:+UseG1GC"]);

        tuning.gc = Gc::Parallel;
        assert_eq!(tuning.flags(25), vec!["-XX:+UseParallelGC"]);

        tuning.gc = Gc::Default;
        assert_eq!(tuning.flags(25), Vec::<String>::new());
    }

    #[test]
    fn test_gc_log_flag_exact() {
        let tuning = Tuning {
            gc_log: Some(PathBuf::from("/var/log/bolt")),
            ..Tuning::default()
        };
        assert_eq!(
            tuning.flags(21),
            vec!["-Xlog:gc*:file=/var/log/bolt/gc-%p.log:time,uptime:filecount=3,filesize=10m"]
        );
    }

    #[test]
    fn test_aot_cache_for_absent_and_present() {
        let dir = TempDir::new("aot");
        let client_jar = dir.path().join("client-1.0.0.jar");

        let absent = aot_cache_for(dir.path(), &client_jar);
        assert_eq!(absent.path, dir.path().join("client-1.0.0.aot"));
        assert_eq!(absent.mode, AotMode::Record);

        write_file(&absent.path, "cache");
        let present = aot_cache_for(dir.path(), &client_jar);
        assert_eq!(present.path, dir.path().join("client-1.0.0.aot"));
        assert_eq!(present.mode, AotMode::Load);
    }

    #[test]
    fn test_aot_cache_record_flag_string() {
        let tuning = Tuning {
            aot_cache: Some(AotCache {
                path: PathBuf::from("/tmp/bolt/new.aot"),
                mode: AotMode::Record,
            }),
            ..Tuning::default()
        };
        assert_eq!(
            tuning.flags(25),
            vec!["-XX:AOTCacheOutput=/tmp/bolt/new.aot"]
        );
        assert!(tuning.flags(21).is_empty());
    }

    #[test]
    fn test_prune_aot_caches() {
        let dir = TempDir::new("prune");
        let keep = dir.path().join("client-1.0.0.aot");
        let stale_one = dir.path().join("client-0.9.0.aot");
        let stale_two = dir.path().join("client-0.8.0.aot");
        let other = dir.path().join("notes.txt");

        write_file(&keep, "keep");
        write_file(&stale_one, "stale");
        write_file(&stale_two, "stale");
        write_file(&other, "note");

        let removed = prune_aot_caches(dir.path(), &keep).unwrap();

        assert_eq!(removed, 2);
        assert!(keep.exists());
        assert!(!stale_one.exists());
        assert!(!stale_two.exists());
        assert!(other.exists());
    }

    #[test]
    fn test_newest_jar_picks_newest() {
        let dir = TempDir::new("newest");
        let old = dir.path().join("client-0.9.0.jar");
        let middle = dir.path().join("client-0.9.5.jar");
        let newest = dir.path().join("client-1.0.0.jar");
        let other = dir.path().join("other-2.0.0.jar");
        let text = dir.path().join("client-2.0.0.txt");

        write_file(&old, "old");
        write_file(&middle, "middle");
        write_file(&newest, "newest");
        write_file(&other, "other");
        write_file(&text, "text");
        set_modified(&old, 1_000);
        set_modified(&middle, 1_500);
        set_modified(&newest, 2_000);
        set_modified(&other, 3_000);
        set_modified(&text, 4_000);

        assert_eq!(newest_jar(dir.path(), "client-"), Some(newest));
        assert_eq!(newest_jar(dir.path(), "other-"), Some(other));
        assert_eq!(newest_jar(dir.path(), "missing-"), None);
    }

    #[test]
    fn test_branded_java_makes_link() {
        let dir = TempDir::new("brand");
        let java = dir.path().join("java-real");
        write_file(&java, "jvm");

        let link = branded_java(&java, dir.path(), "RuneLite");
        assert_eq!(link, dir.path().join("RuneLite"));
        assert_eq!(std::fs::read_to_string(&link).unwrap(), "jvm");

        let again = branded_java(&java, dir.path(), "RuneLite");
        assert_eq!(again, link);
        assert_eq!(std::fs::read_to_string(&again).unwrap(), "jvm");
    }

    #[test]
    fn test_branded_java_missing_directory_returns_original() {
        let dir = TempDir::new("brand-missing");
        let java = dir.path().join("java-real");
        write_file(&java, "jvm");
        let missing = dir.path().join("nope").join("runtime");

        assert_eq!(branded_java(&java, &missing, "RuneLite"), java);
    }
}
