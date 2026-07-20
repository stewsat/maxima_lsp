// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

use std::collections::HashMap;
use tower_lsp::lsp_types::{self, Url};

const DEF_OPERATORS: &[&str] = &[":=", "::=", ":", "::"];

pub fn collect_definitions(
    tree: &tree_sitter::Tree,
    source: &str,
    uri: &Url,
) -> HashMap<String, lsp_types::Location> {
    let mut defs = HashMap::new();
    let mut cursor = tree.walk();
    let mut entering = true;

    loop {
        let node = cursor.node();
        if node.kind() == "binary_expression" {
            if let Some(op) = node.child(1) {
                let op_text = op.kind();
                if DEF_OPERATORS.contains(&op_text) {
                    if let Some(name) = extract_name(node.child(0), source) {
                        insert_definition(&mut defs, name, node.range(), uri);
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

pub fn collect_definitions_from_source(source: &str, uri: &Url, lang: tree_sitter::Language) -> HashMap<String, lsp_types::Location> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return HashMap::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return HashMap::new();
    };
    collect_definitions(&tree, source, uri)
}

fn insert_definition(
    defs: &mut HashMap<String, lsp_types::Location>,
    name: String,
    range: tree_sitter::Range,
    uri: &Url,
) {
    if name.is_empty() {
        return;
    }
    defs.insert(
        name,
        lsp_types::Location {
            uri: uri.clone(),
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: range.start_point.row as u32,
                    character: range.start_point.column as u32,
                },
                end: lsp_types::Position {
                    line: range.end_point.row as u32,
                    character: range.end_point.column as u32,
                },
            },
        },
    );
}

pub fn extract_name(node: Option<tree_sitter::Node>, source: &str) -> Option<String> {
    let n = node?;
    let mut cursor = n.walk();
    loop {
        let current = cursor.node();
        if current.kind() == "identifier" {
            return current
                .utf8_text(source.as_bytes())
                .ok()
                .map(|s| s.to_string());
        }
        if cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            continue;
        }
        loop {
            if !cursor.goto_parent() {
                return None;
            }
            if cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

pub fn extract_load_argument(node: tree_sitter::Node, source: &str) -> Option<String> {
    match node.kind() {
        "string" => node
            .utf8_text(source.as_bytes())
            .ok()
            .map(|s| s.trim_matches('"').to_string())
            .filter(|s| !s.is_empty()),
        "identifier" => node
            .utf8_text(source.as_bytes())
            .ok()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
        "atom" => node
            .named_child(0)
            .and_then(|child| extract_load_argument(child, source)),
        _ => None,
    }
}

pub fn identifier_at_position(
    root: tree_sitter::Node,
    byte: usize,
    source: &str,
) -> Option<String> {
    let node = deepest_node_at(root, byte)?;
    match node.kind() {
        "identifier" => node
            .utf8_text(source.as_bytes())
            .ok()
            .map(|s| s.to_string()),
        "atom" => node
            .named_child(0)
            .filter(|n| n.kind() == "identifier")
            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string()),
        _ => None,
    }
}

pub fn deepest_node_at(root: tree_sitter::Node, byte: usize) -> Option<tree_sitter::Node> {
    let mut best = root;
    let mut cursor = root.walk();
    loop {
        let mut found = false;
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.start_byte() <= byte && byte < child.end_byte() {
                    best = child;
                    found = true;
                    break;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        if found {
            continue;
        }
        break;
    }
    Some(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_maxima(source: &str) -> tree_sitter::Tree {
        let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_collect_colon_assignment() {
        let src = "colorsEsc: sconcat(ascii(27))$\ncolorsRed(x) := x$";
        let tree = parse_maxima(src);
        let uri = Url::parse("file:///tmp/test.mac").unwrap();
        let defs = collect_definitions(&tree, src, &uri);
        assert!(defs.contains_key("colorsEsc"));
        assert!(defs.contains_key("colorsRed"));
    }
}
