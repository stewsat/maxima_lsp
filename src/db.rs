use std::collections::HashMap;
use tower_lsp::lsp_types::{self, Url};
use tree_sitter::Parser;

pub struct Document {
    pub uri: Url,
    pub text: String,
    pub version: i32,
    pub tree: tree_sitter::Tree,
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
        self.docs.insert(uri.clone(), Document { uri: uri.clone(), text: text.to_string(), version, tree });
    }

    pub fn get(&self, uri: &Url) -> Option<&Document> {
        self.docs.get(uri)
    }

    pub fn remove(&mut self, uri: &Url) {
        self.docs.remove(uri);
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
