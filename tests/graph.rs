use arch_gates::graph::{
    CoverageViolation, DirectionViolation, Edge, check_coverage, check_direction, crate_edges,
    module_id,
};
use std::collections::BTreeSet;

#[test]
fn coverage_accepts_exact_inventory_and_reports_each_failure() {
    let disk = ["src/lib.rs".to_owned(), "src/model.rs".to_owned()]
        .into_iter()
        .collect();
    assert!(check_coverage(&disk, &["src/lib.rs", "src/model.rs"]).is_empty());
    let found = check_coverage(
        &disk,
        &["src/lib.rs", "src/lib.rs", "src/gone.rs", "bad.txt"],
    );
    assert!(found.contains(&CoverageViolation::Duplicate("src/lib.rs".into())));
    assert!(found.contains(&CoverageViolation::Missing("src/model.rs".into())));
    assert!(found.contains(&CoverageViolation::Stale("src/gone.rs".into())));
    assert!(found.contains(&CoverageViolation::Invalid("bad.txt".into())));
}

#[test]
fn ids_preserve_crate_boundaries_and_module_shapes() {
    let id = module_id("crates/widgets/core/src/plan/mod.rs").unwrap();
    assert_eq!(id.crate_dir, "crates/widgets/core");
    assert_eq!(id.path, "plan");
    assert_eq!(module_id("src/lib.rs").unwrap().path, "crate");
    for invalid in ["src/mod.rs", "src//rm.rs", "/src/lib.rs", "src/cli.txt"] {
        assert!(module_id(invalid).is_none(), "{invalid}");
    }
}

fn edge(from: &str, to: &str) -> Edge {
    Edge {
        crate_dir: "".into(),
        from: from.into(),
        to: to.into(),
    }
}

#[test]
fn direction_enforces_local_policy_and_exact_nonstale_exceptions() {
    let forbidden = edge("model", "effects");
    let edges: BTreeSet<Edge> = [forbidden.clone()].into();
    assert_eq!(
        check_direction(&edges, &[], |_| false),
        vec![DirectionViolation::Forbidden(forbidden.clone())]
    );
    assert!(check_direction(&edges, std::slice::from_ref(&forbidden), |_| false).is_empty());
    assert!(check_direction(&edges, &[], |_| true).is_empty());
    assert_eq!(
        check_direction(&BTreeSet::new(), std::slice::from_ref(&forbidden), |_| {
            false
        }),
        vec![DirectionViolation::Stale(forbidden.clone())]
    );
    assert!(
        check_direction(&edges, &[forbidden.clone(), forbidden.clone()], |_| false)
            .contains(&DirectionViolation::Duplicate(forbidden))
    );
    let foreign = Edge {
        crate_dir: "crates/other/core".into(),
        ..edge("model", "effects")
    };
    assert!(
        check_direction(&edges, &[foreign], |_| false)
            .iter()
            .any(|p| matches!(p, DirectionViolation::Forbidden(_)))
    );
}

#[test]
fn ordinary_internal_paths_and_known_scanner_limits() {
    let known = ["crate", "model", "effects", "commands", "commands::run"].map(str::to_owned);
    assert_eq!(
        crate_edges(
            "model",
            "use crate::effects as fx; fn f() { fx::run(); }",
            &known
        ),
        ["effects".to_owned()].into()
    );
    assert!(
        crate_edges(
            "model",
            r#"// crate::effects::run()
const S: &str = "crate::effects";"#,
            &known
        )
        .is_empty()
    );
    assert!(crate_edges("crate", "use effects::run;", &known).is_empty());
    assert!(crate_edges("commands", "use self::run::go;", &known).is_empty());
}
