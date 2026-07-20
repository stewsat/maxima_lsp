// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const LOAD_EXTENSIONS: &[&str] = &["", ".mac", ".max", ".lisp", ".lsp", ".wxm", ".dem"];

/// Resolves Maxima `load`/`batch` paths using the installed Maxima runtime when
/// available, with template-based and maxpack fallbacks.
pub struct PathResolver {
    maxima_cmd: Option<PathBuf>,
    maxima_userdir: Option<PathBuf>,
    maxpack_root: Option<PathBuf>,
    templates: Vec<SearchTemplate>,
    maxpack_packages: HashMap<String, PathBuf>,
    cache: HashMap<String, PathBuf>,
}

#[derive(Clone, Debug)]
struct SearchTemplate {
    root: PathBuf,
    recursive: bool,
    extensions: Vec<String>,
}

impl PathResolver {
    pub fn discover() -> Self {
        let maxima_cmd = find_maxima_executable();
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()
            .map(PathBuf::from);

        let mut resolver = Self {
            maxima_cmd: maxima_cmd.clone(),
            maxima_userdir: None,
            maxpack_root: None,
            templates: Vec::new(),
            maxpack_packages: HashMap::new(),
            cache: HashMap::new(),
        };

        if let Some(ref cmd) = maxima_cmd {
            resolver.load_from_maxima(cmd);
        }

        if resolver.templates.is_empty() {
            if let Some(ref home) = home {
                resolver.templates.extend(fallback_templates(home));
            }
        }

        if let Some(ref home) = home {
            resolver.maxpack_root = Some(home.join(".maxpack"));
            resolver.maxpack_packages = discover_maxpack_packages(home);
            if resolver.maxima_userdir.is_none() {
                resolver.maxima_userdir = Some(home.join(".maxima"));
            }
        }

        if let Some(ref cmd) = maxima_cmd {
            tracing::info!(
                "Path resolver: maxima={:?}, templates={}, maxpack packages={}",
                cmd,
                resolver.templates.len(),
                resolver.maxpack_packages.len()
            );
        } else {
            tracing::warn!(
                "Maxima executable not found (set MAXIMA env var); using template/maxpack fallback only"
            );
        }

        resolver
    }

    pub fn resolve(&mut self, name: &str, base_dir: &Path) -> Option<PathBuf> {
        self.resolve_import(name, None, base_dir)
    }

    pub fn resolve_import(
        &mut self,
        package: &str,
        version: Option<&str>,
        base_dir: &Path,
    ) -> Option<PathBuf> {
        let key = format!(
            "{}::{}::{}",
            base_dir.display(),
            package,
            version.unwrap_or("")
        );
        if let Some(hit) = self.cache.get(&key) {
            return Some(hit.clone());
        }

        if let Some(path) = self.resolve_import_uncached(package, version, base_dir) {
            self.cache.insert(key, path.clone());
            return Some(path);
        }
        None
    }

