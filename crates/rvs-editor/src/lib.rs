//! Vim-like multi-line buffer with undo/redo and grapheme-aware editing.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Default, Clone)]
struct Snapshot {
    lines: Vec<String>,
    row: usize,
    col: usize,
}

/// Multi-line editor with undo/redo and modified tracking.
#[derive(Default)]
pub struct Editor {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
    pub offset_row: usize,
    pub offset_col: usize,
    pub modified: bool,
    pub yank_buffer: String,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

/// Byte offset of the `char_col`-th char (line length when past the end).
fn byte_of_char(line: &str, char_col: usize) -> usize {
    line.char_indices()
        .nth(char_col)
        .map(|(b, _)| b)
        .unwrap_or(line.len())
}

fn is_grapheme_boundary(line: &str, byte: usize) -> bool {
    byte == line.len() || line.grapheme_indices(true).any(|(b, _)| b == byte)
}

/// Largest grapheme-cluster start at or before `char_col`.
fn snap_boundary(line: &str, char_col: usize) -> usize {
    let mut col = char_col.min(line.chars().count());
    while col > 0 && !is_grapheme_boundary(line, byte_of_char(line, col)) {
        col -= 1;
    }
    col
}

/// Grapheme-cluster start strictly before `char_col`.
fn prev_boundary(line: &str, char_col: usize) -> usize {
    let col = char_col.min(line.chars().count());
    if col == 0 {
        return 0;
    }
    snap_boundary(line, col - 1)
}

/// Grapheme-cluster start strictly after `char_col`.
fn next_boundary(line: &str, char_col: usize) -> usize {
    let count = line.chars().count();
    if char_col >= count {
        return count;
    }
    let mut col = char_col + 1;
    while col < count && !is_grapheme_boundary(line, byte_of_char(line, col)) {
        col += 1;
    }
    col
}

/// Smallest grapheme-cluster start at or after `char_col`.
fn ceil_boundary(line: &str, char_col: usize) -> usize {
    let clamped = char_col.min(line.chars().count());
    let col = snap_boundary(line, clamped);
    if col == clamped {
        col
    } else {
        next_boundary(line, col)
    }
}

impl Editor {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            lines: self.lines.clone(),
            row: self.row,
            col: self.col,
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.lines = text.lines().map(|l| l.to_string()).collect();
        if self.lines.is_empty() {
            self.lines = vec![String::new()];
        }
        self.row = 0;
        self.col = 0;
        self.offset_row = 0;
        self.offset_col = 0;
        self.undo.clear();
        self.redo.clear();
    }

    pub fn replace_text(&mut self, text: &str) {
        self.push_undo();
        self.lines = text.lines().map(|l| l.to_string()).collect();
        if self.lines.is_empty() {
            self.lines = vec![String::new()];
        }
        self.modified = true;
        self.clamp_cursor();
    }

    pub fn text(&self) -> String {
        let mut s = self.lines.join("\n");
        if !self.lines.is_empty() {
            s.push('\n');
        }
        s
    }

    pub fn needs_save(&self) -> bool {
        self.modified
            && self
                .lines
                .iter()
                .any(|l| !l.trim_matches(['\n', '\r']).is_empty())
    }

    fn push_undo(&mut self) {
        self.undo.push(self.snapshot());
        if self.undo.len() > 1000 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn clamp_cursor(&mut self) {
        let lines = self.lines.len().max(1);
        self.row = self.row.min(lines - 1);
        let line = &self.lines[self.row];
        self.col = snap_boundary(line, self.col);
    }

    pub fn insert_char(&mut self, c: char) {
        self.push_undo();
        let line = &mut self.lines[self.row];
        let byte = line
            .char_indices()
            .nth(self.col)
            .map(|(b, _)| b)
            .unwrap_or(line.len());
        line.insert(byte, c);
        self.col += 1;
        self.modified = true;
    }

    pub fn newline(&mut self) {
        self.push_undo();
        let line = self.lines[self.row].clone();
        let byte = line
            .char_indices()
            .nth(self.col)
            .map(|(b, _)| b)
            .unwrap_or(line.len());
        let (head, tail) = line.split_at(byte);
        self.lines[self.row] = head.to_string();
        self.lines.insert(self.row + 1, tail.to_string());
        self.row += 1;
        self.col = 0;
        self.modified = true;
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            let line = &self.lines[self.row];
            let end = byte_of_char(line, self.col);
            let start_col = prev_boundary(line, self.col);
            let start = byte_of_char(line, start_col);
            if start < end {
                self.push_undo();
                self.lines[self.row].replace_range(start..end, "");
                self.col = start_col;
                self.modified = true;
            }
        } else if self.row > 0 {
            self.push_undo();
            let cur = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.lines[self.row].chars().count();
            self.lines[self.row].push_str(&cur);
            self.modified = true;
        }
    }

