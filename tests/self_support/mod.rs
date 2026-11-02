use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use arch_gates::graph::{self, DirectionViolation, Edge};
use arch_gates::metadata::{DependencyKind, Member, parse_members};
use arch_gates::packages::{CrateClass, ExternalAllowlists};
use arch_gates::{check_pure_source, scan};

/// Every production Rust file has an explicit Pure classification.
const PURE_SOURCES: &[&str] = &[
    "src/graph.rs",
    "src/lib.rs",
    "src/manifest.rs",
    "src/metadata.rs",
    "src/packages.rs",
    "src/scan.rs",
    "src/tokens.rs",
];

#[derive(Clone, Copy)]
struct ModulePolicy {
    module: &'static str,
    allowed_imports: &'static [&'static str],
    facade: bool,
}

const MODULE_POLICIES: &[ModulePolicy] = &[
    ModulePolicy {
        module: "crate",
        allowed_imports: &[],
        facade: true,
    },
    ModulePolicy {
        module: "graph",
        allowed_imports: &["tokens"],
        facade: false,
    },
    ModulePolicy {
        module: "manifest",
        allowed_imports: &[],
        facade: false,
    },
    ModulePolicy {
        module: "metadata",
        allowed_imports: &["manifest"],
        facade: false,
    },
    ModulePolicy {
        module: "packages",
        allowed_imports: &["metadata"],
        facade: false,
    },
    ModulePolicy {
        module: "scan",
        allowed_imports: &["tokens"],
        facade: false,
    },
    ModulePolicy {
        module: "tokens",
        allowed_imports: &[],
        facade: false,
    },
];

/// The only declared Pure external reference. Keep this list synchronized with package policy;
/// the test also rejects entries that production source no longer uses.
const PURE_EXTERNALS: &[(&str, &[&str])] = &[("serde_json", &[])];

const CRATES: &[(&str, CrateClass)] = &[(
    "arch-gates",
    CrateClass::Pure {
        feature: "architecture-validation",
    },
)];

const ALLOWED_EFFECTFUL_EXTERNALS: &[&str] = &[];
const ALLOWED_EDGE_EXTERNALS: &[&str] = &[];
const FORBIDDEN_TOOLING_PACKAGES: &[&str] = &[];

#[derive(Debug)]
pub(super) struct ProductionSource {
    relative_path: String,
    module: String,
    source: String,
}

pub(super) fn check_source_policy() -> Result<Vec<ProductionSource>, String> {
    let root = engine_root();
    let discovered = discover_rust_sources(&root.join("src"))?;
    let coverage = graph::check_coverage(&discovered, PURE_SOURCES);
    if !coverage.is_empty() {
        return Err(format!("production source coverage failed: {coverage:?}"));
    }

    let sources = PURE_SOURCES
        .iter()
        .map(|relative_path| read_source(&root, relative_path))
        .collect::<Result<Vec<_>, _>>()?;

    for source in &sources {
        let violations = check_pure_source(&source.relative_path, &source.source);
        if !violations.is_empty() {
            return Err(format!(
                "Pure source violations in {}: {violations:?}",
                source.relative_path
            ));
        }
    }

    check_external_references(&sources)?;
    Ok(sources)
}

