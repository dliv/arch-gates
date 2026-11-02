//! Source-level ambient, external-crate and trait checks over caller-supplied text.
//! These checks share a bounded token grammar; they do not parse Rust or resolve types.

use std::collections::BTreeSet;

use crate::tokens::{
    ImportKind, after_turbofish, expand_std_paths, is_stdin_path, normalized_path,
    production_tokens, std_aliases, token_identifier, use_imports,
};

const STDIN: &str = "std::io::stdin";

/// `std::path` names with an ambient effect. This is a name scan, so same-named methods on
/// other types are also reported.
const EFFECTFUL_PATH_NAMES: [&str; 11] = [
    "exists",
    "try_exists",
    "metadata",
    "symlink_metadata",
    "canonicalize",
    "read_dir",
    "read_link",
    "is_file",
    "is_dir",
    "is_symlink",
    "absolute",
];

#[derive(Clone, Copy)]
enum EffectProfile<'a> {
    Pure,
    Root { allowed_env: &'a [&'a str] },
}

impl EffectProfile<'_> {
    fn forbidden_modules(self) -> &'static [&'static str] {
        match self {
            Self::Pure => &["fs", "env", "process", "net", "time", "thread"],
            Self::Root { .. } => &["fs", "env", "net", "thread"],
        }
    }

    fn sanctions(self, path: &[String]) -> bool {
        match self {
            Self::Pure => false,
            Self::Root { allowed_env } => {
                let path = path.iter().map(String::as_str).collect::<Vec<_>>();
                path.as_slice() == ["std", "env"]
                    || matches!(
                        path.as_slice(),
                        ["std", "env", item] if allowed_env.contains(item)
                    )
            }
        }
    }

    fn reports_stdin(self) -> bool {
        matches!(self, Self::Pure)
    }

    fn path_violations(self, path: &[String], violations: &mut BTreeSet<String>) {
        if self.reports_stdin() && is_stdin_path(path) {
            violations.insert(STDIN.to_owned());
        }
        if self.sanctions(path) {
            return;
        }
        for module in path.iter().skip(1) {
            if self.forbidden_modules().contains(&module.as_str()) {
                violations.insert(format!("std::{module}"));
            }
        }
    }
}

/// Ambient-effect violations for a Pure module. Reports the same listed effects and source
/// escapes as the historical Workforest scanner.
pub(crate) fn pure_violations(source: &str) -> Vec<String> {
    effect_violations(source, EffectProfile::Pure)
}

/// Ambient-effect violations for a Root module.
///
/// Filesystem, network and thread paths are forbidden. The `std::env` namespace can be imported,
/// but only exact `std::env::<name>` paths named by the consumer are permitted. Process, clock,
/// stream and other environment restrictions remain the consumer's native compiler policy.
pub fn root_ambient_violations(source: &str, allowed_env: &[&str]) -> Vec<String> {
    effect_violations(source, EffectProfile::Root { allowed_env })
}