    pub fn delete_word_backward(&mut self) {
        if self.col == 0 {
            self.backspace();
            return;
        }
        self.push_undo();
        let line = &mut self.lines[self.row];
        let chars: Vec<char> = line.chars().collect();
        self.col = self.col.min(chars.len());
        let mut new_col = self.col;
        while new_col > 0
            && chars
                .get(new_col - 1)
                .map(|c| c.is_whitespace())
                .unwrap_or(false)
        {
            new_col -= 1;
        }
        while new_col > 0
            && chars
                .get(new_col - 1)
                .map(|c| !c.is_whitespace())
                .unwrap_or(false)
        {
            new_col -= 1;
        }
        new_col = snap_boundary(line, new_col);
        let byte_start = byte_of_char(line, new_col);
        let byte_end = byte_of_char(line, self.col);
        if byte_start < byte_end {
            line.replace_range(byte_start..byte_end, "");
            self.col = new_col;
            self.modified = true;
        }
    }

    pub fn delete(&mut self) {
        let line = &self.lines[self.row];
        let byte = byte_of_char(line, self.col);
        if byte < line.len() {
            let next = byte_of_char(line, next_boundary(line, self.col));
            self.push_undo();
            self.lines[self.row].replace_range(byte..next, "");
            self.modified = true;
        } else if self.row + 1 < self.lines.len() {
            self.push_undo();
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
            self.modified = true;
        }
    }

    pub fn move_left(&mut self) {
        if self.col > 0 {
            let line = &self.lines[self.row];
            self.col = prev_boundary(line, self.col);
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.lines[self.row].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        let line = &self.lines[self.row];
        let len = line.chars().count();
        if self.col < len {
            self.col = next_boundary(line, self.col);
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.clamp_cursor();
        }
    }

    pub fn move_down(&mut self) {
        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.clamp_cursor();
        }
    }

    pub fn move_home(&mut self) {
        self.col = 0;
    }

    pub fn move_end(&mut self) {
        self.col = self.lines[self.row].chars().count();
    }

    pub fn page_up(&mut self, n: usize) {
        self.row = self.row.saturating_sub(n);
        self.clamp_cursor();
    }

