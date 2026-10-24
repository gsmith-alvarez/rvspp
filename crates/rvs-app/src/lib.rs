//! Application state and simulator actions for the RVS++ TUI.

use std::path::PathBuf;
use std::time::{Duration, Instant};


use rvs_ecall::{poll_dma_input, poll_dma_output, submit_dma_input, submit_ecall_input};
use rvs_editor::Editor;
use rvs_executor::{step, StepResult};
use rvs_sim::Simulator;
use rvs_tcl as tcl;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Normal,
    Edit,
    Input,
    Running,
    Command,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pane {
    Source,
    Listing,
    Regs,
    Mem,
    Exec,
    Out,
    Inp,
}

impl Pane {
    pub const ALL: [Pane; 7] = [
        Pane::Source,
        Pane::Listing,
        Pane::Regs,
        Pane::Mem,
        Pane::Exec,
        Pane::Out,
        Pane::Inp,
    ];

    pub fn next(self) -> Self {
        match self {
            Pane::Source => Pane::Listing,
            Pane::Listing => Pane::Regs,
            Pane::Regs => Pane::Mem,
            Pane::Mem => Pane::Exec,
            Pane::Exec => Pane::Out,
            Pane::Out => Pane::Inp,
            Pane::Inp => Pane::Source,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Pane::Source => Pane::Inp,
            Pane::Inp => Pane::Out,
            Pane::Out => Pane::Exec,
            Pane::Exec => Pane::Mem,
            Pane::Mem => Pane::Regs,
            Pane::Regs => Pane::Listing,
            Pane::Listing => Pane::Source,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Pane::Source => "Source",
            Pane::Listing => "Listing",
            Pane::Regs => "Regs",
            Pane::Mem => "Mem",
            Pane::Exec => "Exec",
            Pane::Out => "OUT",
            Pane::Inp => "INP",
        }
    }
}



