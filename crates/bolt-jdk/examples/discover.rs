//! Prints every Java runtime that this machine has.

fn main() {
    for runtime in bolt_jdk::discover() {
        let version = runtime
            .version
            .as_ref()
            .map(|v| format!("{} (feature {})", v.raw, v.feature))
            .unwrap_or_else(|| "unknown".to_string());
        println!("{:?}\t{}\t{}", runtime.source, version, runtime.path.display());
    }
}
