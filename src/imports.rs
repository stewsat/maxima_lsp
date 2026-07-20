// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{self, Url};

use crate::definitions::{collect_definitions_from_source, deepest_node_at, extract_load_argument};
use crate::docstring::{self, ExternalDoc};
use crate::lisp_extractor;
use crate::paths::PathResolver;

const IMPORT_FUNCS: &[&str] = &["load", "batch", "batchload", "import"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSpec {
    pub package: String,
    pub version: Option<String>,
}

/// If cursor is on a module argument of load/import/batch, return the module name.
pub fn import_module_at_position(
    root: tree_sitter::Node,
    byte: usize,
    source: &str,
) -> Option<String> {
    let mut node = deepest_node_at(root, byte)?;
    let mut cursor = node.walk();

    loop {
        if node.kind() == "function_call" {
            if let Some(spec) = extract_import_from_call(node, source) {
                let start = node.start_byte();
                let end = node.end_byte();
                if byte >= start && byte < end {
                    return Some(spec.package);
                }
            }
        }
        if !cursor.goto_parent() {
            break;
        }
        node = cursor.node();
    }
    None
}

fn call_name(node: tree_sitter::Node, source: &str) -> Option<String> {
    node.named_child(0)
        .and_then(|n| {
            if n.kind() == "atom" {
                n.named_child(0).or(n.child(0))
            } else {
                Some(n)
            }
        })
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string())
}

fn extract_import_from_call(node: tree_sitter::Node, source: &str) -> Option<ImportSpec> {
    let fname = call_name(node, source)?;
    if !IMPORT_FUNCS.contains(&fname.as_str()) {
        return None;
    }

    let mut package = None;
    let mut version = None;
    let nc = node.named_child_count();

    for ci in 1..nc {
        let Some(ch) = node.named_child(ci as u32) else {
            continue;
        };
        if package.is_none() {
            if let Some(raw) = extract_load_argument(ch, source) {
                package = Some(raw);
                continue;
            }
        }
        if version.is_none() {
            version = extract_version_argument(ch, source);
        }
    }

    package.map(|package| ImportSpec { package, version })
}

fn extract_version_argument(node: tree_sitter::Node, source: &str) -> Option<String> {
    if node.kind() != "list" {
        return None;
    }
    let nc = node.named_child_count();
    for ci in 0..nc {
        let ch = node.named_child(ci as u32)?;
        if let Some(v) = extract_load_argument(ch, source) {
            return Some(v);
        }
    }
    None
}

