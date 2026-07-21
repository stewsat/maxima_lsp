// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

//! Integration tests for parsing and resolving load/import/batch calls.

use maxima_lsp::definitions::deepest_node_at;
use maxima_lsp::imports::{find_imports, load_target_at_position, resolve_imports};
use maxima_lsp::paths::PathResolver;
use std::fs;
use std::path::PathBuf;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maxima_lsp_import_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn parse_maxima(src: &str) -> tree_sitter::Tree {
    let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).unwrap();
    parser.parse(src, None).unwrap()
}

fn byte_on(src: &str, needle: &str) -> usize {
    src.find(needle).unwrap() + needle.len() / 2
}

#[test]
fn find_imports_load_batch_and_import() {
    let dir = temp_dir("multi");
    fs::write(dir.join("a.mac"), "a() := 1$\n").unwrap();
    fs::write(dir.join("b.mac"), "b() := 2$\n").unwrap();
    fs::write(dir.join("c.mac"), "c() := 3$\n").unwrap();

    let src = r#"
load("a");
import("b");
batch("c");
"#;
    let mut resolver = PathResolver::with_home(&dir);
    let paths = find_imports(src, &dir, &mut resolver);
    assert_eq!(paths.len(), 3);
    assert!(paths.iter().any(|p| p.ends_with("a.mac")));
    assert!(paths.iter().any(|p| p.ends_with("b.mac")));
    assert!(paths.iter().any(|p| p.ends_with("c.mac")));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resolve_imports_indexes_local_definitions() {
    let dir = temp_dir("index_local");
    fs::write(
        dir.join("lib.mac"),
        "/* add one */\ninc(x) := x + 1$\n",
    )
    .unwrap();

    let src = r#"load("lib")$ inc(3)$"#;
    let mut resolver = PathResolver::with_home(&dir);
    let (docs, defs) = resolve_imports(src, &dir, &mut resolver);
    assert!(
        defs.contains_key("inc") || docs.contains_key("inc"),
        "expected inc from local load, defs={:?} docs={:?}",
        defs.keys().collect::<Vec<_>>(),
        docs.keys().collect::<Vec<_>>()
    );
    if let Some(doc) = docs.get("inc") {
        assert!(doc.doc.contains("add one") || doc.doc.contains("inc"));
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resolve_imports_follows_chain() {
    let dir = temp_dir("chain");
    fs::write(dir.join("leaf.mac"), "leaf(x) := x$\n").unwrap();
    fs::write(dir.join("mid.mac"), "load(\"leaf\")$\nmid(x) := leaf(x)$\n").unwrap();

    let src = r#"load("mid")$"#;
    let mut resolver = PathResolver::with_home(&dir);
    let (_docs, defs) = resolve_imports(src, &dir, &mut resolver);
    assert!(defs.contains_key("mid"), "mid missing: {:?}", defs.keys());
    assert!(defs.contains_key("leaf"), "leaf missing: {:?}", defs.keys());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn load_target_goto_on_quoted_and_bare_identifiers() {
    for (src, needle, expected) in [
        (r#"load("draw")$"#, "draw", "draw"),
        (r#"load(draw)$"#, "draw", "draw"),
        (r#"import(colors)$"#, "colors", "colors"),
        (r#"batch("setup")$"#, "setup", "setup"),
    ] {
        let tree = parse_maxima(src);
        let byte = byte_on(src, needle);
        let got = load_target_at_position(tree.root_node(), byte, src);
        assert_eq!(
            got.as_ref().map(|(n, _)| n.as_str()),
            Some(expected),
            "src={src:?} deepest={:?}",
            deepest_node_at(tree.root_node(), byte).map(|n| n.kind())
        );
    }
}

#[test]
fn load_target_not_triggered_outside_import_call() {
    let src = r#"f(x) := x$ draw: 1$"#;
    let tree = parse_maxima(src);
    let byte = byte_on(src, "draw");
    assert!(load_target_at_position(tree.root_node(), byte, src).is_none());
}

#[test]
fn resolve_imports_from_lisp_load() {
    let dir = temp_dir("lisp_load");
    let sub = dir.join("numerical");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("fzero.lisp"), "(defun $fzero (x) x)\n").unwrap();

    // Relative lisp-style load path as used inside Maxima share packages.
    let src = r#"(load "numerical/fzero")"#;
    let mut resolver = PathResolver::with_home(&dir);
    let paths = find_imports(src, &dir, &mut resolver);
    assert_eq!(paths.len(), 1, "paths={paths:?}");
    assert!(paths[0].ends_with("fzero.lisp") || paths[0].ends_with("fzero"));
    let _ = fs::remove_dir_all(&dir);
}
