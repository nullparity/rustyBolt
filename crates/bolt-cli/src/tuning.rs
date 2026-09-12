//! The `tuning` and `profile` commands.
//!
//! The `tuning` command shows the RuneLite tuning of the config, and it changes
//! that tuning. The `profile` command applies the profile overrides to the
//! RuneLite profile files.

use std::path::PathBuf;

use bolt_core::{apply_to_profiles, Config, GcChoice, Paths, PropertyOverrides, TuningConfig};

use crate::java;
use crate::{no_arguments, CliError};

/// Feature number of the first Java release with the tuned JVM flags.
const TUNED_FEATURE: u32 = 24;

/// Every key of `tuning set`, in the order of the help.
const KEYS: [&str; 15] = [
    "heap_min",
    "heap_max",
    "stack_size",
    "garbage_collector",
    "compact_object_headers",
    "string_deduplication",
    "native_access",
    "aot_cache",
    "gc_log",
    "java2d_metal",
    "launcher_nojvm",
    "application_name",
    "process_name",
    "extra_jvm_args",
    "extra_app_args",
];

/// The word that clears an optional text key.
const CLEAR: &str = "none";

/// Runs `rustybolt tuning [on|off|set|reset|flags]`.
///
/// Without an argument the command prints the tuning as JSON.
pub(crate) fn run(args: &[String]) -> Result<(), CliError> {
    let paths = Paths::resolve()?;
    let mut config = Config::load(&paths);
    match args.first().map(String::as_str) {
        None => {
            println!("{}", serde_json::to_string_pretty(&config.runelite_tuning)?);
            Ok(())
        }
        Some("on") | Some("off") => {
            no_arguments(&args[1..])?;
            let enabled = args[0] == "on";
            config.runelite_tuning.enabled = enabled;
            config.save(&paths)?;
            println!("The RuneLite tuning is {}.", setting(enabled));
            Ok(())
        }
        Some("set") => set(&args[1..]),
        Some("reset") => reset(&args[1..]),
        Some("flags") => flags(&args[1..]),
        Some(other) => Err(CliError::Message(format!(
            "`{other}` is not an argument of `tuning`. Use `on`, `off`, `set`, `reset` or `flags`."
        ))),
    }
}

/// Runs `rustybolt tuning set <key> <value>`.
fn set(args: &[String]) -> Result<(), CliError> {
    let key = args.first().ok_or_else(|| {
        CliError::Usage(format!(
            "`tuning set` needs a key. Valid keys: {}",
            KEYS.join(", ")
        ))
    })?;
    let value = args
        .get(1)
        .ok_or_else(|| CliError::Message(format!("`tuning set {key}` needs a value")))?;
    no_arguments(&args[2..])?;
    if !KEYS.contains(&key.as_str()) {
        return Err(CliError::Usage(format!(
            "`{key}` is not a tuning key. Valid keys: {}",
            KEYS.join(", ")
        )));
    }

    let paths = Paths::resolve()?;
    let mut config = Config::load(&paths);
    let shown = write_key(&mut config, key, value)?;
    config.save(&paths)?;
    println!("{key} = {shown}");
    Ok(())
}

/// Runs `rustybolt tuning reset`.
fn reset(args: &[String]) -> Result<(), CliError> {
    no_arguments(args)?;
    let paths = Paths::resolve()?;
    let mut config = Config::load(&paths);
    config.runelite_tuning = TuningConfig::default();
    config.save(&paths)?;
    println!("The RuneLite tuning holds the default values again.");
    Ok(())
}