    pub fn page_down(&mut self, n: usize) {
        self.row = (self.row + n).min(self.lines.len().saturating_sub(1));
        self.clamp_cursor();
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.lines = s.lines;
            self.row = s.row;
            self.col = s.col;
            self.clamp_cursor();
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.lines = s.lines;
            self.row = s.row;
            self.col = s.col;
            self.clamp_cursor();
        }
    }

    /// Every `(row, char_col)` where `needle` starts, in buffer order.
    fn matches_of(&self, needle: &str) -> Vec<(usize, usize)> {
        let ndl: Vec<char> = needle.chars().collect();
        if ndl.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (row, line) in self.lines.iter().enumerate() {
            let hay: Vec<char> = line.chars().collect();
            if hay.len() < ndl.len() {
                continue;
            }
            for col in 0..=hay.len() - ndl.len() {
                if hay[col..col + ndl.len()] == ndl[..] {
                    out.push((row, col));
                }
            }
        }
        out
    }

    /// Move the cursor to the next (`forward`) or previous occurrence of
    /// `needle`, wrapping at the ends. Returns false when there is no match.
    pub fn find(&mut self, needle: &str, forward: bool) -> bool {
        if self.lines.is_empty() {
            return false;
        }
        let matches = self.matches_of(needle);
        if matches.is_empty() {
            return false;
        }
        let here = (self.row, self.col);
        let target = if forward {
            matches
                .iter()
                .find(|&&pos| pos > here)
                .or_else(|| matches.first())
        } else {
            matches
                .iter()
                .rev()
                .find(|&&pos| pos < here)
                .or_else(|| matches.last())
        };
        match target {
            Some(&(row, col)) => {
                self.row = row;
                self.col = snap_boundary(&self.lines[row], col);
                self.clamp_cursor();
                true
            }
            None => false,
        }
    }

    pub fn move_word_forward(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        let line = &self.lines[self.row];
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.col;
        if col >= chars.len() {
            if self.row + 1 < self.lines.len() {
                self.row += 1;
                self.col = 0;
            }
            return;
        }
        let is_ident = |c: char| c.is_alphanumeric() || c == '_';
        if is_ident(chars[col]) {
            while col < chars.len() && is_ident(chars[col]) {
                col += 1;
            }
            while col < chars.len() && chars[col].is_whitespace() {
                col += 1;
            }
        } else if chars[col].is_whitespace() {
            while col < chars.len() && chars[col].is_whitespace() {
                col += 1;
            }
        } else {
            while col < chars.len() && !is_ident(chars[col]) && !chars[col].is_whitespace() {
                col += 1;
            }
            while col < chars.len() && chars[col].is_whitespace() {
                col += 1;
            }
        }
        let col = ceil_boundary(line, col);
        if col >= chars.len() && self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        } else {
            self.col = col;
        }
    }

    pub fn move_word_backward(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        if self.col == 0 {
            if self.row > 0 {
                self.row -= 1;
                self.col = self.lines[self.row].chars().count();
            }
            return;
        }
        let line = &self.lines[self.row];
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.col.min(chars.len());
        let is_ident = |c: char| c.is_alphanumeric() || c == '_';

        while col > 0 && (col > chars.len() || chars[col - 1].is_whitespace()) {
            col -= 1;
        }
        if col > 0 {
            if is_ident(chars[col - 1]) {
                while col > 0 && is_ident(chars[col - 1]) {
                    col -= 1;
                }
            } else {
                while col > 0 && !is_ident(chars[col - 1]) && !chars[col - 1].is_whitespace() {
                    col -= 1;
                }
            }
        }
        self.col = snap_boundary(line, col);
    }

    pub fn move_first_non_whitespace(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        let line = &self.lines[self.row];
        let col = line.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
        self.col = snap_boundary(line, col);
    }

    pub fn insert_line_below(&mut self) {
        self.push_undo();
        if self.lines.is_empty() {
            self.lines = vec![String::new()];
            self.row = 0;
            self.col = 0;
        } else {
            self.lines.insert(self.row + 1, String::new());
            self.row += 1;
            self.col = 0;
        }
        self.modified = true;
    }

    pub fn insert_line_above(&mut self) {
        self.push_undo();
        if self.lines.is_empty() {
            self.lines = vec![String::new()];
            self.row = 0;
            self.col = 0;
        } else {
            self.lines.insert(self.row, String::new());
            self.col = 0;
        }
        self.modified = true;
    }

    pub fn delete_line(&mut self) -> String {
        self.push_undo();
        self.modified = true;
        if self.lines.is_empty() {
            String::new()
        } else if self.lines.len() == 1 {
            let old = std::mem::take(&mut self.lines[0]);
            self.col = 0;
            old
        } else {
            let removed = self.lines.remove(self.row);
            if self.row >= self.lines.len() {
                self.row = self.lines.len() - 1;
            }
            self.clamp_cursor();
            removed
        }
    }

    pub fn yank_line(&mut self) {
        if !self.lines.is_empty() && self.row < self.lines.len() {
            self.yank_buffer = self.lines[self.row].clone();
        }
    }

    pub fn paste_below(&mut self) {
        if self.yank_buffer.is_empty() && self.lines.is_empty() {
            return;
        }
        self.push_undo();
        if self.lines.is_empty() {
            self.lines.push(self.yank_buffer.clone());
            self.row = 0;
        } else {
            self.lines.insert(self.row + 1, self.yank_buffer.clone());
            self.row += 1;
        }
        self.col = 0;
        self.modified = true;
    }

    pub fn paste_above(&mut self) {
        if self.yank_buffer.is_empty() && self.lines.is_empty() {
            return;
        }
        self.push_undo();
        if self.lines.is_empty() {
            self.lines.push(self.yank_buffer.clone());
            self.row = 0;
        } else {
            self.lines.insert(self.row, self.yank_buffer.clone());
        }
        self.col = 0;
        self.modified = true;
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    fn boundary_cols(line: &str) -> Vec<usize> {
        let mut cols: Vec<usize> = line
            .grapheme_indices(true)
            .map(|(b, _)| line[..b].chars().count())
            .collect();
        cols.push(line.chars().count());
        cols
    }

    proptest! {
        #[test]
        fn clamp_snaps_to_grapheme_boundary(text in "\\PC{0,40}", col in 0usize..80) {
            let mut ed = Editor::default();
            ed.set_text(&text);
            ed.col = col;
            ed.clamp_cursor();
            let line = &ed.lines[0];
            prop_assert!(
                boundary_cols(line).contains(&ed.col),
                "col {} is not a grapheme boundary of {:?}", ed.col, line
            );
        }

        #[test]
        fn right_then_left_roundtrips(text in "\\PC{1,40}", steps in 0usize..8) {
            let mut ed = Editor::default();
            ed.set_text(&text);
            for _ in 0..steps {
                let before = ed.col;
                let len = ed.lines[0].chars().count();
                if before < len {
                    ed.move_right();
                    prop_assert!(ed.col > before);
                    ed.move_left();
                    prop_assert_eq!(ed.col, before);
                }
            }
        }

        #[test]
        fn backspace_removes_one_grapheme(text in "\\PC{1,40}") {
            let mut ed = Editor::default();
            ed.set_text(&text);
            let line = ed.lines[0].clone();
            let clusters: Vec<&str> = line.graphemes(true).collect();
            ed.col = line.chars().count();
            ed.backspace();
            prop_assert_eq!(&ed.lines[0], &clusters[..clusters.len() - 1].concat());
        }

        #[test]
        fn delete_removes_one_grapheme(text in "\\PC{1,40}") {
            let mut ed = Editor::default();
            ed.set_text(&text);
            let line = ed.lines[0].clone();
            let clusters: Vec<&str> = line.graphemes(true).collect();
            ed.col = 0;
            ed.delete();
            prop_assert_eq!(&ed.lines[0], &clusters[1..].concat());
        }
    }
}
