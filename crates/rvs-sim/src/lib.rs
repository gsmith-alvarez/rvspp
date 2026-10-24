//! rvs-sim: UI-independent simulator state and simulator-level semantics.
//!
//! Holds the `Simulator` state mirroring the Tcl globals, the view/metrics
//! configuration, the RISC-V calling-convention checker, and the shared
//! register/immediate argument accessors used by the executor crates.

use std::collections::{BTreeMap, VecDeque};

use rvs_memory::{Memory, SimError, SimResult};
use rvs_regs::{parse_reg, RegisterFile};
use rvs_tcl as tcl;

/// Which syscall convention `ecall` uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EcallAbi {
    /// The course dialect: `ecall rd, rs1, imm` with immediates 0-9.
    #[default]
    Rvs,
    /// Standard educational RISC-V: `a7` selects the call, `a0`/`a1` carry
    /// arguments (the RARS/GNU set). Makes GNU-mode programs actually run.
    A7,
}

/// View flags matching every checkbutton in `RVS046.tcl`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewConfig {
    pub list_bin: bool,
    pub list_hex: bool,
    pub list_int: bool,
    pub list_txt: bool,
    pub exec_txt: bool,
    pub reg_hex: bool,
    pub reg_int: bool,
    pub reg_flp: bool,
    pub mem_hex: bool,
    pub mem_int: bool,
    pub mem_flp: bool,
    pub mem_txt: bool,
    pub dma_out: bool,
    pub dma_inp: bool,
}

impl Default for ViewConfig {
    /// The reference defaults: every text pane fully enabled, DMA polling
    /// off until the user enables it (`::CheckbuttonOUTVar`/`INPVar`).
    fn default() -> Self {
        Self {
            list_bin: true,
            list_hex: true,
            list_int: true,
            list_txt: true,
            exec_txt: true,
            reg_hex: true,
            reg_int: true,
            reg_flp: true,
            mem_hex: true,
            mem_int: true,
            mem_flp: true,
            mem_txt: true,
            dma_out: false,
            dma_inp: false,
        }
    }
}
/// Dynamic execution metrics and hardware counter statistics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutionMetrics {
    pub instructions: u64,
    pub cycles: u64,
    pub alu_ops: u64,
    pub branches_total: u64,
    pub branches_taken: u64,
    pub jumps: u64,
    pub loads: u64,
    pub stores: u64,
    pub floats: u64,
    pub ecalls: u64,
}

impl ExecutionMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn cpi(&self) -> f64 {
        if self.instructions == 0 {
            0.0
        } else {
            self.cycles as f64 / self.instructions as f64
        }
    }

    pub fn branch_taken_rate(&self) -> f64 {
        if self.branches_total == 0 {
            0.0
        } else {
            (self.branches_taken as f64 / self.branches_total as f64) * 100.0
        }
    }
}

/// A recorded stack frame for calling convention linting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallFrame {
    pub call_pc: u64,
    pub target_pc: u64,
    pub caller_name: String,
    pub function_name: String,
    pub entry_sp: u64,
    pub saved_x: [u64; 12],
    pub saved_f: [u64; 12],
}

/// Single memory slot inspection inside an active stack frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackSlot {
    pub address: u64,
    pub value: u64,
    pub annotation: String,
}

/// High-level stack frame structure for visualization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackFrameInfo {
    pub depth: usize,
    pub function_name: String,
    pub caller_name: String,
    pub call_pc: u64,
    pub target_pc: u64,
    pub frame_sp: u64,
    pub parent_sp: u64,
    pub frame_size: u64,
    pub slots: Vec<StackSlot>,
}

/// Standard RISC-V calling convention checker / linter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConventionChecker {
    pub enabled: bool,
    pub call_stack: Vec<CallFrame>,
    pub violations: Vec<String>,
}

