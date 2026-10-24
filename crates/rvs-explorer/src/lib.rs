//! File explorer and path suggestions engine for file modals.
//!
//! Provides directory scanning, prefix-based matching and suggestions,
//! tab-completion, directory entry navigation, and real-time content preview.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ExplorerMode {
    #[default]
    Normal,
    Filter,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntryItem {
    pub name: String,
    pub path_display: String,
    pub full_path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Clone, Debug, Default)]
pub struct FileExplorerState {
    pub mode: ExplorerMode,
    pub base_dir: PathBuf,
    pub dir_display_prefix: String,
    pub filter_prefix: String,
    pub entries: Vec<DirEntryItem>,
    pub selected: usize,
    pub scroll_list: usize,
    pub preview_title: String,
    pub preview_lines: Vec<String>,
    pub scroll_preview: usize,
}

impl FileExplorerState {
    pub fn new() -> Self {
        let mut s = Self::default();
        s.update("");
        s
    }

    /// Refresh directory listing and suggestions based on raw path input.
    pub fn update(&mut self, input: &str) {
        let (base_dir, prefix, dir_display_prefix) = parse_path_input(input);
        self.base_dir = base_dir;
        self.filter_prefix = prefix;
        self.dir_display_prefix = dir_display_prefix;

        self.scan_entries();

        // Keep selection in bounds
        if self.entries.is_empty() {
            self.selected = 0;
            self.scroll_list = 0;
        } else if self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }

