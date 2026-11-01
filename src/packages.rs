//! Pure dependency evaluation. Consumers own classification, layout and edge permission.
use crate::metadata::{Dependency, DependencyKind, Member};
use std::collections::BTreeMap;
const SECTION: &str = "see consumer package policy";

/// A workspace unit's role. Consumers own layout, containment and internal-edge policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrateClass {
    /// Pure decisions of the named feature; shared checks enforce declared Pure capabilities.
    Pure { feature: &'static str },
    /// Effectful workflows of the named feature.
    Effectful { feature: &'static str },
    /// A protocol boundary.
    Edge,
    /// A composition/application package, including explicit local transition allowances.
    Binary,
    /// Development tooling; production dependencies on this role are prohibited.
    Tooling,
}

/// The external crates each class may declare as normal or build dependencies, passed to
/// `check_edges` together.
#[derive(Clone, Copy, Debug)]
pub struct ExternalAllowlists<'a> {
    /// `PURE_ALLOWED_CRATES`: each crate with the features a Pure crate may declare for it.
    pub pure: &'a [(&'a str, &'a [&'a str])],
    /// `EFFECTFUL_ALLOWED_CRATES`.
    pub effectful: &'a [&'a str],
    /// `EDGE_ALLOWED_CRATES`.
    pub edge: &'a [&'a str],
}

fn describe(class: CrateClass) -> String {
    match class {
        CrateClass::Pure { feature } => format!("Pure crate of feature {feature}"),
        CrateClass::Effectful { feature } => format!("Effectful crate of feature {feature}"),
        CrateClass::Edge => "Edge crate".to_owned(),
        CrateClass::Binary => "Binary crate".to_owned(),
        CrateClass::Tooling => "Tooling crate".to_owned(),
    }
}

/// What `CRATES` says about one package name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Classification {
    /// No entry names the package.
    Missing,
    /// More than one entry names the package, so it has no single class. Reported once, before
    /// any class rule, and no class rule is applied to it.
    Repeated,
    /// Exactly one entry names the package.
    Class(CrateClass),
}

fn classification(crates: &[(&str, CrateClass)], name: &str) -> Classification {
    let mut classes = crates
        .iter()
        .filter(|(crate_name, _)| *crate_name == name)
        .map(|(_, class)| *class);
    match (classes.next(), classes.next()) {
        (None, _) => Classification::Missing,
        (Some(class), None) => Classification::Class(class),
        (Some(_), Some(_)) => Classification::Repeated,
    }
}

/// One entry of a package's `[features]` table, as Cargo reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FeatureEntry<'a> {
    /// `dep:<name>`: enables an optional dependency, with the features its declaration lists.
    OptionalDependency,
    /// `<dep>/<feat>` or `<dep>?/<feat>`: enables feature `feat` of the dependency the manifest
    /// declares as `dep` (with `?`, only when that optional dependency is enabled otherwise).
    DependencyFeature {
        dependency: &'a str,
        feature: &'a str,
    },
    /// A bare name: enables another feature of the same package.
    PackageFeature,
}

fn feature_entry(entry: &str) -> FeatureEntry<'_> {
    if entry.starts_with("dep:") {
        return FeatureEntry::OptionalDependency;
    }
    match entry.split_once('/') {
        Some((dependency, feature)) => FeatureEntry::DependencyFeature {
            dependency: dependency.strip_suffix('?').unwrap_or(dependency),
            feature,
        },
        None => FeatureEntry::PackageFeature,
    }
}

