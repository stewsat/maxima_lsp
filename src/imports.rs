use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{self, Url};

use crate::docstring::{self, ExternalDoc};
use crate::lisp_extractor;

/// Import function names we recognise.
const IMPORT_FUNCS: &[&str] = &["load", "batch", "batchload", "import"];

/// Cached list of Maxima share directories discovered on this machine.
fn maxima_share_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // ── Homebrew on macOS ──
    for base in &[
        "/opt/homebrew/share/maxima/",       // Apple Silicon
        "/usr/local/share/maxima/",          // Intel
        "/opt/local/share/maxima/",          // MacPorts
        "/usr/share/maxima/",                // Linux / generic
    ] {
        // Try to find the actual installed version
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

    // ── macOS Application bundle ──
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

/// Build the list of directories searched for maxpack packages + system libraries.
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // User directories
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        dirs.push(Path::new(&home).join(".maxima").join("packages"));
        dirs.push(Path::new(&home).join(".maxima"));
        dirs.push(Path::new(&home).join(".maxima").join("maxpack"));
        let mp = Path::new(&home).join(".maxpack");
        if mp.exists() {
            dirs.push(mp.join("latest").join("src"));
            dirs.push(mp);
        }
    }

    // System maxima share directories
    dirs.extend(maxima_share_dirs());

    dirs
}

/// Resolve a module name to an actual file path.
fn resolve_package_path(name: &str, base_dir: &Path, searched: &mut HashSet<PathBuf>) -> Option<PathBuf> {
    // ── 1. Relative to the importing file ──
    for ext in &["", ".mac", ".max", ".lisp"] {
        let p = base_dir.join(format!("{}{}", name, ext));
        tracing::debug!("  resolve: checking {:?} (exists={})", p, p.exists());
        if p.exists() && searched.insert(p.clone()) {
            return Some(p);
        }
    }

    // ── 2. Search directories (maxpack, system, maxima share) ──
    for dir in &search_dirs() {
        for ext in &["", ".mac", ".max", ".lisp"] {
            let p = dir.join(format!("{}{}", name, ext));
            tracing::debug!("  resolve: checking {:?} (exists={})", p, p.exists());
            if p.exists() && searched.insert(p.clone()) {
                return Some(p);
            }
        }
        // Sub‑directories: e.g. dir/colors/colors.mac
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

/// Find all import calls in source and return resolved file paths.
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

/// Recursively resolve all imports and return definitions + docstrings.
pub fn resolve_imports(
    source: &str,
    base_dir: &Path,
) -> (HashMap<String, ExternalDoc>, HashMap<String, lsp_types::Location>) {
    let mut all_docs = HashMap::new();
    let mut all_defs = HashMap::new();
    let mut seen = HashSet::new();
    let mut queue: Vec<(String, PathBuf)> = Vec::new();

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

            // Choose extractor based on file extension
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

            let docs = match ext.as_str() {
                "lisp" | "lsp" => {
                    tracing::debug!("  using Lisp extractor for {:?}", path);
                    lisp_extractor::extract_lisp_docs(&content)
                }
                _ => {
                    // .mac / .max / no extension — use tree-sitter Maxima parser
                    docstring::extract_docstrings(&content)
                }
            };

            let mut docs = docs;
            for (_, doc) in docs.iter_mut() {
                doc.source_file = path_str.clone();
            }
            all_docs.extend(docs);

            if let Ok(uri) = Url::from_file_path(&path) {
                match ext.as_str() {
                    "lisp" | "lsp" => {
                        let defs = collect_lisp_defs(&content, &uri);
                        all_defs.extend(defs);
                    }
                    _ => {
                        let defs = collect_defs(&content, &uri);
                        all_defs.extend(defs);
                    }
                }
            }

            // Recursively resolve imports inside the loaded file
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

/// Collect definition locations from Maxima source (tree-sitter).
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

/// Collect definition locations from Lisp source (regex-based).
fn collect_lisp_defs(source: &str, uri: &Url) -> HashMap<String, lsp_types::Location> {
    use regex::Regex;
    let mut defs = HashMap::new();

    let patterns = [
        r"\(defmfun\s+\$(\w[\w-]*)",
        r"\(defmspec\s+\$(\w[\w-]*)",
        r"\(defun\s+(?:[$]?)(\w[\w-]*)",
        r"\(defvar\s+\$(\w[\w-]*)",
        r"\(defmvar\s+\$(\w[\w-]*)",
    ];

    for pattern in &patterns {
        if let Ok(re) = Regex::new(pattern) {
            for cap in re.captures_iter(source) {
                let name = cap[1].to_string();
                let start = cap.get(0).map(|m| m.start()).unwrap_or(0);
                // Estimate line from byte position
                let line = source[..start].chars().filter(|&c| c == '\n').count() as u32;
                let col = source[..start].chars().rev().take_while(|&c| c != '\n').count() as u32;
                defs.insert(name, lsp_types::Location {
                    uri: uri.clone(),
                    range: lsp_types::Range {
                        start: lsp_types::Position { line, character: col },
                        end: lsp_types::Position { line, character: col + 1 },
                    },
                });
            }
        }
    }

    defs
}

/// Extract identifier from a tree-sitter node (navigate through function_call -> atom -> identifier)
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
        None => return calls,
    };

    let mut cursor = tree.walk();
    let mut entering = true;
    loop {
        let node = cursor.node();
        if node.kind() == "function_call" {
            let func_name = node
                .child(0)
                .and_then(|n| n.child(0))
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .map(|s| s.to_string());

            if let Some(ref fname) = func_name {
                if IMPORT_FUNCS.contains(&fname.as_str()) {
                    // Find the first string argument
                    let nc = node.child_count() as usize;
                    for ci in 1..nc {
                        if let Some(ch) = node.child(ci as u32) {
                            if ch.is_named() {
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
        }

        if entering && cursor.goto_first_child() { continue; }
        if cursor.goto_next_sibling() { entering = true; continue; }
        if cursor.goto_parent() { entering = false; continue; }
        break;
    }

    calls
}
