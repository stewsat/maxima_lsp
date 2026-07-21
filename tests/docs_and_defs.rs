// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

//! Integration tests for docstring extraction, definitions, and builtins.

use maxima_lsp::db::Database;
use maxima_lsp::definitions::{
    collect_definitions, collect_definitions_from_source, identifier_at_position,
};
use maxima_lsp::docs::{parse_signature_for_snippet, Builtins, DocEntry};
use maxima_lsp::docstring::{extract_docstrings, parse_docstring_ext};
use maxima_lsp::lisp_extractor::{collect_lisp_defs, extract_lisp_docs};
use tower_lsp::lsp_types::Url;

fn parse_maxima(src: &str) -> tree_sitter::Tree {
    let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).unwrap();
    parser.parse(src, None).unwrap()
}

#[test]
fn parse_docstring_tags() {
    let comment = r#"/* Compute the square.
@param x - input value
@return the square of x
@example square(3) → 9
*/"#;
    let parsed = parse_docstring_ext(comment, "square");
    assert!(parsed.doc.contains("Compute the square"));
    assert_eq!(parsed.params.len(), 1);
    assert!(parsed.params[0].contains("x"));
    assert!(parsed.returns.contains("square"));
    assert_eq!(parsed.examples.len(), 1);
}

#[test]
fn parse_docstring_accepts_returns_alias() {
    let comment = "/* desc\n@returns a value */";
    let parsed = parse_docstring_ext(comment, "f");
    assert_eq!(parsed.returns, "a value");
}

#[test]
fn extract_docstrings_attaches_preceding_comment() {
    let src = r#"
/* Increment by one.
@param x - value
@return x + 1 */
inc(x) := x + 1$

/* no attachment */
orphan(y) := y$
"#;
    let docs = extract_docstrings(src);
    let inc = docs.get("inc").expect("inc");
    assert!(inc.doc.contains("Increment"));
    assert!(!inc.params.is_empty());
    assert!(docs.contains_key("orphan"));
}

#[test]
fn collect_definitions_covers_assignment_forms() {
    let src = r#"
a: 1$
b:: 2$
f(x) := x$
g(x) ::= x$
"#;
    let tree = parse_maxima(src);
    let uri = Url::parse("file:///tmp/defs.mac").unwrap();
    let defs = collect_definitions(&tree, src, &uri);
    for name in ["a", "b", "f", "g"] {
        assert!(defs.contains_key(name), "missing {name}: {:?}", defs.keys());
    }
}

#[test]
fn identifier_at_position_finds_call_name() {
    let src = r#"foo(bar)$"#;
    let tree = parse_maxima(src);
    let byte = src.find("foo").unwrap() + 1;
    assert_eq!(
        identifier_at_position(tree.root_node(), byte, src).as_deref(),
        Some("foo")
    );
}

#[test]
fn lisp_extractor_and_defs() {
    let src = r#"
;;; Add numbers
(defmfun $add (a b) (+ a b))
(defvar $flag t)
"#;
    let docs = extract_lisp_docs(src);
    assert!(docs.contains_key("$add"));
    assert!(docs.get("$add").unwrap().doc.contains("Add"));
    assert!(docs.contains_key("$flag"));

    let uri = Url::parse("file:///tmp/x.lisp").unwrap();
    let defs = collect_lisp_defs(src, &uri);
    assert_eq!(defs.len(), 2);
}

#[test]
fn builtins_snippet_generation() {
    let entry = DocEntry::new(
        "solve(expr, var)  |  solve([eqns], [vars])",
        "Solve equations.",
        &[],
        "solutions",
        &[],
        "algebra",
    );
    let snip = entry.snippet("solve");
    assert!(snip.starts_with("solve("));
    assert!(snip.contains("${1:expr}"));
    assert!(snip.contains("${2:var}"));

    assert_eq!(
        parse_signature_for_snippet("kill(all)", "kill"),
        "kill(${1:all})"
    );
    assert_eq!(parse_signature_for_snippet("timestamp()", "timestamp"), "timestamp()");
}

#[test]
fn builtins_contains_core_entries() {
    let b = Builtins::new();
    assert!(b.functions.contains_key("diff"));
    assert!(b.functions.contains_key("integrate"));
    assert!(b.functions.contains_key("load"));
    assert!(b.constants.contains_key("%pi"));
    assert!(b.keywords.contains(&"block"));
}

#[test]
fn database_upsert_indexes_local_file_and_lookup() {
    let dir = std::env::temp_dir().join(format!("maxima_lsp_db_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("user.mac");
    let text = "/* user fn */\nmyAdd(a, b) := a + b$\n";
    std::fs::write(&file, text).unwrap();

    let uri = Url::from_file_path(&file).unwrap();
    let mut db = Database::new().expect("database");
    db.upsert(&uri, text, 1);

    let doc = db.get(&uri).expect("document");
    assert!(doc.definitions.contains_key("myAdd"));

    let entry = db.lookup_doc("myAdd", &uri).expect("lookup");
    assert!(entry.signature.contains("myAdd") || entry.category == "user");

    let loc = db.goto_definition("myAdd", &uri).expect("goto");
    assert_eq!(loc.uri, uri);

    // Builtin fallback
    let diff = db.lookup_doc("diff", &uri).expect("builtin diff");
    assert_eq!(diff.category, "calculus");

    assert!(db.diagnostics(&uri).is_empty());
    db.remove(&uri);
    assert!(db.get(&uri).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn database_diagnostics_on_syntax_error() {
    let dir = std::env::temp_dir().join(format!("maxima_lsp_diag_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.mac");
    // Intentionally broken Maxima (unclosed paren).
    let text = "f(x := x$\n";
    std::fs::write(&file, text).unwrap();

    let uri = Url::from_file_path(&file).unwrap();
    let mut db = Database::new().expect("database");
    db.upsert(&uri, text, 1);
    let diags = db.diagnostics(&uri);
    assert!(!diags.is_empty(), "expected syntax diagnostics");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn collect_definitions_from_source_helper() {
    let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
    let uri = Url::parse("file:///tmp/h.mac").unwrap();
    let defs = collect_definitions_from_source("h(x) := x$\n", &uri, lang);
    assert!(defs.contains_key("h"));
}
