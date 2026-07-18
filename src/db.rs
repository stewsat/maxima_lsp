use std::collections::HashMap;
use std::path::Path;
use tower_lsp::lsp_types::{self, Url};
use tree_sitter::Parser;

use crate::docstring::{self, ExternalDoc};
use crate::imports;
use crate::docs::DocEntry;

pub struct Document {
    pub uri: Url,
    pub text: String,
    pub version: i32,
    pub tree: tree_sitter::Tree,
    pub external_docs: HashMap<String, ExternalDoc>,
}

pub struct Database {
    parser: Parser,
    docs: HashMap<Url, Document>,
    pub builtins: crate::docs::Builtins,
}

impl Database {
    pub fn new() -> anyhow::Result<Self> {
        let mut parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
        parser.set_language(&lang).map_err(|e| anyhow::anyhow!("Failed to set Maxima language: {}", e))?;
        Ok(Self { parser, docs: HashMap::new(), builtins: crate::docs::Builtins::new() })
    }

    pub fn upsert(&mut self, uri: &Url, text: &str, version: i32) {
        let tree = self.parser.parse(text, None).expect("tree-sitter parse should not fail");

        // Resolve imports and extract docstrings from external files
        let base_dir = uri.to_file_path().ok()
            .and_then(|p| p.parent().map(|pp| pp.to_path_buf()))
            .unwrap_or_else(|| Path::new(".").to_path_buf());

        let external_docs = imports::resolve_imports(text, &base_dir);

        self.docs.insert(uri.clone(), Document {
            uri: uri.clone(),
            text: text.to_string(),
            version,
            tree,
            external_docs,
        });
    }

    pub fn get(&self, uri: &Url) -> Option<&Document> {
        self.docs.get(uri)
    }

    pub fn remove(&mut self, uri: &Url) {
        self.docs.remove(uri);
    }

    /// Look up documentation for a name.
    /// Checks: 1) external docs from imports, 2) built-in functions, 3) built-in constants.
    pub fn lookup_doc(&self, name: &str, uri: &Url) -> Option<DocEntry> {
        // Check external docs from imported files
        if let Some(doc) = self.docs.get(uri) {
            if let Some(ext) = doc.external_docs.get(name) {
                return Some(docstring::external_to_docentry(ext));
            }
        }

        // Check built-in functions
        if let Some(entry) = self.builtins.functions.get(name) {
            return Some(DocEntry::new(
                entry.signature,
                entry.doc,
                entry.params,
                entry.returns,
                entry.examples,
                entry.category,
            ));
        }

        // Check built-in constants
        if let Some(entry) = self.builtins.constants.get(name) {
            return Some(DocEntry::new(
                entry.signature,
                entry.doc,
                entry.params,
                entry.returns,
                entry.examples,
                entry.category,
            ));
        }

        None
    }

    pub fn all_user_functions(&self, uri: &Url) -> Vec<(String, DocEntry)> {
        let mut result = Vec::new();
        if let Some(doc) = self.docs.get(uri) {
            for (name, ext) in &doc.external_docs {
                result.push((name.clone(), docstring::external_to_docentry(ext)));
            }
        }
        result
    }

    pub fn diagnostics(&self, uri: &Url) -> Vec<lsp_types::Diagnostic> {
        let doc = match self.docs.get(uri) {
            Some(d) => d,
            None => return vec![],
        };
        let mut diags = Vec::new();
        let mut cursor = doc.tree.walk();
        let mut entering = true;

        loop {
            let node = cursor.node();
            if entering && (node.is_error() || node.is_missing()) {
                let range = node.range();
                let msg = if node.is_missing() {
                    format!("Missing '{}'", node.kind())
                } else {
                    let end = (range.start_byte + 40).min(doc.text.len());
                    format!("Syntax error: '{}'", &doc.text[range.start_byte..end])
                };
                diags.push(lsp_types::Diagnostic {
                    range: lsp_types::Range {
                        start: lsp_types::Position { line: range.start_point.row as u32, character: range.start_point.column as u32 },
                        end: lsp_types::Position { line: range.end_point.row as u32, character: range.end_point.column as u32 },
                    },
                    severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                    source: Some("maxima-lsp".to_string()),
                    message: msg,
                    ..Default::default()
                });
            }

            if entering && cursor.goto_first_child() { continue; }
            if cursor.goto_next_sibling() { entering = true; continue; }
            if cursor.goto_parent() { entering = false; continue; }
            break;
        }
        diags
    }
}