        self.update_preview();
    }

    fn target_dir(&self) -> &Path {
        Self::target_dir_path(&self.base_dir)
    }

    fn target_dir_path(base_dir: &Path) -> &Path {
        if base_dir.as_os_str().is_empty() {
            Path::new(".")
        } else {
            base_dir
        }
    }

    fn scan_entries(&mut self) {
        self.entries.clear();
        let target_dir = Self::target_dir_path(&self.base_dir);
        if !target_dir.is_dir() {
            return;
        }

        let prefix_lower = self.filter_prefix.to_lowercase();
        let show_hidden = self.filter_prefix.starts_with('.');

        if prefix_lower.is_empty() || "..".starts_with(&prefix_lower) {
            if let Some(parent_display) = self.compute_parent_display(target_dir) {
                self.entries.push(DirEntryItem {
                    name: "..".to_string(),
                    path_display: parent_display,
                    full_path: target_dir.join(".."),
                    is_dir: true,
                    size: 0,
                });
            }
        }

        let read = match std::fs::read_dir(target_dir) {
            Ok(r) => r,
            Err(_) => return,
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in read.filter_map(Result::ok) {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_name.starts_with('.') && !show_hidden {
                continue;
            }

            let file_type = entry.file_type().ok();
            let is_dir = file_type.map(|t| t.is_dir()).unwrap_or(false);
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let full_path = entry.path();

            let name_lower = file_name.to_lowercase();
            if !prefix_lower.is_empty() && !name_lower.starts_with(&prefix_lower) {
                continue;
            }

            let path_display = if is_dir {
                format!("{}{file_name}/", self.dir_display_prefix)
            } else {
                format!("{}{file_name}", self.dir_display_prefix)
            };

            let item = DirEntryItem {
                name: file_name,
                path_display,
                full_path,
                is_dir,
                size,
            };

            if is_dir {
                dirs.push(item);
            } else {
                files.push(item);
            }
        }

        dirs.sort_by_key(|a| a.name.to_lowercase());
        files.sort_by_key(|a| a.name.to_lowercase());

        self.entries.extend(dirs);
        self.entries.extend(files);
    }

    pub fn update_preview(&mut self) {
        self.preview_lines.clear();
        self.scroll_preview = 0;

        if self.entries.is_empty() {
            self.preview_title = "No preview available".to_string();
            if !self.base_dir.exists() {
                self.preview_lines.push(format!(
                    "Directory does not exist: {}",
                    self.base_dir.display()
                ));
            } else {
                self.preview_lines
                    .push("No matching files or directories found.".to_string());
            }
            return;
        }

        let item = &self.entries[self.selected];
        if item.name == ".." {
            self.preview_title = "Parent Directory (..)".to_string();
            self.preview_lines
                .push("Select to navigate up to parent directory.".to_string());
            return;
        }

        if item.is_dir {
            self.preview_title = format!("Dir: {}/", item.name);
            match std::fs::read_dir(&item.full_path) {
                Ok(read) => {
                    let mut sub_entries: Vec<_> = read.filter_map(Result::ok).collect();
                    sub_entries.sort_by_key(|e| {
                        (
                            e.file_type().map(|t| !t.is_dir()).unwrap_or(true),
                            e.file_name(),
                        )
                    });
                    let count = sub_entries.len();
                    self.preview_lines
                        .push(format!("Directory contains {count} items:"));
                    self.preview_lines
                        .push("────────────────────────────────────".to_string());
                    for sub in sub_entries.into_iter().take(30) {
                        let name = sub.file_name().to_string_lossy().into_owned();
                        let is_d = sub.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        if is_d {
                            self.preview_lines.push(format!("  [DIR]  {name}/"));
                        } else {
                            let sz = sub.metadata().map(|m| m.len()).unwrap_or(0);
                            self.preview_lines
                                .push(format!("  [FILE] {name:<26} {:>8}", format_size(sz)));
                        }
                    }
                    if count > 30 {
                        self.preview_lines
                            .push(format!("... and {} more items", count - 30));
                    }
                }
                Err(e) => {
                    self.preview_lines
                        .push(format!("Error reading directory: {e}"));
                }
            }
        } else {
            self.preview_title = format!("File: {} ({})", item.name, format_size(item.size));
            match std::fs::read(&item.full_path) {
                Ok(bytes) => {
                    // Check if binary file (presence of null bytes in first 1KB)
                    let check_len = bytes.len().min(1024);
                    if bytes[..check_len].contains(&0) {
                        self.preview_lines
                            .push(format!("[Binary file - {} bytes]", bytes.len()));
                        self.preview_lines
                            .push("Binary contents cannot be previewed.".to_string());
                    } else {
                        match std::str::from_utf8(&bytes) {
                            Ok(text) => {
                                let lines: Vec<&str> = text.lines().collect();
                                let total = lines.len();
                                for (i, line) in lines.iter().take(80).enumerate() {
                                    self.preview_lines.push(format!("{:>3} │ {}", i + 1, line));
                                }
                                if total > 80 {
                                    self.preview_lines.push(format!(
                                        "... (file has {total} lines, preview truncated)"
                                    ));
                                }
                                if self.preview_lines.is_empty() {
                                    self.preview_lines.push("[Empty file]".to_string());
                                }
                            }
                            Err(_) => {
                                self.preview_lines
                                    .push("[Binary or non-UTF8 encoded file]".to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    self.preview_lines.push(format!("Error reading file: {e}"));
                }
            }
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            if self.selected < self.scroll_list {
                self.scroll_list = self.selected;
            }
            self.update_preview();
        }
    }

    pub fn select_next(&mut self) {
        if !self.entries.is_empty() && self.selected + 1 < self.entries.len() {
            self.selected += 1;
            self.update_preview();
        }
    }

    pub fn page_up(&mut self, n: usize) {
        self.selected = self.selected.saturating_sub(n);
        if self.selected < self.scroll_list {
            self.scroll_list = self.selected;
        }
        self.update_preview();
    }

    pub fn page_down(&mut self, n: usize) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + n).min(self.entries.len() - 1);
            self.update_preview();
        }
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
        self.scroll_list = 0;
        self.update_preview();
    }

    pub fn select_last(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
            self.update_preview();
        }
    }

    pub fn parent_dir_input(&self) -> Option<String> {
        if let Some(parent_entry) = self.entries.iter().find(|e| e.name == "..") {
            return Some(parent_entry.path_display.clone());
        }
        self.compute_parent_display(self.target_dir())
    }

    fn compute_parent_display(&self, target_dir: &Path) -> Option<String> {
        let abs_path = if let Ok(c) = target_dir.canonicalize() {
            c
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(target_dir)
        } else {
            target_dir.to_path_buf()
        };
        let has_parent = abs_path.parent().is_some() && abs_path != Path::new("/");
        if !has_parent {
            return None;
        }

        let parent_display = if self.dir_display_prefix.is_empty()
            || self.dir_display_prefix == "./"
        {
            "../".to_string()
        } else if self.dir_display_prefix.ends_with("/../") || self.dir_display_prefix == "../" {
            format!("{}../", self.dir_display_prefix)
        } else if let Some(parent) = abs_path.parent() {
            if self.dir_display_prefix.starts_with("~/") {
                let home = expand_home();
                if abs_path == Path::new(&home) {
                    format!("{}/", parent.display())
                } else if let Ok(rel) = abs_path.strip_prefix(&home) {
                    if let Some(rel_parent) = rel.parent() {
                        if rel_parent.as_os_str().is_empty() {
                            "~/".to_string()
                        } else {
                            format!("~/{}/", rel_parent.display())
                        }
                    } else {
                        "~/".to_string()
                    }
                } else {
                    format!("{}/", parent.display())
                }
            } else if self.dir_display_prefix.starts_with('/') {
                if parent == Path::new("/") {
                    "/".to_string()
                } else {
                    format!("{}/", parent.display())
                }
            } else {
                let trimmed = self.dir_display_prefix.trim_end_matches('/');
                if let Some(pos) = trimmed.rfind('/') {
                    format!("{}/", &trimmed[..pos])
                } else {
                    "./".to_string()
                }
            }
        } else {
            "../".to_string()
        };

        Some(parent_display)
    }

    pub fn enter_selected(&self) -> Option<(String, bool)> {
        let item = self.selected_item()?;
        Some((item.path_display.clone(), item.is_dir))
    }

    pub fn scroll_preview_down(&mut self, lines: usize) {
        let max_scroll = self.preview_lines.len().saturating_sub(1);
        self.scroll_preview = (self.scroll_preview + lines).min(max_scroll);
    }

    pub fn scroll_preview_up(&mut self, lines: usize) {
        self.scroll_preview = self.scroll_preview.saturating_sub(lines);
    }

    pub fn selected_item(&self) -> Option<&DirEntryItem> {
        self.entries.get(self.selected)
    }

    pub fn selected_is_dir(&self) -> bool {
        self.selected_item()
            .map(|item| item.is_dir)
            .unwrap_or(false)
    }

    /// Tab-completion: complete path with active suggestion or directory.
    pub fn complete(&mut self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        let item = self.selected_item()?;
        Some(item.path_display.clone())
    }
}

