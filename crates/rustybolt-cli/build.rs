//! Compiles the dashboard stylesheet with the Tailwind standalone CLI.
//!
//! The binary comes from `TAILWINDCSS` or `PATH`. See docs/building.md.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=ui/app.css");
    println!("cargo:rerun-if-changed=ui/index.html");
    println!("cargo:rerun-if-env-changed=TAILWINDCSS");

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("app.css");
    let tailwind = env::var("TAILWINDCSS").unwrap_or_else(|_| "tailwindcss".to_string());
    let status = Command::new(&tailwind)
        .args(["-i", "ui/app.css", "-o"])
        .arg(&out)
        .arg("--minify")
        .status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => panic!("tailwindcss exited with {status}"),
        Err(error) => panic!(
            "cannot run `{tailwind}`: {error}\n\
             Install the Tailwind standalone CLI (https://github.com/tailwindlabs/tailwindcss/releases) \
             and put it on PATH, or point TAILWINDCSS at it."
        ),
    }
}
