//! The use records of the launcher.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::file::write_private;
use crate::Paths;

/// The file name below the config directory.
const USAGE_FILE: &str = "usage.json";

/// The file mode of the usage file. The file holds no secret.
const USAGE_MODE: u32 = 0o644;

/// The use count and the last use time of one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Usage {
    /// The number of uses.
    pub count: u64,
    /// The last use time, in seconds since the epoch. Zero means never.
    pub last_used: u64,
}

/// The use records of the user, and the file that holds them.
pub struct UsageStore {
    path: PathBuf,
    records: BTreeMap<String, Usage>,
}

impl UsageStore {
    /// Reads `usage.json` of the config directory. An absent or malformed file gives no records.
    pub fn load(paths: &Paths) -> UsageStore {
        let path = paths.config_dir.join(USAGE_FILE);
        let records = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<BTreeMap<String, Usage>>(&text).ok())
            .unwrap_or_default();
        UsageStore { path, records }
    }

    /// Writes the records as JSON.
    ///
    /// The write goes through a temporary file.
    pub fn save(&self) -> io::Result<()> {
        let text = serde_json::to_vec_pretty(&self.records)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_private(&self.path, &text, USAGE_MODE)
    }

    /// Adds one to the count and sets the time to now.
    pub fn mark_used(&mut self, key: &str) {
        let now = now_secs();
        let entry = self.records.entry(key.to_string()).or_default();
        entry.count = entry.count.saturating_add(1);
        entry.last_used = now;
    }

    /// The use record of one key. An absent key gives the default record.
    pub fn usage(&self, key: &str) -> Usage {
        self.records.get(key).copied().unwrap_or_default()
    }

    /// Orders items so the most useful one comes first.
    ///
    /// An item that the user opened inside `window` seconds comes first, and the
    /// newest of those wins. Every other item follows, ordered by its count, then
    /// by its time. An equal pair keeps the input order.
    pub fn order<'a, T>(&self, items: &'a [T], window: u64, key: impl Fn(&T) -> &str) -> Vec<&'a T> {
        let now = now_secs();
        let mut held: Vec<(&'a T, Usage)> = items
            .iter()
            .map(|item| (item, self.usage(key(item))))
            .collect();
        held.sort_by(|left, right| {
            let left_recent = is_recent(&left.1, now, window);
            let right_recent = is_recent(&right.1, now, window);
            match (left_recent, right_recent) {
                (true, true) => right.1.last_used.cmp(&left.1.last_used),
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                (false, false) => right
                    .1
                    .count
                    .cmp(&left.1.count)
                    .then(right.1.last_used.cmp(&left.1.last_used)),
            }
        });
        held.into_iter().map(|(item, _)| item).collect()
    }
}

/// Tells if the record is inside the recent window.
fn is_recent(usage: &Usage, now: u64, window: u64) -> bool {
    usage.last_used != 0 && now.saturating_sub(usage.last_used) <= window
}

/// The present time in seconds since the epoch. A clock before the epoch gives zero.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{paths, TempDir};

    #[test]
    fn order_puts_a_recent_item_first_and_a_high_count_item_after_it() {
        let temp = TempDir::new("usage-order");
        let store_paths = paths(temp.path());
        let mut store = UsageStore::load(&store_paths);
        let window = 3600;
        store.records.insert(
            "hot".to_string(),
            Usage {
                count: 9,
                last_used: now_secs() - window - 10,
            },
        );
        store.mark_used("recent");
        let items = vec![
            ("hot".to_string(), 1),
            ("recent".to_string(), 2),
            ("other".to_string(), 3),
        ];
        let ordered = store.order(&items, window, |item| item.0.as_str());
        let names: Vec<&str> = ordered.iter().map(|item| item.0.as_str()).collect();
        assert_eq!(names, vec!["recent", "hot", "other"]);
    }

    #[test]
    fn order_keeps_the_input_order_for_an_equal_pair() {
        let temp = TempDir::new("usage-equal");
        let store = UsageStore::load(&paths(temp.path()));
        let items = vec!["first".to_string(), "second".to_string()];
        let ordered = store.order(&items, 3600, |item| item.as_str());
        let names: Vec<&str> = ordered.iter().map(|item| item.as_str()).collect();
        assert_eq!(names, vec!["first", "second"]);
    }

    #[test]
    fn mark_used_then_save_then_load_keeps_the_count() {
        let temp = TempDir::new("usage-save");
        let store_paths = paths(temp.path());
        let mut store = UsageStore::load(&store_paths);
        store.mark_used("character-1");
        store.mark_used("character-1");
        store.save().unwrap();
        let reloaded = UsageStore::load(&store_paths);
        let usage = reloaded.usage("character-1");
        assert_eq!(usage.count, 2);
        assert!(usage.last_used > 0);
    }

    #[test]
    fn an_absent_or_malformed_file_gives_no_records() {
        let temp = TempDir::new("usage-missing");
        let store_paths = paths(temp.path());
        assert_eq!(UsageStore::load(&store_paths).usage("anything").count, 0);
        std::fs::create_dir_all(&store_paths.config_dir).unwrap();
        std::fs::write(store_paths.config_dir.join(USAGE_FILE), b"{ not json").unwrap();
        assert_eq!(UsageStore::load(&store_paths).usage("anything").count, 0);
    }
}