impl ConventionChecker {
    pub const SAVED_X_INDICES: [u8; 12] = [8, 9, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27];

    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.call_stack.clear();
        self.violations.clear();
    }

    pub fn on_call(
        &mut self,
        call_pc: u64,
        target_pc: u64,
        caller_name: String,
        function_name: String,
        regs: &RegisterFile,
    ) -> Option<String> {
        let sp = regs.read_x(2);
        let mut warning = None;
        if !sp.is_multiple_of(16) {
            let msg = format!(
                "Stack pointer sp is not 16-byte aligned at call 0x{:016x} (sp = 0x{:016x})",
                call_pc, sp
            );
            self.violations.push(msg.clone());
            warning = Some(msg);
        }

        let mut saved_x = [0u64; 12];
        for (i, &idx) in Self::SAVED_X_INDICES.iter().enumerate() {
            saved_x[i] = regs.read_x(idx);
        }
        let mut saved_f = [0u64; 12];
        for (i, &idx) in Self::SAVED_X_INDICES.iter().enumerate() {
            saved_f[i] = regs.read_f_bits(idx);
        }

        self.call_stack.push(CallFrame {
            call_pc,
            target_pc,
            caller_name,
            function_name,
            entry_sp: sp,
            saved_x,
            saved_f,
        });

        warning
    }

    pub fn on_return(&mut self, ret_pc: u64, regs: &RegisterFile) -> Vec<String> {
        let mut warnings = Vec::new();
        let Some(frame) = self.call_stack.pop() else {
            return warnings;
        };

        let sp = regs.read_x(2);
        if sp != frame.entry_sp {
            let msg = format!(
                "Stack pointer sp was not restored upon return at 0x{:016x} (entered at 0x{:016x}, returned at 0x{:016x})",
                ret_pc, frame.entry_sp, sp
            );
            self.violations.push(msg.clone());
            warnings.push(msg);
        }

        for (i, &idx) in Self::SAVED_X_INDICES.iter().enumerate() {
            let cur = regs.read_x(idx);
            let prev = frame.saved_x[i];
            if cur != prev {
                let abi = rvs_regs::abi_name(idx);
                let msg = format!(
                    "Saved register {} was modified and not restored before return at 0x{:016x} (entered: 0x{:016x}, returned: 0x{:016x})",
                    abi, ret_pc, prev, cur
                );
                self.violations.push(msg.clone());
                warnings.push(msg);
            }
        }

        for (i, &idx) in Self::SAVED_X_INDICES.iter().enumerate() {
            let cur = regs.read_f_bits(idx);
            let prev = frame.saved_f[i];
            if cur != prev {
                let msg = format!(
                    "Saved float register f{} was modified and not restored before return at 0x{:016x} (entered: 0x{:016x}, returned: 0x{:016x})",
                    idx, ret_pc, prev, cur
                );
                self.violations.push(msg.clone());
                warnings.push(msg);
            }
        }

        warnings
    }
}

/// The complete simulator state, mirroring the Tcl globals.
#[derive(Debug)]
pub struct Simulator {
    pub regs: RegisterFile,
    pub mem: Memory,
    pub sym: BTreeMap<String, String>,
    pub view: ViewConfig,
    /// `TextSource` content.
    pub source: String,
    /// `TextListing` content.
    pub listing: String,
    /// `TextExecution` content (grows during runs).
    pub exec_log: String,
    /// `TextOutput` content.
    pub output: String,
    /// Batch input queue (`::inl`); `None` outside batch mode.
    pub input_lines: Option<VecDeque<String>>,
    /// `::INPprompt`: input prompt already shown for the current request.
    pub inp_prompt: bool,
    /// `::EntryLabelType`: `I`/`F`/`C`/`S`/`X` while an input is pending.
    pub entry_type: String,
    /// Pending interactive `ECALL 5..9` request.
    pub pending: Option<PendingInput>,
    /// `::brk` run-interrupt flag.
    pub brk: bool,
    /// Append a trace line per retired instruction. Off is a batch/`--no-trace`
    /// optimisation only: the report is then empty in its `Run` section.
    pub trace: bool,
    /// First execution error of the current run (`None` after a clean halt).
    pub last_error: Option<String>,
    /// Mapping from source line index (0-based) to instruction/data address.
    pub line_to_addr: BTreeMap<usize, u64>,
    /// Mapping from instruction/data address to source line index (0-based).
    pub addr_to_line: BTreeMap<u64, usize>,
    /// Dynamic execution metrics & hardware counters.
    pub metrics: ExecutionMetrics,
    /// Calling convention compliance checker.
    pub convention: ConventionChecker,
    /// Assembly dialect mode (RVS or GNU).
    pub dialect: rvs_transpile::AssemblyDialect,
    /// Syscall convention used by `ecall`.
    pub ecall_abi: EcallAbi,
    /// Next free address for `sbrk` (a7 ABI only).
    pub heap_break: u64,
    /// Set by an exit syscall; `step` reports it as a halt.
    pub exit_requested: bool,
    /// Exit status passed to the exit syscall, when any.
    pub exit_code: i64,
}

