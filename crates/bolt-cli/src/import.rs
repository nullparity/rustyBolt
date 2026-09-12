//! The `import` command.
//!
//! The command copies the real RuneLite home into the launcher home. It only
//! reads the real home. It never writes to it and never removes a file from it.

use bolt_core::{import_apply, import_plan, Config, CoreError, ImportPlan, Paths};

use crate::CliError;

/// Runs `rustybolt import`.
pub(crate) fn run(args: &[String]) -> Result<(), CliError> {
    let mut overwrite = false;
    let mut secrets = false;
    let mut dry_run = false;
    for argument in args {
        match argument.as_str() {
            "--overwrite" => overwrite = true,
            "--secrets" => secrets = true,
            "--dry-run" => dry_run = true,
            other => {
                return Err(CliError::Usage(format!(
                    "`{other}` is not a flag of `import`. Use --overwrite, --secrets or --dry-run."
                )))
            }
        }
    }

    let paths = Paths::resolve()?;
    let config = Config::load(&paths);
    let plan = match import_plan(&paths, &config) {
        Ok(plan) => plan,
        Err(CoreError::NotInstalled) => {
            return Err(CliError::Message(
                "no ~/.runelite directory exists.".to_string(),
            ))
        }
        Err(error) => return Err(error.into()),
    };

    println!("Source: {}", plan.source.display());
    println!("Target: {}", plan.target.display());
    report(&plan, overwrite, secrets);

    if dry_run {
        println!("Dry run. No file changed.");
        return Ok(());
    }

    let mut copied_names = Vec::new();
    let copied = import_apply(&plan, overwrite, secrets, &mut |name| {
        copied_names.push(name.to_string());
    })?;
    for name in &copied_names {
        println!("copied {name}");
    }
    println!("Copied {copied} of {} entries.", plan.entries.len());
    Ok(())
}

/// Lists every entry with its size and what the import would do with it.
fn report(plan: &ImportPlan, overwrite: bool, secrets: bool) {
    for entry in &plan.entries {
        let action = if entry.secret && !secrets {
            "secret, needs --secrets"
        } else if entry.present && !overwrite {
            "skip, needs --overwrite"
        } else if entry.present {
            "replace"
        } else {
            "copy"
        };
        println!("  {:<34} {:>10}  {action}", entry.name, size(entry.bytes));
    }
    println!(
        "{} entries, {} in total.",
        plan.entries.len(),
        size(plan.total_bytes())
    );
}

/// Shows a byte count in a readable form.
fn size(bytes: u64) -> String {
    const UNITS: [(&str, u64); 3] = [("GiB", 1 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)];
    for (name, scale) in UNITS {
        if bytes >= scale {
            return format!("{:.1} {name}", bytes as f64 / scale as f64);
        }
    }
    format!("{bytes} B")
}
