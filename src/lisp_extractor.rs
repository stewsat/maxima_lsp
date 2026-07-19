// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

use std::collections::HashMap;
use tower_lsp::lsp_types;
use tree_sitter::Parser;

use crate::docstring::ExternalDoc;

const DEFMFUN: &str = "defmfun";
const DEFMSPEC: &str = "defmspec";
const DEFUN: &str = "defun";
const DEFVAR: &str = "defvar";
const DEFMVAR: &str = "defmvar";
const DEFPROP: &str = "defprop";

fn known_keyword(kw: &str) -> bool {
    matches!(kw, DEFMFUN | DEFMSPEC | DEFUN | DEFVAR | DEFMVAR | DEFPROP)
}

fn keyword_kind(kw: &str) -> &str {
    match kw {
        DEFMFUN | DEFMSPEC | DEFUN => "function",
        DEFVAR | DEFMVAR => "variable",
        DEFPROP => "property",
        _ => "unknown",
    }
}

fn is_function_keyword(kw: &str) -> bool {
    matches!(kw, DEFMFUN | DEFMSPEC | DEFUN)
}

fn extract_preceding_comment(source: &str, def_start_byte: usize) -> String {
    let before = &source[..def_start_byte];
    let mut doc = String::new();

    for line in before.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with(";;") || trimmed.starts_with(';') {
            let text = trimmed.trim_start_matches(';').trim();
            if doc.is_empty() {
                doc = text.to_string();
            } else {
                doc = format!("{} {}", text, doc);
            }
        } else if trimmed.starts_with('"') && trimmed.ends_with('"') {
            doc = trimmed.trim_matches('"').to_string();
            break;
        } else if trimmed.is_empty() {
            continue;
        } else if !doc.is_empty() {
            break;
        }
    }

    doc
}

