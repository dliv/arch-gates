use std::collections::BTreeSet;

use arch_gates::scan::{external_crate_references, root_ambient_violations, trait_declarations};

#[test]
fn pure_fixture_preserves_the_original_effect_list() {
    assert_eq!(
        arch_gates::pure_violations(include_str!("fixtures/ambient-planted.rs")),
        vec![
            "std::*",
            "std::env",
            "std::fs",
            "std::net",
            "std::process",
            "std::thread",
            "std::time",
        ]
    );
}

#[test]
fn root_fixture_checks_shared_rules_and_consumer_allowances() {
    assert_eq!(
        root_ambient_violations(
            include_str!("fixtures/root-planted.rs"),
            &["args_os", "ArgsOs", "var", "var_os", "current_dir"]
        ),
        vec![
            "#[path]",
            "path effect exists",
            "std::env",
            "std::fs",
            "std::net",
            "std::thread",
            "unsafe",
        ]
    );
}

#[test]
fn external_reference_fixture_uses_production_tokens_and_exact_dependencies() {
    let dependencies = [
        "anyhow",
        "chrono",
        "clap",
        "directories",
        "libc",
        "semver",
        "serde_json",
        "ureq",
    ]
    .into_iter()
    .map(ToOwned::to_owned)
    .collect::<BTreeSet<_>>();
    assert_eq!(
        external_crate_references(
            include_str!("fixtures/external-references-planted.rs"),
            &dependencies
        ),
        [
            "anyhow",
            "chrono",
            "clap",
            "directories",
            "libc",
            "semver",
            "serde_json",
        ]
        .map(ToOwned::to_owned)
        .into()
    );
}

#[test]
fn trait_fixture_finds_only_production_trait_declarations() {
    assert_eq!(
        trait_declarations(include_str!("fixtures/trait-planted.rs")),
        vec!["A", "B", "C", "D"]
    );
}

#[test]
fn root_profile_uses_exact_caller_allowance_names() {
    let source = r#"
use std::env as environment;
use environment::current_dir as cwd;
fn root() { let _ = cwd(); let _ = environment::var("HOME"); }
"#;
    assert_eq!(
        root_ambient_violations(source, &["current_dir", "var"]),
        Vec::<String>::new()
    );
    assert_eq!(
        root_ambient_violations(source, &["current_dir"]),
        vec!["std::env"]
    );
    assert_eq!(
        root_ambient_violations("fn root() { let _ = std::env::current_exe(); }", &[]),
        vec!["std::env"]
    );
    assert_eq!(
        root_ambient_violations(
            "fn root() { let _ = std::env::current_exe(); }",
            &["current_exe"]
        ),
        Vec::<String>::new()
    );
}

#[test]
fn root_environment_namespace_and_test_module_behavior_are_exact() {
    assert_eq!(
        root_ambient_violations("use std::env; fn root() {}", &[]),
        Vec::<String>::new()
    );
    assert_eq!(
        root_ambient_violations(
            "#[cfg(test)] pub(crate) mod tests { std::fs::read(\"x\"); std::env::temp_dir(); }",
            &[]
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        root_ambient_violations(
            "#[allow(dead_code)] mod tests { std::fs::read(\"x\"); }",
            &[]
        ),
        vec!["std::fs"]
    );
}
