use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
            // Check if it's load/batch/batchload
            let first_child = node.child(0);
            let func_name = first_child
                .and_then(|n| n.child(0))
                .and_then(|n| n.utf8_text(source.as_bytes()).ok());

            if let Some(name) = func_name {
                if name == "load" || name == "batch" || name == "batchload" {
                    // Get the string argument
                    if let Some(arg) = node.child(2) {
                        if arg.kind() == "string" {
                            let arg_text = arg.utf8_text(source.as_bytes())
                                .unwrap_or("").trim_matches('"').to_string();
                            resolve_path(&arg_text, base_dir).into_iter().for_each(|p| paths.push(p));
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

fn resolve_path(name: &str, base_dir: &Path) -> Option<PathBuf> {
    // Try the name as-is relative to the base directory
    let p = base_dir.join(name);
    if p.exists() { return Some(p); }

    // Try with .mac extension
    let with_mac = base_dir.join(format!("{}.mac", name));
    if with_mac.exists() { return Some(with_mac); }

    // Try with .max extension
    let with_max = base_dir.join(format!("{}.max", name));
    if with_max.exists() { return Some(with_max); }

    // Try Maxima's default search paths
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

/// Resolves all imports for a source file and extracts docstrings from them.
pub fn resolve_imports(source: &str, base_dir: &Path) -> HashMap<String, ExternalDoc> {
    let mut all = HashMap::new();
    let imported = find_imports(source, base_dir);

    for path in imported {
        if let Ok(content) = std::fs::read_to_string(&path) {
            let mut docs = docstring::extract_docstrings(&content);
            let path_str = path.to_string_lossy().to_string();
            for (_, doc) in docs.iter_mut() {
                doc.source_file = path_str.clone();
            }
            all.extend(docs);
        }
    }
    all
}