/// Every way `members` break the crate rules, given each member's class in `crates` and the
/// external crates each class may declare (`allowed`: the Pure crates' with their pinned
/// features, the Effectful and Edge crates' by name). A name listed more than once in `crates`
/// is reported once, and no class rule is applied to it. An Effectful or Edge entry that no
/// member of that class declares as a normal or build dependency is reported as stale. Each
/// message names the member, the rule and the fix.
pub fn check_edges(
    members: &[Member],
    crates: &[(&str, CrateClass)],
    allowed: ExternalAllowlists<'_>,
    forbidden_tools: &[&str],
    permits: impl Fn(CrateClass, CrateClass) -> Result<(), &'static str>,
) -> Vec<String> {
    let pure_allowed = allowed.pure;
    let is_member = |name: &str| members.iter().any(|member| member.name == name);

    let mut problems = Vec::new();
    let mut entries: BTreeMap<&str, Vec<CrateClass>> = BTreeMap::new();
    for (name, class) in crates {
        entries.entry(name).or_default().push(*class);
    }
    for (name, classes) in &entries {
        if classes.len() > 1 {
            let listed: Vec<String> = classes.iter().map(|class| describe(*class)).collect();
            problems.push(format!(
                "{name} is listed {} times in CRATES ({}), but every member has exactly one \
                 crate class: keep one entry for it, with its class, in CRATES in \
                 local package policy; {SECTION}",
                classes.len(),
                listed.join(", ")
            ));
        }
        if !is_member(name) {
            problems.push(format!(
                "{name} is in CRATES but is not a workspace member: remove its entry from \
                 CRATES in local package policy; {SECTION}"
            ));
        }
    }
    for member in members {
        let class = classification(crates, &member.name);
        match class {
            Classification::Missing => problems.push(format!(
                "{} is not classified: add it to CRATES in local package policy with its \
                 crate class; {SECTION}",
                member.name
            )),
            // Reported once above.
            Classification::Repeated => {}
            Classification::Class(CrateClass::Pure { .. }) => {
                check_pure_build_script(member, &mut problems);
                check_pure_forwarding(member, &is_member, pure_allowed, &mut problems);
            }
            Classification::Class(
                CrateClass::Effectful { .. }
                | CrateClass::Edge
                | CrateClass::Binary
                | CrateClass::Tooling,
            ) => {}
        }
        if !member.inherits_workspace_lints {
            problems.push(format!(
                "{} does not inherit the workspace lints: its Cargo.toml needs a `[lints]` table \
                 containing `workspace = true` (the only form recognized); {SECTION}",
                member.name
            ));
        }
        for dependency in &member.dependencies {
            if forbidden_tools.contains(&dependency.name.as_str()) {
                if class != Classification::Class(CrateClass::Tooling) {
                    problems.push(format!("{} depends on forbidden tooling package {} ({:?}); production may not depend on tooling; {SECTION}", member.name, dependency.name, dependency.kind));
                } else if dependency.kind != DependencyKind::Dev {
                    problems.push(format!(
                        "{} must use tooling package {} only as a dev-dependency; {SECTION}",
                        member.name, dependency.name
                    ));
                }
            }
            let target = classification(crates, &dependency.name);
            if !is_member(&dependency.name) {
                if dependency.is_path {
                    problems.push(format!(
                        "{} has a path dependency on {}, which is not a workspace member: add it \
                         to [workspace] members in the root Cargo.toml, or remove the \
                         dependency; {SECTION}",
                        member.name, dependency.name
                    ));
                }
                if dependency.kind != DependencyKind::Dev {
                    match class {
                        Classification::Class(CrateClass::Pure { .. }) => {
                            check_pure_external(member, dependency, pure_allowed, &mut problems);
                        }
                        Classification::Class(CrateClass::Effectful { .. }) => {
                            let listed = ListedClass::Effectful;
                            check_listed_external(
                                member,
                                dependency,
                                listed,
                                allowed,
                                &mut problems,
                            );
                        }
                        Classification::Class(CrateClass::Edge) => {
                            let listed = ListedClass::Edge;
                            check_listed_external(
                                member,
                                dependency,
                                listed,
                                allowed,
                                &mut problems,
                            );
                        }
                        Classification::Class(CrateClass::Binary | CrateClass::Tooling)
                        | Classification::Missing
                        | Classification::Repeated => {}
                    }
                }
                continue;
            }
            match (class, target) {
                (_, Classification::Class(CrateClass::Tooling)) => problems.push(format!(
                    "{} depends on the Tooling crate {}: no member may depend on Tooling, in any \
                     dependency table; remove the dependency; {SECTION}",
                    member.name, dependency.name
                )),
                (Classification::Class(from), Classification::Class(to)) => {
                    if dependency.kind != DependencyKind::Dev
                        && let Err(rule) = permits(from, to)
                    {
                        problems.push(format!(
                            "{} ({}) depends on {} ({}), but {}: remove the dependency, or move \
                             the code it needs; {SECTION}",
                            member.name,
                            describe(from),
                            dependency.name,
                            describe(to),
                            rule
                        ));
                    }
                }
                // An unclassified or repeated member, on either side, is reported once above.
                (Classification::Missing | Classification::Repeated, _)
                | (_, Classification::Missing | Classification::Repeated) => {}
            }
        }
    }
    for listed in [ListedClass::Effectful, ListedClass::Edge] {
        check_listed_staleness(members, crates, listed, allowed, &mut problems);
    }
    problems
}