/// First address handed out by `sbrk` in the a7 ABI (page-aligned, far away
/// from the usual code/data/stack addresses).
pub const HEAP_BASE: u64 = 0x1000_0000;

impl Default for Simulator {
    fn default() -> Self {
        Self {
            regs: RegisterFile::new(),
            mem: Memory::new(),
            sym: BTreeMap::new(),
            view: ViewConfig::default(),
            source: String::new(),
            listing: String::new(),
            exec_log: String::new(),
            output: String::new(),
            input_lines: None,
            inp_prompt: true,
            entry_type: String::new(),
            pending: None,
            brk: false,
            trace: true,
            last_error: None,
            line_to_addr: BTreeMap::new(),
            addr_to_line: BTreeMap::new(),
            metrics: ExecutionMetrics::default(),
            convention: ConventionChecker::default(),
            dialect: rvs_transpile::AssemblyDialect::default(),
            ecall_abi: EcallAbi::default(),
            heap_break: HEAP_BASE,
            exit_requested: false,
            exit_code: 0,
        }
    }
}

impl Simulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// The default `START` symbol when no machine instruction exists.
    pub const START_DEFAULT: &'static str = "0xfffffffffffffffc";

    /// Resolves the nearest symbol or function name for an instruction address.
    pub fn find_symbol_name(&self, addr: u64) -> String {
        let hex_addr = rvs_tcl::a64(addr);
        for (sym, target) in &self.sym {
            if target == &hex_addr && sym != "START" {
                return sym.clone();
            }
        }
        if let Some(target) = self.sym.get("START") {
            if target == &hex_addr {
                return "START".to_string();
            }
        }
        format!("sub_0x{:x}", addr)
    }

    /// Inspects the active stack frames based on `call_stack` and current `$sp`.
    pub fn get_stack_frames(&self) -> Vec<StackFrameInfo> {
        let mut frames = Vec::new();
        let cur_sp = self.regs.read_x(2);

        for (i, frame) in self.convention.call_stack.iter().enumerate() {
            let next_sp = if i + 1 < self.convention.call_stack.len() {
                self.convention.call_stack[i + 1].entry_sp
            } else {
                cur_sp
            };

            let frame_sp = std::cmp::min(frame.entry_sp, next_sp);
            let parent_sp = std::cmp::max(frame.entry_sp, next_sp);
            let frame_size = parent_sp.saturating_sub(frame_sp);

            let mut slots = Vec::new();
            if frame_size > 0 && frame_size <= 512 {
                let mut addr = parent_sp.saturating_sub(8);
                while addr >= frame_sp {
                    let val = self.mem.read_bytes(addr, 8).unwrap_or(0);
                    let mut annotation = String::from("Local / arg");

                    if val == frame.call_pc.wrapping_add(4) {
                        annotation = format!("Saved ra (ret 0x{:x})", val);
                    } else {
                        for (idx, &reg_val) in frame.saved_x.iter().enumerate() {
                            if val == reg_val && val != 0 {
                                let abi =
                                    rvs_regs::abi_name(ConventionChecker::SAVED_X_INDICES[idx]);
                                annotation = format!("Saved {}", abi);
                                break;
                            }
                        }
                    }

                    slots.push(StackSlot {
                        address: addr,
                        value: val,
                        annotation,
                    });

                    if addr < 8 || addr == frame_sp {
                        break;
                    }
                    addr -= 8;
                }
            }

            frames.push(StackFrameInfo {
                depth: i,
                function_name: frame.function_name.clone(),
                caller_name: frame.caller_name.clone(),
                call_pc: frame.call_pc,
                target_pc: frame.target_pc,
                frame_sp,
                parent_sp,
                frame_size,
                slots,
            });
        }

        frames
    }
}

