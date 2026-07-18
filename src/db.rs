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
    pub init_functions: HashMap<String, ExternalDoc>,
}

impl Database {
    pub fn new() -> anyhow::Result<Self> {
        let mut parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
        parser.set_language(&lang).map_err(|e| anyhow::anyhow!("Failed to set Maxima language: {}", e))?;

        // Auto-load ~/.maxima/maxima-init.mac
        let init_functions = load_init_file(&mut parser);

        Ok(Self {
            parser,
            docs: HashMap::new(),
            builtins: crate::docs::Builtins::new(),
            init_functions,
        })
    }

    pub fn upsert(&mut self, uri: &Url, text: &str, version: i32) {
        let tree = self.parser.parse(text, None).expect("tree-sitter parse should not fail");

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

    pub fn lookup_doc(&self, name: &str, uri: &Url) -> Option<DocEntry> {
        // 1) Init file functions
        if let Some(doc) = self.init_functions.get(name) {
            return Some(docstring::external_to_docentry(doc));
        }

        // 2) Imported file docs
        if let Some(doc) = self.docs.get(uri) {
            if let Some(ext) = doc.external_docs.get(name) {
                return Some(docstring::external_to_docentry(ext));
            }
        }

        // 3) Built-in functions
        if let Some(entry) = self.builtins.functions.get(name) {
            return Some(DocEntry::new(entry.signature, entry.doc, entry.params, entry.returns, entry.examples, entry.category));
        }

        // 4) Built-in constants
        if let Some(entry) = self.builtins.constants.get(name) {
            return Some(DocEntry::new(entry.signature, entry.doc, entry.params, entry.returns, entry.examples, entry.category));
        }

        None
    }

    pub fn all_user_functions(&self, uri: &Url) -> Vec<(String, DocEntry)> {
        let mut result = Vec::new();
        // Init file functions
        for (name, ext) in &self.init_functions {
            result.push((name.clone(), docstring::external_to_docentry(ext)));
        }
        // Imported file functions
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

fn load_init_file(parser: &mut Parser) -> HashMap<String, ExternalDoc> {
    let home = match std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        Ok(h) => h,
        Err(_) => return HashMap::new(),
    };

    let candidates = [
        format!("{}/.maxima/maxima-init.mac", home),
        format!("{}/.maxima-init.mac", home),
    ];

    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Some(tree) = parser.parse(&content, None) {
                let mut results = HashMap::new();
                let defs = find_function_defs(&tree, &content);
                let comments = find_comments(&tree, &content);

                for i in 0..defs.len() {
                    let (d_start, _d_end, ref name, ref sig) = defs[i];
                    if let Some(comment) = comments.iter()
                        .filter(|(ce, _, _)| {
                            let c = *ce; let ds = d_start;
                            let gap = &content[c..ds];
                            gap.chars().all(|c| c == ' ' || c == '\n' || c == '\t' || c == '\r')
                        })
                        .last()
                    {
                        let parsed = docstring::parse_docstring_ext(&comment.2, name);
                        results.insert(name.clone(), ExternalDoc {
                            name: name.clone(),
                            signature: sig.clone(),
                            doc: parsed.doc.clone(),
                            params: parsed.params.clone(),
                            returns: parsed.returns.clone(),
                            examples: parsed.examples.clone(),
                            source_file: path.clone(),
                        });
                    }
                }

                if !results.is_empty() {
                    tracing::info!("Loaded {} function(s) from {}", results.len(), path);
                    return results;
                }
            }
        }
    }
    HashMap::new()
}

fn find_function_defs(tree: &tree_sitter::Tree, source: &str) -> Vec<(usize, usize, String, String)> {
    let mut defs = Vec::new();
    let mut cursor = tree.walk();
    let mut entering = true;

    loop {
        let node = cursor.node();
        if node.kind() == "binary_expression" {
            if let Some(op) = node.child(1) {
                if op.kind() == ":=" || op.kind() == "::=" {
                    let func_text = node.child(0)
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("").to_string();
                    let name = find_name(node.child(0), source);
                    defs.push((node.start_byte(), node.end_byte(), name, func_text));
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

fn find_name(node: Option<tree_sitter::Node>, source: &str) -> String {
    let n = match node { Some(n) => n, None => return String::new() };
    let mut c = n.walk();
    loop {
        let cn = c.node();
        if cn.kind() == "identifier" {
            return cn.utf8_text(source.as_bytes()).unwrap_or("").to_string();
        }
        if c.goto_first_child() { continue; }
        if c.goto_next_sibling() { continue; }
        loop {
            if !c.goto_parent() { return String::new(); }
            if c.goto_next_sibling() { break; }
        }
    }
}

fn find_comments(tree: &tree_sitter::Tree, source: &str) -> Vec<(usize, usize, String)> {
    let mut comments = Vec::new();
    let mut cursor = tree.walk();
    let mut entering = true;
    loop {
        let node = cursor.node();
        if node.kind() == "comment" {
            let text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
            comments.push((node.start_byte(), node.end_byte(), text));
        }
        if entering && cursor.goto_first_child() { continue; }
        if cursor.goto_next_sibling() { entering = true; continue; }
        if cursor.goto_parent() { entering = false; continue; }
        break;
    }
    comments
}
