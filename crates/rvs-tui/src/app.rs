//! TUI application: state, editing, execution control, mouse and keyboard.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use rvs_core::executor::{self, StepResult};
use rvs_core::Simulator;
use crate::ui;
use crate::Cli;

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

pub use crate::editor::Editor;


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
    pub regs: rvs_core::registers::RegisterFile,
    pub mem: rvs_core::memory::Memory,
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
    pub explorer: crate::explorer::FileExplorerState,
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
            let resolved = crate::explorer::expand_tilde(p);
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
            explorer: crate::explorer::FileExplorerState::new(),
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
        match rvs_core::assembler::assemble(&mut self.sim, &src) {
            Ok(listing) => {
                self.sim.listing = format!("{listing}\n");
            }
            Err(err) => {
                self.sim
                    .sym
                    .insert("START".to_string(), Simulator::START_DEFAULT.to_string());
                self.sim.regs.pc =
                    rvs_core::tcl::parse_int(Simulator::START_DEFAULT).unwrap_or(0) as u64;
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
            .and_then(|s| rvs_core::tcl::parse_int(s))
            .map(|v| v as u64)
            .unwrap_or(0);
        self.sim.regs.pc = pc;
    }

    pub fn refresh_listing(&mut self) {
        if !self.sim.listing.is_empty() && !self.sim.listing.contains("ERROR:") {
            self.sim.listing = format!("{}\n", rvs_core::assembler::dump(&mut self.sim));
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

    fn ensure_pc_wide(&mut self) -> bool {
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

    fn take_reg_snapshot(&mut self) {
        self.last_regs = RegSnapshot {
            x: self.sim.regs.x_values(),
            f: self.sim.regs.f_values(),
        };
        self.changed_x = [false; 32];
        self.changed_f = [false; 32];
    }

    fn do_step(&mut self) {
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
        match executor::step(&mut self.sim) {
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

    fn mark_changed_regs(&mut self) {
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
            match executor::step(&mut self.sim) {
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
            executor::poll_dma_output(&mut self.sim);
        }
        if self.sim.view.dma_inp {
            executor::poll_dma_input(&mut self.sim);
            if !self.sim.entry_type.is_empty() && self.mode != Mode::Input {
                self.mode = Mode::Input;
                self.focus = Pane::Inp;
            }
        }
        self.mark_changed_regs();
    }

    fn cancel_input(&mut self) {
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

    fn submit_input(&mut self) {
        let line = std::mem::take(&mut self.input_line);
        let resume = self.was_running;
        self.was_running = false;

        if self.sim.pending.is_some() {
            let inst_pc = self.sim.pending.as_ref().map(|p| p.inst_pc).unwrap_or(0);
            if let Err(e) = executor::submit_ecall_input(&mut self.sim, &line) {
                self.sim.regs.pc = inst_pc;
                self.sim.exec_log.push_str(&format!("ERROR: {}\n", e.0));
                self.running = false;
                self.mode = Mode::Normal;
                self.focus = Pane::Listing;
                return;
            }
        } else if !self.sim.entry_type.is_empty() {
            if let Err(e) = executor::submit_dma_input(&mut self.sim, &line) {
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

    fn save_file(&mut self, path: &str) {
        match std::fs::write(path, self.editor.text()) {
            Ok(()) => {
                self.fname = path.to_string();
                self.editor.modified = false;
                self.status = format!("Saved {path}");
            }
            Err(e) => self.status = format!("ERROR: {e}"),
        }
    }

    fn run_batch_file_entries(&mut self, path: &str) {
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
            match rvs_core::format::run_batch_file(
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
                crate::explorer::expand_tilde(target)
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

    fn perform_file_action_with_path(&mut self, action: FileAction, path: &str) {
        if path.is_empty() {
            return;
        }
        let resolved = crate::explorer::expand_tilde(path);
        match action {
            FileAction::Load => self.load_file(&resolved, false),
            FileAction::Append => self.load_file(&resolved, true),
            FileAction::SaveAs => self.save_file(&resolved),
            FileAction::Batch => self.run_batch_file_entries(&resolved),
        }
    }

    fn open_modal(&mut self, action: FileAction) {
        self.modal = Modal::FilePrompt(action);
        self.modal_input = match action {
            FileAction::Load | FileAction::Append | FileAction::Batch => self.fname.clone(),
            FileAction::SaveAs => self.fname.clone(),
        };
        self.explorer.mode = crate::explorer::ExplorerMode::Normal;
        self.explorer.update(&self.modal_input);
    }

    fn handle_open_action(&mut self, action: FileAction) {
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

    pub fn on_key(&mut self, key: KeyEvent) {
        if self.modal != Modal::None {
            self.on_key_modal(key);
            return;
        }
        if self.mode == Mode::Command {
            self.on_key_command(key);
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
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.fname.is_empty() {
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
            let is_plain_or_shift = key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;

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

            // Normal mode '?' to open Help/About
            if is_plain_or_shift && key.code == KeyCode::Char('?') {
                self.pending_op = None;
                self.modal = Modal::About;
                return;
            }

            // Normal mode 'z' or 'Z' to toggle zoom
            if is_plain_or_shift && (key.code == KeyCode::Char('z') || key.code == KeyCode::Char('Z')) {
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
                    KeyCode::Char('x') if self.focus != Pane::Source || self.mode == Mode::Running => {
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
                    if is_plain_or_shift && (key.code == KeyCode::Char('i') || key.code == KeyCode::Char('a')) {
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
                    self.editor.move_left();
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    self.pending_op = None;
                    self.editor.move_right();
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.pending_op = None;
                    self.editor.move_down();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.pending_op = None;
                    self.editor.move_up();
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
                    self.editor.move_word_forward();
                }
                KeyCode::Char('b') => {
                    self.pending_op = None;
                    self.editor.move_word_backward();
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
                    self.editor.delete();
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
            KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.editor.undo()
            }
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.editor.redo()
            }
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

    pub fn execute_command(&mut self) {
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
                    let path = crate::explorer::expand_tilde(args);
                    self.save_file(&path);
                }
            }
            "wq" | "x" => {
                let target = if !args.is_empty() {
                    crate::explorer::expand_tilde(args)
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
                    let resolved = crate::explorer::expand_tilde(args);
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
                    let addr = rvs_core::tcl::parse_int(addr_str).unwrap_or(0) as u64;
                    self.toggle_breakpoint(addr);
                } else if let Some(addr) = rvs_core::tcl::parse_int(args) {
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
                    let addr = rvs_core::tcl::parse_int(addr_str).unwrap_or(0) as u64;
                    self.jump_mem_to_address(addr);
                } else if let Some(addr) = rvs_core::tcl::parse_int(args) {
                    self.jump_mem_to_address(addr as u64);
                } else if args.is_empty() {
                    self.jump_mem_to_address(0);
                } else {
                    self.status = format!("ERROR: Invalid memory address or symbol \"{args}\"");
                }
            }
            "mode" | "dialect" => {
                match args.to_lowercase().as_str() {
                    "gnu" => self.sim.dialect = rvs_core::transpiler::AssemblyDialect::Gnu,
                    "rvs" | "dialect" => self.sim.dialect = rvs_core::transpiler::AssemblyDialect::Rvs,
                    "" => self.sim.dialect = self.sim.dialect.toggle(),
                    _ => self.status = format!("ERROR: Expected mode 'gnu' or 'rvs', got \"{args}\""),
                }
                self.status = format!("Assembly dialect: {}", self.sim.dialect.name());
                self.needs_full_clear = true;
            }
            "transpile" | "tp" => {
                let target = args.to_lowercase();
                let src = self.editor.text();
                let is_gnu_target = target == "gnu" || (target.is_empty() && self.sim.dialect == rvs_core::transpiler::AssemblyDialect::Rvs);
                let transpiled = if is_gnu_target {
                    rvs_core::transpiler::transpile_rvs_to_gnu(&src)
                } else {
                    rvs_core::transpiler::transpile_gnu_to_rvs(&src)
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

    pub fn trigger_zoom_or_editor(&mut self) {
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

    fn on_key_modal(&mut self, key: KeyEvent) {
        match (&self.modal, key.code) {
            (Modal::About | Modal::Stats | Modal::CallStack, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) => {
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
            (Modal::FilePrompt(_), _) => {
                match self.explorer.mode {
                    crate::explorer::ExplorerMode::Normal => {
                        let is_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                        let is_plain_or_shift = key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
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
                                    self.explorer.mode = crate::explorer::ExplorerMode::Filter;
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
                    crate::explorer::ExplorerMode::Filter => {
                        match key.code {
                            KeyCode::Esc => {
                                self.explorer.mode = crate::explorer::ExplorerMode::Normal;
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
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn request_quit(&mut self) {
        if self.editor.needs_save() {
            self.modal = Modal::ConfirmSave(ConfirmAction::Quit);
            self.modal_input = self.fname.clone();
        } else {
            self.quit_requested = true;
        }
    }

    pub fn cycle_focus(&mut self) {
        self.focus = self.focus.next();
        if self.mode != Mode::Input && self.mode != Mode::Running {
            self.mode = Mode::Normal;
        }
        if self.zoomed_pane.is_some() {
            self.zoomed_pane = Some(self.focus);
        }
    }

    pub fn cycle_focus_prev(&mut self) {
        self.focus = self.focus.prev();
        if self.mode != Mode::Input && self.mode != Mode::Running {
            self.mode = Mode::Normal;
        }
        if self.zoomed_pane.is_some() {
            self.zoomed_pane = Some(self.focus);
        }
    }

    pub fn scroll_pane_to_top(&mut self, pane: Pane) {
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

    pub fn scroll_pane_to_bottom(&mut self, pane: Pane) {
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

    pub fn on_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                ui::handle_click(self, mouse.column, mouse.row);
            }
            MouseEventKind::ScrollUp => ui::handle_scroll(self, mouse.column, mouse.row, -3),
            MouseEventKind::ScrollDown => ui::handle_scroll(self, mouse.column, mouse.row, 3),
            _ => {}
        }
    }

    pub fn scroll_pane(&mut self, pane: Pane, delta: i32) {
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
    pub fn toggle_pane_option(&mut self, pane: Pane, slot: u8) {
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
                self.sim.dialect = self.sim.dialect.toggle();
                self.status = format!("Assembly dialect: {}", self.sim.dialect.name());
                self.needs_full_clear = true;
            }
            _ => {}
        }
    }

    pub fn clear_pane(&mut self, pane: Pane) {
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

    pub fn explorer_open_selected(&mut self) {
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

    pub fn changed_x(&self, i: usize) -> bool {
        self.changed_x[i]
    }

    pub fn changed_f(&self, i: usize) -> bool {
        self.changed_f[i]
    }

    pub fn blink_on(&self) -> bool {
        (self.tick / 4) % 2 == 0
    }

    pub fn run_external_editor(
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
            let p = PathBuf::from(crate::explorer::expand_tilde(&self.fname));
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
use std::sync::atomic::{AtomicBool, Ordering};

static TERMINATED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn handle_signal(_: libc::c_int) {
    TERMINATED.store(true, Ordering::Relaxed);
}

#[cfg(unix)]
pub fn setup_signal_handlers() {
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = handle_signal as *const () as usize;
        libc::sigaction(libc::SIGINT, &sa, std::ptr::null_mut());
        libc::sigaction(libc::SIGTERM, &sa, std::ptr::null_mut());
        libc::sigaction(libc::SIGHUP, &sa, std::ptr::null_mut());
    }
}

#[cfg(not(unix))]
pub fn setup_signal_handlers() {}

#[cfg(unix)]
fn poll_terminal_event(timeout: Duration) -> Result<bool, Box<dyn std::error::Error>> {
    use std::os::unix::io::AsRawFd;
    if TERMINATED.load(Ordering::Relaxed) {
        return Ok(false);
    }
    let fd = std::io::stdin().as_raw_fd();
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
    let ret = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    if ret < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::Interrupted {
            return Ok(false);
        }
        return Err(Box::new(err));
    }
    if ret == 0 {
        return Ok(false);
    }
    if (pfd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL)) != 0 {
        TERMINATED.store(true, Ordering::Relaxed);
        return Ok(false);
    }
    Ok(crossterm::event::poll(Duration::from_millis(0))?)
}

#[cfg(not(unix))]
fn poll_terminal_event(timeout: Duration) -> Result<bool, Box<dyn std::error::Error>> {
    if TERMINATED.load(Ordering::Relaxed) {
        return Ok(false);
    }
    Ok(crossterm::event::poll(timeout)?)
}


/// Terminal setup/teardown and the event loop.
pub fn run_tui(cli: &Cli) -> ExitCode {
    setup_signal_handlers();
    let mut stdout = std::io::stdout();
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        EnableMouseCapture
    );
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(_) => return ExitCode::FAILURE,
    };
    let result = event_loop(&mut terminal, cli);
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = crossterm::terminal::disable_raw_mode();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new(cli.args.first());
    loop {
        if TERMINATED.load(Ordering::Relaxed) {
            return Ok(());
        }
        if app.needs_full_clear {
            app.needs_full_clear = false;
            terminal.clear()?;
        }
        terminal.draw(|f| ui::draw(f, &mut app))?;
        let timeout = if app.running {
            app.run_speed.delay().unwrap_or(Duration::from_millis(16))
        } else {
            Duration::from_millis(80)
        };
        if poll_terminal_event(timeout)? {
            match crossterm::event::read()? {
                Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => {
                    app.on_key(key)
                }
                Event::Mouse(m) => app.on_mouse(m),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
        if app.pending_external_editor {
            app.pending_external_editor = false;
            let _ = app.run_external_editor(terminal);
        }
        if app.running {
            if app.run_speed == RunSpeed::Max {
                app.run_chunk(Duration::from_millis(8));
            } else {
                app.do_step();
            }
        }
        app.tick += 1;
        if app.quit_requested {
            return Ok(());
        }
    }
}
