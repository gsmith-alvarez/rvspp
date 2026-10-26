//! Application state and actions for the RVS++ TUI.
//!
//! `App<M>` owns UI state and delegates every target-specific operation to a
//! [`rvs_machine::Machine`]. Swapping the backend swaps the language/ISA; the
//! editor, explorer, commands, event loop and renderers stay unchanged.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rvs_editor::Editor;
use rvs_machine::{
    Dialect, LoadOutcome, Machine, RegisterChanges, StepOutcome, ViewFlags, ViewRequest,
};

/// Parse one batch-entry line: `<asm> <in> <out> [timeout_ms]`.
///
/// Uses shell-style quoting so paths containing spaces work.
fn parse_batch_entry(line: &str) -> Option<Result<(PathBuf, PathBuf, PathBuf, u64), String>> {
    let parts = match shell_words::split(line) {
        Ok(parts) => parts,
        Err(e) => return Some(Err(format!("ERROR parsing batch entry: {e}"))),
    };
    if parts.len() < 3 {
        return None;
    }
    let timeout = parts
        .get(3)
        .and_then(|t| t.parse::<u64>().ok())
        .unwrap_or(3000);
    Some(Ok((
        PathBuf::from(&parts[0]),
        PathBuf::from(&parts[1]),
        PathBuf::from(&parts[2]),
        timeout,
    )))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Normal,
    Edit,
    Input,
    Running,
    Command,
    Search,
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
    New,
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

pub struct App<M: Machine> {
    pub machine: M,
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
    /// Digits typed before a motion (`3j`, `2dd`).
    pub pending_count: Option<usize>,
    /// Live `/` prompt text.
    pub search_input: String,
    /// Last committed search pattern (repeat with Ctrl+N / Ctrl+P).
    pub search_query: String,
    pub zoomed_pane: Option<Pane>,
    pub pending_external_editor: bool,
    pub colorize_listing: bool,
    /// UI-owned checkbutton state.
    pub view: ViewFlags,
    pub breakpoints: BTreeSet<u64>,
    /// Address → last seen cell value. Fires when the value changes.
    pub watchpoints: BTreeMap<u64, u64>,
    /// Address → condition expression; the breakpoint only fires when non-zero.
    pub break_conditions: BTreeMap<u64, String>,
    /// Set by `do_step` when the last instruction halted the machine.
    pub last_step_halted: bool,
    pub step_history: VecDeque<M::Snapshot>,
    pub run_speed: RunSpeed,
    pub at_breakpoint: bool,
    last_snapshot: M::Snapshot,
    changed_x: [bool; 32],
    changed_f: [bool; 32],
}

impl<M: Machine> App<M> {
    pub fn new(path: Option<&String>) -> Self {
        let mut machine = M::new();
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
        machine.set_source(&editor.text());
        let detected = machine.detect_dialect(&editor.text());
        machine.set_dialect(detected);
        machine.set_convention_enabled(true);
        let snapshot = machine.snapshot();
        Self {
            machine,
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
            pending_count: None,
            search_input: String::new(),
            search_query: String::new(),
            zoomed_pane: None,
            pending_external_editor: false,
            colorize_listing: true,
            view: ViewFlags::default(),
            breakpoints: BTreeSet::new(),
            watchpoints: BTreeMap::new(),
            break_conditions: BTreeMap::new(),
            last_step_halted: false,
            step_history: VecDeque::new(),
            run_speed: RunSpeed::Max,
            at_breakpoint: false,
            last_snapshot: snapshot,
            changed_x: [false; 32],
            changed_f: [false; 32],
        }
    }

    /// Snapshot of everything the renderers need for this frame.
    pub fn view_request(&self) -> ViewRequest {
        ViewRequest {
            flags: self.view.clone(),
            pc: self.machine.pc(),
            breakpoints: self.breakpoints.clone(),
            changed_x: self.changed_x.to_vec(),
            changed_f: self.changed_f.to_vec(),
            colorize: self.colorize_listing,
        }
    }

    // ----- actions -----------------------------------------------------

    pub fn reset_sim_panes(&mut self) {
        let dialect = self.machine.dialect();
        let convention = self.machine.convention_enabled();
        self.machine.reset();
        self.machine.set_dialect(dialect);
        self.machine.set_convention_enabled(convention);
        self.machine.set_source(&self.editor.text());
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
        self.break_conditions.clear();
        self.watchpoints.clear();
        self.step_history.clear();
    }

    pub fn compile(&mut self) {
        let src = self.editor.text();
        self.running = false;
        self.was_running = false;
        self.input_line.clear();
        self.status = match self.machine.load(&src) {
            LoadOutcome::Ok => "Compiled".to_string(),
            LoadOutcome::Err(e) => format!("ERROR: {}", e.message),
        };
        // Snapshots from the previous program cannot be rewound into this one.
        self.step_history.clear();
        self.take_reg_snapshot();
        let (out, inp) = self.machine.dma_availability();
        self.dma_out_available = out;
        self.dma_inp_available = inp;
        self.view.dma_out = out;
        self.view.dma_inp = inp;
        if !inp {
            // The backend clears its entry state on load; the UI clears here.
            self.input_line.clear();
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
        self.machine.reset_pc();
    }

    pub fn refresh_listing(&mut self) {
        self.machine.refresh_listing();
        self.needs_full_clear = true;
    }

    pub fn action_start(&mut self) {
        self.machine.start();
        self.at_breakpoint = false;
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
        self.machine.append_log("NEXT\n");
        self.do_step();
    }

    pub fn ensure_pc_wide(&mut self) -> bool {
        true
    }

    pub fn action_run(&mut self) {
        self.machine.append_log("RUN\n");
        self.running = true;
        self.was_running = false;
        self.mode = Mode::Running;
    }

    pub fn action_stop(&mut self) {
        self.machine.request_stop();
        self.was_running = false;
        self.running = false;
        if self.mode == Mode::Input {
            self.cancel_input();
        }
        self.mode = Mode::Normal;
    }

    pub fn take_reg_snapshot(&mut self) {
        self.last_snapshot = self.machine.snapshot();
        self.changed_x = [false; 32];
        self.changed_f = [false; 32];
    }

    pub fn do_step(&mut self) {
        if self.step_history.len() >= 128 {
            if let Some(dropped) = self.step_history.pop_front() {
                self.machine.forget(&dropped);
            }
        }
        self.step_history.push_back(self.machine.snapshot());
        self.take_reg_snapshot();
        self.last_step_halted = false;
        match self.machine.step() {
            StepOutcome::Continued => {}
            StepOutcome::Halted => {
                self.last_step_halted = true;
                self.running = false;
                if self.mode == Mode::Running {
                    self.mode = Mode::Normal;
                }
            }
            StepOutcome::WaitingForInput => {
                self.mode = Mode::Input;
                self.focus = Pane::Inp;
                self.was_running = self.running;
                self.running = false;
            }
        }
        self.mark_changed_regs();
    }

    // ----- step over / out, watchpoints --------------------------------

    /// `:next` — step over a call instead of into it.
    pub fn step_over(&mut self) {
        if !self.machine.at_call() {
            self.do_step();
            self.status = "Step over: 1 instruction".to_string();
            return;
        }
        let target = self.machine.pc().wrapping_add(4);
        self.run_until(target, "Step over");
    }

    /// `:finish` — run until the current function returns.
    pub fn step_out(&mut self) {
        match self.machine.return_address() {
            Some(target) => self.run_until(target, "Finish"),
            None => self.status = "No active frame to return to".to_string(),
        }
    }

    /// Run until `target` (temporary breakpoint), another breakpoint, a
    /// watchpoint, a halt or the step budget.
    fn run_until(&mut self, target: u64, label: &str) {
        let inserted = self.breakpoints.insert(target);
        let mut steps = 0usize;
        while steps < 200_000 {
            if self.machine.pc() == target {
                break;
            }
            let pc = self.machine.pc();
            if pc != target && self.breakpoints.contains(&pc) && self.condition_met(pc) {
                break;
            }
            self.do_step();
            steps += 1;
            if self.last_step_halted || self.check_watchpoints() {
                break;
            }
            if self.mode == Mode::Input {
                break;
            }
        }
        if inserted {
            self.breakpoints.remove(&target);
        }
        self.status = format!("{label}: {steps} instruction(s)");
        self.needs_full_clear = true;
    }

    /// A breakpoint with a condition fires only when the expression is non-zero.
    fn condition_met(&self, addr: u64) -> bool {
        match self.break_conditions.get(&addr) {
            None => true,
            // An unevaluatable condition fires: better noisy than silent.
            Some(expr) => self.machine.eval_expr(expr).map(|v| v != 0).unwrap_or(true),
        }
    }

    /// Compare watched cells with their recorded values; stop on change.
    pub fn check_watchpoints(&mut self) -> bool {
        let changed = self
            .watchpoints
            .iter()
            .find(|(&addr, &last)| self.machine.peek_mem(addr) != last)
            .map(|(&addr, _)| addr);
        match changed {
            Some(addr) => {
                let value = self.machine.peek_mem(addr);
                self.watchpoints.insert(addr, value);
                self.running = false;
                self.was_running = false;
                self.at_breakpoint = false;
                if self.mode == Mode::Running {
                    self.mode = Mode::Normal;
                }
                self.status = format!("Watchpoint 0x{addr:016x} = 0x{value:016x}");
                true
            }
            None => false,
        }
    }

    /// `:watch <addr|label|sp>` — stop when the cell's value changes.
    pub fn add_watchpoint(&mut self, addr: u64) {
        let value = self.machine.peek_mem(addr);
        self.watchpoints.insert(addr, value);
        self.status = format!("Watchpoint 0x{addr:016x} (now 0x{value:016x})");
    }

    pub fn remove_watchpoint(&mut self, addr: u64) -> bool {
        self.watchpoints.remove(&addr).is_some()
    }

    pub fn clear_watchpoints(&mut self) {
        self.watchpoints.clear();
        self.status = "Cleared all watchpoints".to_string();
    }

    /// Attach/clear a condition on an existing breakpoint (`:b <t> if <expr>`).
    pub fn set_break_condition(&mut self, addr: u64, condition: Option<String>) {
        match condition {
            Some(expr) if !expr.trim().is_empty() => {
                self.break_conditions.insert(addr, expr.clone());
                self.status = format!("Breakpoint 0x{addr:016x} if {expr}");
            }
            _ => {
                self.break_conditions.remove(&addr);
                self.status = format!("Cleared condition on 0x{addr:016x}");
            }
        }
    }

    pub fn action_step_back(&mut self) {
        if let Some(snap) = self.step_history.pop_back() {
            self.machine.restore(&snap);
            self.take_reg_snapshot();
            self.needs_full_clear = true;
            self.status = format!("Stepped back to 0x{:016x}", self.machine.pc());
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
        self.status = format!(
            "Breakpoint {} at 0x{addr:016x}",
            if is_set { "set" } else { "cleared" }
        );
        self.needs_full_clear = true;
    }

    pub fn toggle_breakpoint_at_pc(&mut self) {
        let pc = self.machine.pc();
        self.toggle_breakpoint(pc);
        self.status = format!("Breakpoint toggled at PC (0x{pc:016x})");
    }

    pub fn has_breakpoint_on_line(&self, line_idx: usize) -> bool {
        self.machine
            .line_address(line_idx)
            .map(|addr| self.breakpoints.contains(&addr))
            .unwrap_or(false)
    }

    pub fn toggle_breakpoint_at_source_line(&mut self, line_idx: usize) {
        if let Some((line, addr)) = self.machine.nearest_line_address(line_idx) {
            self.toggle_breakpoint(addr);
            self.status = format!("Breakpoint toggled at line {} (0x{addr:016x})", line + 1);
        } else {
            self.status = format!("No compiled code for line {}", line_idx + 1);
        }
    }

    /// `:abi` — select the syscall convention.
    pub fn set_abi(&mut self, name: &str) {
        match name.to_ascii_lowercase().as_str() {
            "a7" | "rars" | "gnu" => {
                self.machine.set_a7_abi(true);
                self.status = "Syscall ABI: a7 (a0/a1 args, a7 selector)".to_string();
            }
            "rvs" | "rvs046" | "course" | "" => {
                self.machine.set_a7_abi(false);
                self.status = "Syscall ABI: RVS (ecall rd, rs1, imm)".to_string();
            }
            other => self.status = format!("ERROR: unknown ABI \"{other}\" (rvs|a7)"),
        }
    }

    /// Resolve `pc`, a source line (1-based), a label or an address.
    pub fn resolve_code_target(&self, s: &str) -> Option<u64> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        if s.eq_ignore_ascii_case("pc") {
            return Some(self.machine.pc());
        }
        if let Ok(line) = s.parse::<usize>() {
            if let Some((_, addr)) = self.machine.nearest_line_address(line.saturating_sub(1)) {
                return Some(addr);
            }
        }
        if let Some(addr) = self.machine.symbol_address(s) {
            return Some(addr);
        }
        rvs_machine::parse_u64(s)
    }

    /// Resolve `sp`, a label or an address to a data address.
    pub fn resolve_data_target(&self, s: &str) -> Option<u64> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        if s.eq_ignore_ascii_case("sp") {
            return Some(self.machine.sp());
        }
        if let Some(addr) = self.machine.symbol_address(s) {
            return Some(addr);
        }
        rvs_machine::parse_u64(s)
    }

    /// Set (never toggle) a breakpoint, for `:b <t> if <expr>`.
    pub fn add_breakpoint(&mut self, addr: u64) {
        self.breakpoints.insert(addr);
        self.status = format!("Breakpoint set at 0x{addr:016x}");
        self.needs_full_clear = true;
    }

    pub fn clear_all_breakpoints(&mut self) {
        self.breakpoints.clear();
        self.break_conditions.clear();
        self.status = "Cleared all breakpoints".to_string();
        self.needs_full_clear = true;
    }

    pub fn jump_mem_to_address(&mut self, target_addr: u64) {
        let keys = self.machine.memory_addresses();
        if keys.is_empty() {
            self.scroll_mem = 0;
            self.status = format!("Memory empty; jump to 0x{target_addr:016x} stored");
            return;
        }
        let closest_idx = keys
            .iter()
            .position(|&k| k >= target_addr)
            .unwrap_or(keys.len().saturating_sub(1));
        self.scroll_mem = closest_idx;
        self.status = format!("Jumped memory view to 0x{target_addr:016x}");
        self.needs_full_clear = true;
    }

    pub fn jump_mem_to_sp(&mut self) {
        self.jump_mem_to_address(self.machine.sp());
    }

    pub fn mark_changed_regs(&mut self) {
        let changes: RegisterChanges = self.machine.changes_since(&self.last_snapshot);
        for i in 0..32 {
            self.changed_x[i] = changes.x(i);
            self.changed_f[i] = changes.f(i);
        }
    }

    /// `RunCmd` chunk: bounded stepping so the TUI stays responsive.
    pub fn run_chunk(&mut self, budget: Duration) {
        let start = Instant::now();
        let mut first = true;
        while !self.machine.stop_requested() {
            let pc = self.machine.pc();
            if (!first || !self.at_breakpoint)
                && self.breakpoints.contains(&pc)
                && self.condition_met(pc)
            {
                self.running = false;
                self.was_running = false;
                self.mode = Mode::Normal;
                self.at_breakpoint = true;
                self.status = format!("Breakpoint hit at 0x{pc:016x}");
                break;
            }
            self.at_breakpoint = false;
            first = false;
            if start.elapsed() >= budget {
                break;
            }
            match self.machine.step() {
                StepOutcome::Continued => {}
                StepOutcome::Halted => {
                    self.running = false;
                    self.was_running = false;
                    self.mode = Mode::Normal;
                    break;
                }
                StepOutcome::WaitingForInput => {
                    self.mode = Mode::Input;
                    self.focus = Pane::Inp;
                    self.was_running = true;
                    self.running = false;
                    break;
                }
            }
            if self.check_watchpoints() {
                break;
            }
        }
        if self.machine.stop_requested() {
            self.machine.append_log("ERROR: Stopped");
            self.machine.clear_stop_request();
            self.running = false;
            self.was_running = false;
            self.mode = Mode::Normal;
        }
        self.machine.poll_dma(self.view.dma_out, self.view.dma_inp);
        if self.machine.input_active() && self.mode != Mode::Input {
            self.mode = Mode::Input;
            self.focus = Pane::Inp;
        }
        self.mark_changed_regs();
    }

    pub fn cancel_input(&mut self) {
        self.was_running = false;
        self.running = false;
        self.machine.cancel_input();
        self.input_line.clear();
        self.mode = Mode::Normal;
        self.focus = Pane::Listing;
    }

    pub fn submit_input(&mut self) {
        let line = std::mem::take(&mut self.input_line);
        let resume = self.was_running;
        self.was_running = false;

        if self.machine.submit_input(&line).is_err() {
            self.running = false;
            self.mode = Mode::Normal;
            self.focus = Pane::Listing;
            return;
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

    // ----- search ------------------------------------------------------

    /// `/`: open the search prompt over the source buffer.
    pub fn begin_search(&mut self) {
        self.pending_op = None;
        self.focus = Pane::Source;
        self.mode = Mode::Search;
        self.search_input.clear();
    }

    /// Enter in the search prompt.
    pub fn commit_search(&mut self) {
        self.mode = Mode::Normal;
        let needle = self.search_input.clone();
        if needle.is_empty() {
            return;
        }
        self.search_query = needle;
        self.search_step(true);
    }

    /// Repeat the last search (`Ctrl+N` / `Ctrl+P`).
    pub fn search_repeat(&mut self, forward: bool) {
        if self.search_query.is_empty() {
            self.status = "No previous search".to_string();
            return;
        }
        self.search_step(forward);
    }

    fn search_step(&mut self, forward: bool) {
        let needle = self.search_query.clone();
        if self.editor.find(&needle, forward) {
            self.status = format!(
                "{}: {} @ {}:{}",
                if forward { "Next" } else { "Prev" },
                needle,
                self.editor.row + 1,
                self.editor.col + 1
            );
        } else {
            self.status = format!("Pattern not found: {needle}");
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
                    let detected = self.machine.detect_dialect(&self.editor.text());
                    self.machine.set_dialect(detected);
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
            let (asm, input, out, timeout) = match parse_batch_entry(line) {
                None => continue,
                Some(Err(e)) => {
                    log.push_str(&format!("#{e}\n"));
                    continue;
                }
                Some(Ok(entry)) => entry,
            };
            match self.machine.run_batch_file(&asm, &input, &out, timeout) {
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
                ConfirmAction::New => self.new_file(),
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
                ConfirmAction::New => self.new_file(),
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

    pub fn new_file(&mut self) {
        self.editor.set_text("");
        self.fname.clear();
        self.editor.modified = false;
        self.focus = Pane::Source;
        self.mode = Mode::Normal;
        self.reset_sim_panes();
        self.status = "New file".to_string();
    }

    pub fn handle_new_action(&mut self) {
        if self.editor.needs_save() {
            self.modal = Modal::ConfirmSave(ConfirmAction::New);
            self.modal_input = self.fname.clone();
        } else {
            self.new_file();
        }
    }

    // ----- input events -------------------------------------------------

    pub fn request_quit(&mut self) {
        if self.editor.needs_save() {
            self.modal = Modal::ConfirmSave(ConfirmAction::Quit);
            self.modal_input = self.fname.clone();
        } else {
            self.quit_requested = true;
        }
    }

    pub fn changed_x(&self, i: usize) -> bool {
        self.changed_x.get(i).copied().unwrap_or(false)
    }

    pub fn changed_f(&self, i: usize) -> bool {
        self.changed_f.get(i).copied().unwrap_or(false)
    }

    pub fn blink_on(&self) -> bool {
        (self.tick / 4).is_multiple_of(2)
    }

    /// Dialect exposed to command handlers without importing the backend.
    pub fn dialect(&self) -> Dialect {
        self.machine.dialect()
    }

    /// `:transpile` helper: convert the buffer and switch dialect.
    pub fn transpile_buffer(&mut self) {
        let target = self.machine.dialect().toggle();
        let src = self.editor.text();
        if let Some(text) = self.machine.transpile(&src, target) {
            self.editor.replace_text(&text);
            self.machine.set_dialect(target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_batch_entry;
    use std::path::PathBuf;

    #[test]
    fn batch_entry_supports_quoted_paths() {
        let (asm, input, out, timeout) =
            parse_batch_entry(r#""dir with space/a.asm" in.txt "out file.txt" 500"#)
                .unwrap()
                .unwrap();
        assert_eq!(asm, PathBuf::from("dir with space/a.asm"));
        assert_eq!(input, PathBuf::from("in.txt"));
        assert_eq!(out, PathBuf::from("out file.txt"));
        assert_eq!(timeout, 500);
    }

    #[test]
    fn batch_entry_short_lines_are_skipped_and_bad_quotes_reported() {
        assert!(parse_batch_entry("a.asm in.txt").is_none());
        assert!(parse_batch_entry(r#""a.asm in.txt out.txt"#)
            .unwrap()
            .is_err());
    }
}
