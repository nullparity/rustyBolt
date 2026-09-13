//! Import of the real RuneLite home into the launcher home.
//!
//! The launcher gives the client a private home, so a launch never changes the
//! files of another launcher. A user who wants the settings of the real home
//! copies them once with the functions of this module. The copy only reads the
//! real home; it never writes to it.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Config, CoreError, Paths};

/// Where the RuneLite client keeps its data.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuneLiteHome {
    /// A private home below the launcher data directory. The default.
    #[default]
    Isolated,
    /// The real home of the user, so the client shares `~/.runelite`.
    System,
    /// A directory that the user names.
    Custom(PathBuf),
}

impl std::fmt::Display for RuneLiteHome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuneLiteHome::Isolated => write!(f, "private"),
            RuneLiteHome::System => write!(f, "system"),
            RuneLiteHome::Custom(path) => write!(f, "custom: {}", path.display()),
        }
    }
}

/// `$HOME/.runelite`.
pub fn system_runelite_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".runelite"))
}

/// One entry that an import would copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEntry {
    /// The name below `.runelite`.
    pub name: String,
    /// The file or directory in the real home.
    pub source: PathBuf,
    /// The place in the launcher home.
    pub target: PathBuf,
    /// The size of the whole entry.
    pub bytes: u64,
    /// The target already holds this entry.
    pub present: bool,
    /// The entry holds login data.
    pub secret: bool,
}

/// What an import would copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    /// The real home.
    pub source: PathBuf,
    /// The launcher home.
    pub target: PathBuf,
    /// One entry for each name that the import would copy.
    pub entries: Vec<ImportEntry>,
}

impl ImportPlan {
    /// The size of every entry together.
    pub fn total_bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.bytes).sum()
    }
}

/// The names that an import skips, each with the reason.
///
/// The client makes each of them again, or they hold no setting. Together they
/// are almost the whole size of a used RuneLite home.
const SKIP_NAMES: &[(&str, &str)] = &[
    ("cache", "the client builds it again"),
    ("jagexcache", "the client downloads it again"),
    ("repository", "the launcher downloads it again"),
    ("repository2", "the launcher downloads it again"),
    ("logs", "old log files"),
    ("error", "old error reports"),
    ("screenshots", "pictures, not a setting"),
    ("plugins", "the plugin hub downloads them again"),
    (
        "resource-packs-repository",
        "the client downloads the packs again",
    ),
    (".DS_Store", "a folder setting of the file manager"),
];

/// The reason that an import skips a name, when it skips it.
fn skip_reason(name: &str) -> Option<&'static str> {
    SKIP_NAMES
        .iter()
        .find(|(skipped, _)| *skipped == name)
        .map(|(_, reason)| *reason)
}

/// The files that hold login data.
const SECRET_NAMES: &[&str] = &["credentials.properties"];

/// The file type that holds login data.
const SECRET_EXTENSION: &str = "dat";

fn is_secret(name: &str, path: &Path) -> bool {
    if SECRET_NAMES.contains(&name) {
        return true;
    }
    path.extension()
        .map(|extension| extension.eq_ignore_ascii_case(SECRET_EXTENSION))
        .unwrap_or(false)
}

