//! Neutral machine interface for the RVS++ workbench.
//!
//! The TUI (`rvs-app`, `rvs-ui*`, `rvs-tui`) depends only on this crate. A
//! target-specific backend implements [`Machine`] and returns an owned
//! [`ViewState`] whose lines are already tokenised into semantic [`Tone`]s.
//! Rendering, scrolling, selection, file handling and the event loop stay
//! verbatim when the backend is replaced.
//!
//! Backends own all language knowledge: parsing, decoding, execution,
//! formatting and syntax classification. The UI owns layout and styling only.

use std::collections::BTreeSet;
use std::path::Path;

// ---------------------------------------------------------------------------
// Styled view model
// ---------------------------------------------------------------------------

/// Semantic role of a run of text. The UI maps this to a theme style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Plain,
    Secondary,
    Muted,
    Faint,
    Heading,
    Focus,
    Info,
    InfoPlain,
    Ok,
    OkPlain,
    Warn,
    WarnPlain,
    Err,
    ErrPlain,
    Label,
    Op,
    Arg1,
    Arg2,
    Arg3,
    Punct,
    Comment,
    String,
    Pc,
    Strong,
}

/// A run of text with one semantic tone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub tone: Tone,
}

impl Segment {
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, Tone::Plain)
    }
}

/// One rendered line: a sequence of segments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichLine {
    pub segments: Vec<Segment>,
}

impl RichLine {
    pub fn new(segments: Vec<Segment>) -> Self {
        Self { segments }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(vec![Segment::plain(text)])
    }

    pub fn push(&mut self, segment: Segment) {
        self.segments.push(segment);
    }
}

/// Column visibility, mirroring the reference checkbuttons.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewFlags {
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

impl Default for ViewFlags {
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

/// Assembly/notation dialect exposed to the UI as a toggleable pill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Dialect {
    #[default]
    Rvs,
    Gnu,
}

impl Dialect {
    pub fn name(self) -> &'static str {
        match self {
            Dialect::Rvs => "RVS",
            Dialect::Gnu => "GNU",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Dialect::Rvs => Dialect::Gnu,
            Dialect::Gnu => Dialect::Rvs,
        }
    }
}

/// Everything the renderers need for one frame.
#[derive(Clone, Debug, Default)]
pub struct ViewState {
    pub listing: Vec<RichLine>,
    pub registers: Vec<RichLine>,
    pub memory: Vec<RichLine>,
    pub exec: Vec<RichLine>,
    pub output: Vec<RichLine>,
    pub prompt: Option<PromptView>,
    pub dialect: Dialect,
    pub dma_out_available: bool,
    pub dma_inp_available: bool,
    pub pc: u64,
}

/// Inputs the UI supplies when asking a machine for a view.
#[derive(Clone, Debug, Default)]
pub struct ViewRequest {
    pub flags: ViewFlags,
    pub pc: u64,
    pub breakpoints: BTreeSet<u64>,
    pub changed_x: Vec<bool>,
    pub changed_f: Vec<bool>,
    pub colorize: bool,
}

