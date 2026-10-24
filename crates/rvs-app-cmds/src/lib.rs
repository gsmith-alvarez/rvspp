//! Colon-command execution and pane actions for the RVS++ TUI.

use std::path::PathBuf;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use rvs_app::{App, ConfirmAction, FileAction, Modal, Mode, Pane, RunSpeed};
use rvs_machine::{Dialect, Machine};

pub trait AppCommands {
    fn on_key_command(&mut self, key: KeyEvent);
    fn execute_command(&mut self);
    fn trigger_zoom_or_editor(&mut self);
    fn cycle_focus(&mut self);
    fn cycle_focus_prev(&mut self);
    fn scroll_pane_to_top(&mut self, pane: Pane);
    fn scroll_pane_to_bottom(&mut self, pane: Pane);
    fn scroll_pane(&mut self, pane: Pane, delta: i32);
    fn toggle_pane_option(&mut self, pane: Pane, slot: u8);
    fn clear_pane(&mut self, pane: Pane);
    fn explorer_open_selected(&mut self);
    fn run_external_editor(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

/// Create a private temp buffer whose path ends in `.s`, so editors still
/// detect assembly syntax. The file is unlinked when the returned handle drops.
fn write_temp_buffer(text: &str) -> std::io::Result<(PathBuf, tempfile::NamedTempFile)> {
    use std::io::Write as _;

    let mut file = tempfile::Builder::new()
        .prefix("rvs_buf_")
        .suffix(".s")
        .tempfile_in(std::env::temp_dir())?;
    file.write_all(text.as_bytes())?;
    Ok((file.path().to_path_buf(), file))
}

/// Split an `$EDITOR`/`$VISUAL` command line into program + arguments.
fn split_command(spec: &str) -> Vec<String> {
    match shell_words::split(spec) {
        Ok(argv) if !argv.is_empty() => argv,
        _ => vec!["nvim".to_string()],
    }
}

impl<M: Machine> AppCommands for App<M> {
    fn on_key_command(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.command_input.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                self.execute_command();
                if self.mode == Mode::Command {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Backspace => {
                if self.command_input.is_empty() {
                    self.mode = Mode::Normal;
                } else {
                    self.command_input.pop();
                }
            }
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.command_input.push(c);
            }
            _ => {}
        }
    }

    fn execute_command(&mut self) {
        // Own the text: some arms mutate `self` (and therefore `command_input`).
        let cmd_owned = self.command_input.trim().to_string();
        let cmd = cmd_owned.strip_prefix(':').unwrap_or(&cmd_owned).trim();
        if cmd.is_empty() {
            return;
        }
        let (name, args) = match cmd.split_once(char::is_whitespace) {
            Some((n, a)) => (n, a.trim()),
            None => (cmd, ""),
        };

        match name {
            "q" | "quit" => self.request_quit(),
            "q!" | "quit!" => self.quit_requested = true,
            "w" | "write" => {
                if args.is_empty() {
                    if self.fname.is_empty() {
                        self.open_modal(FileAction::SaveAs);
                    } else {
                        let path = self.fname.clone();
                        self.save_file(&path);
                    }
                } else {
                    let path = rvs_explorer::expand_tilde(args);
                    self.save_file(&path);
                }
            }
            "wq" | "x" => {
                let target = if !args.is_empty() {
                    rvs_explorer::expand_tilde(args)
                } else {
                    self.fname.clone()
                };
                if target.is_empty() {
                    self.open_modal(FileAction::SaveAs);
                } else {
                    self.save_file(&target);
                    self.quit_requested = true;
                }
            }
            "e" | "load" | "open" => {
                if args.is_empty() {
                    self.handle_open_action(FileAction::Load);
                } else {
                    let resolved = rvs_explorer::expand_tilde(args);
                    if self.editor.needs_save() {
                        self.modal = Modal::ConfirmSave(ConfirmAction::Load);
                        self.modal_input = resolved;
                    } else {
                        self.load_file(&resolved, false);
                    }
                }
            }
            "help" | "h" | "about" => {
                self.modal = Modal::About;
            }
            "c" | "compile" => self.compile(),
            "s" | "start" => self.action_start(),
            "n" | "step" => {
                let count = args.parse::<usize>().unwrap_or(1).max(1);
                for _ in 0..count {
                    self.action_next();
                    if !self.running && self.mode != Mode::Running {
                        break;
                    }
                }
            }
            // `:next` steps *over* calls; `:n` keeps its "step N" meaning.
            "next" | "over" | "stepover" => self.step_over(),
            "finish" | "out" | "stepout" | "return" => self.step_out(),
            "search" | "find" | "grep" => {
                if args.is_empty() {
                    self.status = "Usage: :search <pattern>".to_string();
                } else {
                    self.search_query = args.to_string();
                    self.search_repeat(true);
                }
            }
            "watch" => match self.resolve_data_target(args) {
                Some(addr) => self.add_watchpoint(addr),
                None => self.status = format!("ERROR: no such address or label \"{args}\""),
            },
            "unwatch" => match self.resolve_data_target(args) {
                Some(addr) if self.remove_watchpoint(addr) => {
                    self.status = format!("Removed watchpoint 0x{addr:016x}")
                }
                Some(addr) => self.status = format!("No watchpoint at 0x{addr:016x}"),
                None => self.status = format!("ERROR: no such address or label \"{args}\""),
            },
            "cwatch" | "clearwatch" | "clearwatchpoints" => self.clear_watchpoints(),
            "abi" => self.set_abi(args),
            "r" | "run" => self.action_run(),
            "stop" | "halt" => self.action_stop(),
            "reset" | "initpc" => self.reset_pc(),
            "clear" => {
                self.machine.clear_logs();
                self.scroll_exec = 0;
                self.scroll_out = 0;
                self.needs_full_clear = true;
            }
            "edit" | "nvim" | "vim" => {
                self.pending_external_editor = true;
            }
            "zoom" | "z" => {
                self.zoomed_pane = if self.zoomed_pane == Some(self.focus) {
                    None
                } else {
                    Some(self.focus)
                };
            }
            "color" | "colour" | "hl" | "highlight" => {
                if args == "on" {
                    self.colorize_listing = true;
                } else if args == "off" {
                    self.colorize_listing = false;
                } else {
                    self.colorize_listing = !self.colorize_listing;
                }
            }
            "b" | "break" | "breakpoint" => {
                // `:b <target> if <expr>` sets a conditional breakpoint; a
                // bare `:b` keeps its toggle behaviour.
                let (target, condition) = match args.split_once(" if ") {
                    Some((t, c)) => (t.trim().to_string(), Some(c.trim().to_string())),
                    None => (args.to_string(), None),
                };
                if target.is_empty() && condition.is_none() {
                    if self.focus == Pane::Source {
                        let row = self.editor.row;
                        self.toggle_breakpoint_at_source_line(row);
                    } else {
                        self.toggle_breakpoint_at_pc();
                    }
                } else {
                    let resolved = if target.is_empty() {
                        Some(self.machine.pc())
                    } else {
                        self.resolve_code_target(&target)
                    };
                    match resolved {
                        Some(addr) => {
                            if condition.is_some() {
                                self.add_breakpoint(addr);
                            } else {
                                self.toggle_breakpoint(addr);
                            }
                            if let Some(expr) = condition {
                                self.set_break_condition(addr, Some(expr));
                            }
                        }
                        None => {
                            self.status = format!(
                                "ERROR: Invalid address, line number, or label \"{target}\""
                            )
                        }
                    }
                }
            }
            "cb" | "clearbreaks" => {
                self.clear_all_breakpoints();
            }
            "stats" | "stat" | "metrics" => {
                self.modal = Modal::Stats;
                self.needs_full_clear = true;
            }
            "cc" | "convention" => {
                if args == "on" {
                    self.machine.set_convention_enabled(true);
                } else if args == "off" {
                    self.machine.set_convention_enabled(false);
                } else {
                    let next = !self.machine.convention_enabled();
                    self.machine.set_convention_enabled(next);
                }
                self.status = format!(
                    "Calling convention checker: {}",
                    if self.machine.convention_enabled() {
                        "ON"
                    } else {
                        "OFF"
                    }
                );
                self.needs_full_clear = true;
            }
            "p" | "prev" | "back" | "stepback" => {
                self.action_step_back();
            }
            "speed" | "spd" => {
                match args.to_lowercase().as_str() {
                    "1" | "1hz" => self.run_speed = RunSpeed::Hz1,
                    "5" | "5hz" => self.run_speed = RunSpeed::Hz5,
                    "20" | "20hz" => self.run_speed = RunSpeed::Hz20,
                    "max" | "fast" => self.run_speed = RunSpeed::Max,
                    "" => self.run_speed = self.run_speed.next(),
                    _ => {
                        self.status =
                            format!("ERROR: Expected speed 1, 5, 20, or max; got \"{args}\"")
                    }
                }
                self.status = format!("Run speed: {}", self.run_speed.label());
            }
            "mem" | "memory" => {
                if args == "sp" || args == "stack" {
                    self.jump_mem_to_sp();
                } else if let Some(addr) = self.machine.symbol_address(args) {
                    self.jump_mem_to_address(addr);
                } else if let Some(addr) = rvs_machine::parse_u64(args) {
                    self.jump_mem_to_address(addr);
                } else if args.is_empty() {
                    self.jump_mem_to_address(0);
                } else {
                    self.status = format!("ERROR: Invalid memory address or symbol \"{args}\"");
                }
            }
            "mode" | "dialect" => {
                match args.to_lowercase().as_str() {
                    "gnu" => self.machine.set_dialect(Dialect::Gnu),
                    "rvs" | "dialect" => self.machine.set_dialect(Dialect::Rvs),
                    "" => self.machine.set_dialect(self.machine.dialect().toggle()),
                    _ => {
                        self.status = format!("ERROR: Expected mode 'gnu' or 'rvs', got \"{args}\"")
                    }
                }
                self.status = format!("Assembly dialect: {}", self.machine.dialect().name());
                self.needs_full_clear = true;
            }
            "transpile" | "tp" => {
                let target = args.to_lowercase();
                let src = self.editor.text();
                let is_gnu_target = target == "gnu"
                    || (target.is_empty() && self.machine.dialect() == Dialect::Rvs);
                let target = if is_gnu_target {
                    Dialect::Gnu
                } else {
                    Dialect::Rvs
                };
                if let Some(transpiled) = self.machine.transpile(&src, target) {
                    self.editor.replace_text(&transpiled);
                }
                self.status = format!("Transpiled buffer to {} format", target.name());
                self.needs_full_clear = true;
            }
            "stack" | "callstack" | "frames" => {
                self.modal = Modal::CallStack;
                self.needs_full_clear = true;
            }
            other => {
                self.status = format!("ERROR: Unknown command :{other}");
            }
        }
    }

    fn trigger_zoom_or_editor(&mut self) {
        match self.focus {
            Pane::Source => {
                self.pending_external_editor = true;
            }
            Pane::Inp => {
                self.mode = Mode::Input;
            }
            other => {
                self.zoomed_pane = if self.zoomed_pane == Some(other) {
                    None
                } else {
                    Some(other)
                };
            }
        }
    }

    fn cycle_focus(&mut self) {
        self.focus = self.focus.next();
        if self.mode != Mode::Input && self.mode != Mode::Running {
            self.mode = Mode::Normal;
        }
        if self.zoomed_pane.is_some() {
            self.zoomed_pane = Some(self.focus);
        }
    }

    fn cycle_focus_prev(&mut self) {
        self.focus = self.focus.prev();
        if self.mode != Mode::Input && self.mode != Mode::Running {
            self.mode = Mode::Normal;
        }
        if self.zoomed_pane.is_some() {
            self.zoomed_pane = Some(self.focus);
        }
    }

    fn scroll_pane_to_top(&mut self, pane: Pane) {
        match pane {
            Pane::Source => {
                self.editor.offset_row = 0;
                self.editor.row = 0;
            }
            Pane::Listing => self.scroll_listing = 0,
            Pane::Regs => self.scroll_regs = 0,
            Pane::Mem => self.scroll_mem = 0,
            Pane::Exec => self.scroll_exec = 0,
            Pane::Out => self.scroll_out = 0,
            Pane::Inp => {}
        }
    }

    fn scroll_pane_to_bottom(&mut self, pane: Pane) {
        match pane {
            Pane::Source => {
                self.editor.row = self.editor.lines.len().saturating_sub(1);
                self.editor.clamp_cursor();
            }
            Pane::Listing => {
                self.scroll_listing = (self.machine.listing_len() + 4).saturating_sub(1)
            }
            Pane::Regs => self.scroll_regs = 64,
            Pane::Mem => self.scroll_mem = self.machine.memory_len() + 2,
            Pane::Exec => self.scroll_exec = self.machine.exec_len().saturating_sub(1),
            Pane::Out => self.scroll_out = self.machine.output_len().saturating_sub(1),
            Pane::Inp => {}
        }
    }

    // ----- mouse ---------------------------------------------------------

    fn scroll_pane(&mut self, pane: Pane, delta: i32) {
        let (len, off) = match pane {
            Pane::Source => (self.editor.lines.len(), &mut self.editor.offset_row),
            Pane::Listing => (self.machine.listing_len() + 4, &mut self.scroll_listing),
            Pane::Regs => (64, &mut self.scroll_regs),
            Pane::Mem => (self.machine.memory_len() + 2, &mut self.scroll_mem),
            Pane::Exec => (self.machine.exec_len(), &mut self.scroll_exec),
            Pane::Out => (self.machine.output_len(), &mut self.scroll_out),
            Pane::Inp => return,
        };
        if delta < 0 {
            *off = off.saturating_sub((-delta) as usize);
        } else {
            *off = off
                .saturating_add(delta as usize)
                .min(len.saturating_sub(1));
        }
    }
    fn toggle_pane_option(&mut self, pane: Pane, slot: u8) {
        match (pane, slot) {
            (Pane::Listing, 1) => self.view.exec_txt = !self.view.exec_txt,
            (Pane::Listing, 2) => self.colorize_listing = !self.colorize_listing,
            (Pane::Listing, 3) => {
                self.run_speed = self.run_speed.next();
                self.status = format!("Run speed: {}", self.run_speed.label());
            }
            (Pane::Listing, 4) => self.toggle_breakpoint_at_pc(),
            (Pane::Regs, 1) => self.view.reg_hex = !self.view.reg_hex,
            (Pane::Regs, 2) => self.view.reg_int = !self.view.reg_int,
            (Pane::Regs, 3) => self.view.reg_flp = !self.view.reg_flp,
            (Pane::Mem, 1) => self.view.mem_hex = !self.view.mem_hex,
            (Pane::Mem, 2) => self.view.mem_int = !self.view.mem_int,
            (Pane::Mem, 3) => self.view.mem_flp = !self.view.mem_flp,
            (Pane::Mem, 4) => self.view.mem_txt = !self.view.mem_txt,
            (Pane::Mem, 5) => self.jump_mem_to_sp(),
            (Pane::Mem, 6) => self.jump_mem_to_address(0),
            (Pane::Mem, 7) => {
                self.modal = Modal::CallStack;
                self.needs_full_clear = true;
            }
            (Pane::Out, 1 | 5) => {
                if self.dma_out_available {
                    self.view.dma_out = !self.view.dma_out;
                } else {
                    self.status =
                        "DMA OUT requires OUTC (0x0) and OUTD (0x8) labels in source".to_string();
                }
            }
            (Pane::Inp, 1 | 5) => {
                if self.dma_inp_available {
                    self.view.dma_inp = !self.view.dma_inp;
                } else {
                    self.status =
                        "DMA INP requires INPC (0x10) and INPD (0x18) labels in source".to_string();
                }
            }
            (Pane::Exec, 1) => {
                self.modal = Modal::Stats;
                self.needs_full_clear = true;
            }
            (Pane::Exec, 2) => {
                let next = !self.machine.convention_enabled();
                self.machine.set_convention_enabled(next);
                self.status = format!(
                    "Calling convention checker: {}",
                    if next { "ON" } else { "OFF" }
                );
                self.needs_full_clear = true;
            }
            (Pane::Exec, 3) => {
                self.modal = Modal::CallStack;
                self.needs_full_clear = true;
            }
            (Pane::Source, 1) => {
                self.machine.set_dialect(Dialect::Rvs);
                self.status = "Assembly dialect: RVS".to_string();
                self.needs_full_clear = true;
            }
            (Pane::Source, 2) => {
                self.machine.set_dialect(Dialect::Gnu);
                self.status = "Assembly dialect: GNU".to_string();
                self.needs_full_clear = true;
            }
            (Pane::Source, 3) => {
                let is_cur_rvs = self.machine.dialect() == Dialect::Rvs;
                let src = self.editor.text();
                let target = if is_cur_rvs {
                    Dialect::Gnu
                } else {
                    Dialect::Rvs
                };
                if let Some(transpiled) = self.machine.transpile(&src, target) {
                    self.editor.replace_text(&transpiled);
                }
                self.machine.set_dialect(target);
                self.status = format!("Transpiled buffer to {} and switched mode", target.name());
                self.needs_full_clear = true;
            }
            _ => {}
        }
    }

    fn clear_pane(&mut self, pane: Pane) {
        match pane {
            Pane::Exec => {
                self.machine.clear_exec_log();
                self.scroll_exec = 0;
                self.needs_full_clear = true;
            }
            Pane::Out => {
                self.machine.clear_output();
                self.scroll_out = 0;
                self.needs_full_clear = true;
            }
            Pane::Inp => {
                self.action_stop();
            }
            _ => {}
        }
    }

    fn explorer_open_selected(&mut self) {
        if self.explorer.selected_is_dir() {
            if let Some(item) = self.explorer.selected_item() {
                self.modal_input = item.path_display.clone();
                self.explorer.update(&self.modal_input);
            }
        } else {
            if let Some(item) = self.explorer.selected_item() {
                if !self.modal_input.ends_with(&item.name) {
                    self.modal_input = item.path_display.clone();
                }
            }
            self.modal_accept();
        }
    }

    fn run_external_editor(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let editor_cmd = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "nvim".to_string());

        let (target_path, _temp) = if self.fname.is_empty() {
            match write_temp_buffer(&self.editor.text()) {
                Ok((path, file)) => (path, Some(file)),
                Err(e) => {
                    self.status = format!("ERROR creating temp buffer: {e}");
                    return Ok(());
                }
            }
        } else {
            let p = PathBuf::from(rvs_explorer::expand_tilde(&self.fname));
            if self.editor.needs_save() {
                let _ = std::fs::write(&p, self.editor.text());
            }
            (p, None)
        };

        let _ = crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = crossterm::terminal::disable_raw_mode();

        let argv = split_command(&editor_cmd);
        let mut child = std::process::Command::new(&argv[0])
            .args(&argv[1..])
            .arg(&target_path)
            .status();

        if child.is_err() && argv[0] == "nvim" {
            child = std::process::Command::new("vim")
                .arg(&target_path)
                .status()
                .or_else(|_| std::process::Command::new("vi").arg(&target_path).status());
        }

        let _ = crossterm::terminal::enable_raw_mode();
        let _ = crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::EnterAlternateScreen,
            EnableMouseCapture
        );
        let _ = terminal.clear();