/// Walk the tree-sitter-commonlisp AST to find Lisp definitions.
fn walk_lisp_defs<F>(source: &str, mut f: F)
where
    F: FnMut(&str, &str, Option<&str>, usize),
{
    let lang: tree_sitter::Language = tree_sitter_commonlisp::LANGUAGE.into();
    let mut parser = Parser::new();
    if parser.set_language(&lang).is_err() {
        return;
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return,
    };

    let mut cursor = tree.walk();
    let mut entering = true;
    loop {
        let node = cursor.node();
        if node.kind() == "list" {
            let nchildren = node.child_count();
            let mut found_def = false;
            if nchildren >= 3 {
                let kw = node.child(1)
                    .and_then(|n| {
                        if n.kind() == "atom" { n.child(0) } else { Some(n) }
                    })
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .map(|s| s.to_string());

                if let Some(ref kw) = kw {
                    if known_keyword(kw) {
                        let name = node.child(2)
                            .and_then(|n| {
                                if n.kind() == "atom" { n.child(0) } else { Some(n) }
                            })
                            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                            .unwrap_or("")
                            .to_string();

                        let lambda_list = if is_function_keyword(kw) && nchildren > 4 {
                            node.child(3)
                                .filter(|n| n.kind() == "list")
                                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        } else {
                            None
                        };

                        f(kw, &name, lambda_list, node.start_byte());
                        found_def = true;
                    }
                }
            }
            if found_def {
                // Don't descend into definition body — skip children
                entering = false;
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
}

pub fn extract_lisp_docs(source: &str) -> HashMap<String, ExternalDoc> {
    let mut results = HashMap::new();

    walk_lisp_defs(source, |keyword, name, lambda_list, start_byte| {
        let doc = extract_preceding_comment(source, start_byte);

        let params_str = lambda_list.unwrap_or("");
        let sig = if params_str.is_empty() {
            name.to_string()
        } else {
            format!("{}({})", name, &params_str[1..params_str.len().saturating_sub(1)])
        };

        let muted_doc = if !doc.is_empty() {
            doc
        } else {
            format!("{} ({})", name, keyword_kind(keyword))
        };

        results.insert(name.to_string(), ExternalDoc {
            name: name.to_string(),
            signature: sig,
            doc: muted_doc,
            params: vec![],
            returns: String::new(),
            examples: vec![],
            source_file: String::new(),
        });
    });

    results
}

pub fn collect_lisp_defs(source: &str, uri: &lsp_types::Url) -> HashMap<String, lsp_types::Location> {
    let mut defs = HashMap::new();

    walk_lisp_defs(source, |_keyword, name, _lambda_list, start_byte| {
        let line = source[..start_byte].chars().filter(|&c| c == '\n').count() as u32;
        let col = source[..start_byte]
            .chars()
            .rev()
            .take_while(|&c| c != '\n')
            .count() as u32;

        defs.insert(name.to_string(), lsp_types::Location {
            uri: uri.clone(),
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line,
                    character: col,
                },
                end: lsp_types::Position {
                    line,
                    character: col + 1,
                },
            },
        });
    });

    defs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_defmfun() {
        let src = "(defmfun $solve (expr var) (body))";
        let docs = extract_lisp_docs(src);
        assert_eq!(docs.len(), 1, "should find 1 definition");
        let doc = docs.get("$solve").unwrap();
        assert_eq!(doc.name, "$solve");
        assert!(doc.signature.contains("expr"));
        assert!(doc.signature.contains("var"));
    }

    #[test]
    fn test_extract_defun() {
        let src = "(defun solve-poly (expr) (process expr))";
        let docs = extract_lisp_docs(src);
        assert_eq!(docs.len(), 1);
        let doc = docs.get("solve-poly").unwrap();
        assert_eq!(doc.name, "solve-poly");
    }

    #[test]
    fn test_extract_defvar() {
        let src = "(defvar $version \"5.47.0\")";
        let docs = extract_lisp_docs(src);
        assert_eq!(docs.len(), 1);
        let doc = docs.get("$version").unwrap();
        assert_eq!(doc.name, "$version");
    }

    #[test]
    fn test_extract_multiple() {
        let src = "(defmfun $solve (expr var) (body))
(defun helper (x) (x))
(defvar $maxdepth 100)
(defmvar $debug nil)
(defprop $f t $translator)
(defmspec $integrate (expr var) (expr))";
        let docs = extract_lisp_docs(src);
        assert_eq!(docs.len(), 6);
        assert!(docs.contains_key("$solve"));
        assert!(docs.contains_key("helper"));
        assert!(docs.contains_key("$maxdepth"));
        assert!(docs.contains_key("$debug"));
        assert!(docs.contains_key("$f"));
        assert!(docs.contains_key("$integrate"));
    }

    #[test]
    fn test_extract_with_comment() {
        let src = ";;; Solve an equation
;;; Uses polynomial solver
(defmfun $solve (expr var) (body))";
        let docs = extract_lisp_docs(src);
        assert_eq!(docs.len(), 1);
        let doc = docs.get("$solve").unwrap();
        assert!(doc.doc.contains("Solve"));
        assert!(doc.doc.contains("polynomial"));
    }

    #[test]
    fn test_collect_lisp_defs() {
        let src = "(defmfun $foo (x) (body))\n(defvar $bar 42)";
        let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/test.lisp").unwrap();
        let defs = collect_lisp_defs(src, &uri);
        assert_eq!(defs.len(), 2);
        assert!(defs.contains_key("$foo"));
        assert!(defs.contains_key("$bar"));
    }

    #[test]
    fn test_empty_source() {
        let docs = extract_lisp_docs("");
        assert!(docs.is_empty());
    }

    #[test]
    fn test_no_definitions() {
        let src = "(+ 1 2 3)\n(format t \"hello\")";
        let docs = extract_lisp_docs(src);
        assert!(docs.is_empty());
    }

    #[test]
    fn test_nested_defs_outer_only() {
        let src = "(defun outer (x) (defun inner (y) y))";
        let docs = extract_lisp_docs(src);
        assert_eq!(docs.len(), 1, "should only find outer def (skips children)");
        assert!(docs.contains_key("outer"));
    }
}
