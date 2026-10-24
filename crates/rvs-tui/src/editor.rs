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
        if text.is_empty() {
            self.lines = vec![String::new()];
        } else if text.ends_with('\n') {
            self.lines.push(String::new());
        }
        self.row = 0;
        self.col = 0;
        self.offset_row = 0;
        self.offset_col = 0;
        self.undo.clear();
        self.redo.clear();
    }

    pub fn text(&self) -> String {
        let mut s = self.lines.join("\n");
        if !self.lines.is_empty() {
            s.push('\n');
        }
        s
    }

    pub fn needs_save(&self) -> bool {
        self.modified && self.lines.iter().any(|l| !l.trim_matches(['\n', '\r']).is_empty())
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
        let len = self.lines[self.row].chars().count();
        self.col = self.col.min(len);
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
            self.push_undo();
            let line = &mut self.lines[self.row];
            let byte = line
                .char_indices()
                .nth(self.col)
                .map(|(b, _)| b)
                .unwrap_or(line.len());
            let prev = line[..byte]
                .char_indices()
                .last()
                .map(|(b, _)| b)
                .unwrap_or(0);
            line.replace_range(prev..byte, "");
            self.col -= 1;
        } else if self.row > 0 {
            self.push_undo();
            let cur = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.lines[self.row].chars().count();
            self.lines[self.row].push_str(&cur);
        } else {
            return;
        }
        self.modified = true;
    }

    pub fn delete(&mut self) {
        let line = &self.lines[self.row];
        let byte = line
            .char_indices()
            .nth(self.col)
            .map(|(b, _)| b)
            .unwrap_or(line.len());
        if byte < line.len() {
            let next = line[byte..]
                .char_indices()
                .nth(1)
                .map(|(b, _)| byte + b)
                .unwrap_or(line.len());
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
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.lines[self.row].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        let len = self.lines[self.row].chars().count();
        if self.col < len {
            self.col += 1;
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
        if col >= chars.len() && self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        } else {
            self.col = col.min(chars.len());
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
        self.col = col;
    }

    pub fn move_first_non_whitespace(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        let line = &self.lines[self.row];
        self.col = line.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
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