/// Runs `rustybolt tuning flags [--feature <n>]`.
///
/// The function prints the flags for one feature number, and it names every
/// option that the feature gate removes.
fn flags(args: &[String]) -> Result<(), CliError> {
    let mut feature: Option<u32> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--feature" => {
                let value = crate::flag_value(args, index)?;
                feature =
                    Some(value.parse::<u32>().map_err(|_| {
                        CliError::Message(format!("`{value}` is not a whole number"))
                    })?);
                index += 2;
            }
            other => {
                return Err(CliError::Message(format!(
                    "`{other}` is not an argument of `tuning flags`"
                )))
            }
        }
    }

    let paths = Paths::resolve()?;
    let config = Config::load(&paths);
    let tuning = &config.runelite_tuning;

    if !tuning.enabled {
        println!("The RuneLite tuning is off, so the launcher adds no JVM flag.");
        return Ok(());
    }

    let feature = feature.unwrap_or_else(|| java::current_feature(&config));
    // The log directory holds the gc log and the AOT cache of a real launch.
    let log_dir = paths.cache_dir.join("logs");
    let built = tuning.to_tuning(&log_dir, client_repository().as_deref());

    println!("Feature {feature}.");
    for flag in built.flags(feature) {
        println!("{flag}");
    }

    let unused = unused_options(tuning, feature);
    if !unused.is_empty() {
        println!();
        println!("Not used on feature {feature}:");
        for name in &unused {
            println!("  {name}");
        }
    }
    Ok(())
}

/// Writes one key of the config, and returns the new value as text.
///
/// `process_name` writes the process name of the config. Every other key writes
/// the RuneLite tuning.
fn write_key(config: &mut Config, key: &str, value: &str) -> Result<String, CliError> {
    if key == "process_name" {
        let name = optional_text(value);
        let shown = shown_optional(name.as_deref());
        config.runelite_process_name = name;
        return Ok(shown);
    }

    let tuning = &mut config.runelite_tuning;
    let shown = match key {
        "heap_min" => {
            tuning.heap_min = optional_text(value);
            shown_optional(tuning.heap_min.as_deref())
        }
        "heap_max" => {
            tuning.heap_max = optional_text(value);
            shown_optional(tuning.heap_max.as_deref())
        }
        "stack_size" => {
            tuning.stack_size = optional_text(value);
            shown_optional(tuning.stack_size.as_deref())
        }
        "garbage_collector" => {
            tuning.garbage_collector = collector(value)?;
            collector_name(&tuning.garbage_collector).to_string()
        }
        "compact_object_headers" => {
            tuning.compact_object_headers = boolean(value)?;
            setting(tuning.compact_object_headers).to_string()
        }
        "string_deduplication" => {
            tuning.string_deduplication = boolean(value)?;
            setting(tuning.string_deduplication).to_string()
        }
        "native_access" => {
            tuning.native_access = boolean(value)?;
            setting(tuning.native_access).to_string()
        }
        "aot_cache" => {
            tuning.aot_cache = boolean(value)?;
            setting(tuning.aot_cache).to_string()
        }
        "gc_log" => {
            tuning.gc_log = boolean(value)?;
            setting(tuning.gc_log).to_string()
        }
        "java2d_metal" => {
            tuning.java2d_metal = boolean(value)?;
            setting(tuning.java2d_metal).to_string()
        }
        "launcher_nojvm" => {
            tuning.launcher_nojvm = boolean(value)?;
            setting(tuning.launcher_nojvm).to_string()
        }
        "application_name" => {
            tuning.application_name = optional_text(value);
            shown_optional(tuning.application_name.as_deref())
        }
        "extra_jvm_args" => {
            tuning.extra_jvm_args = words(value);
            shown_words(&tuning.extra_jvm_args)
        }
        "extra_app_args" => {
            tuning.extra_app_args = words(value);
            shown_words(&tuning.extra_app_args)
        }
        other => {
            return Err(CliError::Usage(format!(
                "`{other}` is not a tuning key. Valid keys: {}",
                KEYS.join(", ")
            )))
        }
    };
    Ok(shown)
}

/// The names of the options that the feature gate removes.
///
/// The four tuned options need feature 24 or newer. The generational ZGC flag
/// applies below feature 24 only.
fn unused_options(tuning: &TuningConfig, feature: u32) -> Vec<&'static str> {
    let mut names = Vec::new();
    if feature >= TUNED_FEATURE {
        if tuning.garbage_collector == GcChoice::Z {
            names.push("generational ZGC flag");
        }
        return names;
    }
    if tuning.compact_object_headers {
        names.push("compact headers");
    }
    if tuning.string_deduplication {
        names.push("string dedup");
    }
    if tuning.native_access {
        names.push("native access");
    }
    if tuning.aot_cache {
        names.push("AOT cache");
    }
    names
}

