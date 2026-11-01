// Generic dependency-reference input. It is never compiled.
use clap::Parser;
use directories as dirs;
use std::path::Path;

#[derive(clap::Subcommand)]
enum Example {
    One,
}

fn references() {
    let _ = ::chrono::Utc::now();
    let _ = dirs::UserDirs::new();
    let _ = ::semver::Version::parse("1.0.0");
    let _ = ::libc::getpid();
}

fn returns() -> serde_json::Value {
    serde_json::json!(null)
}

fn error() -> anyhow::Result<()> {
    Ok(())
}

fn internal_paths() {
    let _ = crate::chrono::Utc;
    let _ = self::serde_json::Value;
    let _ = super::semver::Version;
    let _ = Self::clap::Parser;
}

// ureq::get("unused") in a comment and string are not references.
const TEXT: &str = "ureq::get";

#[cfg(test)]
mod tests {
    fn excluded() {
        let _ = ureq::get("unused");
    }
}
