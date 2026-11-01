//! Pure coverage and direction checks. Discovery, roles and exceptions belong to callers.
use crate::production_tokens;
use crate::tokens::{normalized_path, positioned_use_imports, token_identifier};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleId {
    pub crate_dir: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoverageViolation {
    Invalid(String),
    Duplicate(String),
    Missing(String),
    Stale(String),
}

/// Compare a caller's discovered source set with its complete classification list.
pub fn check_coverage(
    discovered: &BTreeSet<String>,
    classified: &[&str],
) -> Vec<CoverageViolation> {
    let mut seen = BTreeSet::new();
    let mut problems = Vec::new();
    for file in classified {
        if !seen.insert((*file).to_owned()) {
            problems.push(CoverageViolation::Duplicate((*file).to_owned()));
        }
        if module_id(file).is_none() {
            problems.push(CoverageViolation::Invalid((*file).to_owned()));
        }
    }
    problems.extend(
        discovered
            .difference(&seen)
            .cloned()
            .map(CoverageViolation::Missing),
    );
    problems.extend(
        seen.difference(discovered)
            .cloned()
            .map(CoverageViolation::Stale),
    );
    problems
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub crate_dir: String,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirectionViolation {
    Forbidden(Edge),
    Stale(Edge),
    Duplicate(Edge),
}

/// Apply the caller's direction rule and exact exceptions; unused exceptions fail.
pub fn check_direction(
    edges: &BTreeSet<Edge>,
    exceptions: &[Edge],
    permits: impl Fn(&Edge) -> bool,
) -> Vec<DirectionViolation> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for exception in exceptions {
        if !seen.insert(exception.clone()) {
            problems.push(DirectionViolation::Duplicate(exception.clone()));
        }
        if !edges.contains(exception) {
            problems.push(DirectionViolation::Stale(exception.clone()));
        }
    }
    problems.extend(
        edges
            .iter()
            .filter(|edge| !permits(edge) && !seen.contains(*edge))
            .cloned()
            .map(DirectionViolation::Forbidden),
    );
    problems
}

/// Identity of a conventional repository-relative source path, split at its first src component.
pub fn module_id(relative_file: &str) -> Option<ModuleId> {
    let segments: Vec<&str> = relative_file.split('/').collect();
    let src = segments.iter().position(|segment| *segment == "src")?;
    let (crate_segments, module_segments) = (&segments[..src], &segments[src + 1..]);
    if crate_segments.iter().any(|segment| segment.is_empty()) {
        return None;
    }
    let mut module_segments = module_segments.to_vec();
    let file = module_segments.pop()?.strip_suffix(".rs")?;
    let path = if module_segments.is_empty() && file == "lib" {
        "crate".to_owned()
    } else {
        if file != "mod" {
            module_segments.push(file);
        }
        if module_segments.is_empty() || module_segments.iter().any(|segment| segment.is_empty()) {
            return None;
        }
        module_segments.join("::")
    };
    Some(ModuleId {
        crate_dir: crate_segments.join("/"),
        path,
    })
}

/// Internal paths through crate/self/super; bare child paths and child edges are intentionally not followed.
pub fn crate_edges(module: &str, source: &str, known_modules: &[String]) -> BTreeSet<String> {
    let tokens = without_visibility_qualifiers(production_tokens(source));
    let scopes = inline_module_scopes(&tokens);
    let module_segments: Vec<String> = if module == "crate" {
        Vec::new()
    } else {
        module.split("::").map(ToOwned::to_owned).collect()
    };

    let mut paths: Vec<(usize, Vec<String>)> = positioned_use_imports(&tokens)
        .into_iter()
        .filter_map(|(index, import)| normalized_path(&import.path).map(|path| (index, path)))
        .collect();
    paths.extend(inline_crate_paths(&tokens));

    let mut edges = BTreeSet::new();
    for (index, path) in paths {
        let mut base = module_segments.clone();
        base.extend(scopes[index].iter().cloned());
        let Some(resolved) = resolve_crate_path(&base, &path) else {
            continue;
        };
        let target = longest_known_module(&resolved, known_modules);
        let is_child = module == "crate" || target.starts_with(&format!("{module}::"));
        if target != module && !is_child {
            edges.insert(target);
        }
    }
    edges
}

/// For each token index, the names of the inline `mod <name> { ... }` blocks enclosing it,
/// outermost first. A block opens at the `{` right after `mod <identifier>` and closes at the
/// `}` that brings the brace depth back below it.
fn inline_module_scopes(tokens: &[String]) -> Vec<Vec<String>> {
    let mut scopes = Vec::with_capacity(tokens.len());
    let mut depth = 0_usize;
    // Open inline modules, innermost last, each with the brace depth inside its block.
    let mut open: Vec<(String, usize)> = Vec::new();
    // A module name waiting for its `{`, with the index of that `{`.
    let mut pending: Option<(String, usize)> = None;
    for (index, token) in tokens.iter().enumerate() {
        if token == "mod"
            && tokens.get(index + 2).is_some_and(|brace| brace == "{")
            && let Some(name) = tokens.get(index + 1).and_then(|t| token_identifier(t))
        {
            pending = Some((name.to_owned(), index + 2));
        }
        match token.as_str() {
            "{" => {
                depth += 1;
                if let Some((name, _)) = pending.take_if(|(_, brace)| *brace == index) {
                    open.push((name, depth));
                }
            }
            "}" => {
                if open.last().is_some_and(|(_, inside)| *inside == depth) {
                    open.pop();
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
        scopes.push(open.iter().map(|(name, _)| name.clone()).collect());
    }
    scopes
}

/// Drops the parenthesized part of `pub(crate)`, `pub(super)`, `pub(self)` and `pub(in path)`.
fn without_visibility_qualifiers(tokens: Vec<String>) -> Vec<String> {
    let mut kept = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        kept.push(tokens[index].clone());
        let qualifier_follows = tokens[index] == "pub"
            && tokens.get(index + 1).is_some_and(|t| t == "(")
            && tokens.get(index + 2).is_some_and(|t| {
                t == "in"
                    || (matches!(t.as_str(), "crate" | "super" | "self")
                        && tokens.get(index + 3).is_some_and(|close| close == ")"))
            });
        index += 1;
        if qualifier_follows {
            while index < tokens.len() && tokens[index] != ")" {
                index += 1;
            }
            index += 1;
        }
    }
    kept
}

/// Every `crate::`, `super::` or `self::` path written inline, with its further segments and
/// the index of its first token. A `crate`, `super` or `self` preceded by `::` continues a path
/// already read (`super::super::x`) and does not start another. `use` statements are skipped
/// up to their `;`, because the use-tree reader expands them, so a grouped prefix such as
/// `use super::super::{a, b}` adds no edge of its own; `use<...>`, a precise-capturing bound
/// inside a type, is not a statement and is not skipped.
fn inline_crate_paths(tokens: &[String]) -> Vec<(usize, Vec<String>)> {
    let mut paths = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if token == "use" && tokens.get(index + 1).is_none_or(|next| next != "<") {
            index = tokens[index..]
                .iter()
                .position(|t| t == ";")
                .map_or(tokens.len(), |offset| index + offset + 1);
            continue;
        }
        let continues_path = index > 0 && tokens[index - 1] == "::";
        let starts_path = !continues_path
            && matches!(token.as_str(), "crate" | "super" | "self")
            && tokens.get(index + 1).is_some_and(|t| t == "::")
            && tokens
                .get(index + 2)
                .is_some_and(|t| token_identifier(t).is_some());
        if !starts_path {
            index += 1;
            continue;
        }
        let mut path = vec![token.clone()];
        let mut cursor = index + 1;
        while tokens.get(cursor).is_some_and(|t| t == "::") {
            let Some(segment) = tokens.get(cursor + 1).and_then(|t| token_identifier(t)) else {
                break;
            };
            path.push(segment.to_owned());
            cursor += 2;
        }
        paths.push((index, path));
        index += 1;
    }
    paths
}

/// The module-relative segments a `crate::`, `self::` or `super::` path names, or `None`
/// for paths that start with anything else or climb above the crate root.
fn resolve_crate_path(base: &[String], path: &[String]) -> Option<Vec<String>> {
    let (mut resolved, mut index) = match path.first()?.as_str() {
        "crate" => (Vec::new(), 1),
        "self" => (base.to_vec(), 1),
        "super" => (base.to_vec(), 0),
        _ => return None,
    };
    while path.get(index).is_some_and(|segment| segment == "super") {
        resolved.pop()?;
        index += 1;
    }
    resolved.extend(path[index..].iter().cloned());
    Some(resolved)
}

/// The longest prefix of `resolved` that names a known module, or `crate` when none does.
fn longest_known_module(resolved: &[String], known_modules: &[String]) -> String {
    (1..=resolved.len())
        .rev()
        .map(|length| resolved[..length].join("::"))
        .find(|candidate| known_modules.contains(candidate))
        .unwrap_or_else(|| "crate".to_owned())
}