/// The repository of the RuneLite client jar, normally `~/.runelite/repository2`.
fn client_repository() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".runelite").join("repository2"))
}

/// The word `none` maps to `None`.
fn optional_text(value: &str) -> Option<String> {
    if value == CLEAR {
        None
    } else {
        Some(value.to_string())
    }
}

fn boolean(value: &str) -> Result<bool, CliError> {
    match value {
        "on" | "true" => Ok(true),
        "off" | "false" => Ok(false),
        other => Err(CliError::Message(format!(
            "`{other}` is not a setting. Use `on`, `off`, `true` or `false`."
        ))),
    }
}

/// Reads a garbage collector name.
fn collector(value: &str) -> Result<GcChoice, CliError> {
    match value {
        "default" => Ok(GcChoice::Default),
        "z" => Ok(GcChoice::Z),
        "g1" => Ok(GcChoice::G1),
        "parallel" => Ok(GcChoice::Parallel),
        other => Err(CliError::Message(format!(
            "`{other}` is not a garbage collector. Use `default`, `z`, `g1` or `parallel`."
        ))),
    }
}

/// The word of one garbage collector.
fn collector_name(collector: &GcChoice) -> &'static str {
    match collector {
        GcChoice::Default => "default",
        GcChoice::Z => "z",
        GcChoice::G1 => "g1",
        GcChoice::Parallel => "parallel",
    }
}

/// Splits a value into words on ASCII whitespace.
fn words(value: &str) -> Vec<String> {
    value.split_ascii_whitespace().map(str::to_string).collect()
}

fn shown_optional(value: Option<&str>) -> String {
    match value {
        Some(text) => text.to_string(),
        None => CLEAR.to_string(),
    }
}

/// The text of a word list.
fn shown_words(words: &[String]) -> String {
    if words.is_empty() {
        return CLEAR.to_string();
    }
    words.join(" ")
}

/// Runs `rustybolt profile apply [--dry-run]`.
pub(crate) fn profile(args: &[String]) -> Result<(), CliError> {
    match args.first().map(String::as_str) {
        Some("apply") => apply(&args[1..]),
        Some(other) => Err(CliError::Message(format!(
            "`{other}` is not a subcommand of `profile`. Use `profile apply`."
        ))),
        None => Err(CliError::Message(
            "`profile` needs a subcommand. Use `profile apply`.".to_string(),
        )),
    }
}

/// Applies the profile overrides of the config to the profile directory.
fn apply(args: &[String]) -> Result<(), CliError> {
    let mut dry_run = false;
    for argument in args {
        match argument.as_str() {
            "--dry-run" => dry_run = true,
            other => {
                return Err(CliError::Message(format!(
                    "`{other}` is not an argument of `profile apply`"
                )))
            }
        }
    }

    let paths = Paths::resolve()?;
    let config = Config::load(&paths);
    // The config file wins. Without overrides the command uses the graphics profile.
    let overrides = config
        .runelite_profile_overrides
        .clone()
        .unwrap_or_else(PropertyOverrides::gpu_defaults);
    let dir = config.profile_dir(&paths);

    if dry_run {
        println!("directory: {}", dir.display());
        println!("strip prefixes:");
        for prefix in &overrides.strip_prefixes {
            println!("  {prefix}");
        }
        println!("forced pairs:");
        for (key, value) in &overrides.force {
            println!("  {key}={value}");
        }
        println!("Dry run. No file changed.");
        return Ok(());
    }

    let changed = apply_to_profiles(&dir, &overrides)?;
    let files = if changed == 1 {
        "1 file".to_string()
    } else {
        format!("{changed} files")
    };
    println!("Changed {files} in {}.", dir.display());
    Ok(())
}

/// The word of one setting value.
fn setting(enabled: bool) -> &'static str {
    if enabled {
        "on"
    } else {
        "off"
    }
}
