//! The compiled policy host owns local observation; `src` stays a pure library.

mod self_support;

use self_support::{check_module_direction, check_package_policy, check_source_policy};

#[test]
fn actual_production_sources_follow_the_local_policy() {
    let sources = check_source_policy().expect("actual production source policy");
    check_module_direction(&sources).expect("actual module direction policy");
}

#[test]
fn actual_cargo_workspace_follows_the_local_package_policy() {
    check_package_policy().expect("actual Cargo package policy");
}
