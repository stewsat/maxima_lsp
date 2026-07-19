// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use std::path::Path;
use crate::db::Database;
use crate::handlers;
use crate::imports;

pub struct Backend {
    pub client: Client,
    pub db: Arc<Mutex<Database>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        let db = Database::new().expect("Failed to initialize Maxima parser");
        Self { client, db: Arc::new(Mutex::new(db)) }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::INCREMENTAL)),
                semantic_tokens_provider: Some(
                    SemanticTokensOptions {
                        legend: handlers::semantic_tokens::legend(),
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                        range: Some(true),
                        ..Default::default()
                    }.into(),
                ),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    ..Default::default()
                }),
                hover_provider: Some(true.into()),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn shutdown(&self) -> Result<()> { Ok(()) }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let mut db = self.db.lock().await;
        let uri = params.text_document.uri.clone();
        db.upsert(&uri, &params.text_document.text, params.text_document.version);
        let diags = db.diagnostics(&uri);
        self.client.publish_diagnostics(uri, diags, Some(params.text_document.version)).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut db = self.db.lock().await;
        let uri = params.text_document.uri.clone();
        let version = params.text_document.version;

        let mut new_text = match db.get(&uri) {
            Some(doc) => doc.text.clone(),
            None => return,
        };

        for change in &params.content_changes {
            match change.range {
                Some(range) => {
                    if let (Some(s), Some(e)) = (pos_to_byte(&new_text, range.start), pos_to_byte(&new_text, range.end)) {
                        new_text.replace_range(s..e, &change.text);
                    }
                }
                None => new_text = change.text.clone(),
            }
        }

        db.upsert(&uri, &new_text, version);
        let diags = db.diagnostics(&uri);
        self.client.publish_diagnostics(uri, diags, Some(version)).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(text) = params.text {
            let mut db = self.db.lock().await;
            let uri = params.text_document.uri.clone();
            db.upsert(&uri, &text, 0);
            let diags = db.diagnostics(&uri);
            self.client.publish_diagnostics(uri, diags, None).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut db = self.db.lock().await;
        db.remove(&params.text_document.uri);
    }

    async fn semantic_tokens_full(&self, params: SemanticTokensParams) -> Result<Option<SemanticTokensResult>> {
        let db = self.db.lock().await;
        let doc = match db.get(&params.text_document.uri) {
            Some(d) => d,
            None => return Ok(None),
        };
        match handlers::semantic_tokens::tokens(&doc.tree, &doc.text) {
            Ok(t) => Ok(Some(t)),
            Err(e) => { tracing::error!("semantic_tokens error: {}", e); Ok(None) }
        }
    }

    async fn document_symbol(&self, params: DocumentSymbolParams) -> Result<Option<DocumentSymbolResponse>> {
        let db = self.db.lock().await;
        let doc = match db.get(&params.text_document.uri) { Some(d) => d, None => return Ok(None) };
        Ok(handlers::symbols::document_symbols(&doc.tree, &doc.text))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let db = self.db.lock().await;
        let uri = params.text_document_position_params.text_document.uri.clone();
        let pos = params.text_document_position_params.position;
        let doc = match db.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let byte = lsp_pos_to_byte(&doc.text, pos);

        let maybe_path = byte.and_then(|b| {
            let node = deepest_node_at(doc.tree.root_node(), b)?;
            let kind = node.kind();
            if kind != "string" && kind != "atom" { return None; }
            let text = node.utf8_text(doc.text.as_bytes()).ok()?;
            let name = text.trim_matches('"');
            if name.is_empty() || name.contains('(') || name.contains(')') { return None; }
            let base_dir = uri.to_file_path().ok()
                .and_then(|p| p.parent().map(|pp| pp.to_path_buf()))
                .unwrap_or_else(|| Path::new(".").to_path_buf());
            imports::resolve_import_path(name, &base_dir)
        });
        if let Some(path) = maybe_path {
            if let Ok(url) = tower_lsp::lsp_types::Url::from_file_path(&path) {
                return Ok(Some(GotoDefinitionResponse::Scalar(
                    tower_lsp::lsp_types::Location {
                        uri: url,
                        range: tower_lsp::lsp_types::Range::default(),
                    },
                )));
            }
        }

        // Fallback: try to resolve as a symbol definition
        if let Some(byte) = byte {
            if let Some(name) = find_identifier_at(doc.tree.root_node(), byte, &doc.text) {
                if let Some(loc) = db.goto_definition(&name, &uri) {
                    return Ok(Some(GotoDefinitionResponse::Scalar(loc)));
                }
            }
        }
        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let db = self.db.lock().await;
        let uri = params.text_document_position.text_document.uri;
        Ok(handlers::completion::completion(&db, &uri))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let db = self.db.lock().await;
        let uri = params.text_document_position_params.text_document.uri.clone();
        let doc = match db.get(&uri) { Some(d) => d, None => return Ok(None) };
        Ok(handlers::hover::hover(&db, params.text_document_position_params.position, &uri, &doc.tree, &doc.text))
    }
}

fn deepest_node_at(root: tree_sitter::Node, byte: usize) -> Option<tree_sitter::Node> {
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
                if !cursor.goto_next_sibling() { break; }
            }
        }
        if found { continue; }
        break;
    }
    Some(best)
}

fn find_identifier_at(root: tree_sitter::Node, byte: usize, source: &str) -> Option<String> {
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
                if !cursor.goto_next_sibling() { break; }
            }
        }
        if found { continue; }
        break;
    }
    if best.kind() == "identifier" {
        best.utf8_text(source.as_bytes()).ok().map(|s| s.to_string())
    } else {
        None
    }
}

fn lsp_pos_to_byte(text: &str, pos: Position) -> Option<usize> {
    let mut line = 0u32;
    let mut byte = 0usize;
    for ch in text.chars() {
        if line == pos.line {
            return Some((byte + pos.character as usize).min(text.len()));
        }
        if ch == '\n' { line += 1; }
        byte += ch.len_utf8();
    }
    None
}

fn pos_to_byte(text: &str, pos: Position) -> Option<usize> {
    let mut line = 0u32;
    let mut byte = 0usize;
    for ch in text.chars() {
        if line == pos.line {
            return Some((byte + pos.character as usize).min(text.len()));
        }
        if ch == '\n' {
            line += 1;
        }
        byte += ch.len_utf8();
    }
    None
}
