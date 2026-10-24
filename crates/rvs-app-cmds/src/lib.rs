//! Colon-command execution and pane actions for the RVS++ TUI.

use std::path::PathBuf;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use rvs_app::{App, ConfirmAction, FileAction, Modal, Mode, Pane, RunSpeed};
use rvs_tcl as tcl;

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

impl AppCommands for App {
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
            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                self.command_input.push(c);
            }
            _ => {}
        }
    }

    fn execute_command(&mut self) {
        let cmd = self.command_input.trim();
        let cmd = cmd.strip_prefix(':').unwrap_or(cmd).trim();
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
            "n" | "next" | "step" => {
                let count = args.parse::<usize>().unwrap_or(1).max(1);
                for _ in 0..count {
                    self.action_next();
                    if !self.running && self.mode != Mode::Running {
                        break;
                    }
                }
            }
            "r" | "run" => self.action_run(),
            "stop" | "halt" => self.action_stop(),
            "reset" | "initpc" => self.reset_pc(),
            "clear" => {
                self.sim.exec_log.clear();
                self.sim.output.clear();
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
                if args.is_empty() {
                    if self.focus == Pane::Source {
                        let row = self.editor.row;
                        self.toggle_breakpoint_at_source_line(row);
                    } else {
                        self.toggle_breakpoint_at_pc();
                    }
                } else if args.eq_ignore_ascii_case("pc") {
                    self.toggle_breakpoint_at_pc();
                } else if let Ok(line_num) = args.parse::<usize>() {
                    let line_idx = line_num.saturating_sub(1);
                    self.toggle_breakpoint_at_source_line(line_idx);
                } else if let Some(addr_str) = self.sim.sym.get(args) {
                    let addr = tcl::parse_int(addr_str).unwrap_or(0) as u64;
                    self.toggle_breakpoint(addr);
                } else if let Some(addr) = tcl::parse_int(args) {
                    self.toggle_breakpoint(addr as u64);
                } else {
                    self.status = format!("ERROR: Invalid address, line number, or label \"{args}\"");
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
                    self.sim.convention.enabled = true;
                } else if args == "off" {
                    self.sim.convention.enabled = false;
                } else {
                    self.sim.convention.enabled = !self.sim.convention.enabled;
                }
                self.status = format!(
                    "Calling convention checker: {}",
                    if self.sim.convention.enabled { "ON" } else { "OFF" }
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
                    _ => self.status = format!("ERROR: Expected speed 1, 5, 20, or max; got \"{args}\""),
                }
                self.status = format!("Run speed: {}", self.run_speed.label());
            }
            "mem" | "memory" => {
                if args == "sp" || args == "stack" {
                    self.jump_mem_to_sp();
                } else if let Some(addr_str) = self.sim.sym.get(args) {
                    let addr = tcl::parse_int(addr_str).unwrap_or(0) as u64;
                    self.jump_mem_to_address(addr);
                } else if let Some(addr) = tcl::parse_int(args) {
                    self.jump_mem_to_address(addr as u64);
                } else if args.is_empty() {
                    self.jump_mem_to_address(0);
                } else {
                    self.status = format!("ERROR: Invalid memory address or symbol \"{args}\"");
                }
            }
            "mode" | "dialect" => {
                match args.to_lowercase().as_str() {
                    "gnu" => self.sim.dialect = rvs_transpile::AssemblyDialect::Gnu,
                    "rvs" | "dialect" => self.sim.dialect = rvs_transpile::AssemblyDialect::Rvs,
                    "" => self.sim.dialect = self.sim.dialect.toggle(),
                    _ => self.status = format!("ERROR: Expected mode 'gnu' or 'rvs', got \"{args}\""),
                }
                self.status = format!("Assembly dialect: {}", self.sim.dialect.name());
                self.needs_full_clear = true;
            }
            "transpile" | "tp" => {
                let target = args.to_lowercase();
                let src = self.editor.text();
                let is_gnu_target = target == "gnu" || (target.is_empty() && self.sim.dialect == rvs_transpile::AssemblyDialect::Rvs);
                let transpiled = if is_gnu_target {
                    rvs_transpile::transpile_rvs_to_gnu(&src)
                } else {
                    rvs_transpile::transpile_gnu_to_rvs(&src)
                };
                self.editor.replace_text(&transpiled);
                self.status = format!("Transpiled buffer to {} format", if is_gnu_target { "GNU" } else { "RVS" });
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
            Pane::Listing => self.scroll_listing = (self.sim.listing.lines().count() + 4).saturating_sub(1),
            Pane::Regs => self.scroll_regs = 64,
            Pane::Mem => self.scroll_mem = self.sim.mem.cells.len() + 2,
            Pane::Exec => self.scroll_exec = self.sim.exec_log.lines().count().saturating_sub(1),
            Pane::Out => self.scroll_out = self.sim.output.lines().count().saturating_sub(1),
            Pane::Inp => {}
        }
    }

    // ----- mouse ---------------------------------------------------------

    fn scroll_pane(&mut self, pane: Pane, delta: i32) {
        let (len, off) = match pane {
            Pane::Source => (self.editor.lines.len(), &mut self.editor.offset_row),
            Pane::Listing => (self.sim.listing.lines().count() + 4, &mut self.scroll_listing),
            Pane::Regs => (64, &mut self.scroll_regs),
            Pane::Mem => (self.sim.mem.cells.len() + 2, &mut self.scroll_mem),
            Pane::Exec => (self.sim.exec_log.lines().count(), &mut self.scroll_exec),
            Pane::Out => (self.sim.output.lines().count(), &mut self.scroll_out),
            Pane::Inp => return,
        };
        if delta < 0 {
            *off = off.saturating_sub((-delta) as usize);
        } else {
            *off = off.saturating_add(delta as usize).min(len.saturating_sub(1));
        }
    }
    fn toggle_pane_option(&mut self, pane: Pane, slot: u8) {
        match (pane, slot) {
            (Pane::Listing, 1) => self.sim.view.exec_txt = !self.sim.view.exec_txt,
            (Pane::Listing, 2) => self.colorize_listing = !self.colorize_listing,
            (Pane::Listing, 3) => {
                self.run_speed = self.run_speed.next();
                self.status = format!("Run speed: {}", self.run_speed.label());
            }
            (Pane::Listing, 4) => self.toggle_breakpoint_at_pc(),
            (Pane::Regs, 1) => self.sim.view.reg_hex = !self.sim.view.reg_hex,
            (Pane::Regs, 2) => self.sim.view.reg_int = !self.sim.view.reg_int,
            (Pane::Regs, 3) => self.sim.view.reg_flp = !self.sim.view.reg_flp,
            (Pane::Mem, 1) => self.sim.view.mem_hex = !self.sim.view.mem_hex,
            (Pane::Mem, 2) => self.sim.view.mem_int = !self.sim.view.mem_int,
            (Pane::Mem, 3) => self.sim.view.mem_flp = !self.sim.view.mem_flp,
            (Pane::Mem, 4) => self.sim.view.mem_txt = !self.sim.view.mem_txt,
            (Pane::Mem, 5) => self.jump_mem_to_sp(),
            (Pane::Mem, 6) => self.jump_mem_to_address(0),
            (Pane::Mem, 7) => {
                self.modal = Modal::CallStack;
                self.needs_full_clear = true;
            }
            (Pane::Out, 1 | 5) => {
                if self.dma_out_available {
                    self.sim.view.dma_out = !self.sim.view.dma_out;
                } else {
                    self.status = "DMA OUT requires OUTC (0x0) and OUTD (0x8) labels in source".to_string();
                }
            }
            (Pane::Inp, 1 | 5) => {
                if self.dma_inp_available {
                    self.sim.view.dma_inp = !self.sim.view.dma_inp;
                } else {
                    self.status = "DMA INP requires INPC (0x10) and INPD (0x18) labels in source".to_string();
                }
            }
            (Pane::Exec, 1) => {
                self.modal = Modal::Stats;
                self.needs_full_clear = true;
            }
            (Pane::Exec, 2) => {
                self.sim.convention.enabled = !self.sim.convention.enabled;
                self.status = format!(
                    "Calling convention checker: {}",
                    if self.sim.convention.enabled { "ON" } else { "OFF" }
                );
                self.needs_full_clear = true;
            }
            (Pane::Exec, 3) => {
                self.modal = Modal::CallStack;
                self.needs_full_clear = true;
            }
            (Pane::Source, 1) => {
                self.sim.dialect = rvs_transpile::AssemblyDialect::Rvs;
                self.status = "Assembly dialect: RVS".to_string();
                self.needs_full_clear = true;
            }
            (Pane::Source, 2) => {
                self.sim.dialect = rvs_transpile::AssemblyDialect::Gnu;
                self.status = "Assembly dialect: GNU".to_string();
                self.needs_full_clear = true;
            }
            (Pane::Source, 3) => {
                let is_cur_rvs = self.sim.dialect == rvs_transpile::AssemblyDialect::Rvs;
                let src = self.editor.text();
                let transpiled = if is_cur_rvs {
                    rvs_transpile::transpile_rvs_to_gnu(&src)
                } else {
                    rvs_transpile::transpile_gnu_to_rvs(&src)
                };
                self.editor.replace_text(&transpiled);
                self.sim.dialect = self.sim.dialect.toggle();
                self.status = format!("Transpiled buffer to {} and switched mode", self.sim.dialect.name());
                self.needs_full_clear = true;
            }
            _ => {}
        }
    }

    fn clear_pane(&mut self, pane: Pane) {
        match pane {
            Pane::Exec => {
                self.sim.exec_log.clear();
                self.scroll_exec = 0;
                self.needs_full_clear = true;
            }
            Pane::Out => {
                self.sim.output.clear();
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

        let (target_path, is_temp) = if self.fname.is_empty() {
            let temp_dir = std::env::temp_dir();
            let p = temp_dir.join(format!("rvs_buf_{}.s", std::process::id()));
            let _ = std::fs::write(&p, self.editor.text());
            (p, true)
        } else {
            let p = PathBuf::from(rvs_explorer::expand_tilde(&self.fname));
            if self.editor.needs_save() {
                let _ = std::fs::write(&p, self.editor.text());
            }
            (p, false)
        };

        let _ = crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = crossterm::terminal::disable_raw_mode();

        let mut child = std::process::Command::new(&editor_cmd)
            .arg(&target_path)
            .status();

        if child.is_err() && editor_cmd == "nvim" {
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
            Ok(status) => {
                if status.success() {
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
            }
            Err(e) => {
                self.status = format!("ERROR spawning editor {editor_cmd}: {e}");
            }
        }

        if is_temp {
            let _ = std::fs::remove_file(target_path);
        }

        Ok(())
    }
}
