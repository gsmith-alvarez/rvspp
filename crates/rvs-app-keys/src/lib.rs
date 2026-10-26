//! Keyboard handling for the RVS++ TUI.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use rvs_app::{App, FileAction, Modal, Mode, Pane};
use rvs_app_cmds::AppCommands;
use rvs_machine::Machine;

pub trait AppKeys {
    fn on_key(&mut self, key: KeyEvent);
    fn on_normal_key_source(&mut self, key: KeyEvent);
    fn on_normal_key_other_pane(&mut self, key: KeyEvent, pane: Pane);
    fn editor_key(&mut self, key: KeyEvent);
    fn on_key_input(&mut self, key: KeyEvent);
    fn on_key_modal(&mut self, key: KeyEvent);
    fn on_key_search(&mut self, key: KeyEvent);
}

impl<M: Machine> AppKeys for App<M> {
    // ----- input events -------------------------------------------------

    fn on_key(&mut self, key: KeyEvent) {
        if self.modal != Modal::None {
            self.on_key_modal(key);
            return;
        }
        if self.mode == Mode::Command {
            self.on_key_command(key);
            return;
        }
        if self.mode == Mode::Search {
            self.on_key_search(key);
            return;
        }
        if self.mode == Mode::Input {
            self.on_key_input(key);
            return;
        }
        // Global shortcuts work in every mode.
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.quit_requested = true;
                return;
            }
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_open_action(FileAction::Load);
                return;
            }
            KeyCode::Char('s' | 'S') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.open_modal(FileAction::SaveAs);
                return;
            }
            KeyCode::Char('S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_modal(FileAction::SaveAs);
                return;
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if key.modifiers.contains(KeyModifiers::SHIFT) || self.fname.is_empty() {
                    self.open_modal(FileAction::SaveAs);
                } else {
                    let f = self.fname.clone();
                    self.save_file(&f);
                }
                return;
            }
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.request_quit();
                return;
            }
            KeyCode::BackTab => {
                self.cycle_focus_prev();
                return;
            }
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.cycle_focus_prev();
                return;
            }
            KeyCode::Tab => {
                self.cycle_focus();
                return;
            }
            _ => {}
        }

        if key.code == KeyCode::Esc {
            if self.zoomed_pane.is_some() {
                self.zoomed_pane = None;
            } else if self.mode == Mode::Running {
                self.action_stop();
            } else if self.mode == Mode::Edit {
                self.mode = Mode::Normal;
                // stays on Pane::Source!
            } else if self.pending_op.is_some() {
                self.pending_op = None;
            } else {
                self.request_quit();
            }
            return;
        }

        if self.mode == Mode::Edit && self.focus == Pane::Source {
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                if let KeyCode::Char(c) = key.code {
                    self.editor.insert_char(c);
                } else {
                    self.editor_key(key);
                }
            } else {
                self.editor_key(key);
            }
            return;
        }

        if self.mode == Mode::Normal || self.mode == Mode::Running {
            let is_plain_or_shift =
                key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;

            // Enter key: user interjection primary behavior
            if key.code == KeyCode::Enter {
                self.pending_op = None;
                self.trigger_zoom_or_editor();
                return;
            }

            // Normal mode ':' to enter command mode
            if is_plain_or_shift && key.code == KeyCode::Char(':') {
                self.pending_op = None;
                self.mode = Mode::Command;
                self.command_input.clear();
                return;
            }

            // Normal mode '/' to search the source buffer
            if is_plain_or_shift && key.code == KeyCode::Char('/') && self.focus == Pane::Source {
                self.begin_search();
                return;
            }

            // Normal mode '?' to open Help/About
            if is_plain_or_shift && key.code == KeyCode::Char('?') {
                self.pending_op = None;
                self.modal = Modal::About;
                return;
            }

            // Normal mode 'z' or 'Z' to toggle zoom
            if is_plain_or_shift
                && (key.code == KeyCode::Char('z') || key.code == KeyCode::Char('Z'))
            {
                self.pending_op = None;
                self.zoomed_pane = if self.zoomed_pane.is_some() {
                    None
                } else {
                    Some(self.focus)
                };
                return;
            }

            // Ctrl+D / Ctrl+U for half-page scroll across panes
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('d') => {
                        self.pending_op = None;
                        if self.focus == Pane::Source {
                            self.editor.page_down(10);
                        } else {
                            self.scroll_pane(self.focus, 10);
                        }
                        return;
                    }
                    KeyCode::Char('u') => {
                        self.pending_op = None;
                        if self.focus == Pane::Source {
                            self.editor.page_up(10);
                        } else {
                            self.scroll_pane(self.focus, -10);
                        }
                        return;
                    }
                    KeyCode::Char('r') if self.focus == Pane::Source => {
                        self.pending_op = None;
                        self.editor.redo();
                        return;
                    }
                    KeyCode::Char('n') => {
                        self.pending_op = None;
                        self.search_repeat(true);
                        return;
                    }
                    KeyCode::Char('p') => {
                        self.pending_op = None;
                        self.search_repeat(false);
                        return;
                    }
                    _ => {}
                }
            }

            // Global simulation single keys in Normal mode across panes
            if is_plain_or_shift {
                match key.code {
                    KeyCode::Char('c') => {
                        self.pending_op = None;
                        self.compile();
                        return;
                    }
                    KeyCode::Char('s') => {
                        self.pending_op = None;
                        self.action_start();
                        return;
                    }
                    KeyCode::Char('n') => {
                        self.pending_op = None;
                        self.action_next();
                        return;
                    }
                    KeyCode::Char('r') => {
                        self.pending_op = None;
                        self.action_run();
                        return;
                    }
                    KeyCode::Char('q') => {
                        self.pending_op = None;
                        self.request_quit();
                        return;
                    }
                    KeyCode::Char('p') if self.focus != Pane::Source => {
                        self.pending_op = None;
                        self.action_step_back();
                        return;
                    }
                    KeyCode::Char('P') | KeyCode::Char('N') => {
                        self.pending_op = None;
                        self.action_step_back();
                        return;
                    }
                    KeyCode::Char('+') | KeyCode::Char(']') => {
                        self.pending_op = None;
                        self.run_speed = self.run_speed.next();
                        self.status = format!("Run speed: {}", self.run_speed.label());
                        return;
                    }
                    KeyCode::Char('-') | KeyCode::Char('[') => {
                        self.pending_op = None;
                        self.run_speed = self.run_speed.prev();
                        self.status = format!("Run speed: {}", self.run_speed.label());
                        return;
                    }
                    KeyCode::Char('B') => {
                        self.pending_op = None;
                        if self.focus == Pane::Source {
                            let row = self.editor.row;
                            self.toggle_breakpoint_at_source_line(row);
                        } else {
                            self.toggle_breakpoint_at_pc();
                        }
                        return;
                    }
                    KeyCode::Char('x')
                        if self.focus != Pane::Source || self.mode == Mode::Running =>
                    {
                        self.pending_op = None;
                        self.action_stop();
                        return;
                    }
                    _ => {}
                }
            }
            // Pane-specific keys
            match self.focus {
                Pane::Source => {
                    self.on_normal_key_source(key);
                }
                Pane::Inp => {
                    if is_plain_or_shift
                        && (key.code == KeyCode::Char('i') || key.code == KeyCode::Char('a'))
                    {
                        self.mode = Mode::Input;
                    } else {
                        self.on_normal_key_other_pane(key, Pane::Inp);
                    }
                }
                other_pane => {
                    self.on_normal_key_other_pane(key, other_pane);
                }
            }
        }
    }

    fn on_normal_key_source(&mut self, key: KeyEvent) {
        let is_plain_or_shift = key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
        if is_plain_or_shift {
            // Counts: `12j`, `3w`, `4x`. `0` extends a count, otherwise it is
            // vim's "start of line".
            if let KeyCode::Char(c @ '1'..='9') = key.code {
                let digit = c.to_digit(10).unwrap_or(0) as usize;
                self.pending_count = Some(self.pending_count.unwrap_or(0) * 10 + digit);
                self.pending_op = None;
                return;
            }
            if key.code == KeyCode::Char('0') && self.pending_count.is_some() {
                let n = self.pending_count.unwrap_or(0);
                self.pending_count = Some(n * 10);
                self.pending_op = None;
                return;
            }
            let count = self.pending_count.take().unwrap_or(1).max(1);
            match key.code {
                KeyCode::Char('i') => {
                    self.pending_op = None;
                    self.mode = Mode::Edit;
                }
                KeyCode::Char('a') => {
                    self.pending_op = None;
                    let len = self.editor.lines[self.editor.row].chars().count();
                    if self.editor.col < len {
                        self.editor.col += 1;
                    }
                    self.mode = Mode::Edit;
                }
                KeyCode::Char('I') => {
                    self.pending_op = None;
                    self.editor.move_first_non_whitespace();
                    self.mode = Mode::Edit;
                }
                KeyCode::Char('A') => {
                    self.pending_op = None;
                    self.editor.move_end();
                    self.mode = Mode::Edit;
                }
                KeyCode::Char('B') => {
                    self.pending_op = None;
                    let row = self.editor.row;
                    self.toggle_breakpoint_at_source_line(row);
                }
                KeyCode::Char('o') => {
                    self.pending_op = None;
                    self.editor.insert_line_below();
                    self.mode = Mode::Edit;
                }
                KeyCode::Char('O') => {
                    self.pending_op = None;
                    self.editor.insert_line_above();
                    self.mode = Mode::Edit;
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    self.pending_op = None;
                    for _ in 0..count {
                        self.editor.move_left();
                    }
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    self.pending_op = None;
                    for _ in 0..count {
                        self.editor.move_right();
                    }
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.pending_op = None;
                    for _ in 0..count {
                        self.editor.move_down();
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.pending_op = None;
                    for _ in 0..count {
                        self.editor.move_up();
                    }
                }
                KeyCode::Char('0') | KeyCode::Home => {
                    self.pending_op = None;
                    self.editor.move_home();
                }
                KeyCode::Char('^') => {
                    self.pending_op = None;
                    self.editor.move_first_non_whitespace();
                }
                KeyCode::Char('$') | KeyCode::End => {
                    self.pending_op = None;
                    self.editor.move_end();
                }
                KeyCode::Char('w') => {
                    self.pending_op = None;
                    for _ in 0..count {
                        self.editor.move_word_forward();
                    }
                }
                KeyCode::Char('b') => {
                    self.pending_op = None;
                    for _ in 0..count {
                        self.editor.move_word_backward();
                    }
                }
                KeyCode::Char('G') => {
                    self.pending_op = None;
                    self.editor.row = self.editor.lines.len().saturating_sub(1);
                    self.editor.clamp_cursor();
                }
                KeyCode::Char('g') => {
                    if self.pending_op == Some('g') {
                        self.pending_op = None;
                        self.editor.row = 0;
                        self.editor.col = 0;
                        self.editor.offset_row = 0;
                    } else {
                        self.pending_op = Some('g');
                    }
                }
                KeyCode::Char('x') => {
                    self.pending_op = None;
                    for _ in 0..count {
                        self.editor.delete();
                    }
                }
                KeyCode::Char('d') => {
                    if self.pending_op == Some('d') {
                        self.pending_op = None;
                        self.editor.yank_buffer = self.editor.delete_line();
                    } else {
                        self.pending_op = Some('d');
                    }
                }
                KeyCode::Char('y') => {
                    if self.pending_op == Some('y') {
                        self.pending_op = None;
                        self.editor.yank_line();
                    } else {
                        self.pending_op = Some('y');
                    }
                }
                KeyCode::Char('Y') => {
                    self.pending_op = None;
                    self.editor.yank_line();
                }
                KeyCode::Char('p') => {
                    self.pending_op = None;
                    self.editor.paste_below();
                }
                KeyCode::Char('P') => {
                    self.pending_op = None;
                    self.editor.paste_above();
                }
                KeyCode::Char('u') => {
                    self.pending_op = None;
                    self.editor.undo();
                }
                KeyCode::PageUp => {
                    self.pending_op = None;
                    self.editor.page_up(20);
                }
                KeyCode::PageDown => {
                    self.pending_op = None;
                    self.editor.page_down(20);
                }
                _ => {
                    self.pending_op = None;
                }
            }
        } else {
            self.pending_op = None;
        }
    }

    fn on_normal_key_other_pane(&mut self, key: KeyEvent, pane: Pane) {
        let is_plain_or_shift = key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
        if is_plain_or_shift {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.pending_op = None;
                    self.scroll_pane(pane, 1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.pending_op = None;
                    self.scroll_pane(pane, -1);
                }
                KeyCode::PageDown => {
                    self.pending_op = None;
                    self.scroll_pane(pane, 10);
                }
                KeyCode::PageUp => {
                    self.pending_op = None;
                    self.scroll_pane(pane, -10);
                }
                KeyCode::Char('g') => {
                    if self.pending_op == Some('g') {
                        self.pending_op = None;
                        self.scroll_pane_to_top(pane);
                    } else {
                        self.pending_op = Some('g');
                    }
                }
                KeyCode::Home => {
                    self.pending_op = None;
                    self.scroll_pane_to_top(pane);
                }
                KeyCode::Char('G') | KeyCode::End => {
                    self.pending_op = None;
                    self.scroll_pane_to_bottom(pane);
                }
                KeyCode::Char('1') => {
                    self.pending_op = None;
                    self.toggle_pane_option(pane, 1);
                }
                KeyCode::Char('2') => {
                    self.pending_op = None;
                    self.toggle_pane_option(pane, 2);
                }
                KeyCode::Char('3') => {
                    self.pending_op = None;
                    self.toggle_pane_option(pane, 3);
                }
                KeyCode::Char('4') => {
                    self.pending_op = None;
                    self.toggle_pane_option(pane, 4);
                }
                KeyCode::Char('d') => {
                    self.pending_op = None;
                    self.toggle_pane_option(pane, 5);
                }
                KeyCode::Char('b') if pane == Pane::Listing => {
                    self.pending_op = None;
                    self.toggle_breakpoint_at_pc();
                }
                KeyCode::Char('s') | KeyCode::Char('S') if pane == Pane::Mem => {
                    self.pending_op = None;
                    self.jump_mem_to_sp();
                }
                KeyCode::Char('0') if pane == Pane::Mem => {
                    self.pending_op = None;
                    self.jump_mem_to_address(0);
                }
                KeyCode::Char('i') | KeyCode::Char('0') if pane == Pane::Listing => {
                    self.pending_op = None;
                    self.reset_pc();
                }
                _ => {
                    self.pending_op = None;
                }
            }
        } else {
            self.pending_op = None;
        }
    }

    fn editor_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Backspace | KeyCode::Char('w' | 'W' | '\u{8}') => {
                    self.editor.delete_word_backward();
                    return;
                }
                KeyCode::Char('z') => {
                    self.editor.undo();
                    return;
                }
                KeyCode::Char('y') => {
                    self.editor.redo();
                    return;
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Enter => self.editor.newline(),
            KeyCode::Backspace => self.editor.backspace(),
            KeyCode::Delete => self.editor.delete(),
            KeyCode::Left => self.editor.move_left(),
            KeyCode::Right => self.editor.move_right(),
            KeyCode::Up => self.editor.move_up(),
            KeyCode::Down => self.editor.move_down(),
            KeyCode::Home => self.editor.move_home(),
            KeyCode::End => self.editor.move_end(),
            KeyCode::PageUp => self.editor.page_up(20),
            KeyCode::PageDown => self.editor.page_down(20),
            _ => {}
        }
    }

    fn on_key_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.submit_input(),
            KeyCode::Esc => self.cancel_input(),
            KeyCode::Backspace => {
                self.input_line.pop();
            }
            KeyCode::Char(c) => self.input_line.push(c),
            _ => {}
        }
    }

    fn on_key_modal(&mut self, key: KeyEvent) {
        match (&self.modal, key.code) {
            (
                Modal::About | Modal::Stats | Modal::CallStack,
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q'),
            ) => {
                self.modal = Modal::None;
                self.needs_full_clear = true;
            }
            (Modal::ConfirmSave(_), _) => {
                let is_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                let is_alt = key.modifiers.contains(KeyModifiers::ALT);
                if is_ctrl && matches!(key.code, KeyCode::Char('d' | 'D')) {
                    self.modal_discard();
                } else {
                    match key.code {
                        KeyCode::Esc => {
                            self.modal = Modal::None;
                            self.needs_full_clear = true;
                        }
                        KeyCode::Enter => self.modal_save_and_proceed(),
                        KeyCode::Backspace => {
                            self.modal_input.pop();
                        }
                        KeyCode::Char(c) if !is_ctrl && !is_alt => {
                            self.modal_input.push(c);
                        }
                        _ => {}
                    }
                }
            }
            (Modal::FilePrompt(_), _) => match self.explorer.mode {
                rvs_explorer::ExplorerMode::Normal => {
                    let is_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                    let is_plain_or_shift =
                        key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
                    if is_ctrl {
                        match key.code {
                            KeyCode::Char('d') => self.explorer.page_down(10),
                            KeyCode::Char('u') => self.explorer.page_up(10),
                            _ => {}
                        }
                    } else if is_plain_or_shift {
                        match key.code {
                            KeyCode::Char('j') | KeyCode::Down => {
                                self.explorer.select_next();
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                self.explorer.select_prev();
                            }
                            KeyCode::Char('h') | KeyCode::Left => {
                                if let Some(p) = self.explorer.parent_dir_input() {
                                    self.modal_input = p;
                                    self.explorer.update(&self.modal_input);
                                }
                            }
                            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                                self.explorer_open_selected();
                            }
                            KeyCode::Char('g') | KeyCode::Home => {
                                self.explorer.select_first();
                            }
                            KeyCode::Char('G') | KeyCode::End => {
                                self.explorer.select_last();
                            }
                            KeyCode::PageDown => self.explorer.page_down(10),
                            KeyCode::PageUp => self.explorer.page_up(10),
                            KeyCode::Char('J') => self.explorer.scroll_preview_down(3),
                            KeyCode::Char('K') => self.explorer.scroll_preview_up(3),
                            KeyCode::Char('/') | KeyCode::Char('i') | KeyCode::Char('a') => {
                                self.explorer.mode = rvs_explorer::ExplorerMode::Filter;
                            }
                            KeyCode::Char('q') | KeyCode::Esc => {
                                self.modal = Modal::None;
                                self.needs_full_clear = true;
                            }
                            KeyCode::Tab => {
                                if let Some(comp) = self.explorer.complete() {
                                    self.modal_input = comp;
                                    self.explorer.update(&self.modal_input);
                                }
                            }
                            KeyCode::BackTab => {
                                self.explorer.select_prev();
                            }
                            _ => {}
                        }
                    }
                }
                rvs_explorer::ExplorerMode::Filter => match key.code {
                    KeyCode::Esc => {
                        self.explorer.mode = rvs_explorer::ExplorerMode::Normal;
                    }
                    KeyCode::Up => self.explorer.select_prev(),
                    KeyCode::Down => self.explorer.select_next(),
                    KeyCode::PageUp => self.explorer.page_up(10),
                    KeyCode::PageDown => self.explorer.page_down(10),
                    KeyCode::Tab => {
                        if let Some(comp) = self.explorer.complete() {
                            self.modal_input = comp;
                            self.explorer.update(&self.modal_input);
                        }
                    }
                    KeyCode::BackTab => self.explorer.select_prev(),
                    KeyCode::Enter => {
                        self.explorer_open_selected();
                    }
                    KeyCode::Backspace => {
                        self.modal_input.pop();
                        self.explorer.update(&self.modal_input);
                    }
                    KeyCode::Char(c) => {
                        self.modal_input.push(c);
                        self.explorer.update(&self.modal_input);
                    }
                    _ => {}
                },
            },
            _ => {}
        }
    }

    /// The `/` prompt: type a pattern, Enter searches, Esc cancels.
    fn on_key_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_input.clear();
            }
            KeyCode::Enter => self.commit_search(),
            KeyCode::Backspace => {
                self.search_input.pop();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search_input.push(c);
            }
            _ => {}
        }
    }
}