fn effect_violations(source: &str, profile: EffectProfile<'_>) -> Vec<String> {
    let tokens = production_tokens(source);
    let forbidden_imported_macros = ["include"];
    let imports = use_imports(&tokens);
    let aliases = std_aliases(&imports);
    let mut violations = BTreeSet::new();

    for import in &imports {
        if let Some(path) = normalized_path(&import.path) {
            if import.path.last().is_some_and(|segment| segment == "*")
                && matches!(path.first().map(String::as_str), Some("std" | "core"))
            {
                violations.insert(format!("{}::*", path[0]));
            }
            if path.as_slice() == ["core"] && import.alias.is_some() {
                violations.insert("core-alias".to_owned());
            }
            if path
                .last()
                .is_some_and(|name| forbidden_imported_macros.contains(&name.as_str()))
            {
                violations.insert("include!".to_owned());
            }
            if let Some(name) = path.last().filter(|name| {
                import.kind == ImportKind::Name && EFFECTFUL_PATH_NAMES.contains(&name.as_str())
            }) {
                violations.insert(format!("path effect {name}"));
            }
        }
        for path in expand_std_paths(&import.path, &aliases) {
            if import.path.last().is_some_and(|segment| segment == "*") {
                violations.insert("std::*".to_owned());
            }
            if path.len() == 1 {
                violations.insert("std-root-import".to_owned());
            }
            profile.path_violations(&path, &mut violations);
        }
    }

    for (index, token) in tokens.iter().enumerate() {
        if token_identifier(token).is_some() && !token.is_ascii() {
            violations.insert("non-ascii-identifier".to_owned());
        }
        if token == "unsafe" || token == "extern" {
            violations.insert(token.clone());
        }
        if token == "macro_rules" && tokens.get(index + 1).is_some_and(|token| token == "!") {
            violations.insert("macro_rules!".to_owned());
        }
        if token_identifier(token) == Some("include")
            && tokens.get(index + 1).is_some_and(|token| token == "!")
        {
            violations.insert("include!".to_owned());
        }
        if token == "#" && tokens.get(index + 1).is_some_and(|token| token == "[") {
            let mut cursor = index + 2;
            let mut depth = 1_usize;
            while cursor < tokens.len() && depth != 0 {
                match tokens[cursor].as_str() {
                    "[" => depth += 1,
                    "]" => depth -= 1,
                    _ if depth != 0
                        && token_identifier(&tokens[cursor]) == Some("path")
                        && tokens.get(cursor + 1).is_some_and(|token| token == "=") =>
                    {
                        violations.insert("#[path]".to_owned());
                    }
                    _ => {}
                }
                cursor += 1;
            }
        }

        if let Some(name) =
            token_identifier(token).filter(|name| EFFECTFUL_PATH_NAMES.contains(name))
        {
            let previous = index.checked_sub(1).map(|before| tokens[before].as_str());
            // A module/type namespace component cannot itself be a filesystem query.
            // Preserve terminal names and turbofish (`metadata::<T>`) as query forms.
            let namespace_component = tokens.get(index + 1).is_some_and(|next| next == "::")
                && tokens.get(index + 2).is_some_and(|next| {
                    token_identifier(next).is_some() || matches!(next.as_str(), "{" | "*")
                });
            let after_path_separator = previous == Some("::") && !namespace_component;
            let called_as_method = previous == Some(".")
                && tokens
                    .get(after_turbofish(&tokens, index + 1))
                    .is_some_and(|next| next == "(");
            if after_path_separator || called_as_method {
                violations.insert(format!("path effect {name}"));
            }
        }

        let prefixes = if token_identifier(token) == Some("std") {
            [vec!["std".to_owned()]].into_iter().collect()
        } else {
            aliases
                .get(token)
                .or_else(|| token_identifier(token).and_then(|identifier| aliases.get(identifier)))
                .cloned()
                .unwrap_or_default()
        };
        for mut path in prefixes {
            let mut cursor = index + 1;
            while tokens.get(cursor).is_some_and(|token| token == "::") {
                cursor += 1;
                if tokens.get(cursor).is_some_and(|token| token == "$") {
                    violations.insert("std::$dynamic".to_owned());
                    break;
                }
                let Some(segment) = tokens.get(cursor).and_then(|token| token_identifier(token))
                else {
                    break;
                };
                path.push(segment.to_owned());
                cursor += 1;
            }
            profile.path_violations(&path, &mut violations);
        }
    }

    violations.into_iter().collect()
}

/// Dependency crates named by production source, using the bounded donor grammar. The scan
/// finds the first `use` path segment and dependency identifiers followed by `::`, except for
/// qualified crate-internal paths.
pub fn external_crate_references(
    source: &str,
    dependencies: &BTreeSet<String>,
) -> BTreeSet<String> {
    let tokens = production_tokens(source);
    let mut references = BTreeSet::new();
    for import in use_imports(&tokens) {
        let first = normalized_path(&import.path).and_then(|path| path.into_iter().next());
        if let Some(name) = first.filter(|name| dependencies.contains(name)) {
            references.insert(name);
        }
    }
    for (index, token) in tokens.iter().enumerate() {
        let Some(name) = token_identifier(token).filter(|name| dependencies.contains(*name)) else {
            continue;
        };
        if tokens.get(index + 1).is_none_or(|next| next != "::") {
            continue;
        }
        let crate_internal = index >= 2
            && tokens[index - 1] == "::"
            && matches!(
                tokens[index - 2].as_str(),
                "crate" | "self" | "super" | "Self"
            );
        if !crate_internal {
            references.insert(name.to_owned());
        }
    }
    references
}

/// Trait names declared by production tokens (`trait` followed by an identifier).
pub fn trait_declarations(source: &str) -> Vec<String> {
    let tokens = production_tokens(source);
    tokens
        .windows(2)
        .filter(|pair| pair[0] == "trait")
        .filter_map(|pair| token_identifier(&pair[1]).map(ToOwned::to_owned))
        .collect()
}
