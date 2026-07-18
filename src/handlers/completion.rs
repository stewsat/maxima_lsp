use tower_lsp::lsp_types::*;
use url::Url;

use crate::db::Database;
use crate::docs::parse_signature_for_snippet;

pub fn completion(db: &Database, uri: &Url) -> Option<CompletionResponse> {
    let mut items: Vec<CompletionItem> = Vec::new();

    // Built-in functions
    for (name, entry) in &db.builtins.functions {
        let mut md = format!("**{}**\n\n{}", entry.signature, entry.doc);
        if !entry.params.is_empty() {
            md.push_str("\n\n**Parameters:**\n");
            for p in entry.params { md.push_str(&format!("- {}\n", p)); }
        }
        if !entry.returns.is_empty() {
            md.push_str(&format!("\n**Returns:** {}\n", entry.returns));
        }

        let snippet = entry.snippet(name);

        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(entry.category.to_string()),
            documentation: Some(Documentation::String(md)),
            insert_text: Some(snippet),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }

    // User-defined functions from imports
    for (name, entry) in db.all_user_functions(uri) {
        let snippet = parse_signature_for_snippet(entry.signature, &name);

        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some("user function".to_string()),
            documentation: Some(Documentation::String(format!("{}", entry.doc))),
            insert_text: Some(snippet),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }

    // Constants
    for (name, entry) in &db.builtins.constants {
        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: Some(entry.category.to_string()),
            documentation: Some(Documentation::String(format!("**{}**\n\n{}", entry.signature, entry.doc))),
            ..Default::default()
        });
    }

    // Keywords
    for kw in &db.builtins.keywords {
        items.push(CompletionItem {
            label: kw.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..Default::default()
        });
    }

    if items.is_empty() { None } else { Some(CompletionResponse::Array(items)) }
}
