use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{self, Url};

use crate::docstring::{self, ExternalDoc};

/// Finds `load("file")`, `batch("file")`, `batchload("file")` calls
/// in the source and returns the referenced file paths.
pub fn find_imports(source: &str, base_dir: &Path) -> Vec<PathBuf> {
    let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return vec![];
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };

    let mut paths = Vec::new();
    let mut cursor = tree.walk();
    let mut entering = true;

    loop {
        let node = cursor.node();
        if node.kind() == "function_call" {
            let first = node.child(0)
                .and_then(|n| n.child(0))
                .and_then(|n| n.utf8_text(source.as_bytes()).ok());

            if let Some(name) = first {
                if name == "load" || name == "batch" || name == "batchload" {
                    if let Some(arg) = node.child(2) {
                        if arg.kind() == "string" {
                            let raw = arg.utf8_text(source.as_bytes()).unwrap_or("").trim_matches('"').to_string();
                            resolve_path(&raw, base_dir).into_iter().for_each(|p| paths.push(p));
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
    paths
}

/// Resolves all imports for a source file and returns:
/// - docstrings from imported files
/// - definition locations from imported files
pub fn resolve_imports(
    source: &str,
    base_dir: &Path,
    _current_uri: &Url,
) -> (HashMap<String, ExternalDoc>, HashMap<String, lsp_types::Location>) {
    let mut all_docs = HashMap::new();
    let mut all_defs = HashMap::new();
    let imported = find_imports(source, base_dir);

    for path in imported {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(uri) = Url::from_file_path(&path) {
                let path_str = path.to_string_lossy().to_string();

                // Extract docstrings
                let mut docs = docstring::extract_docstrings(&content);
                for (_, doc) in docs.iter_mut() {
                    doc.source_file = path_str.clone();
                }
                all_docs.extend(docs);

                // Extract definition locations
                let defs = collect_import_defs(&content, &uri);
                all_defs.extend(defs);
            }
        }
    }

    (all_docs, all_defs)
}

fn collect_import_defs(source: &str, uri: &Url) -> HashMap<String, lsp_types::Location> {
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

fn resolve_path(name: &str, base_dir: &Path) -> Option<PathBuf> {
    let p = base_dir.join(name);
    if p.exists() { return Some(p); }

    let with_mac = base_dir.join(format!("{}.mac", name));
    if with_mac.exists() { return Some(with_mac); }

    let with_max = base_dir.join(format!("{}.max", name));
    if with_max.exists() { return Some(with_max); }

    for sys_dir in &[
        "/usr/share/maxima/5.47.0/share/",
        "/usr/local/share/maxima/5.47.0/share/",
        "/opt/local/share/maxima/5.47.0/share/",
    ] {
        let sp = Path::new(sys_dir).join(name);
        if sp.exists() { return Some(sp); }
        let sp2 = Path::new(sys_dir).join(format!("{}.mac", name));
        if sp2.exists() { return Some(sp2); }
    }

    None
}
