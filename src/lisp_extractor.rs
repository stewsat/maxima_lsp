use std::collections::HashMap;
use regex::Regex;

use crate::docstring::{ExternalDoc, ParsedDoc};

/// Try to extract function definitions from a Lisp source file.
/// Maxima .lisp files define functions with defmfun, defmspec, defun, etc.
pub fn extract_lisp_docs(source: &str) -> HashMap<String, ExternalDoc> {
    let mut results = HashMap::new();

    // Patterns for Lisp function definition forms in Maxima
    let def_patterns: &[(&str, &str, fn(&str, &str, &[&str]) -> ExternalDoc)] = &[
        // (defmfun $name (params) ...)
        (r"\(defmfun\s+\$(\w[\w-]*)\s*\(([^)]*)\)", "defmfun", make_def),
        // (defmspec $name (params) ...)
        (r"\(defmspec\s+\$(\w[\w-]*)\s*\(([^)]*)\)", "defmspec", make_def),
        // (defun name (params) ...)
        (r"\(defun\s+(?:[$]?)(\w[\w-]*)\s*\(([^)]*)\)", "defun", make_def),
        // (defvar $name value) — variable-like
        (r"\(defvar\s+\$(\w[\w-]*)\s", "defvar", make_var),
        // (defmvar $name value) — Maxima global variable
        (r"\(defmvar\s+\$(\w[\w-]*)\s", "defmvar", make_var),
        // (defprop $name ...) — property definitions
        (r"\(defprop\s+\$(\w[\w-]*)\s", "defprop", make_var),
    ];

    for (pattern, kind, maker) in def_patterns {
        let re = match Regex::new(pattern) {
            Ok(r) => r,
            Err(_) => continue,
        };

        for cap in re.captures_iter(source) {
            let name = cap[1].to_string();
            let params_str = if cap.len() > 2 { cap[2].trim().to_string() } else { String::new() };
            let doc = extract_preceding_comment(source, cap.get(0).unwrap().start());
            let sig = format!("{}({})", name, params_str);
            let muted_doc = if !doc.doc.is_empty() { doc.doc } else { format!("{} ({})", name, kind) };

            let entry = maker(&name, &sig, &[]);
            results.insert(name, ExternalDoc {
                name: entry.name,
                signature: sig,
                doc: muted_doc,
                params: vec![],
                returns: String::new(),
                examples: vec![],
                source_file: String::new(),
            });
        }
    }

    results
}

fn extract_preceding_comment(source: &str, def_start: usize) -> ParsedDoc {
    // Look backwards from the definition for Lisp comments (;; ...)
    // and docstrings ("...")
    let before = &source[..def_start];
    let mut doc = String::new();

    // Find the last comment block before the definition
    for line in before.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with(";;") {
            let text = trimmed.trim_start_matches(";;").trim();
            if doc.is_empty() {
                doc = text.to_string();
            } else {
                doc = format!("{} {}", text, doc);
            }
        } else if trimmed.starts_with('"') && trimmed.ends_with('"') {
            // Lisp docstring
            doc = trimmed.trim_matches('"').to_string();
            break;
        } else if trimmed.is_empty() {
            continue;
        } else {
            // Stop at first non-comment, non-empty line (going backwards)
            if !doc.is_empty() { break; }
        }
    }

    ParsedDoc { doc, params: vec![], returns: String::new(), examples: vec![] }
}

fn make_def(name: &str, sig: &str, _extra: &[&str]) -> ExternalDoc {
    ExternalDoc {
        name: name.to_string(),
        signature: sig.to_string(),
        doc: String::new(),
        params: vec![],
        returns: String::new(),
        examples: vec![],
        source_file: String::new(),
    }
}

fn make_var(name: &str, sig: &str, _extra: &[&str]) -> ExternalDoc {
    ExternalDoc {
        name: name.to_string(),
        signature: sig.to_string(),
        doc: String::new(),
        params: vec![],
        returns: String::new(),
        examples: vec![],
        source_file: String::new(),
    }
}