pub(super) fn check_module_direction(sources: &[ProductionSource]) -> Result<(), String> {
    let actual_modules = sources
        .iter()
        .map(|source| source.module.as_str())
        .collect::<BTreeSet<_>>();
    if actual_modules.len() != sources.len() {
        return Err("multiple production source files map to the same module identity".to_owned());
    }
    let classified_modules = MODULE_POLICIES
        .iter()
        .map(|policy| policy.module)
        .collect::<BTreeSet<_>>();
    if classified_modules.len() != MODULE_POLICIES.len() {
        return Err("duplicate production module policy entries".to_owned());
    }
    if actual_modules != classified_modules {
        return Err(format!(
            "module policy coverage differs; unclassified modules: {:?}; stale module entries: {:?}",
            actual_modules
                .difference(&classified_modules)
                .collect::<Vec<_>>(),
            classified_modules
                .difference(&actual_modules)
                .collect::<Vec<_>>()
        ));
    }

    let known_modules = sources
        .iter()
        .filter(|source| source.module != "crate")
        .map(|source| source.module.clone())
        .collect::<Vec<_>>();
    let mut edges = BTreeSet::new();
    for source in sources {
        for target in graph::crate_edges(&source.module, &source.source, &known_modules) {
            edges.insert(Edge {
                crate_dir: String::new(),
                from: source.module.clone(),
                to: target,
            });
        }
    }

    let violations = graph::check_direction(&edges, &[], |edge| {
        MODULE_POLICIES
            .iter()
            .find(|policy| policy.module == edge.from)
            .is_some_and(|policy| {
                policy.facade || policy.allowed_imports.contains(&edge.to.as_str())
            })
    });

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "module direction policy failed: {}",
            format_direction_violations(&violations)
        ))
    }
}

pub(super) fn check_package_policy() -> Result<(), String> {
    let root = engine_root();
    let metadata = cargo_metadata(&root)?;
    let listed = parse_members(&metadata)?;

    let members = listed
        .into_iter()
        .map(|member| {
            let manifest_text = fs::read_to_string(&member.manifest_path).map_err(|error| {
                format!(
                    "cannot read workspace member manifest {}: {error}",
                    member.manifest_path.display()
                )
            })?;
            Ok(member.with_manifest(&manifest_text))
        })
        .collect::<Result<Vec<Member>, String>>()?;

    let allowed = ExternalAllowlists {
        pure: PURE_EXTERNALS,
        effectful: ALLOWED_EFFECTFUL_EXTERNALS,
        edge: ALLOWED_EDGE_EXTERNALS,
    };
    let violations = arch_gates::packages::check_edges(
        &members,
        CRATES,
        allowed,
        FORBIDDEN_TOOLING_PACKAGES,
        |_, _| Err("this library workspace has no internal production package edges"),
    );
    if !violations.is_empty() {
        return Err(format!("Cargo package policy failed: {violations:?}"));
    }

    if members.len() != 1 || members[0].name != "arch-gates" {
        return Err(format!(
            "expected the single arch-gates workspace member; found {}",
            members
                .iter()
                .map(|member| member.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    check_library_targets(&root, &metadata)?;
    let expected_dependency = match members[0].dependencies.as_slice() {
        [dependency] => {
            dependency.name == "serde_json"
                && dependency.rename.is_none()
                && dependency.kind == DependencyKind::Normal
                && !dependency.is_path
                && dependency.features.is_empty()
        }
        _ => false,
    };
    if !expected_dependency {
        return Err(
            "expected only a normal serde_json dependency, with no rename, path, or declared features"
                .to_owned(),
        );
    }

    Ok(())
}

/// Cargo targets are build units too: a binary outside src/ must not escape source coverage.
fn check_library_targets(root: &Path, metadata: &str) -> Result<(), String> {
    let document: serde_json::Value = serde_json::from_str(metadata)
        .map_err(|error| format!("cannot parse Cargo target facts: {error}"))?;
    let packages = document["packages"]
        .as_array()
        .ok_or("Cargo metadata has no package list")?;
    let package = packages
        .iter()
        .find(|package| package["name"] == "arch-gates")
        .ok_or("Cargo metadata omitted the engine package")?;
    let targets = package["targets"]
        .as_array()
        .ok_or("engine has no Cargo target list")?;
    let expected_source = root
        .join("src/lib.rs")
        .canonicalize()
        .map_err(|error| format!("cannot resolve production library source: {error}"))?;
    let mut libraries = 0;
    for target in targets {
        let kinds = target["kind"]
            .as_array()
            .ok_or("Cargo target has no kind list")?;
        if kinds.len() == 1 && kinds[0] == "test" {
            continue;
        }
        if kinds.len() != 1 || kinds[0] != "lib" {
            return Err(format!(
                "unsupported Cargo target {}: {kinds:?}; only the production library and integration tests are classified",
                target["name"]
            ));
        }
        libraries += 1;
        let source = target["src_path"]
            .as_str()
            .ok_or("library target has no source path")?;
        let source = Path::new(source)
            .canonicalize()
            .map_err(|error| format!("cannot resolve library target source: {error}"))?;
        if source != expected_source {
            return Err(
                "production library source must remain the classified src/lib.rs".to_owned(),
            );
        }
    }
    if libraries != 1 {
        return Err(format!(
            "expected one production library target, found {libraries}"
        ));
    }
    Ok(())
}

fn check_external_references(sources: &[ProductionSource]) -> Result<(), String> {
    let root = engine_root();
    let manifest = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|error| format!("cannot read root Cargo.toml: {error}"))?;
    let actual_normal_dependencies = arch_gates::manifest::dependency_crate_names(&manifest);
    let references = sources
        .iter()
        .flat_map(|source| {
            scan::external_crate_references(&source.source, &actual_normal_dependencies)
        })
        .collect::<BTreeSet<_>>();
    let allowed = PURE_EXTERNALS
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    let unexpected = references.difference(&allowed).cloned().collect::<Vec<_>>();
    let unused = allowed.difference(&references).cloned().collect::<Vec<_>>();

    if unexpected.is_empty() && unused.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Pure external-reference policy failed; unapproved references: {unexpected:?}; unused allowances: {unused:?}"
        ))
    }
}

