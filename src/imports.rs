use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{self, Url};

use crate::docstring::{self, ExternalDoc};

/// Import function names we recognise.
const IMPORT_FUNCS: &[&str] = &["load", "batch", "batchload", "import"];

/// Build the list of directories searched for maxpack packages.
fn maxpack_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        dirs.push(Path::new(&home).join(".maxima").join("packages"));
        dirs.push(Path::new(&home).join(".maxima"));
        dirs.push(Path::new(&home).join(".maxima").join("maxpack"));
    }

    // Common maxpack system paths
    for base in &[
        "/usr/share/maxima/5.47.0/share/",
        "/usr/local/share/maxima/5.47.0/share/",
        "/opt/local/share/maxima/5.47.0/share/",
    ] {
        dirs.push(Path::new(base).to_path_buf());
    }

    dirs
}

/// Try to resolve a module name to an actual file path.
///   name           – e.g. "colors", "vect", "eigen"
///   base_dir       – directory of the file that contains the import call
///   searched_set   – set of already‑resolved paths (to avoid cycles)
fn resolve_package_path(name: &str, base_dir: &Path, searched: &mut HashSet<PathBuf>) -> Option<PathBuf> {
    // ── 1. Relative to the importing file ──
    for ext in &["", ".mac", ".max", ".lisp"] {
        let p = base_dir.join(format!("{}{}", name, ext));
        tracing::debug!("  resolve: checking {:?} (exists={})", p, p.exists());
        if p.exists() && searched.insert(p.clone()) {
            return Some(p);
        }
    }

    // ── 2. maxpack & system directories ──
    for dir in &maxpack_dirs() {
        for ext in &["", ".mac", ".max", ".lisp"] {
            let p = dir.join(format!("{}{}", name, ext));
            tracing::debug!("  resolve: checking {:?} (exists={})", p, p.exists());
            if p.exists() && searched.insert(p.clone()) {
                return Some(p);
            }
        }
        // Sub‑directories  e.g. packages/colors/colors.mac
        for ext in &[".mac", ".max", ".lisp"] {
            let p = dir.join(name).join(format!("{}{}", name, ext));
            tracing::debug!("  resolve: checking {:?} (exists={})", p, p.exists());
            if p.exists() && searched.insert(p.clone()) {
                return Some(p);
            }
        }
    }

    None
}

/// Find all import calls in `source` and return the resolved file paths.
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

/// Recursively resolve all imports (including imports from imported files)
/// and return every discovered definition + docstring.
pub fn resolve_imports(
    source: &str,
    base_dir: &Path,
) -> (HashMap<String, ExternalDoc>, HashMap<String, lsp_types::Location>) {
    let mut all_docs = HashMap::new();
    let mut all_defs = HashMap::new();
    let mut seen = HashSet::new();
    let mut queue: Vec<(String, PathBuf)> = Vec::new(); // (raw_name, resolved_path)

    // Seed the queue
    for raw_arg in find_import_call_nodes(source) {
        tracing::debug!("resolve_imports: trying to resolve '{}' from {:?}", raw_arg, base_dir);
        if let Some(path) = resolve_package_path(&raw_arg, base_dir, &mut seen) {
            tracing::debug!("resolve_imports: resolved '{}' -> {:?}", raw_arg, path);
            queue.push((raw_arg, path));
        }
    }

    while let Some((raw_name, path)) = queue.pop() {
        tracing::debug!("resolve_imports: processing {:?} (from import '{}')", path, raw_name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            let path_str = path.to_string_lossy().to_string();

            // ── Extract docstrings ──
            let mut docs = docstring::extract_docstrings(&content);
            for (_, doc) in docs.iter_mut() {
                doc.source_file = path_str.clone();
            }
            all_docs.extend(docs);

            // ── Extract definition locations ──
            if let Ok(uri) = Url::from_file_path(&path) {
                let defs = collect_defs(&content, &uri);
                all_defs.extend(defs);
            }

            // ── Recursively resolve imports inside the loaded file ──
            let sub_dir = path.parent().unwrap_or(base_dir);
            for sub_arg in find_import_call_nodes(&content) {
                if let Some(sub_path) = resolve_package_path(&sub_arg, sub_dir, &mut seen) {
                    queue.push((sub_arg, sub_path));
                }
            }
        }
    }

    (all_docs, all_defs)
}

/// Walk the AST and return the string arguments of every import call.
fn find_import_call_nodes(source: &str) -> Vec<String> {
    let mut calls = Vec::new();

    let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return calls;
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            tracing::debug!("find_import_call_nodes: parse failed");
            return calls;
        }
    };

    let mut cursor = tree.walk();
    let mut entering = true;
    loop {
        let node = cursor.node();
        if node.kind() == "function_call" {
            let func_name = node
                .child(0)
                .and_then(|n| {
                    let text = n.utf8_text(source.as_bytes()).ok();
                    tracing::debug!("  function_call child[0] kind={} text={:?}", n.kind(), text);
                    n.child(0)
                })
                .and_then(|n| {
                    let text = n.utf8_text(source.as_bytes()).ok();
                    tracing::debug!("  function_call child[0][0] kind={} text={:?}", n.kind(), text);
                    text
                })
                .map(|s| s.to_string());

            if let Some(ref fname) = func_name {
                tracing::debug!("find_import_call_nodes: found function_call '{}'", fname);
                if IMPORT_FUNCS.contains(&fname.as_str()) {
                    // Collect all children of the function_call for debugging
                    let nc = node.child_count() as usize;
                    for ci in 0..nc {
                        if let Some(ch) = node.child(ci as u32) {
                            let txt = ch.utf8_text(source.as_bytes()).unwrap_or("");
                            tracing::debug!("  child[{}] kind={} named={} text={:?}", ci, ch.kind(), ch.is_named(), txt);
                        }
                    }

                    // Find the string argument - it's the first _expression child after the function name
                    let mut found = false;
                    let mut ci: usize = 1;
                    while !found && ci < nc {
                        if let Some(ch) = node.child(ci as u32) {
                            if ch.is_named() {
                                // ch is an _expression (atom) containing the string
                                let raw = ch
                                    .child(0)
                                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                                    .unwrap_or("")
                                    .trim_matches('"')
                                    .to_string();
                                if !raw.is_empty() {
                                    tracing::debug!("  -> import argument at child[{}]: '{}'", ci, raw);
                                    calls.push(raw);
                                    found = true;
                                }
                            }
                        }
                        ci += 1;
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

    tracing::debug!("find_import_call_nodes: found {} import(s)", calls.len());
    calls
}

fn collect_defs(source: &str, uri: &Url) -> HashMap<String, lsp_types::Location> {
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
                        defs.insert(
                            name,
                            lsp_types::Location {
                                uri: uri.clone(),
                                range: lsp_types::Range {
                                    start: lsp_types::Position {
                                        line: r.start_point.row as u32,
                                        character: r.start_point.column as u32,
                                    },
                                    end: lsp_types::Position {
                                        line: r.end_point.row as u32,
                                        character: r.end_point.column as u32,
                                    },
                                },
                            },
                        );
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
        if c.goto_first_child() {
            continue;
        }
        if c.goto_next_sibling() {
            continue;
        }
        loop {
            if !c.goto_parent() {
                return None;
            }
            if c.goto_next_sibling() {
                break;
            }
        }
    }
}