        match child {
            Ok(status) if status.success() => {
                if let Ok(new_text) = std::fs::read_to_string(&target_path) {
                    let changed = new_text != self.editor.text();
                    if changed {
                        self.editor.set_text(&new_text);
                        self.editor.modified = true;
                        self.reset_sim_panes();
                        self.status = "Reloaded buffer from external editor".to_string();
                    }
                }
            }
            Ok(status) => {
                self.status = format!("Editor {} exited with {status}", argv[0]);
            }
            Err(e) => {
                self.status = format!("ERROR spawning editor {editor_cmd}: {e}");
            }
        }

        // `_temp` unlinks the temp buffer when this function returns.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::split_command;

    #[test]
    fn editor_command_lines_split_like_a_shell() {
        assert_eq!(split_command("nvim"), vec!["nvim"]);
        assert_eq!(
            split_command("code --wait"),
            vec!["code".to_string(), "--wait".to_string()]
        );
        assert_eq!(
            split_command("nvim -u \"my config.vim\""),
            vec![
                "nvim".to_string(),
                "-u".to_string(),
                "my config.vim".to_string()
            ]
        );
    }

    #[test]
    fn empty_or_broken_command_falls_back_to_nvim() {
        assert_eq!(split_command(""), vec!["nvim"]);
        assert_eq!(split_command("   "), vec!["nvim"]);
        assert_eq!(split_command("\"unterminated"), vec!["nvim"]);
    }

    #[test]
    fn temp_buffer_is_private_suffixed_and_cleaned_up() {
        let text = "addi x1, x0, 1\n";
        let (path, file) = super::write_temp_buffer(text).unwrap();

        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("s"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "temp buffer must not be world-readable");
        }

        drop(file);
        assert!(!path.exists(), "temp buffer must be removed on drop");
    }
}