fn read_source(root: &Path, relative_path: &str) -> Result<ProductionSource, String> {
    let path = root.join(relative_path);
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read production source {}: {error}", path.display()))?;
    let identity = graph::module_id(relative_path)
        .ok_or_else(|| format!("cannot derive module identity for {relative_path}"))?;
    if !identity.crate_dir.is_empty() {
        return Err(format!(
            "production source path unexpectedly identifies crate directory {}: {relative_path}",
            identity.crate_dir
        ));
    }
    Ok(ProductionSource {
        relative_path: relative_path.to_owned(),
        module: identity.path,
        source,
    })
}

fn discover_rust_sources(src: &Path) -> Result<BTreeSet<String>, String> {
    let mut discovered = BTreeSet::new();
    let mut pending = vec![src.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|error| {
            format!(
                "cannot enumerate production source directory {}: {error}",
                directory.display()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "cannot read an entry under {}: {error}",
                    directory.display()
                )
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| {
                format!(
                    "cannot inspect production source path {}: {error}",
                    path.display()
                )
            })?;
            if file_type.is_symlink() {
                return Err(format!(
                    "production source tree contains an unsupported symlink: {}",
                    path.display()
                ));
            }
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file()
                && path.extension().is_some_and(|extension| extension == "rs")
            {
                let relative = path
                    .strip_prefix(engine_root().join("src"))
                    .map_err(|error| format!("source path escaped src/: {error}"))?;
                discovered.insert(format!("src/{}", path_to_slashes(relative)));
            }
        }
    }
    Ok(discovered)
}

fn path_to_slashes(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn cargo_metadata(root: &Path) -> Result<String, String> {
    let manifest = root.join("Cargo.toml");
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest)
        .current_dir(root)
        .output()
        .map_err(|error| {
            format!(
                "cannot run cargo metadata for {}: {error}",
                manifest.display()
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed for {} with {}: {}",
            manifest.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("cargo metadata output is not UTF-8: {error}"))
}

fn engine_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn format_direction_violations(violations: &[DirectionViolation]) -> String {
    violations
        .iter()
        .map(|violation| match violation {
            DirectionViolation::Forbidden(edge) => {
                format!("{} may not import {}", edge.from, edge.to)
            }
            DirectionViolation::Stale(edge) => {
                format!("unused exception {} -> {}", edge.from, edge.to)
            }
            DirectionViolation::Duplicate(edge) => {
                format!("duplicate exception {} -> {}", edge.from, edge.to)
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}
