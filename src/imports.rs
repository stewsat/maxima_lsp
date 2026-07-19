use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{self, Url};

use crate::docstring::{self, ExternalDoc};
use crate::lisp_extractor;

const IMPORT_FUNCS: &[&str] = &["load", "batch", "batchload", "import"];

fn maxima_share_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    for base in &[
        "/opt/homebrew/share/maxima/",
        "/usr/local/share/maxima/",
        "/opt/local/share/maxima/",
        "/usr/share/maxima/",
    ] {
        if let Ok(entries) = std::fs::read_dir(Path::new(base)) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let share = path.join("share");
                    if share.is_dir() {
                        tracing::debug!("  found Maxima share: {:?}", share);
                        dirs.push(share);
                    }
                }
            }
        }
    }

    for base in &["/Applications/Maxima.app/Contents/Resources/maxima/"] {
        if let Ok(entries) = std::fs::read_dir(Path::new(base)) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let share = path.join("share");
                    if share.is_dir() {
                        dirs.push(share);
                    }
                }
            }
        }
    }

    dirs
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        dirs.push(Path::new(&home).join(".maxima").join("packages"));
        dirs.push(Path::new(&home).join(".maxima"));
        dirs.push(Path::new(&home).join(".maxima").join("maxpack"));
        let mp = Path::new(&home).join(".maxpack");
        if mp.exists() {
            dirs.push(mp.join("latest").join("src"));
            dirs.push(mp.join("latest").join("pkgs"));
            dirs.push(mp);
        }
    }

    dirs.extend(maxima_share_dirs());
    dirs
}

fn resolve_package_path(name: &str, base_dir: &Path, searched: &mut HashSet<PathBuf>) -> Option<PathBuf> {
    for ext in &["", ".mac", ".max", ".lisp"] {
        let p = base_dir.join(format!("{}{}", name, ext));
        if p.exists() && searched.insert(p.clone()) {
            return Some(p);
        }
    }

    for dir in &search_dirs() {
        for ext in &["", ".mac", ".max", ".lisp"] {
            let p = dir.join(format!("{}{}", name, ext));
            if p.exists() && searched.insert(p.clone()) {
                return Some(p);
            }
        }
        for ext in &[".mac", ".max", ".lisp"] {
            let p = dir.join(name).join(format!("{}{}", name, ext));
            if p.exists() && searched.insert(p.clone()) {
                return Some(p);
            }
        }
    }

    None
}

