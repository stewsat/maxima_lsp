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
    pub definitions: HashMap<String, lsp_types::Location>,
}

pub struct Database {
    parser: Parser,
    docs: HashMap<Url, Document>,
    pub builtins: crate::docs::Builtins,
    pub init_functions: HashMap<String, ExternalDoc>,
    pub init_definitions: HashMap<String, lsp_types::Location>,
}

impl Database {
    pub fn new() -> anyhow::Result<Self> {
        let mut parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
        parser.set_language(&lang).map_err(|e| anyhow::anyhow!("Failed to set Maxima language: {}", e))?;

        let (init_functions, init_definitions) = load_init_file(&mut parser);

        Ok(Self {
            parser,
            docs: HashMap::new(),
            builtins: crate::docs::Builtins::new(),
            init_functions,
            init_definitions,
        })
    }

    pub fn upsert(&mut self, uri: &Url, text: &str, version: i32) {
        let tree = self.parser.parse(text, None).expect("tree-sitter parse should not fail");

        let base_dir = uri.to_file_path().ok()
            .and_then(|p| p.parent().map(|pp| pp.to_path_buf()))
            .unwrap_or_else(|| Path::new(".").to_path_buf());

        let (external_docs, _external_defs) = imports::resolve_imports(text, &base_dir, uri);

        let definitions = collect_definitions(&tree, text, uri);

        self.docs.insert(uri.clone(), Document {
            uri: uri.clone(),
            text: text.to_string(),
            version,
            tree,
            external_docs,
            definitions,
        });
    }

    pub fn get(&self, uri: &Url) -> Option<&Document> {
        self.docs.get(uri)
    }

    pub fn remove(&mut self, uri: &Url) {
        self.docs.remove(uri);
    }

    pub fn lookup_doc(&self, name: &str, uri: &Url) -> Option<DocEntry> {
        if let Some(doc) = self.init_functions.get(name) {
            return Some(docstring::external_to_docentry(doc));
        }
        if let Some(doc) = self.docs.get(uri) {
            if let Some(ext) = doc.external_docs.get(name) {
                return Some(docstring::external_to_docentry(ext));
            }
        }
        if let Some(entry) = self.builtins.functions.get(name) {
            return Some(DocEntry::new(entry.signature, entry.doc, entry.params, entry.returns, entry.examples, entry.category));
        }
        if let Some(entry) = self.builtins.constants.get(name) {
            return Some(DocEntry::new(entry.signature, entry.doc, entry.params, entry.returns, entry.examples, entry.category));
        }
        None
    }

    pub fn goto_definition(&self, name: &str, current_uri: &Url) -> Option<lsp_types::Location> {
        // 1) Init file definitions
        if let Some(loc) = self.init_definitions.get(name) {
            return Some(loc.clone());
        }
        // 2) Current document definitions
        if let Some(doc) = self.docs.get(current_uri) {
            if let Some(loc) = doc.definitions.get(name) {
                return Some(loc.clone());
            }
        }
        // 3) Imported file definitions
        if let Some(doc) = self.docs.get(current_uri) {
            for (_n, ext) in &doc.external_docs {
                if ext.name == name && !ext.source_file.is_empty() {
                    if let Ok(url) = Url::from_file_path(&ext.source_file) {
                        // We don't have exact position, just the file
                        return Some(lsp_types::Location {
                            uri: url,
                            range: lsp_types::Range::default(),
                        });
                    }
                }
            }
        }
        None
    }

    pub fn all_user_functions(&self, uri: &Url) -> Vec<(String, DocEntry)> {
        let mut result = Vec::new();
        for (name, ext) in &self.init_functions {
            result.push((name.clone(), docstring::external_to_docentry(ext)));
        }
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

fn collect_definitions(tree: &tree_sitter::Tree, source: &str, uri: &Url) -> HashMap<String, lsp_types::Location> {
    let mut defs = HashMap::new();
    let mut cursor = tree.walk();
    let mut entering = true;

    loop {
        let node = cursor.node();
        if node.kind() == "binary_expression" {
            if let Some(op) = node.child(1) {
                if op.kind() == ":=" || op.kind() == "::=" {
                    if let Some(name) = extract_name(node.child(0), source) {
                        let r = node.range();
                        defs.insert(name, lsp_types::Location {
                            uri: uri.clone(),
                            range: lsp_types::Range {
                                start: lsp_types::Position { line: r.start_point.row as u32, character: r.start_point.column as u32 },
                                end: lsp_types::Position { line: r.end_point.row as u32, character: r.end_point.column as u32 },
                            },
                        });
                    }
                }
            }
        }
        if entering && cursor.goto_first_child() { continue; }
        if cursor.goto_next_sibling() { entering = true; continue; }
        if cursor.goto_parent() { entering = false; continue; }
        break;
    }
    defs
}

fn extract_name(node: Option<tree_sitter::Node>, source: &str) -> Option<String> {
    let n = node?;
    let mut c = n.walk();
    loop {
        let cn = c.node();
        if cn.kind() == "identifier" {
            return cn.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
        }
        if c.goto_first_child() { continue; }
        if c.goto_next_sibling() { continue; }
        loop {
            if !c.goto_parent() { return None; }
            if c.goto_next_sibling() { break; }
        }
    }
}

fn load_init_file(parser: &mut Parser) -> (HashMap<String, ExternalDoc>, HashMap<String, lsp_types::Location>) {
    let home = match std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        Ok(h) => h,
        Err(_) => return (HashMap::new(), HashMap::new()),
    };

    let candidates = [
        format!("{}/.maxima/maxima-init.mac", home),
        format!("{}/.maxima-init.mac", home),
    ];

    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Some(tree) = parser.parse(&content, None) {
                if let Ok(uri) = Url::from_file_path(path) {
                    let docs = docstring::extract_docstrings(&content);
                    let defs = collect_definitions(&tree, &content, &uri);

                    if !docs.is_empty() {
                        tracing::info!("Loaded {} function(s) from {}", docs.len(), path);
                    }
                    return (docs, defs);
                }
            }
        }
    }
    (HashMap::new(), HashMap::new())
}