#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileAction {
    Load,
    Append,
    SaveAs,
    Batch,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConfirmAction {
    Quit,
    Load,
    Append,
    Batch,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Modal {
    None,
    FilePrompt(FileAction),
    ConfirmSave(ConfirmAction),
    About,
    Stats,
    CallStack,
}


/// Register-file snapshot used to highlight changed registers.
#[derive(Clone, Default)]
struct RegSnapshot {
    x: [u64; 32],
    f: [u64; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RunSpeed {
    Hz1,
    Hz5,
    Hz20,
    #[default]
    Max,
}

impl RunSpeed {
    pub fn next(self) -> Self {
        match self {
            RunSpeed::Hz1 => RunSpeed::Hz5,
            RunSpeed::Hz5 => RunSpeed::Hz20,
            RunSpeed::Hz20 => RunSpeed::Max,
            RunSpeed::Max => RunSpeed::Hz1,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            RunSpeed::Hz1 => RunSpeed::Max,
            RunSpeed::Hz5 => RunSpeed::Hz1,
            RunSpeed::Hz20 => RunSpeed::Hz5,
            RunSpeed::Max => RunSpeed::Hz20,
        }
    }
    pub fn delay(self) -> Option<Duration> {
        match self {
            RunSpeed::Hz1 => Some(Duration::from_millis(1000)),
            RunSpeed::Hz5 => Some(Duration::from_millis(200)),
            RunSpeed::Hz20 => Some(Duration::from_millis(50)),
            RunSpeed::Max => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            RunSpeed::Hz1 => "1Hz",
            RunSpeed::Hz5 => "5Hz",
            RunSpeed::Hz20 => "20Hz",
            RunSpeed::Max => "Max",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SimStateSnapshot {
    pub regs: rvs_regs::RegisterFile,
    pub mem: rvs_memory::Memory,
    pub exec_log: String,
    pub output: String,
    pub pc: u64,
}


pub struct App {
    pub sim: Simulator,
    pub editor: Editor,
    pub mode: Mode,
    pub focus: Pane,
    pub fname: String,
    pub modal: Modal,
    pub modal_input: String,
    pub input_line: String,
    pub running: bool,
    pub was_running: bool,
    pub dma_out_available: bool,
    pub dma_inp_available: bool,
    pub quit_requested: bool,
    pub status: String,
    pub scroll_listing: usize,
    pub scroll_regs: usize,
    pub scroll_mem: usize,
    pub scroll_exec: usize,
    pub scroll_out: usize,
    pub needs_full_clear: bool,
    pub tick: usize,
    pub last_area: ratatui::layout::Rect,
    pub explorer: rvs_explorer::FileExplorerState,
    pub command_input: String,
    pub pending_op: Option<char>,
    pub zoomed_pane: Option<Pane>,
    pub pending_external_editor: bool,
    pub colorize_listing: bool,
    pub breakpoints: std::collections::BTreeSet<u64>,
    pub step_history: std::collections::VecDeque<SimStateSnapshot>,
    pub run_speed: RunSpeed,
    pub at_breakpoint: bool,
    last_regs: RegSnapshot,
    changed_x: [bool; 32],
    changed_f: [bool; 32],
}

impl App {
    pub fn new(path: Option<&String>) -> Self {
        let mut sim = Simulator::new();
        let mut editor = Editor::default();
        editor.set_text("");
        let mut fname = String::new();
        let mut status = String::new();
        if let Some(p) = path {
            let resolved = rvs_explorer::expand_tilde(p);
            match std::fs::read_to_string(&resolved) {
                Ok(text) => {
                    editor.set_text(&text);
                    fname = p.clone();
                }
                Err(e) => {
                    editor.set_text("");
                    status = format!("ERROR: {e}");
                }
            }
        }
        sim.source = editor.text();
        sim.convention.enabled = true;
        Self {
            sim,
            editor,
            mode: Mode::Normal,
            focus: Pane::Source,
            fname,
            modal: Modal::None,
            modal_input: String::new(),
            input_line: String::new(),
            running: false,
            was_running: false,
            dma_out_available: false,
            dma_inp_available: false,
            quit_requested: false,
            status,
            scroll_listing: 0,
            scroll_regs: 0,
            scroll_mem: 0,
            scroll_exec: 0,
            scroll_out: 0,
            needs_full_clear: false,
            tick: 0,
            last_area: ratatui::layout::Rect::default(),
            explorer: rvs_explorer::FileExplorerState::new(),
            command_input: String::new(),
            pending_op: None,
            zoomed_pane: None,
            pending_external_editor: false,
            colorize_listing: true,
            breakpoints: std::collections::BTreeSet::new(),
            step_history: std::collections::VecDeque::new(),
            run_speed: RunSpeed::Max,
            at_breakpoint: false,
            last_regs: RegSnapshot::default(),
            changed_x: [false; 32],
            changed_f: [false; 32],
        }
    }

    // ----- actions -----------------------------------------------------

    pub fn reset_sim_panes(&mut self) {
        let view = self.sim.view.clone();
        let conv_enabled = self.sim.convention.enabled;
        self.sim = Simulator::new();
        self.sim.convention.enabled = conv_enabled;
        self.sim.view = view;
        self.sim.source = self.editor.text();
        self.running = false;
        self.was_running = false;
        self.dma_out_available = false;
        self.dma_inp_available = false;
        self.input_line.clear();
        self.scroll_listing = 0;
        self.scroll_regs = 0;
        self.scroll_mem = 0;
        self.scroll_exec = 0;
        self.scroll_out = 0;
        self.take_reg_snapshot();
        self.needs_full_clear = true;
        self.breakpoints.clear();
        self.step_history.clear();
    }

    pub fn compile(&mut self) {
        let src = self.editor.text();
        self.sim.inp_prompt = false;
        self.running = false;
        self.was_running = false;
        self.sim.brk = false;
        self.sim.pending = None;
        self.sim.entry_type.clear();
        self.input_line.clear();
        self.sim.convention.reset();
        self.sim.metrics.reset();
        match rvs_assembler::assemble(&mut self.sim, &src) {
            Ok(listing) => {
                self.sim.listing = format!("{listing}\n");
            }
            Err(err) => {
                self.sim
                    .sym
                    .insert("START".to_string(), Simulator::START_DEFAULT.to_string());
                self.sim.regs.pc =
                    tcl::parse_int(Simulator::START_DEFAULT).unwrap_or(0) as u64;
                self.sim.listing =
                    format!("{}ERROR: {}\n\n", err.progress, err.message);
            }
        }
        self.sim.source = src;
        self.sim.exec_log.clear();
        self.sim.output.clear();
        self.dma_out_available = self.sim.sym.contains_key("OUTC") && self.sim.sym.contains_key("OUTD");
        self.dma_inp_available = self.sim.sym.contains_key("INPC") && self.sim.sym.contains_key("INPD");
        if self.dma_out_available {
            self.sim.view.dma_out = true;
        } else {
            self.sim.view.dma_out = false;
        }
        if self.dma_inp_available {
            self.sim.view.dma_inp = true;
        } else {
            self.sim.view.dma_inp = false;
            self.sim.entry_type.clear();
        }
        self.scroll_listing = 0;
        self.scroll_regs = 0;
        self.scroll_mem = 0;
        self.scroll_exec = 0;
        self.scroll_out = 0;
        self.needs_full_clear = true;
        self.take_reg_snapshot();
    }

    pub fn reset_pc(&mut self) {
        let pc = self
            .sim
            .sym
            .get("START")
            .and_then(|s| tcl::parse_int(s))
            .map(|v| v as u64)
            .unwrap_or(0);
        self.sim.regs.pc = pc;
    }

    pub fn refresh_listing(&mut self) {
        if !self.sim.listing.is_empty() && !self.sim.listing.contains("ERROR:") {
            self.sim.listing = format!("{}\n", rvs_assembler::dump(&mut self.sim));
            self.needs_full_clear = true;
        }
    }

    pub fn action_start(&mut self) {
        self.reset_pc();
        self.sim.exec_log.push_str("START\n");
        self.sim.inp_prompt = false;
        self.at_breakpoint = false;
        self.sim.convention.reset();
        self.sim.metrics.reset();
        self.running = true;
        self.was_running = false;
        self.mode = Mode::Running;
    }

    pub fn action_next(&mut self) {
        if !self.ensure_pc_wide() {
            return;
        }
        self.running = false;
        if self.mode == Mode::Running {
            self.mode = Mode::Normal;
        }
        self.sim.exec_log.push_str("NEXT\n");
        self.do_step();
    }

    pub fn ensure_pc_wide(&mut self) -> bool {
        // The reference validates the PC entry; ours always holds a number.
        true
    }

    pub fn action_run(&mut self) {
        self.sim.exec_log.push_str("RUN\n");
        self.running = true;
        self.was_running = false;
        self.mode = Mode::Running;
    }

    pub fn action_stop(&mut self) {
        self.sim.brk = true;
        self.was_running = false;
        self.running = false;
        if self.mode == Mode::Input {
            self.cancel_input();
        }
        self.mode = Mode::Normal;
    }

    pub fn take_reg_snapshot(&mut self) {
        self.last_regs = RegSnapshot {
            x: self.sim.regs.x_values(),
            f: self.sim.regs.f_values(),
        };
        self.changed_x = [false; 32];
        self.changed_f = [false; 32];
    }

    pub fn do_step(&mut self) {
        if self.step_history.len() >= 128 {
            self.step_history.pop_front();
        }
        self.step_history.push_back(SimStateSnapshot {
            regs: self.sim.regs.clone(),
            mem: self.sim.mem.clone(),
            exec_log: self.sim.exec_log.clone(),
            output: self.sim.output.clone(),
            pc: self.sim.regs.pc,
        });
        self.take_reg_snapshot();
        match step(&mut self.sim) {
            StepResult::Continued => {}
            StepResult::Halted => {
                self.running = false;
                if self.mode == Mode::Running {
                    self.mode = Mode::Normal;
                }
            }
            StepResult::WaitingForInput => {
                self.mode = Mode::Input;
                self.focus = Pane::Inp;
                self.was_running = self.running;
                self.running = false;
            }
        }
        self.mark_changed_regs();
    }

    pub fn action_step_back(&mut self) {
        if let Some(snap) = self.step_history.pop_back() {
            self.sim.regs = snap.regs;
            self.sim.mem = snap.mem;
            self.sim.exec_log = snap.exec_log;
            self.sim.output = snap.output;
            self.sim.regs.pc = snap.pc;
            self.take_reg_snapshot();
            self.needs_full_clear = true;
            self.status = format!("Stepped back to 0x{:016x}", self.sim.regs.pc);
        } else {
            self.status = "No earlier step history to rewind".to_string();
        }
    }

    pub fn toggle_breakpoint(&mut self, addr: u64) {
        let is_set = if self.breakpoints.contains(&addr) {
            self.breakpoints.remove(&addr);
            false
        } else {
            self.breakpoints.insert(addr);
            true
        };
        self.status = format!("Breakpoint {} at 0x{addr:016x}", if is_set { "set" } else { "cleared" });
        self.needs_full_clear = true;
    }

    pub fn toggle_breakpoint_at_pc(&mut self) {
        self.toggle_breakpoint(self.sim.regs.pc);
        self.status = format!("Breakpoint toggled at PC (0x{:016x})", self.sim.regs.pc);
    }

    pub fn has_breakpoint_on_line(&self, line_idx: usize) -> bool {
        if let Some(&addr) = self.sim.line_to_addr.get(&line_idx) {
            self.breakpoints.contains(&addr)
        } else {
            false
        }
    }

    pub fn toggle_breakpoint_at_source_line(&mut self, line_idx: usize) {
        if let Some(&addr) = self.sim.line_to_addr.get(&line_idx) {
            self.toggle_breakpoint(addr);
            self.status = format!("Breakpoint toggled at line {} (0x{addr:016x})", line_idx + 1);
        } else {
            let closest = self.sim.line_to_addr.range(line_idx..).next();
            if let Some((&l, &addr)) = closest {
                self.toggle_breakpoint(addr);
                self.status = format!("Breakpoint toggled at line {} (0x{addr:016x})", l + 1);
            } else if let Some((&l, &addr)) = self.sim.line_to_addr.iter().next_back() {
                self.toggle_breakpoint(addr);
                self.status = format!("Breakpoint toggled at line {} (0x{addr:016x})", l + 1);
            } else {
                self.status = format!("No compiled code for line {}", line_idx + 1);
            }
        }
    }

    pub fn clear_all_breakpoints(&mut self) {
        self.breakpoints.clear();
        self.status = "Cleared all breakpoints".to_string();
        self.needs_full_clear = true;
    }

    pub fn jump_mem_to_address(&mut self, target_addr: u64) {
        if self.sim.mem.cells.is_empty() {
            self.scroll_mem = 0;
            self.status = format!("Memory empty; jump to 0x{target_addr:016x} stored");
            return;
        }
        let keys: Vec<u64> = self.sim.mem.cells.keys().copied().collect();
        let closest_idx = keys.iter().position(|&k| k >= target_addr).unwrap_or(keys.len().saturating_sub(1));
        self.scroll_mem = closest_idx;
        self.status = format!("Jumped memory view to 0x{target_addr:016x}");
        self.needs_full_clear = true;
    }

    pub fn jump_mem_to_sp(&mut self) {
        let sp = self.sim.regs.read_x(2);
        self.jump_mem_to_address(sp);
    }

    pub fn mark_changed_regs(&mut self) {
        for i in 0..32 {
            self.changed_x[i] = self.sim.regs.x_values()[i] != self.last_regs.x[i];
            self.changed_f[i] = self.sim.regs.f_values()[i] != self.last_regs.f[i];
        }
    }

    /// `RunCmd` chunk: bounded stepping so the TUI stays responsive.
    pub fn run_chunk(&mut self, budget: Duration) {
        let start = Instant::now();
        let mut first = true;
        while !self.sim.brk {
            if (!first || !self.at_breakpoint) && self.breakpoints.contains(&self.sim.regs.pc) {
                self.running = false;
                self.was_running = false;
                self.mode = Mode::Normal;
                self.at_breakpoint = true;
                self.status = format!("Breakpoint hit at 0x{:016x}", self.sim.regs.pc);
                break;
            }
            self.at_breakpoint = false;
            first = false;
            if start.elapsed() >= budget {
                break;
            }
            match step(&mut self.sim) {
                StepResult::Continued => {}
                StepResult::Halted => {
                    self.running = false;
                    self.was_running = false;
                    self.mode = Mode::Normal;
                    break;
                }
                StepResult::WaitingForInput => {
                    self.mode = Mode::Input;
                    self.focus = Pane::Inp;
                    self.was_running = true;
                    self.running = false;
                    break;
                }
            }
        }
        if self.sim.brk {
            self.sim.exec_log.push_str("ERROR: Stopped");
            self.sim.brk = false;
            self.running = false;
            self.was_running = false;
            self.mode = Mode::Normal;
        }
        if self.sim.view.dma_out {
            poll_dma_output(&mut self.sim);
        }
        if self.sim.view.dma_inp {
            poll_dma_input(&mut self.sim);
            if !self.sim.entry_type.is_empty() && self.mode != Mode::Input {
                self.mode = Mode::Input;
                self.focus = Pane::Inp;
            }
        }
        self.mark_changed_regs();
    }

    pub fn cancel_input(&mut self) {
        self.was_running = false;
        self.running = false;
        if self.sim.pending.take().is_some() {
            self.sim.output.push_str("Cancelled\n");
            self.sim.exec_log.push_str("ERROR: Cancelled\n");
        }
        self.sim.entry_type.clear();
        self.sim.inp_prompt = false;
        self.input_line.clear();
        self.mode = Mode::Normal;
        self.focus = Pane::Listing;
    }

    pub fn submit_input(&mut self) {
        let line = std::mem::take(&mut self.input_line);
        let resume = self.was_running;
        self.was_running = false;

        if self.sim.pending.is_some() {
            let inst_pc = self.sim.pending.as_ref().map(|p| p.inst_pc).unwrap_or(0);
            if let Err(e) = submit_ecall_input(&mut self.sim, &line) {
                self.sim.regs.pc = inst_pc;
                self.sim.exec_log.push_str(&format!("ERROR: {}\n", e.0));
                self.running = false;
                self.mode = Mode::Normal;
                self.focus = Pane::Listing;
                return;
            }
        } else if !self.sim.entry_type.is_empty() {
            if let Err(e) = submit_dma_input(&mut self.sim, &line) {
                self.sim.exec_log.push_str(&format!("ERROR: {}\n", e.0));
                self.running = false;
                self.mode = Mode::Normal;
                self.focus = Pane::Listing;
                return;
            }
        }

        if resume {
            self.running = true;
            self.mode = Mode::Running;
            self.focus = Pane::Listing;
        } else {
            self.running = false;
            self.mode = Mode::Normal;
            self.focus = Pane::Listing;
        }
    }

    // ----- file actions ------------------------------------------------

    pub fn load_file(&mut self, path: &str, append: bool) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                if append {
                    let mut t = self.editor.text();
                    t.push_str(&text);
                    self.editor.set_text(&t);
                    self.fname.clear();
                    self.editor.modified = true;
                } else {
                    self.editor.set_text(&text);
                    self.fname = path.to_string();
                    self.editor.modified = false;
                }
                self.reset_sim_panes();
                self.status = format!("Loaded {path}");
            }
            Err(e) => self.status = format!("ERROR: {e}"),
        }
    }

    pub fn save_file(&mut self, path: &str) {
        match std::fs::write(path, self.editor.text()) {
            Ok(()) => {
                self.fname = path.to_string();
                self.editor.modified = false;
                self.status = format!("Saved {path}");
            }
            Err(e) => self.status = format!("ERROR: {e}"),
        }
    }

    pub fn run_batch_file_entries(&mut self, path: &str) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                self.status = format!("ERROR: {e}");
                return;
            }
        };
        let mut log = String::new();
        for line in content.split('\n') {
            let line = line.trim();
            log.push_str(line);
            log.push('\n');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }
            let timeout = parts
                .get(3)
                .and_then(|t| t.parse::<u64>().ok())
                .unwrap_or(3000);
            match rvs_format::run_batch_file(
                PathBuf::from(parts[0]).as_path(),
                PathBuf::from(parts[1]).as_path(),
                PathBuf::from(parts[2]).as_path(),
                timeout,
            ) {
                Ok(()) => {}
                Err(e) => log.push_str(&format!("#{e}\n")),
            }
        }
        self.editor.set_text(&format!("{content}{log}"));
        self.fname = path.to_string();
        self.editor.modified = false;
        self.reset_sim_panes();
    }

    pub fn modal_accept(&mut self) {
        let path = self.modal_input.trim().to_string();
        if let Modal::FilePrompt(action) = self.modal {
            self.modal = Modal::None;
            self.needs_full_clear = true;
            self.perform_file_action_with_path(action, &path);
        }
    }

    pub fn modal_save_and_proceed(&mut self) {
        if let Modal::ConfirmSave(action) = self.modal {
            let target = self.modal_input.trim();
            let path = if !target.is_empty() {
                rvs_explorer::expand_tilde(target)
            } else if !self.fname.is_empty() {
                self.fname.clone()
            } else {
                self.open_modal(FileAction::SaveAs);
                return;
            };
            self.save_file(&path);
            self.editor.modified = false;
            self.modal = Modal::None;
            self.needs_full_clear = true;
            match action {
                ConfirmAction::Quit => self.quit_requested = true,
                ConfirmAction::Load => self.open_modal(FileAction::Load),
                ConfirmAction::Append => self.open_modal(FileAction::Append),
                ConfirmAction::Batch => self.open_modal(FileAction::Batch),
            }
        }
    }

    pub fn modal_discard(&mut self) {
        if let Modal::ConfirmSave(action) = self.modal {
            self.editor.modified = false;
            self.modal = Modal::None;
            self.needs_full_clear = true;
            match action {
                ConfirmAction::Quit => self.quit_requested = true,
                ConfirmAction::Load => self.open_modal(FileAction::Load),
                ConfirmAction::Append => self.open_modal(FileAction::Append),
                ConfirmAction::Batch => self.open_modal(FileAction::Batch),
            }
        }
    }

    pub fn perform_file_action_with_path(&mut self, action: FileAction, path: &str) {
        if path.is_empty() {
            return;
        }
        let resolved = rvs_explorer::expand_tilde(path);
        match action {
            FileAction::Load => self.load_file(&resolved, false),
            FileAction::Append => self.load_file(&resolved, true),
            FileAction::SaveAs => self.save_file(&resolved),
            FileAction::Batch => self.run_batch_file_entries(&resolved),
        }
    }

    pub fn open_modal(&mut self, action: FileAction) {
        self.modal = Modal::FilePrompt(action);
        self.modal_input = match action {
            FileAction::Load | FileAction::Append | FileAction::Batch => self.fname.clone(),
            FileAction::SaveAs => self.fname.clone(),
        };
        self.explorer.mode = rvs_explorer::ExplorerMode::Normal;
        self.explorer.update(&self.modal_input);
    }

    pub fn handle_open_action(&mut self, action: FileAction) {
        if self.editor.needs_save() {
            let confirm = match action {
                FileAction::Load => ConfirmAction::Load,
                FileAction::Append => ConfirmAction::Append,
                FileAction::Batch => ConfirmAction::Batch,
                FileAction::SaveAs => {
                    self.open_modal(FileAction::SaveAs);
                    return;
                }
            };
            self.modal = Modal::ConfirmSave(confirm);
            self.modal_input = self.fname.clone();
        } else {
            self.open_modal(action);
        }
    }

    // ----- input events -------------------------------------------------

}

impl App {
    pub fn request_quit(&mut self) {
        if self.editor.needs_save() {
            self.modal = Modal::ConfirmSave(ConfirmAction::Quit);
            self.modal_input = self.fname.clone();
        } else {
            self.quit_requested = true;
        }
    }

    pub fn changed_x(&self, i: usize) -> bool {
        self.changed_x[i]
    }

    pub fn changed_f(&self, i: usize) -> bool {
        self.changed_f[i]
    }

    pub fn blink_on(&self) -> bool {
        (self.tick / 4) % 2 == 0
    }

}