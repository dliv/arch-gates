use arch_gates::graph::crate_edges;
use std::collections::BTreeSet;
const SCANNER_MODULES: &[&str] = &[
    "crate",
    "channel",
    "cli",
    "commands",
    "commands::branch_state",
    "commands::reset",
    "commands::rm",
    "config",
    "forest",
    "git",
    "meta",
    "paths",
    "plan",
    "plan::rm",
    "version_check",
];

fn known_modules() -> Vec<String> {
    SCANNER_MODULES
        .iter()
        .map(|module| (*module).to_owned())
        .collect()
}

#[test]
fn planted_paths_resolve_to_exactly_the_real_edges() {
    let source = include_str!("fixtures/direction-planted.rs");
    let edges = crate_edges("commands::rm", source, &known_modules());
    let expected: BTreeSet<String> = ["commands::branch_state", "forest", "meta", "paths"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(edges, expected);
}

fn module_set(modules: &[&str]) -> BTreeSet<String> {
    modules.iter().map(|module| (*module).to_owned()).collect()
}

#[test]
fn nested_super_paths_resolve_once() {
    assert_eq!(
        crate_edges(
            "commands::rm",
            "const _: &str = super::super::channel::APP_NAME;",
            &known_modules()
        ),
        module_set(&["channel"])
    );
}

#[test]
fn inline_module_parent_references_are_self_edges() {
    let source = r#"pub const APP_NAME: &str = "x";
pub mod child {
    pub fn name() -> &'static str {
        super::APP_NAME
    }
    pub fn same() -> &'static str {
        self::name()
    }
}
"#;
    assert_eq!(
        crate_edges("channel", source, &known_modules()),
        module_set(&[])
    );
}

#[test]
fn inline_module_super_chains_resolve_against_their_scope() {
    assert_eq!(
        crate_edges(
            "commands::rm",
            "mod inner { fn f() { let _ = super::super::branch_state::X; } }",
            &known_modules()
        ),
        module_set(&["commands::branch_state"])
    );
}

#[test]
fn use_trees_inside_inline_modules_resolve_against_their_scope() {
    assert_eq!(
        crate_edges(
            "channel",
            "mod inner { use super::super::meta::ForestMode; }",
            &known_modules()
        ),
        module_set(&["meta"])
    );
    assert_eq!(
        crate_edges(
            "commands::rm",
            "mod inner { use super::plan_rm; }",
            &known_modules()
        ),
        module_set(&[])
    );
}

#[test]
fn grouped_use_prefixes_add_no_edge_of_their_own() {
    assert_eq!(
        crate_edges(
            "commands::rm",
            "use super::super::{channel::APP_NAME, meta::ForestMode};",
            &known_modules()
        ),
        module_set(&["channel", "meta"])
    );
    assert_eq!(
        crate_edges(
            "commands::rm",
            "mod inner { use super::super::super::{channel::APP_NAME, meta::ForestMode}; }",
            &known_modules()
        ),
        module_set(&["channel", "meta"])
    );
}
