// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

use std::collections::HashMap;
use tower_lsp::lsp_types::*;
use tree_sitter::StreamingIterator;

const HIGHLIGHTS_QUERY: &str = tree_sitter_maxima::HIGHLIGHTS_QUERY;

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            "comment".into(),
            "string".into(),
            "number".into(),
            "keyword".into(),
            "operator".into(),
            "function".into(),
            "variable".into(),
            "parameter".into(),
        ],
        token_modifiers: vec!["readonly".into()],
    }
}

fn capture_to_ttype(name: &str) -> Option<(u32, u32)> {
    match name {
        "comment" => Some((0, 0)),
        "string" => Some((1, 0)),
        "number" => Some((2, 0)),
        "constant.builtin" => Some((2, 1)),
        "keyword" => Some((3, 0)),
        "operator" => Some((4, 0)),
        "function" => Some((5, 0)),
        "variable.parameter" => Some((7, 0)),
        "variable" => Some((6, 0)),
        "punctuation.bracket" => Some((4, 0)),
        "punctuation.delimiter" => Some((4, 0)),
        _ => None,
    }
}

pub fn tokens(tree: &tree_sitter::Tree, source: &str) -> anyhow::Result<SemanticTokensResult> {
    let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
    let query = tree_sitter::Query::new(&lang, HIGHLIGHTS_QUERY)
        .map_err(|e| anyhow::anyhow!("Failed to compile highlights query: {}", e))?;

    let mut cursor = tree_sitter::QueryCursor::new();
    let cap_names = query.capture_names();
    let mut qm = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let mut map: HashMap<(u32, u32, u32), (u32, u32)> = HashMap::new();

    qm.advance();
    while qm.get().is_some() {
        let m = qm.get().unwrap();
        for capture in m.captures {
            let name = &cap_names[capture.index as usize];
            let node = capture.node;
            if node.is_error() || node.is_missing() {
                continue;
            }
            let r = node.range();
            let (ti, tm) = match capture_to_ttype(name) {
                Some(v) => v,
                None => continue,
            };
            map.insert(
                (r.start_point.row as u32, r.start_point.column as u32, (r.end_byte - r.start_byte) as u32),
                (ti, tm),
            );
        }
        qm.advance();
    }

    let mut tokens: Vec<SemanticToken> = Vec::new();
    let mut sorted: Vec<_> = map.into_iter().collect();
    sorted.sort_by_key(|&((l, c, _), _)| (l, c));

    let mut pl = 0u32;
    let mut ps = 0u32;
    for ((l, c, len), (ti, tm)) in sorted {
        let dl = l - pl;
        let ds = if dl == 0 { c - ps } else { c };
        tokens.push(SemanticToken { delta_line: dl, delta_start: ds, length: len, token_type: ti, token_modifiers_bitset: tm });
        pl = l;
        ps = if dl == 0 { c } else { c };
    }

    Ok(SemanticTokensResult::Tokens(SemanticTokens { result_id: None, data: tokens }))
}
