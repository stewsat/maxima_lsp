use std::collections::HashMap;
use crate::docs::DocEntry;

pub struct ParsedDoc {
    pub doc: String,
    pub params: Vec<String>,
    pub returns: String,
    pub examples: Vec<String>,
}

/// Extracted documentation for a user-defined function from a comment.
#[derive(Clone)]
pub struct ExternalDoc {
    pub name: String,
    pub signature: String,
    pub doc: String,
    pub params: Vec<String>,
    pub returns: String,
    pub examples: Vec<String>,
    pub source_file: String,
}

/// Finds function definitions and their preceding docstring comments
/// in a source file, returning a mapping of function name -> ExternalDoc.
pub fn extract_docstrings(source: &str) -> HashMap<String, ExternalDoc> {
    let mut results = HashMap::new();

    let lang: tree_sitter::Language = tree_sitter_maxima::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return results;
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return results,
    };

    let defs = find_defs(&tree, source);
    let comments = find_all_comments(&tree, source);

    tracing::debug!("extract_docstrings: {} defs, {} comments", defs.len(), comments.len());

    for (d_start, _d_end, ref name, ref raw_sig) in &defs {
        tracing::debug!("  def '{}' at {}", name, d_start);
        let comment = comments.iter()
            .filter(|(cs, ce, _)| {
                let ds = *d_start;
                if *ce >= ds { return false; }
                let gap = &source[*ce..ds];
                let ok = gap.chars().all(|c| c == ' ' || c == '\n' || c == '\t' || c == '\r');
                tracing::debug!("    comment [{},{}] gap={:?} ok={}", cs, ce, gap, ok);
                ok
            })
            .last();

        if let Some((_, _, comment_text)) = comment {
            tracing::debug!("  -> matched comment for '{}'", name);
            let parsed = parse_docstring_ext(comment_text, name);
            results.insert(name.clone(), ExternalDoc {
                name: name.clone(),
                signature: raw_sig.clone(),
                doc: parsed.doc,
                params: parsed.params,
                returns: parsed.returns,
                examples: parsed.examples,
                source_file: String::new(),
            });
        } else {
            tracing::debug!("  -> no matching comment for '{}', including with default empty doc", name);
            results.insert(name.clone(), ExternalDoc {
                name: name.clone(),
                signature: raw_sig.clone(),
                doc: format!("{} (user-defined)", name),
                params: vec![],
                returns: String::new(),
                examples: vec![],
                source_file: String::new(),
            });
        }
    }

    results
}

fn find_defs(tree: &tree_sitter::Tree, source: &str) -> Vec<(usize, usize, String, String)> {
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
                    let name = extract_name(node.child(0), source);
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

fn find_all_comments(tree: &tree_sitter::Tree, source: &str) -> Vec<(usize, usize, String)> {
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

fn extract_name(node: Option<tree_sitter::Node>, source: &str) -> String {
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

/// Parse a Maxima comment into structured documentation.
/// Recognizes patterns like:
///   /* Description
///      @param x - description of x
///      @returns description
///      @example usage */
///   /* f(x) - description */
pub fn parse_docstring_ext(comment: &str, _name: &str) -> ParsedDoc {
    let mut text = comment.trim();
    if text.starts_with("/*") { text = &text[2..]; }
    if text.ends_with("*/") { text = &text[..text.len()-2]; }
    text = text.trim();

    let lines: Vec<&str> = text.lines().map(|l| l.trim()).collect();
    let mut doc = String::new();
    let mut params = Vec::new();
    let mut returns = String::new();
    let mut examples = Vec::new();

    for line in &lines {
        let t = line.trim();
        if let Some(p) = t.strip_prefix("@param") {
            params.push(p.trim().to_string());
        } else if let Some(r) = t.strip_prefix("@returns") {
            returns = r.trim().to_string();
        } else if let Some(e) = t.strip_prefix("@example") {
            examples.push(e.trim().to_string());
        } else if !t.is_empty() {
            if doc.is_empty() { doc.push_str(t); }
            else { doc.push(' '); doc.push_str(t); }
        }
    }

    ParsedDoc { doc, params, returns, examples }
}

/// Converts external doc to the DocEntry format for display.
pub fn external_to_docentry(doc: &ExternalDoc) -> DocEntry {
    let params: Vec<&'static str> = doc.params.iter()
        .map(|s| Box::leak(s.clone().into_boxed_str()) as &str)
        .collect();
    let examples: Vec<&'static str> = doc.examples.iter()
        .map(|s| Box::leak(s.clone().into_boxed_str()) as &str)
        .collect();

    DocEntry::new(
        Box::leak(doc.signature.clone().into_boxed_str()),
        Box::leak(doc.doc.clone().into_boxed_str()),
        Box::leak(params.into_boxed_slice()),
        Box::leak(doc.returns.clone().into_boxed_str()),
        Box::leak(examples.into_boxed_slice()),
        Box::leak(format!("user: {}", doc.source_file).into_boxed_str()),
    )
}