/// Assembly failure carrying the reference progress text and message.
#[derive(Debug, Clone)]
pub struct AsmError {
    /// `::res`: every source line consumed up to and including the failure.
    pub progress: String,
    pub message: String,
}

impl std::fmt::Display for AsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

// ----- shared register/immediate argument accessors -----

pub fn reg_index(arg: &str) -> SimResult<u8> {
    parse_reg(arg).ok_or_else(|| {
        SimError(format!(
            "can't read \"::{}\": no such variable",
            arg.chars().next().unwrap_or('x')
        ))
    })
}

/// Read `::x{idx}` for an argument (the reference indexes the x-file through
/// `reg2i` regardless of the argument's prefix).
pub fn read_x_arg(sim: &Simulator, arg: &str) -> SimResult<u64> {
    let idx = reg_index(arg)?;
    Ok(sim.regs.read_x(idx))
}

/// Read `::{prefix}{idx}`: the prefix comes from the argument itself, so
/// `f3` reads the FP register and `x3` the integer register.
pub fn read_reg_arg(sim: &Simulator, arg: &str) -> SimResult<u64> {
    let idx = reg_index(arg)?;
    if arg.to_ascii_lowercase().starts_with('f') {
        Ok(sim.regs.read_f_bits(idx))
    } else {
        Ok(sim.regs.read_x(idx))
    }
}

pub fn write_x(sim: &mut Simulator, arg: &str, val: u64, upper: bool) -> SimResult<()> {
    let idx = reg_index(arg)?;
    sim.regs.write_x(idx, val, upper);
    Ok(())
}

pub fn write_reg_arg(sim: &mut Simulator, arg: &str, val: u64, upper: bool) -> SimResult<()> {
    let idx = reg_index(arg)?;
    if arg.to_ascii_lowercase().starts_with('f') {
        sim.regs.write_f_bits(idx, val, upper);
    } else {
        sim.regs.write_x(idx, val, upper);
    }
    Ok(())
}

/// The register's display string (case preserved), as Tcl would hold it.
pub fn reg_string_of(sim: &Simulator, arg: &str) -> String {
    match parse_reg(arg) {
        Some(idx) if arg.to_ascii_lowercase().starts_with('f') => sim.regs.f_string(idx),
        Some(idx) => sim.regs.x_string(idx),
        None => "0x0000000000000000".to_string(),
    }
}

/// `x12i` over the hex immediate string held in the memory cell.
pub fn imm12(arg: &str) -> SimResult<i64> {
    let v = tcl::parse_int(arg)
        .ok_or_else(|| SimError(format!("expected integer but got \"{arg}\"")))?;
    Ok(tcl::x12i(v as i64 as u64))
}

/// Raw immediate used by `JALR`, `FLD`, `FSD` (no sign extension: the
/// reference passes the hex string straight into `expr`).
pub fn imm_raw(arg: &str) -> SimResult<u64> {
    let v = tcl::parse_int(arg)
        .ok_or_else(|| SimError(format!("expected integer but got \"{arg}\"")))?;
    Ok(v as i64 as u64)
}

pub fn imm20(arg: &str) -> SimResult<i64> {
    let v = tcl::parse_int(arg)
        .ok_or_else(|| SimError(format!("expected integer but got \"{arg}\"")))?;
    Ok(tcl::x20i(v as i64 as u64))
}

// ----- pending interactive input request -----

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingInput {
    pub rd_arg: String,
    pub rs1_arg: String,
    pub imm: u64,
    pub inst_pc: u64,
}
