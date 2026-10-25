use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process;

use repository_guards::banned_tokens;
use repository_guards::check_machine_seam;
use repository_guards::crates_over;
use repository_guards::files_over;
use repository_guards::glob_imports;
use repository_guards::production_files_over;
use repository_guards::production_lines;
use repository_guards::production_rust_files;
use repository_guards::re_exports;
use repository_guards::rust_files;
use repository_guards::split_use_statements;

fn fixture(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("guards-{name}-{}", process::id()));
    let _ = fs::remove_dir_all(&dir);
    for (relative, body) in files {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture dir");
        }
        fs::write(&path, body).expect("fixture file");
    }
    dir
}

#[test]
fn production_lines_ignores_test_module() {
    let source = "fn a() {}\nfn b() {}\n#[cfg(test)]\nmod tests {\n    fn c() {}\n}\n";
    assert_eq!(
        production_lines(source),
        2,
        "only code above the test module counts"
    );
}

#[test]
fn file_size_guard_catches_oversize() {
    let body = "fn f() {}\n".repeat(30);
    let dir = fixture(
        "size",
        &[("src/big.rs", &body), ("src/small.rs", "fn f() {}\n")],
    );
    let over = files_over(dir.as_path(), 10);
    assert_eq!(
        over.len(),
        1,
        "exactly one file exceeds the budget: {over:?}"
    );
    assert!(
        over[0].0.ends_with("src/big.rs"),
        "the oversize file is big.rs"
    );
}

#[test]
fn crate_size_guard_sums_members() {
    let dir = fixture(
        "crate",
        &[
            ("crates/a/src/lib.rs", &"fn f() {}\n".repeat(7)),
            ("crates/a/src/extra.rs", &"fn g() {}\n".repeat(7)),
            ("crates/b/src/lib.rs", "fn h() {}\n"),
        ],
    );
    let over = crates_over(dir.as_path(), 10);
    assert_eq!(over.len(), 1, "crate a totals 14 lines: {over:?}");
    assert!(over[0].0.ends_with("crates/a"), "crate a is the offender");
}

#[test]
fn use_statement_guard_flags_mixed_modules() {
    let bad = "use foo::{Bar, bar::Baz};\n";
    let good = "use foo::bar::{Bar, Baz};\n";
    assert_eq!(
        split_use_statements(bad).len(),
        1,
        "mixed modules are a violation"
    );
    assert!(
        split_use_statements(good).is_empty(),
        "single module is fine"
    );
}

#[test]
fn glob_guard_allows_super_star_in_tests_only() {
    let source = "use super::*;\nuse foo::*;\n";
    let hits = glob_imports(source);
    assert_eq!(
        hits,
        vec!["use foo::*;".to_owned()],
        "super::* stays allowed"
    );
}

#[test]
fn reexport_guard_flags_pub_use() {
    let source = "pub use foo::Bar;\npub(crate) use baz::Qux;\nuse foo::Bar;\n";
    assert_eq!(
        re_exports(source).len(),
        2,
        "both pub use forms violate the rule"
    );
}

#[test]
fn token_guard_catches_unit_mutex() {
    let source =
        "let s: Arc<Mutex<()>> = Arc::new(Mutex::new(()));\n// Arc<Mutex<()>> in a comment\n";
    let hits = banned_tokens(source, &["Arc<Mutex<()>>", "Rc<RefCell<()>>"]);
    assert_eq!(hits.len(), 1, "comments are ignored, code is not: {hits:?}");
}

#[test]
fn guard_helpers_tolerate_missing_dirs() {
    let missing = Path::new("/nonexistent/repository-guard-fixture");
    assert!(
        rust_files(missing).is_empty(),
        "missing root yields no files"
    );
    assert!(
        crates_over(missing, 1).is_empty(),
        "missing crates dir yields no violations"
    );
}

// ---------------------------------------------------------------------------
// Repository-level tree tests against AGENTS.md rules
// ---------------------------------------------------------------------------

const KNOWN_OVERSIZE_FILES: &[&str] = &[
    "crates/rvs-machine-riscv/src/lib.rs",
    "crates/rvs-app/src/lib.rs",
];

const KNOWN_OVERSIZE_CRATES: &[&str] = &["crates/rvs-machine-riscv", "crates/rvs-app"];

#[test]
fn repo_no_new_oversize_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut hits = production_files_over(&root, 1000);
    hits.retain(|(path, _)| {
        let shown = path.to_string_lossy();
        !KNOWN_OVERSIZE_FILES
            .iter()
            .any(|known| shown.ends_with(known))
    });
    assert!(
        hits.is_empty(),
        "files over 1000 production lines budget: {hits:?}"
    );
}

#[test]
fn repo_no_new_oversize_crates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut hits = crates_over(&root, 1000);
    hits.retain(|(path, _)| {
        let shown = path.to_string_lossy();
        !KNOWN_OVERSIZE_CRATES
            .iter()
            .any(|known| shown.ends_with(known))
    });
    assert!(
        hits.is_empty(),
        "crates over 1000 production lines budget: {hits:?}"
    );
}

#[test]
fn repo_no_reexports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = production_rust_files(&root);
    let mut violations = Vec::new();
    for file in files {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        let bad = re_exports(&content);
        if !bad.is_empty() {
            violations.push((file, bad));
        }
    }
    assert!(
        violations.is_empty(),
        "found pub use / pub(crate) use re-exports: {violations:?}"
    );
}

#[test]
fn repo_no_path_or_include_attributes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = production_rust_files(&root);
    let mut violations = Vec::new();
    for file in files {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        let bad = banned_tokens(&content, &["#[path =", "#[path=", "include!("]);
        if !bad.is_empty() {
            violations.push((file, bad));
        }
    }
    assert!(
        violations.is_empty(),
        "found #[path] or include! in source: {violations:?}"
    );
}

#[test]
fn repo_no_banned_generic_types() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = production_rust_files(&root);
    let mut violations = Vec::new();
    for file in files {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        let bad = banned_tokens(&content, &["Arc<Mutex<()>>", "Rc<RefCell<()>>"]);
        if !bad.is_empty() {
            violations.push((file, bad));
        }
    }
    assert!(
        violations.is_empty(),
        "found banned generic types: {violations:?}"
    );
}

#[test]
fn repo_machine_seam_isolation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let violations = check_machine_seam(&root);
    assert!(
        violations.is_empty(),
        "machine seam violated (UI crate depends directly on engine crate): {violations:?}"
    );
}
