// Copyright (c) Stewsat
// Author: Yassin Achengli Benmouais
// SPDX-License-Identifier: BSD

//! Integration tests for maxpack package layout discovery and resolution.

use maxima_lsp::imports::{find_imports, resolve_imports};
use maxima_lsp::paths::PathResolver;
use std::fs;
use std::path::PathBuf;

fn temp_home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maxima_lsp_maxpack_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Classic maxpack layout: `~/.maxpack/<pkg>/latest/src/init.mac`
fn install_latest_init(home: &std::path::Path, pkg: &str, body: &str) -> PathBuf {
    let src = home
        .join(".maxpack")
        .join(pkg)
        .join("latest")
        .join("src");
    fs::create_dir_all(&src).unwrap();
    let init = src.join("init.mac");
    fs::write(&init, body).unwrap();
    init
}

/// Versioned layout: `~/.maxpack/<pkg>/<version>/src/init.mac`
fn install_versioned_init(home: &std::path::Path, pkg: &str, version: &str, body: &str) -> PathBuf {
    let src = home
        .join(".maxpack")
        .join(pkg)
        .join(version)
        .join("src");
    fs::create_dir_all(&src).unwrap();
    let init = src.join("init.mac");
    fs::write(&init, body).unwrap();
    init
}

#[test]
fn resolves_latest_init_mac() {
    let home = temp_home("latest");
    let init = install_latest_init(
        &home,
        "colors",
        "/* Red text */\ncolorsRed(x) := sconcat(x)$\n",
    );

    let mut resolver = PathResolver::with_home(&home);
    assert_eq!(resolver.resolve("colors", &home), Some(init));
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn resolves_package_named_mac_under_latest() {
    let home = temp_home("named_mac");
    let src = home.join(".maxpack").join("utils").join("latest").join("src");
    fs::create_dir_all(&src).unwrap();
    let file = src.join("utils.mac");
    fs::write(&file, "util(x) := x$\n").unwrap();

    let mut resolver = PathResolver::with_home(&home);
    assert_eq!(resolver.resolve("utils", &home), Some(file));
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn resolves_versioned_import() {
    let home = temp_home("versioned");
    let v1 = install_versioned_init(&home, "colors", "1.0.0", "colorsV1(x) := x$\n");
    let _v2 = install_versioned_init(&home, "colors", "2.0.0", "colorsV2(x) := x$\n");
    // Also provide latest so unversioned import still works.
    let _ = install_latest_init(&home, "colors", "colorsLatest(x) := x$\n");

    let mut resolver = PathResolver::with_home(&home);
    assert_eq!(
        resolver.resolve_import("colors", Some("1.0.0"), &home),
        Some(v1)
    );
    let latest = resolver.resolve_import("colors", None, &home);
    assert!(
        latest
            .as_ref()
            .is_some_and(|p| p.to_string_lossy().contains("latest")),
        "unversioned import should prefer latest, got {latest:?}"
    );
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn skips_repo_bin_and_tmp_directories() {
    let home = temp_home("skip_dirs");
    for skip in ["repo", "bin", ".tmp"] {
        let src = home.join(".maxpack").join(skip).join("latest").join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("init.mac"), "noop() := 0$\n").unwrap();
    }
    let real = install_latest_init(&home, "realpkg", "real() := 1$\n");

    let mut resolver = PathResolver::with_home(&home);
    assert_eq!(resolver.resolve("realpkg", &home), Some(real));
    assert!(resolver.resolve("repo", &home).is_none());
    assert!(resolver.resolve("bin", &home).is_none());
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn find_imports_resolves_maxpack_package() {
    let home = temp_home("find_imports");
    let init = install_latest_init(
        &home,
        "colors",
        "colorsRed(x) := x$\n",
    );

    let mut resolver = PathResolver::with_home(&home);
    let paths = find_imports(r#"import(colors)$"#, &home, &mut resolver);
    assert_eq!(paths, vec![init]);
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn resolve_imports_indexes_functions_from_maxpack() {
    let home = temp_home("index_fns");
    let _ = install_latest_init(
        &home,
        "colors",
        "/* Paint red */\ncolorsRed(x) := sconcat(x)$\ncolorsBlue(x) := sconcat(x)$\n",
    );

    let mut resolver = PathResolver::with_home(&home);
    let (docs, defs) = resolve_imports(r#"import(colors)$"#, &home, &mut resolver);
    assert!(
        defs.contains_key("colorsRed") || docs.contains_key("colorsRed"),
        "expected colorsRed, defs={:?} docs={:?}",
        defs.keys().collect::<Vec<_>>(),
        docs.keys().collect::<Vec<_>>()
    );
    assert!(
        defs.contains_key("colorsBlue") || docs.contains_key("colorsBlue"),
        "expected colorsBlue"
    );
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn resolve_imports_follows_nested_load_inside_package() {
    let home = temp_home("nested_load");
    let pkg_src = home
        .join(".maxpack")
        .join("parent")
        .join("latest")
        .join("src");
    fs::create_dir_all(&pkg_src).unwrap();
    fs::write(
        pkg_src.join("child.mac"),
        "/* nested helper */\nchildFn(x) := x$\n",
    )
    .unwrap();
    fs::write(
        pkg_src.join("init.mac"),
        "load(\"child\")$\nparentFn(x) := childFn(x)$\n",
    )
    .unwrap();

    let mut resolver = PathResolver::with_home(&home);
    let (docs, defs) = resolve_imports(r#"import(parent)$"#, &home, &mut resolver);
    assert!(
        defs.contains_key("childFn") || docs.contains_key("childFn"),
        "nested load should index childFn, defs={:?} docs={:?}",
        defs.keys().collect::<Vec<_>>(),
        docs.keys().collect::<Vec<_>>()
    );
    let _ = fs::remove_dir_all(&home);
}
