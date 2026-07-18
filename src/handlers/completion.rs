use tower_lsp::lsp_types::*;

use crate::db::Database;

pub fn completion(db: &Database) -> Option<CompletionResponse> {
    let mut items: Vec<CompletionItem> = Vec::new();

    for (name, entry) in &db.builtins.functions {
        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(entry.category.to_string()),
            documentation: Some(Documentation::String(entry.doc.to_string())),
            insert_text: Some(format!("{}(", name)),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        });
    }

    for (name, entry) in &db.builtins.constants {
        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: Some(entry.category.to_string()),
            documentation: Some(Documentation::String(entry.doc.to_string())),
            ..Default::default()
        });
    }

    for kw in &db.builtins.keywords {
        items.push(CompletionItem {
            label: kw.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..Default::default()
        });
    }

    if items.is_empty() { None } else { Some(CompletionResponse::Array(items)) }
}