/// The two classes whose external crates are allowed by name alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListedClass {
    Effectful,
    Edge,
}

impl ListedClass {
    fn holds(self, class: CrateClass) -> bool {
        match (self, class) {
            (ListedClass::Effectful, CrateClass::Effectful { .. }) => true,
            (ListedClass::Edge, CrateClass::Edge) => true,
            (
                ListedClass::Effectful,
                CrateClass::Pure { .. }
                | CrateClass::Edge
                | CrateClass::Binary
                | CrateClass::Tooling,
            ) => false,
            (
                ListedClass::Edge,
                CrateClass::Pure { .. }
                | CrateClass::Effectful { .. }
                | CrateClass::Binary
                | CrateClass::Tooling,
            ) => false,
        }
    }

    /// The class as a message names it, with its article.
    fn noun(self) -> &'static str {
        match self {
            ListedClass::Effectful => "an Effectful crate",
            ListedClass::Edge => "an Edge crate",
        }
    }

    /// The class name alone, for "no ... crate declares it".
    fn name(self) -> &'static str {
        match self {
            ListedClass::Effectful => "Effectful",
            ListedClass::Edge => "Edge",
        }
    }

    fn list<'a>(self, allowed: ExternalAllowlists<'a>) -> &'a [&'a str] {
        match self {
            ListedClass::Effectful => allowed.effectful,
            ListedClass::Edge => allowed.edge,
        }
    }

    fn list_name(self) -> &'static str {
        match self {
            ListedClass::Effectful => "EFFECTFUL_ALLOWED_CRATES",
            ListedClass::Edge => "EDGE_ALLOWED_CRATES",
        }
    }
}

fn check_pure_build_script(member: &Member, problems: &mut Vec<String>) {
    if member.has_build_script {
        problems.push(format!(
            "{} is a Pure crate with a build script, which runs code at build time: remove the \
             build script, or reclassify the crate in CRATES; {SECTION}",
            member.name
        ));
    }
}

/// A normal or build dependency of an Effectful or Edge crate on an external crate must be in
/// that class's list.
fn check_listed_external(
    member: &Member,
    dependency: &Dependency,
    listed: ListedClass,
    allowed: ExternalAllowlists<'_>,
    problems: &mut Vec<String>,
) {
    if !listed.list(allowed).contains(&dependency.name.as_str()) {
        problems.push(format!(
            "{} is {} and depends on {}, which is not in {} (local package policy): \
             remove the dependency, or allow the crate there with a reason; {SECTION}",
            member.name,
            listed.noun(),
            dependency.name,
            listed.list_name()
        ));
    }
}

