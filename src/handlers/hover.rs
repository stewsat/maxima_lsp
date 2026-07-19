// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

use tower_lsp::lsp_types::*;
use url::Url;

use crate::db::Database;

pub fn hover(db: &Database, pos: Position, uri: &Url, tree: &tree_sitter::Tree, source: &str) -> Option<Hover> {
    let byte = position_to_byte(source, pos)?;
    let node = deepest_at(tree.root_node(), byte)?;
    if node.kind() != "identifier" {
        return None;
    }
    let name = node.utf8_text(source.as_bytes()).ok()?;

    let mk_range = || -> Range {
        Range {
            start: Position { line: node.start_position().row as u32, character: node.start_position().column as u32 },
            end: Position { line: node.end_position().row as u32, character: node.end_position().column as u32 },
        }
    };

    let entry = db.lookup_doc(name, uri);

    if let Some(entry) = entry {
        let mut md = String::new();

        md.push_str(&format!("**{}**  ·  {}\n\n", name, entry.category));
        md.push_str(&format!("`{}`\n\n", entry.signature));
        md.push_str(entry.doc);
        md.push('\n');

        if !entry.params.is_empty() {
            md.push_str("\n---\n**Parameters:**\n");
            for p in entry.params {
                md.push_str(&format!("- {}\n", p));
            }
        }

        if !entry.returns.is_empty() && entry.returns != name {
            md.push_str(&format!("\n**Returns:** {}\n", entry.returns));
        }

        if !entry.examples.is_empty() {
            md.push_str("\n**Examples:**\n");
            for ex in entry.examples {
                md.push_str(&format!("```maxima\n{}\n```\n", ex));
            }
        }

        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: md }),
            range: Some(mk_range()),
        });
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: format!("**{}**  ·  _variable_", name) }),
        range: Some(mk_range()),
    })
}

fn position_to_byte(source: &str, pos: Position) -> Option<usize> {
    let mut line = 0u32;
    let mut byte = 0usize;
    for ch in source.chars() {
        if line == pos.line {
            return Some((byte + pos.character as usize).min(source.len()));
        }
        if ch == '\n' { line += 1; }
        byte += ch.len_utf8();
    }
    None
}

fn deepest_at(node: tree_sitter::Node, byte: usize) -> Option<tree_sitter::Node> {
    let mut best = node;
    let mut cursor = node.walk();
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
                if !cursor.goto_next_sibling() { break; }
            }
        }
        if found { continue; }
        break;
    }
    Some(best)
}
