use std::collections::HashMap;
use crate::docs::DocEntry;

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

    // Collect all function definition positions
    let mut defs: Vec<(usize, usize, String, String)> = Vec::new(); // (start_byte, end_byte, name, raw_signature)
    let mut cursor = tree.walk();
    let mut entering = true;

    loop {
        let node = cursor.node();

        if node.kind() == "binary_expression" {
            // Check if this is a function definition (:= operator)
            if let Some(op) = node.child(1) {
                if op.kind() == ":=" || op.kind() == "::=" {
                    // Extract function name from left side
                    if let Some(func_part) = node.child(0) {
                        let func_text = func_part.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                        let name = extract_function_name(&func_part, source);

                        defs.push((node.start_byte(), node.end_byte(), name, func_text));
                    }
                }
            }
        }

        if entering && cursor.goto_first_child() { continue; }
        if cursor.goto_next_sibling() { entering = true; continue; }
        if cursor.goto_parent() { entering = false; continue; }
        break;
    }

    // Collect all comment positions
    let mut comments: Vec<(usize, usize, String)> = Vec::new();
    let mut c2 = tree.walk();
    let mut entering2 = true;
    loop {
        let node = c2.node();
        if node.kind() == "comment" {
            let text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
            comments.push((node.start_byte(), node.end_byte(), text));
        }
        if entering2 && c2.goto_first_child() { continue; }
        if c2.goto_next_sibling() { entering2 = true; continue; }
        if c2.goto_parent() { entering2 = false; continue; }
        break;
    }

    // Match comments to the following function definition
    for i in 0..defs.len() {
        let (d_start, d_end, ref name, ref raw_sig) = defs[i];
        let comment = comments.iter()
            .filter(|(c_end, _, _)| {
                let ce = *c_end;
                let gap = &source[ce..d_start];
                gap.chars().all(|c| c == ' ' || c == '\n' || c == '\t' || c == '\r')
            })
            .last();

        if let Some((_, _, comment_text)) = comment {
            let parsed = parse_docstring(comment_text, name, &raw_sig, source);
            results.insert(name.clone(), parsed);
        }
    }

    results
}

fn extract_function_name(node: &tree_sitter::Node, source: &str) -> String {
    // Navigate to find the identifier: binary_expression -> function_call -> atom -> identifier
    // or binary_expression -> atom -> identifier
    let mut n = *node;
    loop {
        let kind = n.kind();
        if kind == "identifier" {
            return n.utf8_text(source.as_bytes()).unwrap_or("").to_string();
        }
        if n.child_count() == 0 {
            break;
        }
        n = n.child(0).unwrap();
    }
    // Fallback: try to find any identifier child
    if let Some(id) = find_identifier(*node, source) {
        return id;
    }
    node.utf8_text(source.as_bytes()).unwrap_or("").to_string()
}

fn find_identifier(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    loop {
        let n = cursor.node();
        if n.kind() == "identifier" {
            return n.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
        }
        if cursor.goto_first_child() { continue; }
        if cursor.goto_next_sibling() { continue; }
        loop {
            if !cursor.goto_parent() { return None; }
            if cursor.goto_next_sibling() { break; }
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
fn parse_docstring(comment: &str, name: &str, _sig: &str, _source: &str) -> ExternalDoc {
    // Strip /* and */ and trim
    let mut text = comment.trim();
    if text.starts_with("/*") { text = &text[2..]; }
    if text.ends_with("*/") { text = &text[..text.len()-2]; }
    text = text.trim();

    let lines: Vec<&str> = text.lines().map(|l| l.trim()).collect();

    let mut doc = String::new();
    let mut params: Vec<String> = Vec::new();
    let mut returns = String::new();
    let mut examples: Vec<String> = Vec::new();
    let mut signature = format!("{} ...", name);

    for line in &lines {
        let trimmed = line.trim();
        if let Some(p) = trimmed.strip_prefix("@param") {
            params.push(p.trim().to_string());
        } else if let Some(r) = trimmed.strip_prefix("@returns") {
            returns = r.trim().to_string();
        } else if let Some(e) = trimmed.strip_prefix("@example") {
            examples.push(e.trim().to_string());
        } else if trimmed.starts_with('(') || trimmed.contains(" - ") {
            // Try to extract signature from first line
            if trimmed.starts_with('(') || (trimmed.contains(name) && trimmed.contains(" -")) {
                signature = trimmed.to_string();
            } else if doc.is_empty() {
                doc.push_str(trimmed);
            } else {
                doc.push(' ');
                doc.push_str(trimmed);
            }
        } else if !trimmed.is_empty() {
            if doc.is_empty() {
                doc.push_str(trimmed);
            } else {
                doc.push(' ');
                doc.push_str(trimmed);
            }
        }
    }

    ExternalDoc {
        name: name.to_string(),
        signature,
        doc,
        params,
        returns,
        examples,
        source_file: String::new(),
    }
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