/// Every entry of an Effectful or Edge list must be declared, as a normal or build dependency,
/// by some member of that class, so the list holds exactly what those crates need.
fn check_listed_staleness(
    members: &[Member],
    crates: &[(&str, CrateClass)],
    listed: ListedClass,
    allowed: ExternalAllowlists<'_>,
    problems: &mut Vec<String>,
) {
    let declared = |name: &str| {
        members.iter().any(|member| {
            let in_class = match classification(crates, &member.name) {
                Classification::Class(class) => listed.holds(class),
                Classification::Missing | Classification::Repeated => false,
            };
            in_class
                && member.dependencies.iter().any(|dependency| {
                    dependency.kind != DependencyKind::Dev && dependency.name == name
                })
        })
    };
    for name in listed.list(allowed) {
        if !declared(name) {
            problems.push(format!(
                "{name} is in {} (local package policy), but no {} crate declares it as a \
                 normal or build dependency: remove the entry; {SECTION}",
                listed.list_name(),
                listed.name()
            ));
        }
    }
}

/// The features `PURE_ALLOWED_CRATES` pins for the external crate `name`; None when the crate
/// is not allowed in Pure crates.
fn pinned_features<'a>(
    pure_allowed: &[(&str, &'a [&'a str])],
    name: &str,
) -> Option<&'a [&'a str]> {
    pure_allowed
        .iter()
        .find(|(allowed, _)| *allowed == name)
        .map(|(_, pinned)| *pinned)
}

fn check_pure_external(
    member: &Member,
    dependency: &Dependency,
    pure_allowed: &[(&str, &[&str])],
    problems: &mut Vec<String>,
) {
    let Some(pinned) = pinned_features(pure_allowed, &dependency.name) else {
        problems.push(format!(
            "{} is a Pure crate and depends on {}, which is not in PURE_ALLOWED_CRATES \
             (local pure policy): remove the dependency, or allow the crate \
             there with a reason; {SECTION}",
            member.name, dependency.name
        ));
        return;
    };
    for feature in &dependency.features {
        if !pinned.contains(&feature.as_str()) {
            problems.push(format!(
                "{} declares feature {feature} of {}, which PURE_ALLOWED_CRATES \
                 (local pure policy) does not pin for it (pinned: [{}]): \
                 remove the feature, or pin it there with a reason; {SECTION}",
                member.name,
                dependency.name,
                pinned.join(", ")
            ));
        }
    }
}

/// Every `<dep>/<feat>` or `<dep>?/<feat>` entry of a Pure member's own `[features]` table
/// whose `<dep>` is a normal or build dependency on an allowed external crate must forward a
/// feature that `PURE_ALLOWED_CRATES` pins for that crate. Forwarding to a dev-dependency is
/// exempt, like the dev-dependency itself; a dependency on a crate that is not allowed is
/// reported by `check_pure_external`, and a dependency on a member is checked by the direction
/// rule.
fn check_pure_forwarding(
    member: &Member,
    is_member: &impl Fn(&str) -> bool,
    pure_allowed: &[(&str, &[&str])],
    problems: &mut Vec<String>,
) {
    for (package_feature, entries) in &member.features {
        for entry in entries {
            match feature_entry(entry) {
                FeatureEntry::OptionalDependency | FeatureEntry::PackageFeature => {}
                FeatureEntry::DependencyFeature {
                    dependency: manifest_name,
                    feature,
                } => {
                    let Some(dependency) = member.dependencies.iter().find(|dependency| {
                        dependency.kind != DependencyKind::Dev
                            && dependency.manifest_name() == manifest_name
                    }) else {
                        continue;
                    };
                    if is_member(&dependency.name) {
                        continue;
                    }
                    let Some(pinned) = pinned_features(pure_allowed, &dependency.name) else {
                        continue;
                    };
                    if !pinned.contains(&feature) {
                        problems.push(format!(
                            "{} forwards feature {feature} of {} through its feature \
                             {package_feature} (`{entry}`), which PURE_ALLOWED_CRATES \
                             (local pure policy) does not pin for it (pinned: \
                             [{}]): remove the entry from its [features] table, or pin the \
                             feature there with a reason; {SECTION}",
                            member.name,
                            dependency.name,
                            pinned.join(", ")
                        ));
                    }
                }
            }
        }
    }
}