// ---------------------------------------------------------------------------
// Execution and diagnostics
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepOutcome {
    Continued,
    Halted,
    WaitingForInput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadError {
    pub progress: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadOutcome {
    Ok,
    Err(LoadError),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegisterChanges {
    pub x: Vec<bool>,
    pub f: Vec<bool>,
}

impl RegisterChanges {
    pub fn x(&self, i: usize) -> bool {
        self.x.get(i).copied().unwrap_or(false)
    }

    pub fn f(&self, i: usize) -> bool {
        self.f.get(i).copied().unwrap_or(false)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MetricsView {
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
    pub cpi: u64,        // hundredths, so the view model stays Eq
    pub taken_rate: u64, // tenths of a percent
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConventionView {
    pub enabled: bool,
    pub violations: Vec<String>,
    pub depth: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotView {
    pub address: u64,
    pub value: u64,
    pub annotation: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameView {
    pub depth: usize,
    pub function_name: String,
    pub caller_name: String,
    pub call_pc: u64,
    pub target_pc: u64,
    pub frame_sp: u64,
    pub parent_sp: u64,
    pub frame_size: u64,
    pub slots: Vec<SlotView>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StackView {
    pub frames: Vec<FrameView>,
    pub sp: u64,
    pub aligned: bool,
}

/// The prompt shown by the input pane.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptView {
    pub label: String,
    pub awaiting: bool,
}

// ---------------------------------------------------------------------------
// The seam
// ---------------------------------------------------------------------------

/// A target-specific execution backend.
///
/// Defaults are no-ops so a minimal backend only implements `load`, `step`,
/// `view` and `highlight_source_line`.
pub trait Machine: Sized {
    /// Opaque state used for reverse stepping.
    type Snapshot: Clone;

    fn new() -> Self;

    /// Assemble/parse `source` into the machine. Replaces prior state.
    fn load(&mut self, source: &str) -> LoadOutcome;

    /// Reset machine state, preserving nothing (the app restores UI flags).
    fn reset(&mut self);

    /// Set the source buffer without assembling it.
    fn set_source(&mut self, _source: &str) {}

    /// Prepare for a fresh run (`START`).
    fn start(&mut self);

    fn step(&mut self) -> StepOutcome;

    /// Ask a running loop to stop.
    fn request_stop(&mut self);
    fn stop_requested(&self) -> bool;
    fn clear_stop_request(&mut self);

    fn snapshot(&self) -> Self::Snapshot;
    fn restore(&mut self, snapshot: &Self::Snapshot);
    fn changes_since(&self, snapshot: &Self::Snapshot) -> RegisterChanges;

    /// Called when the app drops a snapshot from its history ring: any
    /// resources pinned by that snapshot may be released.
    fn forget(&mut self, _snapshot: &Self::Snapshot) {}

    /// Build one frame of renderable state.
    fn view(&self, request: &ViewRequest) -> ViewState;

    /// Classify one source line for the editor pane.
    fn highlight_source_line(&self, line: &str) -> Vec<Segment>;

    fn dialect(&self) -> Dialect;
    fn set_dialect(&mut self, _dialect: Dialect) {}

    /// Guess a freshly loaded file's dialect from its own syntax.
    fn detect_dialect(&self, _source: &str) -> Dialect {
        Dialect::Rvs
    }

    fn convention_enabled(&self) -> bool {
        false
    }
    fn set_convention_enabled(&mut self, _on: bool) {}

    fn metrics(&self) -> MetricsView {
        MetricsView::default()
    }
    fn convention(&self) -> ConventionView {
        ConventionView::default()
    }
    fn stack(&self) -> StackView {
        StackView::default()
    }

    fn pc(&self) -> u64;
    fn sp(&self) -> u64 {
        0
    }
    fn reset_pc(&mut self);

    /// Value of the cell at `addr` (0 when absent), for watchpoints.
    fn peek_mem(&self, _addr: u64) -> u64 {
        0
    }

    /// Evaluate a user expression (breakpoint conditions) against current
    /// register/memory state. `None` when the expression cannot be evaluated.
    fn eval_expr(&self, _expr: &str) -> Option<i64> {
        None
    }

    /// Does the instruction at the PC write a link register (i.e. is a call)?
    fn at_call(&self) -> bool {
        false
    }

    /// Where the current function returns to, for `:finish`.
    fn return_address(&self) -> Option<u64> {
        None
    }

    /// Is the `a7` syscall convention active? (`:abi a7`)
    fn a7_abi(&self) -> bool {
        false
    }
    fn set_a7_abi(&mut self, _on: bool) {}
    fn refresh_listing(&mut self) {}

    /// Address of a source line (0-based), used for source-pane breakpoints.
    fn line_address(&self, _line: usize) -> Option<u64> {
        None
    }
    /// Closest compiled source line at or after `line`, with its address.
    fn nearest_line_address(&self, _line: usize) -> Option<(usize, u64)> {
        None
    }
    /// Address of a named symbol (data or code).
    fn symbol_address(&self, _name: &str) -> Option<u64> {
        None
    }
    /// All memory-cell addresses in ascending order.
    fn memory_addresses(&self) -> Vec<u64> {
        Vec::new()
    }

    fn input_active(&self) -> bool {
        false
    }
    fn submit_input(&mut self, _value: &str) -> Result<(), String> {
        Ok(())
    }
    fn cancel_input(&mut self);

    fn append_log(&mut self, _line: &str) {}
    fn clear_logs(&mut self) {}
    fn clear_exec_log(&mut self) {}
    fn clear_output(&mut self) {}

    fn poll_dma(&mut self, _out: bool, _inp: bool) {}

    fn dma_availability(&self) -> (bool, bool) {
        (false, false)
    }

    /// Line counts used by scroll/page commands (pre-decoration).
    fn listing_len(&self) -> usize {
        0
    }
    fn exec_len(&self) -> usize {
        0
    }
    fn output_len(&self) -> usize {
        0
    }
    fn memory_len(&self) -> usize {
        0
    }

    /// Run one headless batch entry (backend may override).
    fn run_batch_file(
        &self,
        _asm: &Path,
        _input: &Path,
        _out: &Path,
        _timeout_ms: u64,
    ) -> Result<(), String> {
        Err("batch mode is not supported by this machine".to_string())
    }

    /// Optional dialect conversion hook for `:transpile`.
    fn transpile(&mut self, _source: &str, _target: Dialect) -> Option<String> {
        None
    }
}

// ---------------------------------------------------------------------------
// Shared text helpers
// ---------------------------------------------------------------------------

/// Expand tab stops to spaces, preserving grapheme clusters.
pub fn expand_tabs(s: &str, tab_width: usize) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    let mut res = String::new();
    let mut col = 0usize;
    for grapheme in s.graphemes(true) {
        if grapheme == "\r" {
            continue;
        }
        if grapheme == "\t" {
            let spaces = tab_width - (col % tab_width);
            res.extend(std::iter::repeat_n(' ', spaces));
            col += spaces;
        } else {
            res.push_str(grapheme);
            col += UnicodeWidthStr::width(grapheme);
        }
    }
    res
}

/// Visual column of a character index, accounting for tabs and wide glyphs.
pub fn visual_col_at(line: &str, char_col: usize, tab_width: usize) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    let mut col = 0usize;
    let mut seen = 0usize;
    for grapheme in line.graphemes(true) {
        let chars = grapheme.chars().count();
        if seen >= char_col || seen + chars > char_col {
            return col;
        }
        if grapheme == "\r" {
            // Zero-width control character.
        } else if grapheme == "\t" {
            col += tab_width - (col % tab_width);
        } else {
            col += UnicodeWidthStr::width(grapheme);
        }
        seen += chars;
    }
    if char_col > seen {
        col += char_col - seen;
    }
    col
}

/// Parse a hex (`0x`), binary (`0b`) or decimal address without Tcl semantics.
pub fn parse_u64(s: &str) -> Option<u64> {
    let t = s.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()
    } else if let Some(b) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
        u64::from_str_radix(b, 2).ok()
    } else {
        t.parse::<u64>().ok()
    }
}
