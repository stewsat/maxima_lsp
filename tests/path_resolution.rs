// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

//! Integration tests for Maxima `load` / `batch` path resolution.

use maxima_lsp::paths::PathResolver;
use std::fs;
use std::path::{Path, PathBuf};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maxima_lsp_it_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn resolve_relative_mac_with_and_without_extension() {
    let dir = temp_dir("rel_mac");
    let file = dir.join("helper.mac");
    fs::write(&file, "f(x) := x$\n").unwrap();

    let mut resolver = PathResolver::with_home(&dir);
    assert_eq!(resolver.resolve("helper", &dir), Some(file.clone()));
    assert_eq!(resolver.resolve("helper.mac", &dir), Some(file.clone()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resolve_absolute_path() {
    let dir = temp_dir("abs");
    let file = dir.join("abs_helper.mac");
    fs::write(&file, "g(x) := x$\n").unwrap();

    let mut resolver = PathResolver::with_home(&dir);
    let path_str = file.to_string_lossy().to_string();
    assert_eq!(
        resolver.resolve(&path_str, Path::new("/tmp")),
        Some(file.clone())
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resolve_lisp_extension() {
    let dir = temp_dir("lisp_ext");
    let file = dir.join("pkg.lisp");
    fs::write(&file, "(defun foo () t)\n").unwrap();

    let mut resolver = PathResolver::with_home(&dir);
    assert_eq!(resolver.resolve("pkg", &dir), Some(file.clone()));
    assert_eq!(resolver.resolve("pkg.lisp", &dir), Some(file));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resolve_does_not_match_directory_named_like_package() {
    let dir = temp_dir("dir_not_file");
    // Use a name that cannot collide with Maxima share packages.
    let name = "local_only_dir_pkg_xyz";
    fs::create_dir_all(dir.join(name)).unwrap();

    let mut resolver = PathResolver::with_home(&dir);
    assert!(
        resolver.resolve(name, &dir).is_none(),
        "a bare directory must not resolve as a loadable file"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resolve_missing_package_returns_none() {
    let dir = temp_dir("missing");
    let mut resolver = PathResolver::with_home(&dir);
    assert!(resolver
        .resolve("definitely_not_a_real_maxima_pkg_xyz", &dir)
        .is_none());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resolve_nested_relative_path() {
    let dir = temp_dir("nested");
    let sub = dir.join("lib");
    fs::create_dir_all(&sub).unwrap();
    let file = sub.join("utils.mac");
    fs::write(&file, "u(x) := x$\n").unwrap();

    let mut resolver = PathResolver::with_home(&dir);
    assert_eq!(resolver.resolve("lib/utils", &dir), Some(file.clone()));
    assert_eq!(resolver.resolve("lib/utils.mac", &dir), Some(file));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn discover_resolves_share_package_when_maxima_available() {
    let mut resolver = PathResolver::discover();
    // Skip when Maxima is not installed in the CI/dev environment.
    if std::env::var_os("MAXIMA").is_none()
        && which_maxima().is_none()
        && !Path::new("/usr/bin/maxima").is_file()
        && !Path::new("/opt/homebrew/bin/maxima").is_file()
        && !Path::new("/opt/homebrew/bin/rmaxima").is_file()
    {
        return;
    }

    let base = std::env::temp_dir();
    let draw = resolver.resolve("draw", &base);
    assert!(
        draw.as_ref()
            .is_some_and(|p| p.is_file() && p.to_string_lossy().contains("draw")),
        "expected share library draw via maxima file_search, got {draw:?}"
    );
}

fn which_maxima() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        for name in ["maxima", "rmaxima"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
