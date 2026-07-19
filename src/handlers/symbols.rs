// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

use tower_lsp::lsp_types::*;
use tree_sitter::StreamingIterator;

const TAGS_QUERY: &str = include_str!("../../../tree-sitter-maxima/queries/tags.scm");

pub fn document_symbols(tree: &tree_sitter::Tree, source: &str) -> Option<DocumentSymbolResponse> {
    let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
    let query = tree_sitter::Query::new(&lang, TAGS_QUERY).ok()?;
    let mut cursor = tree_sitter::QueryCursor::new();
    let cap_names = query.capture_names();
    let mut mq = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut symbols: Vec<DocumentSymbol> = Vec::new();

    mq.advance();
    while mq.get().is_some() {
        let m = mq.get().unwrap();
        let mut name = String::new();
        let mut kind = SymbolKind::VARIABLE;
        let mut range = Range::default();
        let mut selection = Range::default();

        for c in m.captures {
            let cname = &cap_names[c.index as usize];
            let node = c.node;
            let r = node.range();

            match *cname {
                "name" => {
                    name = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                    selection = Range {
                        start: Position { line: r.start_point.row as u32, character: r.start_point.column as u32 },
                        end: Position { line: r.end_point.row as u32, character: r.end_point.column as u32 },
                    };
                }
                "definition.function" => {
                    kind = SymbolKind::FUNCTION;
                    range = Range {
                        start: Position { line: r.start_point.row as u32, character: r.start_point.column as u32 },
                        end: Position { line: r.end_point.row as u32, character: r.end_point.column as u32 },
                    };
                }
                "definition.variable" => {
                    kind = SymbolKind::VARIABLE;
                    range = Range {
                        start: Position { line: r.start_point.row as u32, character: r.start_point.column as u32 },
                        end: Position { line: r.end_point.row as u32, character: r.end_point.column as u32 },
                    };
                }
                _ => {}
            }
        }

        if !name.is_empty() {
            symbols.push(DocumentSymbol {
                name,
                kind,
                range,
                selection_range: selection,
                children: None,
                detail: None,
                deprecated: None,
                tags: None,
            });
        }
        mq.advance();
    }

    if symbols.is_empty() { None } else { Some(DocumentSymbolResponse::Nested(symbols)) }
}
