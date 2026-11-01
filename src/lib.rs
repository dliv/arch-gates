//! Source checks for repository-owned Rust architecture policy.
//! Reads no files and starts no processes.

pub mod graph;
pub mod manifest;
pub mod metadata;
pub mod packages;
pub mod scan;
mod tokens;

/// One forbidden source form, identified within a consumer-owned file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PureViolation {
    pub file: String,
    pub form: String,
}

/// Check supplied production source; the caller owns reading and selecting files.
pub fn check_pure_source(file: &str, source: &str) -> Vec<PureViolation> {
    pure_violations(source)
        .into_iter()
        .map(|form| PureViolation {
            file: file.to_owned(),
            form,
        })
        .collect()
}

/// Forbidden forms in production tokens, sorted and deduplicated. This is a source scan, not a
/// transitive effect system.
pub fn pure_violations(source: &str) -> Vec<String> {
    scan::pure_violations(source)
}