pub fn expand_home() -> String {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/home".to_string())
}

/// Expand leading `~` or `~/` to the user's home directory.
pub fn expand_tilde(path_str: &str) -> String {
    let home = expand_home();
    let trimmed = home.trim_end_matches('/');
    if path_str == "~" {
        format!("{trimmed}/")
    } else if let Some(rest) = path_str.strip_prefix("~/") {
        format!("{trimmed}/{rest}")
    } else if let Some(rest) = path_str.strip_prefix("~\\") {
        format!("{trimmed}/{rest}")
    } else {
        path_str.to_string()
    }
}

/// Parse user's path input into `(base_dir, filter_prefix, dir_display_prefix)`.
pub fn parse_path_input(input: &str) -> (PathBuf, String, String) {
    let raw = input.trim();
    if raw.is_empty() {
        return (PathBuf::from("."), String::new(), String::new());
    }

    let (expanded_str, was_tilde) = if raw == "~" {
        (expand_home(), true)
    } else if let Some(rest) = raw.strip_prefix("~/") {
        (
            format!("{}/{}", expand_home().trim_end_matches('/'), rest),
            true,
        )
    } else {
        (raw.to_string(), false)
    };

    let is_dir_query = expanded_str.ends_with('/') || expanded_str.ends_with('\\') || raw == "~";

    if is_dir_query {
        let path = PathBuf::from(&expanded_str);
        let display = if raw == "~" {
            "~/".to_string()
        } else {
            raw.to_string()
        };
        return (path, String::new(), display);
    }

    if let Some(pos) = raw.rfind(['/', '\\']) {
        let dir_display_part = &raw[..=pos];
        let file_part = &raw[pos + 1..];

        let expanded_dir_part = if was_tilde {
            let home = expand_home();
            let rest = &raw[2..=pos];
            format!("{}/{}", home.trim_end_matches('/'), rest)
        } else {
            dir_display_part.to_string()
        };

        let base_dir = PathBuf::from(expanded_dir_part);
        (
            base_dir,
            file_part.to_string(),
            dir_display_part.to_string(),
        )
    } else {
        (PathBuf::from("."), raw.to_string(), String::new())
    }
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_expands_path_inputs() {
        let (dir, pfx, disp) = parse_path_input("");
        assert_eq!(dir, PathBuf::from("."));
        assert_eq!(pfx, "");
        assert_eq!(disp, "");

        let (dir, pfx, disp) = parse_path_input("tests/");
        assert_eq!(dir, PathBuf::from("tests/"));
        assert_eq!(pfx, "");
        assert_eq!(disp, "tests/");

        let (dir, pfx, disp) = parse_path_input("tests/t1");
        assert_eq!(dir, PathBuf::from("tests/"));
        assert_eq!(pfx, "t1");
        assert_eq!(disp, "tests/");

        let (dir, pfx, disp) = parse_path_input("~/");
        assert_eq!(dir, PathBuf::from(expand_home()));
        assert_eq!(pfx, "");
        assert_eq!(disp, "~/");

        let (dir, pfx, disp) = parse_path_input("~/tests");
        assert_eq!(dir, PathBuf::from(expand_home()));
        assert_eq!(pfx, "tests");
        assert_eq!(disp, "~/");

        let (dir, pfx, disp) = parse_path_input("README.md");
        assert_eq!(dir, PathBuf::from("."));
        assert_eq!(pfx, "README.md");
        assert_eq!(disp, "");

        assert!(expand_tilde("~/file.asm").ends_with("/file.asm"));
    }

    #[test]
    fn directory_navigation_and_bounds_clamping() {
        let mut explorer = FileExplorerState::new();
        explorer.update("");
        assert_eq!(explorer.parent_dir_input(), Some("../".to_string()));

        // Even when filtered so ".." is not in entries, parent_dir_input still works
        explorer.update("Cargo");
        assert!(explorer.entries.iter().all(|e| e.name != ".."));
        assert_eq!(explorer.parent_dir_input(), Some("../".to_string()));

        let (path, is_dir) = explorer.enter_selected().expect("should have Cargo entry");
        assert!(path.starts_with("Cargo"));
        if path == "Cargo.toml" {
            assert!(!is_dir);
        }

        // List selection and scroll reset boundaries
        explorer.select_last();
        assert_eq!(explorer.selected, explorer.entries.len() - 1);
        explorer.scroll_list = 3;
        explorer.select_first();
        assert_eq!(explorer.selected, 0);
        assert_eq!(explorer.scroll_list, 0);

        // Preview scroll clamping boundaries
        explorer.preview_lines = (0..10).map(|i| format!("line {i}")).collect();
        explorer.scroll_preview = 0;

        explorer.scroll_preview_down(4);
        assert_eq!(explorer.scroll_preview, 4);

        explorer.scroll_preview_down(100);
        assert_eq!(explorer.scroll_preview, 9);

        explorer.scroll_preview_up(3);
        assert_eq!(explorer.scroll_preview, 6);

        explorer.scroll_preview_up(100);
        assert_eq!(explorer.scroll_preview, 0);
    }
}