fn find_lisp_imports(source: &str) -> Vec<ImportSpec> {
    let mut calls = Vec::new();
    let lang: tree_sitter::Language = tree_sitter_commonlisp::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return calls;
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return calls,
    };

    let mut cursor = tree.walk();
    let mut entering = true;
    loop {
        let node = cursor.node();
        if entering && node.kind() == "list" {
            let first = node.named_child(0);
            if let Some(first) = first {
                if first.kind() == "symbol" {
                    if let Ok(name) = first.utf8_text(source.as_bytes()) {
                        if IMPORT_FUNCS.contains(&name) {
                            let nc = node.named_child_count();
                            for ci in 1..nc {
                                if let Some(arg) = node.named_child(ci as u32) {
                                    if let Some(raw) = extract_lisp_load_arg(arg, source) {
                                        calls.push(ImportSpec {
                                            package: raw,
                                            version: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if entering && cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            entering = true;
            continue;
        }
        if cursor.goto_parent() {
            entering = false;
            continue;
        }
        break;
    }

    calls
}

fn extract_lisp_load_arg(node: tree_sitter::Node, source: &str) -> Option<String> {
    match node.kind() {
        "string" => node
            .utf8_text(source.as_bytes())
            .ok()
            .map(|s| s.trim_matches('"').to_string())
            .filter(|s| !s.is_empty()),
        "symbol" | "atom" => node
            .utf8_text(source.as_bytes())
            .ok()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn find_maxima_imports(source: &str) -> Vec<ImportSpec> {
    let mut calls = Vec::new();
    let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return calls;
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return calls,
    };

    let mut cursor = tree.walk();
    let mut entering = true;
    loop {
        let node = cursor.node();
        if entering && node.kind() == "function_call" {
            if let Some(spec) = extract_import_from_call(node, source) {
                calls.push(spec);
            }
        }
        if entering && cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            entering = true;
            continue;
        }
        if cursor.goto_parent() {
            entering = false;
            continue;
        }
        break;
    }

    calls
}

fn find_import_specs(source: &str) -> Vec<ImportSpec> {
    let maxima = find_maxima_imports(source);
    if !maxima.is_empty() {
        return maxima;
    }
    find_lisp_imports(source)
}

pub fn find_imports(source: &str, base_dir: &Path, resolver: &mut PathResolver) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for spec in find_import_specs(source) {
        if let Some(path) = resolver.resolve_import(&spec.package, spec.version.as_deref(), base_dir) {
            if seen.insert(path.clone()) {
                results.push(path);
            }
        }
    }
    results
}

pub fn resolve_import_path(
    name: &str,
    base_dir: &Path,
    resolver: &mut PathResolver,
) -> Option<PathBuf> {
    resolver.resolve_import(name, None, base_dir)
}

pub fn resolve_imports(
    source: &str,
    base_dir: &Path,
    resolver: &mut PathResolver,
) -> (HashMap<String, ExternalDoc>, HashMap<String, lsp_types::Location>) {
    let mut all_docs = HashMap::new();
    let mut all_defs = HashMap::new();
    let mut seen = HashSet::new();
    let mut queue: Vec<(ImportSpec, PathBuf)> = Vec::new();

    for spec in find_import_specs(source) {
        if let Some(path) = resolver.resolve_import(&spec.package, spec.version.as_deref(), base_dir) {
            if seen.insert(path.clone()) {
                queue.push((spec, path));
            }
        }
    }

    while let Some((_spec, path)) = queue.pop() {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        let path_str = path.to_string_lossy().to_string();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let (docs, defs) = match ext.as_str() {
            "lisp" | "lsp" => {
                let docs = lisp_extractor::extract_lisp_docs(&content);
                if let Ok(uri) = Url::from_file_path(&path) {
                    let defs = lisp_extractor::collect_lisp_defs(&content, &uri);
                    (docs, defs)
                } else {
                    (docs, HashMap::new())
                }
            }
            _ => {
                let docs = docstring::extract_docstrings(&content);
                if let Ok(uri) = Url::from_file_path(&path) {
                    let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
                    let defs = collect_definitions_from_source(&content, &uri, lang);
                    (docs, defs)
                } else {
                    (docs, HashMap::new())
                }
            }
        };

        let mut docs = docs;
        for (_, doc) in docs.iter_mut() {
            doc.source_file = path_str.clone();
        }

        for (name, _) in &docs {
            if all_docs.contains_key(name) {
                tracing::debug!("Duplicate function definition '{name}' in import resolution");
            }
        }

        all_docs.extend(docs);
        for (name, loc) in defs {
            all_defs.entry(name).or_insert(loc);
        }

        let sub_dir = path.parent().unwrap_or(base_dir);
        let sub_imports = match ext.as_str() {
            "lisp" | "lsp" => find_lisp_imports(&content),
            _ => find_maxima_imports(&content),
        };
        for spec in sub_imports {
            if let Some(sub_path) =
                resolver.resolve_import(&spec.package, spec.version.as_deref(), sub_dir)
            {
                if seen.insert(sub_path.clone()) {
                    queue.push((spec, sub_path));
                }
            }
        }
    }

    (all_docs, all_defs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packages(specs: &[ImportSpec]) -> Vec<String> {
        specs.iter().map(|s| s.package.clone()).collect()
    }

    #[test]
    fn test_find_maxima_imports_load() {
        let src = r#"load("utils");"#;
        assert_eq!(packages(&find_maxima_imports(src)), vec!["utils"]);
    }

    #[test]
    fn test_find_maxima_imports_identifier() {
        let src = r#"import(colors);"#;
        assert_eq!(packages(&find_maxima_imports(src)), vec!["colors"]);
    }

    #[test]
    fn test_find_maxima_imports_import() {
        let src = r#"import("colors");"#;
        assert_eq!(packages(&find_maxima_imports(src)), vec!["colors"]);
    }

    #[test]
    fn test_find_maxima_imports_versioned() {
        let src = r#"import(colors, ["1.0.0"]);"#;
        let specs = find_maxima_imports(src);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].package, "colors");
        assert_eq!(specs[0].version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn test_find_maxima_imports_batch() {
        let src = r#"batch("setup");"#;
        assert_eq!(packages(&find_maxima_imports(src)), vec!["setup"]);
    }

    #[test]
    fn test_find_maxima_imports_multiple() {
        let src = r#"
load("utils");
import("colors");
batch("setup");
"#;
        let calls = packages(&find_maxima_imports(src));
        assert_eq!(calls.len(), 3);
        assert!(calls.contains(&"utils".to_string()));
        assert!(calls.contains(&"colors".to_string()));
        assert!(calls.contains(&"setup".to_string()));
    }

    #[test]
    fn test_find_maxima_imports_no_match() {
        let src = r#"f(x) := x^2$"#;
        assert!(find_maxima_imports(src).is_empty());
    }

    #[test]
    fn test_find_lisp_imports_load() {
        let src = r#"(load "numerical/fzero")"#;
        assert_eq!(packages(&find_lisp_imports(src)), vec!["numerical/fzero"]);
    }
}

#[cfg(test)]
mod maxpack_integration {
    use super::*;
    use crate::paths::PathResolver;

    #[test]
    fn resolve_import_colors_indexes_functions() {
        let mut resolver = PathResolver::discover();
        let home = std::env::var("HOME").unwrap();
        let colors_init = format!("{home}/.maxpack/colors/latest/src/init.mac");
        if !std::path::Path::new(&colors_init).exists() {
            return;
        }

        let src = r#"import(colors)$ colorsRed("hi");"#;
        let (docs, defs) = resolve_imports(src, std::path::Path::new("/tmp"), &mut resolver);
        assert!(
            defs.contains_key("colorsRed") || docs.contains_key("colorsRed"),
            "expected colorsRed from maxpack import, defs={:?} docs={:?}",
            defs.keys().collect::<Vec<_>>(),
            docs.keys().collect::<Vec<_>>()
        );
    }
}