    fn resolve_import_uncached(
        &self,
        name: &str,
        version: Option<&str>,
        base_dir: &Path,
    ) -> Option<PathBuf> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return None;
        }

        if let Some(path) = resolve_existing_path(trimmed, base_dir) {
            return Some(path);
        }

        let stem = Path::new(trimmed)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(trimmed);

        if stem != trimmed {
            if let Some(path) = resolve_existing_path(stem, base_dir) {
                return Some(path);
            }
        }

        if let Some(path) = self.resolve_maxpack(stem, version) {
            return Some(path);
        }

        if version.is_none() {
            if let Some(path) = self.maxpack_packages.get(stem) {
                if path.is_file() {
                    return Some(path.clone());
                }
            }
        }

        if let Some(path) = self.maxima_file_search(stem) {
            return Some(path);
        }

        if let Some(path) = self.search_templates(stem) {
            return Some(path);
        }

        None
    }

    fn resolve_maxpack(&self, package: &str, version: Option<&str>) -> Option<PathBuf> {
        if let Some(ver) = version {
            let home = self.maxpack_home()?;
            let base = home.join(package);
            for candidate in [
                base.join(ver).join("src").join("init.mac"),
                base.join(ver).join("src").join(format!("{package}.mac")),
                base.join(ver).join("src").join("init.lisp"),
            ] {
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            return None;
        }

        self.maxpack_packages
            .get(package)
            .filter(|p| p.is_file())
            .cloned()
    }

    fn maxpack_home(&self) -> Option<&Path> {
        self.maxpack_root.as_deref()
    }

    fn load_from_maxima(&mut self, cmd: &Path) {
        let script = r#"block([paths],
  print("USERDIR:", maxima_userdir),
  paths : append(append(file_search_lisp, file_search_maxima), file_search_demo),
  for p in paths do (
    if p # false and stringp(p) then print("TEMPLATE:", p)
  )
)$"#;

        let output = run_maxima_batch(cmd, script);
        for line in output.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("USERDIR:") {
                let dir = rest.trim();
                if !dir.is_empty() && dir != "false" {
                    self.maxima_userdir = Some(PathBuf::from(dir));
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("TEMPLATE:") {
                let template = rest.trim();
                if template.is_empty() {
                    continue;
                }
                if let Some(parsed) = parse_template(template) {
                    self.templates.push(parsed);
                }
            }
        }
    }

    fn maxima_file_search(&self, name: &str) -> Option<PathBuf> {
        let cmd = self.maxima_cmd.as_ref()?;
        let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(r#"print(file_search("{escaped}"))$"#);
        let output = run_maxima_batch(cmd, &script);
        parse_maxima_file_path(&output)
    }

    fn search_templates(&self, name: &str) -> Option<PathBuf> {
        for template in &self.templates {
            if let Some(path) = search_template(template, name) {
                return Some(path);
            }
        }
        None
    }
}

fn resolve_existing_path(name: &str, base_dir: &Path) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute() {
        if path.is_file() {
            return Some(path.to_path_buf());
        }
        return try_extensions(path, LOAD_EXTENSIONS);
    }

    let relative = base_dir.join(name);
    if relative.is_file() {
        return Some(relative);
    }
    if let Some(found) = try_extensions(&relative, LOAD_EXTENSIONS) {
        return Some(found);
    }

    None
}

fn try_extensions(base: &Path, extensions: &[&str]) -> Option<PathBuf> {
    if base.is_file() {
        return Some(base.to_path_buf());
    }

    for ext in extensions {
        if ext.is_empty() {
            continue;
        }
        let candidate = if base.extension().is_some() {
            base.with_extension(ext.trim_start_matches('.'))
        } else {
            PathBuf::from(format!("{}{}", base.to_string_lossy(), ext))
        };
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn parse_template(template: &str) -> Option<SearchTemplate> {
    let template = template.trim();
    if template.is_empty() {
        return None;
    }

    let recursive = template.contains("/**/");
    let (root_part, pattern_part) = if let Some(idx) = template.find("/**/") {
        (&template[..idx], &template[idx + 4..])
    } else if let Some(idx) = template.rfind('/') {
        (&template[..idx], &template[idx + 1..])
    } else {
        return None;
    };

    let extensions = parse_glob_extensions(pattern_part);
    if extensions.is_empty() {
        return None;
    }

    Some(SearchTemplate {
        root: PathBuf::from(root_part),
        recursive,
        extensions,
    })
}

fn parse_glob_extensions(pattern: &str) -> Vec<String> {
    if pattern.contains('{') {
        let mut extensions = Vec::new();
        if let Some(start) = pattern.find('{') {
            if let Some(end) = pattern[start..].find('}') {
                for part in pattern[start + 1..start + end].split(',') {
                    let part = part.trim().trim_start_matches('*').trim_start_matches('.');
                    if !part.is_empty() {
                        extensions.push(format!(".{part}"));
                    }
                }
            }
        }
        if !extensions.is_empty() {
            return extensions;
        }
    }

    if let Some(dot) = pattern.rfind('.') {
        let ext = pattern[dot..].trim_start_matches('*');
        if ext.starts_with('.') {
            return vec![ext.to_string()];
        }
    }

    vec![".mac".into(), ".lisp".into(), ".max".into()]
}

fn search_template(template: &SearchTemplate, name: &str) -> Option<PathBuf> {
    if !template.root.is_dir() {
        return None;
    }

    for ext in &template.extensions {
        let direct = template.root.join(format!("{name}{ext}"));
        if direct.is_file() {
            return Some(direct);
        }

        let nested = template.root.join(name).join(format!("{name}{ext}"));
        if nested.is_file() {
            return Some(nested);
        }
    }

    if template.recursive {
        search_dir_recursive(&template.root, name, &template.extensions)
    } else {
        None
    }
}

fn search_dir_recursive(dir: &Path, name: &str, extensions: &[String]) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = search_dir_recursive(&path, name, extensions) {
                return Some(found);
            }
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if file_stem != name {
            continue;
        }

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let dotted = format!(".{ext}");
            if extensions.iter().any(|e| e == &dotted) {
                return Some(path);
            }
        }
    }
    None
}

