// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

use std::collections::HashMap;
use std::path::Path;
use tower_lsp::lsp_types::{self, Url};
use tree_sitter::Parser;

use crate::definitions::collect_definitions;
use crate::docstring::{self, ExternalDoc};
use crate::imports;
use crate::paths::PathResolver;

pub enum Lang {
    Maxima,
    CommonLisp,
}

fn extension_to_lang(path: &Path) -> Lang {
    match path.extension().and_then(|e| e.to_str()) {
        Some("lisp") | Some("lsp") => Lang::CommonLisp,
        _ => Lang::Maxima,
    }
}

pub struct Document {
    pub uri: Url,
    pub text: String,
    pub version: i32,
    pub tree: tree_sitter::Tree,
    pub lang: Lang,
    pub external_docs: HashMap<String, ExternalDoc>,
    pub definitions: HashMap<String, lsp_types::Location>,
}

pub struct Database {
    maxima_parser: Parser,
    lisp_parser: Parser,
    paths: PathResolver,
    docs: HashMap<Url, Document>,
    pub builtins: crate::docs::Builtins,
    pub init_functions: HashMap<String, ExternalDoc>,
    pub init_definitions: HashMap<String, lsp_types::Location>,
}

impl Database {
    pub fn new() -> anyhow::Result<Self> {
        let mut maxima_parser = Parser::new();
        let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
        maxima_parser
            .set_language(&lang)
            .map_err(|e| anyhow::anyhow!("Failed to set Maxima language: {e}"))?;

        let mut lisp_parser = Parser::new();
        let lisp_lang: tree_sitter::Language = tree_sitter_commonlisp::LANGUAGE.into();
        lisp_parser
            .set_language(&lisp_lang)
            .map_err(|e| anyhow::anyhow!("Failed to set Common Lisp language: {e}"))?;

        let mut paths = PathResolver::discover();
        let (init_functions, init_definitions) = load_init_file(&mut maxima_parser, &mut paths);

        Ok(Self {
            maxima_parser,
            lisp_parser,
            paths,
            docs: HashMap::new(),
            builtins: crate::docs::Builtins::new(),
            init_functions,
            init_definitions,
        })
    }

    pub fn parser_for(&mut self, path: &Path) -> &mut Parser {
        match extension_to_lang(path) {
            Lang::CommonLisp => &mut self.lisp_parser,
            Lang::Maxima => &mut self.maxima_parser,
        }
    }

    pub fn upsert(&mut self, uri: &Url, text: &str, version: i32) {
        let path = uri.to_file_path().unwrap_or_default();
        let parser = self.parser_for(&path);

        let tree = parser
            .parse(text, None)
            .expect("tree-sitter parse should not fail");

        let base_dir = uri
            .to_file_path()
            .ok()
            .and_then(|p| p.parent().map(|pp| pp.to_path_buf()))
            .unwrap_or_else(|| Path::new(".").to_path_buf());

        let (external_docs, external_defs) =
            imports::resolve_imports(text, &base_dir, &mut self.paths);

        let mut definitions = collect_definitions(&tree, text, uri);
        for (k, v) in external_defs {
            definitions.entry(k).or_insert(v);
        }

        let lang = extension_to_lang(&path);

        self.docs.insert(
            uri.clone(),
            Document {
                uri: uri.clone(),
                text: text.to_string(),
                version,
                tree,
                lang,
                external_docs,
                definitions,
            },
        );
    }

    pub fn get(&self, uri: &Url) -> Option<&Document> {
        self.docs.get(uri)
    }

    pub fn remove(&mut self, uri: &Url) {
        self.docs.remove(uri);
    }