/// Lists what an import would copy from the real home into the launcher home.
pub fn import_plan(paths: &Paths, config: &Config) -> Result<ImportPlan, CoreError> {
    let source = system_runelite_dir().ok_or(CoreError::NotInstalled)?;
    if !source.is_dir() {
        return Err(CoreError::NotInstalled);
    }
    let target = config.runelite_home(paths).join(".runelite");

    if same_directory(&source, &target) {
        return Err(CoreError::Command(
            "the system home is the launcher home, so an import would copy a directory onto itself"
                .to_string(),
        ));
    }

    let mut entries = Vec::new();
    for item in fs::read_dir(&source)? {
        let item = item?;
        let name = item.file_name().to_string_lossy().into_owned();
        if skip_reason(&name).is_some() {
            continue;
        }
        let path = item.path();
        let target_path = target.join(&name);
        entries.push(ImportEntry {
            bytes: entry_size(&path)?,
            present: target_path.exists(),
            secret: is_secret(&name, &path),
            name,
            source: path,
            target: target_path,
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(ImportPlan {
        source,
        target,
        entries,
    })
}

/// Copies the entries of a plan. Returns the number of copied entries.
///
/// The function skips a secret entry unless `secrets` is true, and it skips an
/// entry that the target already holds unless `overwrite` is true.
pub fn import_apply(
    plan: &ImportPlan,
    overwrite: bool,
    secrets: bool,
    progress: &mut dyn FnMut(&str),
) -> Result<usize, CoreError> {
    if same_directory(&plan.source, &plan.target) {
        return Err(CoreError::Command(
            "the system home is the launcher home, so an import would copy a directory onto itself"
                .to_string(),
        ));
    }
    fs::create_dir_all(&plan.target)?;

    let mut copied = 0;
    for entry in &plan.entries {
        if entry.secret && !secrets {
            continue;
        }
        if entry.present && !overwrite {
            continue;
        }
        progress(&entry.name);
        copy_entry(&entry.source, &entry.target)?;
        copied += 1;
    }
    Ok(copied)
}

/// Two paths name the same directory.
fn same_directory(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

/// The size of a file, or of every file of a directory.
///
/// The function does not follow a symbolic link, so a link that points out of the
/// tree never adds its target size.
fn entry_size(path: &Path) -> Result<u64, CoreError> {
    let data = fs::symlink_metadata(path)?;
    if data.file_type().is_symlink() {
        return Ok(0);
    }
    if data.is_file() {
        return Ok(data.len());
    }
    let mut total = 0;
    for item in fs::read_dir(path)? {
        total += entry_size(&item?.path())?;
    }
    Ok(total)
}

/// Copies a file or a whole directory.
///
/// The function does not follow a symbolic link. A link inside the tree is left
/// out, because its target may sit outside the tree.
fn copy_entry(source: &Path, target: &Path) -> Result<(), CoreError> {
    let data = fs::symlink_metadata(source)?;
    if data.file_type().is_symlink() {
        return Ok(());
    }
    if data.is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
        return Ok(());
    }
    if !data.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(target)?;
    for item in fs::read_dir(source)? {
        let item = item?;
        copy_entry(&item.path(), &target.join(item.file_name()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{paths, TempDir};

    /// Builds a fake source tree. The real home of the user is never touched.
    fn fake_home(root: &Path) -> PathBuf {
        let home = root.join("fake-home/.runelite");
        fs::create_dir_all(home.join("profiles2")).expect("cannot make the directory");
        fs::create_dir_all(home.join("cache")).expect("cannot make the directory");
        fs::create_dir_all(home.join("flipping/sub")).expect("cannot make the directory");
        fs::write(home.join("profiles2/one.properties"), "gpu.fogDepth=3\n").expect("write");
        fs::write(home.join("flipping/sub/deep.json"), "{}").expect("write");
        fs::write(home.join("credentials.properties"), "secret").expect("write");
        fs::write(home.join("jagex_cl_oldschool_LIVE.dat"), "secret").expect("write");
        fs::write(home.join("cache/big.bin"), vec![0u8; 4096]).expect("write");
        home
    }

    fn plan_for(source: &Path, target: &Path) -> ImportPlan {
        let mut entries = Vec::new();
        for item in fs::read_dir(source).expect("cannot read the directory") {
            let item = item.expect("cannot read the entry");
            let name = item.file_name().to_string_lossy().into_owned();
            if skip_reason(&name).is_some() {
                continue;
            }
            let path = item.path();
            let target_path = target.join(&name);
            entries.push(ImportEntry {
                bytes: entry_size(&path).expect("cannot size the entry"),
                present: target_path.exists(),
                secret: is_secret(&name, &path),
                name,
                source: path,
                target: target_path,
            });
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        ImportPlan {
            source: source.to_path_buf(),
            target: target.to_path_buf(),
            entries,
        }
    }

    #[test]
    fn a_plan_skips_the_cache_and_marks_the_login_files() {
        let dir = TempDir::new("import-plan");
        let source = fake_home(dir.path());
        let plan = plan_for(&source, &dir.path().join("target"));

        let names: Vec<&str> = plan.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&"cache"));
        assert_eq!(
            names,
            vec![
                "credentials.properties",
                "flipping",
                "jagex_cl_oldschool_LIVE.dat",
                "profiles2",
            ]
        );

        let secret: Vec<&str> = plan
            .entries
            .iter()
            .filter(|e| e.secret)
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(
            secret,
            vec!["credentials.properties", "jagex_cl_oldschool_LIVE.dat"]
        );
    }

    #[test]
    fn a_directory_size_counts_every_file_below_it() {
        let dir = TempDir::new("import-size");
        let source = fake_home(dir.path());
        let plan = plan_for(&source, &dir.path().join("target"));
        let flipping = plan
            .entries
            .iter()
            .find(|e| e.name == "flipping")
            .expect("the entry exists");
        assert_eq!(flipping.bytes, 2);
    }

    #[test]
    fn an_import_copies_the_whole_tree_and_leaves_the_login_files() {
        let dir = TempDir::new("import-apply");
        let source = fake_home(dir.path());
        let target = dir.path().join("target");
        let plan = plan_for(&source, &target);

        let copied = import_apply(&plan, false, false, &mut |_| {}).expect("cannot import");
        assert_eq!(copied, 2);
        assert!(target.join("profiles2/one.properties").is_file());
        // The copy goes down the whole tree, not one level.
        assert!(target.join("flipping/sub/deep.json").is_file());
        assert!(!target.join("credentials.properties").exists());
        assert!(!target.join("jagex_cl_oldschool_LIVE.dat").exists());
    }

    #[test]
    fn an_import_takes_the_login_files_when_the_caller_asks() {
        let dir = TempDir::new("import-secrets");
        let source = fake_home(dir.path());
        let target = dir.path().join("target");
        let plan = plan_for(&source, &target);

        let copied = import_apply(&plan, false, true, &mut |_| {}).expect("cannot import");
        assert_eq!(copied, 4);
        assert!(target.join("credentials.properties").is_file());
    }

    #[test]
    fn an_import_keeps_a_present_entry_unless_the_caller_replaces_it() {
        let dir = TempDir::new("import-present");
        let source = fake_home(dir.path());
        let target = dir.path().join("target");
        fs::create_dir_all(target.join("profiles2")).expect("cannot make the directory");
        fs::write(target.join("profiles2/one.properties"), "mine").expect("write");

        let plan = plan_for(&source, &target);
        let copied = import_apply(&plan, false, false, &mut |_| {}).expect("cannot import");
        assert_eq!(copied, 1);
        assert_eq!(
            fs::read_to_string(target.join("profiles2/one.properties")).expect("read"),
            "mine"
        );

        let plan = plan_for(&source, &target);
        import_apply(&plan, true, false, &mut |_| {}).expect("cannot import");
        assert_eq!(
            fs::read_to_string(target.join("profiles2/one.properties")).expect("read"),
            "gpu.fogDepth=3\n"
        );
    }

    #[test]
    fn an_import_refuses_to_copy_a_directory_onto_itself() {
        let dir = TempDir::new("import-same");
        let source = fake_home(dir.path());
        let plan = plan_for(&source, &source);
        let error = import_apply(&plan, false, false, &mut |_| {}).expect_err("must refuse");
        assert!(matches!(error, CoreError::Command(_)));
    }

    #[test]
    fn the_plan_target_follows_the_chosen_home() {
        let dir = TempDir::new("import-home");
        let paths = paths(dir.path());
        let mut config = Config::default();
        assert_eq!(
            config.runelite_home(&paths).join(".runelite"),
            paths.data_dir.join(".runelite")
        );

        config.runelite_home_kind = RuneLiteHome::Custom(dir.path().join("elsewhere"));
        assert_eq!(
            config.runelite_home(&paths).join(".runelite"),
            dir.path().join("elsewhere/.runelite")
        );
    }
}
