//! Pure-source checks extracted from Workforest. Reads no files and starts no processes.
mod tokens;
use std::collections::BTreeSet;
use tokens::{
    expand_std_paths, normalized_path, rust_code_tokens, std_aliases, token_identifier, use_imports,
};

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

fn record_path(path: &[String], violations: &mut BTreeSet<String>) {
    if is_stdin_path(path) {
        violations.insert(STDIN.to_owned());
    }
    for module in path.iter().skip(1) {
        if ["fs", "env", "process", "net", "time", "thread"].contains(&module.as_str()) {
            violations.insert(format!("std::{module}"));
        }
    }
}

/// Reported for any use of standard input, which reads the outside world.
const STDIN: &str = "std::io::stdin";

/// `std::path` names with an ambient effect: the `Path` methods that query the filesystem,
/// and `absolute`, which reads the working directory. Reported by name when called as a
/// method, with or without a turbofish (`path.exists()`, `path.exists::<>()`), after `::`
/// whether called or not (`Path::exists(p)`, `.any(Path::exists)`, `std::path::absolute(p)`),
/// and as the last segment of a `use` import (`use std::path::{absolute, Path};`). The scan
/// does not know types, so a same-named item of another type is reported too.
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

/// Tokens of the file minus every test-only inline module: a `mod <name> { ... }` block
/// (matched braces) whose outer attributes include `#[cfg(test)]`, whatever other attributes
/// come before or after it and whatever its visibility. The attributes are removed with it.
fn production_tokens(source: &str) -> Vec<String> {
    let tokens = rust_code_tokens(source);
    let mut kept = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        match test_module_end(&tokens, index) {
            Some(end) => index = end,
            None => {
                kept.push(tokens[index].clone());
                index += 1;
            }
        }
    }
    kept
}

/// If a test-only inline module starts at `start`, the index just past its matching closing
/// brace. The module starts with one or more consecutive outer attributes `#[ ... ]`
/// (balanced brackets), one of which is exactly `#[cfg(test)]`, then an optional `pub` or
/// `pub(...)`, then `mod <name> {`. Any other shape, and an unterminated block, is production
/// code.
fn test_module_end(tokens: &[String], start: usize) -> Option<usize> {
    let mut cursor = start;
    let mut has_cfg_test = false;
    while tokens.get(cursor).is_some_and(|t| t == "#") && tokens.get(cursor + 1)? == "[" {
        let close = matching_close(tokens, cursor + 1, "[", "]")?;
        has_cfg_test |= tokens[cursor + 2..close] == ["cfg", "(", "test", ")"];
        cursor = close + 1;
    }
    if !has_cfg_test {
        return None;
    }
    if tokens.get(cursor)? == "pub" {
        cursor += 1;
        if tokens.get(cursor)? == "(" {
            cursor = matching_close(tokens, cursor, "(", ")")? + 1;
        }
    }
    if tokens.get(cursor)? != "mod" {
        return None;
    }
    token_identifier(tokens.get(cursor + 1)?)?;
    let open = cursor + 2;
    if tokens.get(open)? != "{" {
        return None;
    }
    matching_close(tokens, open, "{", "}").map(|close| close + 1)
}

/// The index of the token closing the `open_token` at `open`, counting nested pairs, or
/// `None` when it is never closed.
fn matching_close(
    tokens: &[String],
    open: usize,
    open_token: &str,
    close_token: &str,
) -> Option<usize> {
    let mut depth = 0_usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if token == open_token {
            depth += 1;
        } else if token == close_token {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn is_stdin_path(path: &[String]) -> bool {
    matches!(
        path,
        [root, io, name, ..] if root == "std" && io == "io" && (name == "stdin" || name == "Stdin")
    )
}

/// The index just past a turbofish `::<...>` (balanced `<` and `>` tokens) starting at
/// `index`, or `index` itself when none starts there.
fn after_turbofish(tokens: &[String], index: usize) -> usize {
    let starts_turbofish = tokens.get(index).is_some_and(|t| t == "::")
        && tokens.get(index + 1).is_some_and(|t| t == "<");
    if !starts_turbofish {
        return index;
    }
    let mut depth = 0_usize;
    for (cursor, token) in tokens.iter().enumerate().skip(index + 1) {
        match token.as_str() {
            "<" => depth += 1,
            ">" => {
                depth -= 1;
                if depth == 0 {
                    return cursor + 1;
                }
            }
            _ => {}
        }
    }
    index
}

/// Forbidden forms in production tokens, sorted and deduplicated. This is a source scan, not a transitive effect system.
pub fn pure_violations(source: &str) -> Vec<String> {
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
            let imported_macro = path
                .last()
                .filter(|name| forbidden_imported_macros.contains(&name.as_str()));
            if let Some(name) = imported_macro {
                violations.insert(format!("{name}!"));
            }
            let imported_path_effect = path
                .last()
                .filter(|name| EFFECTFUL_PATH_NAMES.contains(&name.as_str()));
            if let Some(name) = imported_path_effect {
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
            record_path(&path, &mut violations);
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
            let after_path_separator = previous == Some("::");
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
        if prefixes.is_empty() {
            continue;
        }
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
            record_path(&path, &mut violations);
        }
    }

    violations.into_iter().collect()
}
