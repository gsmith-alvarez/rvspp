//! Repository guard helpers: pure scans that tests assert on.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Collect every `.rs` file under `dir`, skipping generated output.
#[must_use]
pub fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(dir, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name == "target" || name == ".git" {
                continue;
            }
            collect(&path, out);
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
}

/// Number of lines before the first `#[cfg(test)]` module, i.e. production code.
#[must_use]
pub fn production_lines(source: &str) -> usize {
    source
        .lines()
        .position(|line| line.trim_start().starts_with("#[cfg(test)]"))
        .unwrap_or_else(|| source.lines().count())
}

/// Files that exceed `max_lines` of production code.
#[must_use]
pub fn files_over(dir: &Path, max_lines: usize) -> Vec<(PathBuf, usize)> {
    rust_files(dir)
        .into_iter()
        .filter_map(|path| {
            let source = fs::read_to_string(&path).ok()?;
            let lines = production_lines(&source);
            (lines > max_lines).then_some((path, lines))
        })
        .collect()
}

/// Collect every production `.rs` file under `crates/*/src` across first-party crates.
#[must_use]
pub fn production_rust_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root.join("crates")) else {
        return out;
    };
    for entry in entries.flatten() {
        if entry.file_name() == "repository-guards" {
            continue;
        }
        let src = entry.path().join("src");
        if src.is_dir() {
            collect(&src, &mut out);
        }
    }
    out.sort();
    out
}

/// Production files under `crates/*/src` exceeding `max_lines`.
#[must_use]
pub fn production_files_over(root: &Path, max_lines: usize) -> Vec<(PathBuf, usize)> {
    production_rust_files(root)
        .into_iter()
        .filter_map(|path| {
            let source = fs::read_to_string(&path).ok()?;
            let lines = production_lines(&source);
            (lines > max_lines).then_some((path, lines))
        })
        .collect()
}

/// Crate-level totals for `crates/*/src`.
#[must_use]
pub fn crates_over(root: &Path, max_lines: usize) -> Vec<(PathBuf, usize)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root.join("crates")) else {
        return out;
    };
    for entry in entries.flatten() {
        if entry.file_name() == "repository-guards" {
            continue;
        }
        let src = entry.path().join("src");
        if !src.is_dir() {
            continue;
        }
        let total: usize = rust_files(&src)
            .iter()
            .filter_map(|path| fs::read_to_string(path).ok())
            .map(|source| production_lines(&source))
            .sum();
        if total > max_lines {
            out.push((entry.path(), total));
        }
    }
    out
}

/// `use a::b::{C, d::E};` mixes modules in one statement.
#[must_use]
pub fn split_use_statements(source: &str) -> Vec<String> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("use ") && line.ends_with(';'))
        .filter(|line| {
            let Some(open) = line.find('{') else {
                return false;
            };
            line[open..].contains("::")
        })
        .map(str::to_owned)
        .collect()
}

/// Glob imports, except `use super::*;` inside test modules.
#[must_use]
pub fn glob_imports(source: &str) -> Vec<String> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("use ") && line.contains("::*;"))
        .filter(|line| !line.starts_with("use super::*"))
        .map(str::to_owned)
        .collect()
}

/// `pub use` re-exports.
#[must_use]
pub fn re_exports(source: &str) -> Vec<String> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("pub use ") || line.starts_with("pub(crate) use "))
        .map(str::to_owned)
        .collect()
}

/// Generic ban list that Clippy cannot express (`disallowed-types` has no generics).
#[must_use]
pub fn banned_tokens(source: &str, needles: &[&str]) -> Vec<String> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .filter(|(_, line)| needles.iter().any(|needle| line.contains(needle)))
        .map(|(index, line)| format!("{}: {}", index + 1, line.trim()))
        .collect()
}

/// Check UI crate manifests for forbidden dependencies on engine crates.
#[must_use]
pub fn check_machine_seam(root: &Path) -> Vec<String> {
    const UI_CRATES: &[&str] = &[
        "rvs-app",
        "rvs-app-keys",
        "rvs-app-cmds",
        "rvs-ui",
        "rvs-ui-panes",
        "rvs-ui-modal",
        "rvs-tui",
    ];
    const ENGINE_CRATES: &[&str] = &[
        "rvs-sim",
        "rvs-memory",
        "rvs-executor",
        "rvs-assembler",
        "rvs-fpu",
        "rvs-ecall",
        "rvs-format",
        "rvs-asm-syntax",
        "rvs-tcl",
        "rvs-regs",
        "rvs-transpile",
    ];

    let mut violations = Vec::new();
    for ui_crate in UI_CRATES {
        let manifest_path = root.join("crates").join(ui_crate).join("Cargo.toml");
        let Ok(content) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        for engine_crate in ENGINE_CRATES {
            let pattern = format!("{engine_crate} =");
            if content.contains(&pattern) {
                violations.push(format!(
                    "UI crate {ui_crate} imports engine crate {engine_crate}"
                ));
            }
        }
    }
    violations
}