    pub fn lookup_doc(&self, name: &str, uri: &Url) -> Option<crate::docs::DocEntry> {
        if let Some(doc) = self.init_functions.get(name) {
            return Some(docstring::external_to_docentry(doc));
        }
        if let Some(doc) = self.docs.get(uri) {
            if let Some(ext) = doc.external_docs.get(name) {
                return Some(docstring::external_to_docentry(ext));
            }
            if doc.definitions.contains_key(name) {
                let sig: &'static str =
                    Box::leak(format!("{name} (user-defined)").into_boxed_str());
                let cat: &'static str = "user";
                return Some(crate::docs::DocEntry::new(sig, "", &[], "", &[], cat));
            }
        }
        for (other_uri, doc) in &self.docs {
            if other_uri != uri {
                if let Some(ext) = doc.external_docs.get(name) {
                    return Some(docstring::external_to_docentry(ext));
                }
                if doc.definitions.contains_key(name) {
                    let sig: &'static str =
                        Box::leak(format!("{name} (user-defined)").into_boxed_str());
                    return Some(crate::docs::DocEntry::new(
                        sig,
                        "",
                        &[],
                        "",
                        &[],
                        "user",
                    ));
                }
            }
        }
        if let Some(entry) = self.builtins.functions.get(name) {
            return Some(crate::docs::DocEntry::new(
                entry.signature,
                entry.doc,
                entry.params,
                entry.returns,
                entry.examples,
                entry.category,
            ));
        }
        if let Some(entry) = self.builtins.constants.get(name) {
            return Some(crate::docs::DocEntry::new(
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

    pub fn goto_definition(&self, name: &str, current_uri: &Url) -> Option<lsp_types::Location> {
        if let Some(loc) = self.init_definitions.get(name) {
            return Some(loc.clone());
        }
        if let Some(doc) = self.docs.get(current_uri) {
            if let Some(loc) = doc.definitions.get(name) {
                return Some(loc.clone());
            }
        }
        for (other_uri, doc) in &self.docs {
            if other_uri != current_uri {
                if let Some(loc) = doc.definitions.get(name) {
                    return Some(loc.clone());
                }
            }
        }
        None
    }

    pub fn resolve_path(&mut self, name: &str, base_dir: &Path) -> Option<std::path::PathBuf> {
        self.paths.resolve(name, base_dir)
    }

    pub fn all_user_functions(&self, uri: &Url) -> Vec<(String, crate::docs::DocEntry)> {
        let mut result = Vec::new();
        for (name, ext) in &self.init_functions {
            result.push((name.clone(), docstring::external_to_docentry(ext)));
        }
        if let Some(doc) = self.docs.get(uri) {
            for (name, ext) in &doc.external_docs {
                result.push((name.clone(), docstring::external_to_docentry(ext)));
            }
            for name in doc.definitions.keys() {
                if !result.iter().any(|(n, _)| n == name) {
                    let sig: &'static str =
                        Box::leak(format!("{name} (user-defined)").into_boxed_str());
                    result.push((
                        name.clone(),
                        crate::docs::DocEntry::new(sig, "", &[], "", &[], "user"),
                    ));
                }
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
                    format!(
                        "Syntax error: '{}'",
                        &doc.text[range.start_byte..end]
                    )
                };
                diags.push(lsp_types::Diagnostic {
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: range.start_point.row as u32,
                            character: range.start_point.column as u32,
                        },
                        end: lsp_types::Position {
                            line: range.end_point.row as u32,
                            character: range.end_point.column as u32,
                        },
                    },
                    severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                    source: Some("maxima-lsp".to_string()),
                    message: msg,
                    ..Default::default()
                });
            }
            if entering && cursor.goto_first_child() {
                continue;
            }
            if cursor.goto_next_sibling() {
                entering = true;
                continue;
            }
            if cursor.goto_parent() {
                entering = false;
                continue;
            }
            break;
        }
        diags
    }
}

fn load_init_file(
    parser: &mut Parser,
    paths: &mut PathResolver,
) -> (HashMap<String, ExternalDoc>, HashMap<String, lsp_types::Location>) {
    let home = match std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        Ok(h) => h,
        Err(_) => return (HashMap::new(), HashMap::new()),
    };

    let candidates = [
        format!("{home}/.maxima/maxima-init.mac"),
        format!("{home}/.maxima-init.mac"),
    ];

    for path_str in &candidates {
        let path = Path::new(path_str);
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Some(tree) = parser.parse(&content, None) {
                if let Ok(uri) = Url::from_file_path(path) {
                    let mut docs = docstring::extract_docstrings(&content);
                    let mut defs = collect_definitions(&tree, &content, &uri);

                    let base = path.parent().unwrap_or(Path::new("."));
                    let (import_docs, import_defs) =
                        imports::resolve_imports(&content, base, paths);

                    docs.extend(import_docs);
                    for (k, v) in import_defs {
                        defs.entry(k).or_insert(v);
                    }

                    if !docs.is_empty() {
                        tracing::info!(
                            "Loaded {} function(s) from {path_str} (including imports)",
                            docs.len()
                        );
                    }
                    return (docs, defs);
                }
            }
        }
    }
    (HashMap::new(), HashMap::new())
}
