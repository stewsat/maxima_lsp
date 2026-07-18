use tower_lsp::lsp_types::*;

use crate::db::Database;

pub fn hover(db: &Database, pos: Position, tree: &tree_sitter::Tree, source: &str) -> Option<Hover> {
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

    let make = |contents: String| -> Hover {
        Hover { contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: contents }), range: Some(mk_range()) }
    };

    if let Some(entry) = db.builtins.functions.get(name) {
        return Some(make(format!("**{}**  ·  {}\n\n{}\n\n---\n_category_: {}", name, entry.signature, entry.doc, entry.category)));
    }

    if let Some(entry) = db.builtins.constants.get(name) {
        return Some(make(format!("**{}**\n\n{}\n\n---\n_category_: {}", name, entry.doc, entry.category)));
    }

    Some(make(format!("**{}**  ·  _variable_", name)))
}

fn position_to_byte(source: &str, pos: Position) -> Option<usize> {
    let mut line = 0u32;
    let mut byte = 0usize;
    for ch in source.chars() {
        if line == pos.line {
            return Some((byte + pos.character as usize).min(source.len()));
        }
        if ch == '\n' {
            line += 1;
        }
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
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        if found { continue; }
        break;
    }
    Some(best)
}