fn discover_maxpack_packages(home: &Path) -> HashMap<String, PathBuf> {
    let mut packages = HashMap::new();
    let root = home.join(".maxpack");
    if !root.is_dir() {
        return packages;
    }

    const SKIP: &[&str] = &["repo", "bin", ".tmp"];

    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(_) => return packages,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(pkg_name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };

        if pkg_name.starts_with('.') || SKIP.contains(&pkg_name) {
            continue;
        }

        let candidates = [
            path.join("latest").join("src").join("init.mac"),
            path.join("latest").join("src").join(format!("{pkg_name}.mac")),
            path.join("latest").join("src").join("init.lisp"),
            path.join("src").join("init.mac"),
        ];

        for candidate in candidates {
            if candidate.is_file() {
                tracing::debug!("maxpack package '{pkg_name}' -> {candidate:?}");
                packages.insert(pkg_name.to_string(), candidate);
                break;
            }
        }
    }

    packages
}

fn fallback_templates(home: &Path) -> Vec<SearchTemplate> {
    let userdir = home.join(".maxima");
    let mut templates = Vec::new();

    if userdir.is_dir() {
        for ext in [".mac", ".lisp", ".wxm"] {
            templates.push(SearchTemplate {
                root: userdir.clone(),
                recursive: true,
                extensions: vec![ext.to_string()],
            });
        }
    }

    templates
}

fn find_maxima_executable() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("MAXIMA") {
        let path = PathBuf::from(&raw);
        if path.is_file() {
            return Some(path);
        }
    }

    for candidate in ["maxima", "rmaxima", "xmaxima", "smaxima"] {
        if let Some(path) = find_on_path(candidate) {
            return Some(path);
        }
    }

    None
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn run_maxima_batch(cmd: &Path, script: &str) -> String {
    let output = Command::new(cmd)
        .args(["-q", "--batch-string", script])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            format!("{stdout}{stderr}")
        }
        Err(err) => {
            tracing::debug!("Failed to run maxima batch: {err}");
            String::new()
        }
    }
}

fn parse_maxima_file_path(output: &str) -> Option<PathBuf> {
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("Loading ")
            || line.starts_with("(%i")
            || line.starts_with("(%o")
            || line == "false"
        {
            continue;
        }

        for token in line.split_whitespace() {
            let token = token.trim_matches('"');
            if token == "false" || token.is_empty() {
                continue;
            }
            let path = PathBuf::from(token);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_template_recursive() {
        let t = parse_template("/opt/share/maxima/5.49.0/share/**/*.lisp").unwrap();
        assert!(t.recursive);
        assert_eq!(t.root, PathBuf::from("/opt/share/maxima/5.49.0/share"));
        assert!(t.extensions.contains(&".lisp".to_string()));
    }

    #[test]
    fn test_parse_template_flat() {
        let t = parse_template("/opt/share/maxima/5.49.0/src/*.mac").unwrap();
        assert!(!t.recursive);
        assert_eq!(t.root, PathBuf::from("/opt/share/maxima/5.49.0/src"));
    }

    #[test]
    fn test_resolve_existing_relative() {
        let dir = std::env::temp_dir().join("maxima_lsp_paths_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("helper.mac");
        std::fs::write(&file, "f(x) := x$").unwrap();

        let mut resolver = PathResolver {
            maxima_cmd: None,
            maxima_userdir: None,
            maxpack_root: None,
            templates: Vec::new(),
            maxpack_packages: HashMap::new(),
            cache: HashMap::new(),
        };

        assert_eq!(resolver.resolve("helper", &dir), Some(file));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_try_extensions_skips_directories() {
        let dir = std::env::temp_dir().join("maxima_lsp_paths_dir_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("draw")).unwrap();

        assert!(try_extensions(&dir.join("draw"), LOAD_EXTENSIONS).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_and_resolve_with_maxima_if_available() {
        let mut resolver = PathResolver::discover();
        if resolver.maxima_cmd.is_none() {
            return;
        }

        let base = std::env::temp_dir();
        let draw = resolver.resolve("draw", &base);
        assert!(
            draw.as_ref().is_some_and(|p| p.is_file() && p.to_string_lossy().contains("draw")),
            "expected draw.lisp from maxima file_search, got {draw:?}"
        );

        let colors = resolver.resolve("colors", &base);
        assert!(
            colors.as_ref().is_some_and(|p| p.is_file() && p.to_string_lossy().contains("colors")),
            "expected maxpack colors init.mac, got {colors:?}"
        );
    }
}
