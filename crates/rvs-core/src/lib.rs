//! rvs-core: pure, UI-independent RVS (RISC-V Visual Simulator) engine.
//!
//! A faithful Rust port of the reverse-engineered `RVS046.tcl` simulator:
//! two-pass assembler with RVS directives, sparse segmented memory,
//! RV64IMFD execution, `ECALL` syscalls 0-9, DMA memory-mapped I/O at
//! `0x0`/`0x8`/`0x10`/`0x18` and the exact `kkbatch` report format.

pub mod assembler;
pub mod executor;
pub mod format;
pub mod memory;
pub mod registers;
pub mod tcl;
pub mod transpiler;

use std::collections::{BTreeMap, VecDeque};

use memory::Memory;
use registers::RegisterFile;

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
        if sp % 16 != 0 {
            let msg = format!("Stack pointer sp is not 16-byte aligned at call 0x{:016x} (sp = 0x{:016x})", call_pc, sp);
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
                let abi = crate::registers::abi_name(idx);
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
    pub pending: Option<executor::PendingInput>,
    /// `::brk` run-interrupt flag.
    pub brk: bool,
    /// Mapping from source line index (0-based) to instruction/data address.
    pub line_to_addr: BTreeMap<usize, u64>,
    /// Mapping from instruction/data address to source line index (0-based).
    pub addr_to_line: BTreeMap<u64, usize>,
    /// Dynamic execution metrics & hardware counters.
    pub metrics: ExecutionMetrics,
    /// Calling convention compliance checker.
    pub convention: ConventionChecker,
    /// Assembly dialect mode (RVS or GNU).
    pub dialect: transpiler::AssemblyDialect,
}

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
            line_to_addr: BTreeMap::new(),
            addr_to_line: BTreeMap::new(),
            metrics: ExecutionMetrics::default(),
            convention: ConventionChecker::default(),
            dialect: transpiler::AssemblyDialect::default(),
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
        let hex_addr = tcl::a64(addr);
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
                                let abi = crate::registers::abi_name(ConventionChecker::SAVED_X_INDICES[idx]);
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