/// Parse Lisp source to find (load "file") calls using tree-sitter-commonlisp.
fn find_lisp_imports(source: &str) -> Vec<String> {
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
                                    if arg.kind() == "string" {
                                        if let Ok(text) = arg.utf8_text(source.as_bytes()) {
                                            let raw = text.trim_matches('"').to_string();
                                            if !raw.is_empty() {
                                                calls.push(raw);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if entering && cursor.goto_first_child() { continue; }
        if cursor.goto_next_sibling() { entering = true; continue; }
        if cursor.goto_parent() { entering = false; continue; }
        break;
    }

    calls
}

/// Parse Maxima source to find load("file") calls using tree-sitter-maxima.
fn find_maxima_imports(source: &str) -> Vec<String> {
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
            let func_name = node
                .named_child(0)
                .and_then(|n| n.child(0))
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .map(|s| s.to_string());

            if let Some(ref fname) = func_name {
                if IMPORT_FUNCS.contains(&fname.as_str()) {
                    let nc = node.named_child_count() as usize;
                    for ci in 1..nc {
                        if let Some(ch) = node.named_child(ci as u32) {
                            if let Some(raw) = ch.child(0)
                                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                            {
                                let raw = raw.trim_matches('"').to_string();
                                if !raw.is_empty() {
                                    calls.push(raw);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        if entering && cursor.goto_first_child() { continue; }
        if cursor.goto_next_sibling() { entering = true; continue; }
        if cursor.goto_parent() { entering = false; continue; }
        break;
    }

    calls
}

/// Detect file language by extension and find imports using the right parser.
pub fn find_imports(source: &str, base_dir: &Path) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for raw_arg in find_import_call_nodes(source) {
        if let Some(path) = resolve_package_path(&raw_arg, base_dir, &mut seen) {
            results.push(path);
        }
    }
    results
}

/// Use the appropriate parser based on file extension.
fn find_import_call_nodes(source: &str) -> Vec<String> {
    let maxima = find_maxima_imports(source);
    if !maxima.is_empty() {
        return maxima;
    }
    find_lisp_imports(source)
}

pub fn resolve_imports(
    source: &str,
    base_dir: &Path,
) -> (HashMap<String, ExternalDoc>, HashMap<String, lsp_types::Location>) {
    let mut all_docs = HashMap::new();
    let mut all_defs = HashMap::new();
    let mut seen = HashSet::new();
    let mut queue: Vec<(String, PathBuf)> = Vec::new();

    for raw_arg in find_import_call_nodes(source) {
        if let Some(path) = resolve_package_path(&raw_arg, base_dir, &mut seen) {
            queue.push((raw_arg, path));
        }
    }

    while let Some((_raw_name, path)) = queue.pop() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            let path_str = path.to_string_lossy().to_string();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

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
                        let defs = collect_maxima_defs(&content, &uri);
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
            // Bug 8 mitigation: warn on duplicate names
            for (name, _) in &docs {
                if all_docs.contains_key(name) {
                    tracing::debug!("Duplicate function definition '{}' in import resolution", name);
                }
            }
            all_docs.extend(docs);
            all_defs.extend(defs);

            // Bug 6 fix: use correct parser for recursive imports
            let sub_dir = path.parent().unwrap_or(base_dir);
            let sub_imports = match ext.as_str() {
                "lisp" | "lsp" => find_lisp_imports(&content),
                _ => find_maxima_imports(&content),
            };
            for sub_arg in sub_imports {
                if let Some(sub_path) = resolve_package_path(&sub_arg, sub_dir, &mut seen) {
                    queue.push((sub_arg, sub_path));
                }
            }
        }
    }

    (all_docs, all_defs)
}

fn collect_maxima_defs(source: &str, uri: &Url) -> HashMap<String, lsp_types::Location> {
    let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return HashMap::new();
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return HashMap::new(),
    };

    let mut defs = HashMap::new();
    let mut cursor = tree.walk();
    let mut entering = true;
    loop {
        let node = cursor.node();
        if node.kind() == "binary_expression" {
            if let Some(op) = node.child(1) {
                if op.kind() == ":=" || op.kind() == "::=" {
                    if let Some(name) = extract_name(node.child(0), source) {
                        let r = node.range();
                        defs.insert(name, lsp_types::Location {
                            uri: uri.clone(),
                            range: lsp_types::Range {
                                start: lsp_types::Position { line: r.start_point.row as u32, character: r.start_point.column as u32 },
                                end: lsp_types::Position { line: r.end_point.row as u32, character: r.end_point.column as u32 },
                            },
                        });
                    }
                }
            }
        }
        if entering && cursor.goto_first_child() { continue; }
        if cursor.goto_next_sibling() { entering = true; continue; }
        if cursor.goto_parent() { entering = false; continue; }
        break;
    }
    defs
}

fn extract_name(node: Option<tree_sitter::Node>, source: &str) -> Option<String> {
    let n = node?;
    let mut c = n.walk();
    loop {
        let cn = c.node();
        if cn.kind() == "identifier" {
            return cn.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
        }
        if c.goto_first_child() { continue; }
        if c.goto_next_sibling() { continue; }
        loop {
            if !c.goto_parent() { return None; }
            if c.goto_next_sibling() { break; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_maxima_imports_load() {
        let src = r#"load("utils");"#;
        let calls = find_maxima_imports(src);
        assert_eq!(calls, vec!["utils"]);
    }

    #[test]
    fn test_find_maxima_imports_import() {
        let src = r#"import("colors");"#;
        let calls = find_maxima_imports(src);
        assert_eq!(calls, vec!["colors"]);
    }

    #[test]
    fn test_find_maxima_imports_batch() {
        let src = r#"batch("setup");"#;
        let calls = find_maxima_imports(src);
        assert_eq!(calls, vec!["setup"]);
    }

    #[test]
    fn test_find_maxima_imports_multiple() {
        let src = r#"
load("utils");
import("colors");
batch("setup");
"#;
        let calls = find_maxima_imports(src);
        assert_eq!(calls.len(), 3);
        assert!(calls.contains(&"utils".to_string()));
        assert!(calls.contains(&"colors".to_string()));
        assert!(calls.contains(&"setup".to_string()));
    }

    #[test]
    fn test_find_maxima_imports_no_match() {
        let src = r#"f(x) := x^2$"#;
        let calls = find_maxima_imports(src);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_find_lisp_imports_load() {
        let src = r#"(load "numerical/fzero")"#;
        let calls = find_lisp_imports(src);
        assert_eq!(calls, vec!["numerical/fzero"]);
    }
}
