//! Small file helpers that several modules share.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Sets the permissions of a file. Windows has no mode, so the call does nothing.
pub(crate) fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

/// Writes `data` to `path` through a temporary file in the same directory.
///
/// The function sets the mode before the rename, so a reader never sees a
/// file with the wrong mode. A failure removes the temporary file.
pub(crate) fn write_private(path: &Path, data: &[u8], mode: u32) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = temporary_path(path);
    let result = write_then_rename(&temporary, path, data, mode);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_then_rename(temporary: &Path, path: &Path, data: &[u8], mode: u32) -> io::Result<()> {
    let mut file = fs::File::create(temporary)?;
    file.write_all(data)?;
    file.flush()?;
    set_mode(temporary, mode)?;
    drop(file);
    fs::rename(temporary, path)
}

/// Builds a temporary file name next to `path`.
pub(crate) fn temporary_path(path: &Path) -> PathBuf {
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "tmp".to_string());
    path.with_file_name(format!(".{}.{}.{}.tmp", name, std::process::id(), count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_path_stays_next_to_the_file() {
        let path = Path::new("/tmp/dir/runelite.jar");
        let temporary = temporary_path(path);
        assert_eq!(temporary.parent(), path.parent());
        assert!(temporary
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".runelite.jar."));
    }

    #[test]
    fn write_private_replaces_the_file_and_removes_the_temporary() {
        let temp = crate::test_support::TempDir::new("file");
        let path = temp.path().join("creds.json");
        write_private(&path, b"first", 0o600).unwrap();
        write_private(&path, b"second", 0o600).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        let entries = fs::read_dir(temp.path()).unwrap().count();
        assert_eq!(entries, 1);
    }
}
