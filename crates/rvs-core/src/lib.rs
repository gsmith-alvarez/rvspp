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
    /// `::INPprompt` — input prompt already shown for the current request.
    pub inp_prompt: bool,
    /// `::EntryLabelType` — `I`/`F`/`C`/`S`/`X` while an input is pending.
    pub entry_type: String,
    /// Pending interactive `ECALL 5..9` request.
    pub pending: Option<executor::PendingInput>,
    /// `::brk` run-interrupt flag.
    pub brk: bool,
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
        }
    }
}

impl Simulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// The default `START` symbol when no machine instruction exists.
    pub const START_DEFAULT: &'static str = "0xfffffffffffffffc";
}

/// Assembly failure carrying the reference progress text and message.
#[derive(Debug, Clone)]
pub struct AsmError {
    /// `::res` — every source line consumed up to and including the failure.
    pub progress: String,
    pub message: String,
}

impl std::fmt::Display for AsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
