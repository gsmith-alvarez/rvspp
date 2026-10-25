//! RV64IMFD backend for the neutral [`rvs_machine::Machine`] interface.
//!
//! Everything RISC-V-specific lives here: assembling, stepping, DMA/ECALL
//! input, formatting and selection classification. The TUI never imports this
//! crate directly; it receives [`rvs_machine::ViewState`] values.

use rvs_machine::{
    expand_tabs, ConventionView, Dialect, FrameView, LoadError, LoadOutcome, Machine, MetricsView,
    PromptView, RegisterChanges, RichLine, Segment, SlotView, StackView, StepOutcome, Tone,
    ViewRequest, ViewState,
};
use rvs_memory::MemoryCell;
use rvs_sim::Simulator;

/// The RISC-V backend.
pub struct RiscV {
    sim: Simulator,
}

/// Cut a growing log back to a recorded byte length (no-op when it is already
/// shorter, which happens after a partial restore).
fn truncate_to(s: &mut String, len: usize) {
    if s.len() > len {
        s.truncate(len);
    }
}

/// Append one expression token, substituting registers with their current
/// value. Hex literals (`0x1000`) are left alone.
fn push_expr_token(out: &mut String, token: &str, m: &RiscV) {
    if token.is_empty() {
        return;
    }
    let is_hex_digits = token.starts_with("0x") || token.starts_with("0X");
    match rvs_regs::parse_reg(token) {
        Some(idx) if !is_hex_digits => {
            out.push_str(&m.sim.regs.read_x(idx).to_string());
        }
        _ => out.push_str(token),
    }
}

/// Reverse-step state: registers plus a mark into the memory undo journal.
/// Memory and the trace are not copied — that is what keeps stepping O(1) in
/// program size instead of O(state).
#[derive(Clone)]
pub struct RiscVSnapshot {
    regs: rvs_regs::RegisterFile,
    mem_mark: u64,
    exec_log_len: usize,
    output_len: usize,
    pc: u64,
}

impl RiscV {
    pub fn new() -> Self {
        Self {
            sim: Simulator::new(),
        }
    }

    fn sym_address(&self, name: &str) -> Option<u64> {
        self.sim
            .sym
            .get(name)
            .and_then(|s| rvs_tcl::parse_int(s))
            .map(|v| v as u64)
    }
}

impl RiscV {
    /// Read-only backend state (tests, diagnostics).
    pub fn sim(&self) -> &Simulator {
        &self.sim
    }

    /// Mutable backend state (tests, diagnostics).
    pub fn sim_mut(&mut self) -> &mut Simulator {
        &mut self.sim
    }
}

impl Default for RiscV {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine for RiscV {
    type Snapshot = RiscVSnapshot;

    fn new() -> Self {
        Self::new()
    }

    fn load(&mut self, source: &str) -> LoadOutcome {
        self.sim.inp_prompt = false;
        self.sim.brk = false;
        self.sim.pending = None;
        self.sim.entry_type.clear();
        self.sim.convention.reset();
        self.sim.metrics.reset();
        let outcome = match rvs_assembler::assemble(&mut self.sim, source) {
            Ok(listing) => {
                self.sim.listing = format!("{listing}\n");
                LoadOutcome::Ok
            }
            Err(err) => {
                self.sim
                    .sym
                    .insert("START".to_string(), Simulator::START_DEFAULT.to_string());
                self.sim.regs.pc = rvs_tcl::parse_int(Simulator::START_DEFAULT).unwrap_or(0) as u64;
                self.sim.listing = format!("{}ERROR: {}\n\n", err.progress, err.message);
                LoadOutcome::Err(LoadError {
                    progress: err.progress,
                    message: err.message,
                })
            }
        };
        self.sim.source = source.to_string();
        self.sim.exec_log.clear();
        self.sim.output.clear();
        // A compile is not an undoable step: start a fresh journal so reverse
        // stepping only ever rewinds execution of the current program.
        self.sim.mem.start_journal();
        outcome
    }

    fn reset(&mut self) {
        self.sim = Simulator::new();
    }

    fn set_source(&mut self, source: &str) {
        self.sim.source = source.to_string();
    }

    fn start(&mut self) {
        self.reset_pc();
        self.sim.exec_log.push_str("START\n");
        self.sim.inp_prompt = false;
        self.sim.convention.reset();
        self.sim.metrics.reset();
    }

    fn step(&mut self) -> StepOutcome {
        match rvs_executor::step(&mut self.sim) {
            rvs_executor::StepResult::Continued => StepOutcome::Continued,
            rvs_executor::StepResult::Halted => StepOutcome::Halted,
            rvs_executor::StepResult::WaitingForInput => StepOutcome::WaitingForInput,
        }
    }

    fn request_stop(&mut self) {
        self.sim.brk = true;
    }

    fn stop_requested(&self) -> bool {
        self.sim.brk
    }

    fn clear_stop_request(&mut self) {
        self.sim.brk = false;
    }

    fn snapshot(&self) -> Self::Snapshot {
        RiscVSnapshot {
            regs: self.sim.regs.clone(),
            mem_mark: self.sim.mem.journal_mark(),
            exec_log_len: self.sim.exec_log.len(),
            output_len: self.sim.output.len(),
            pc: self.sim.regs.pc,
        }
    }

    fn restore(&mut self, snapshot: &Self::Snapshot) {
        self.sim.regs = snapshot.regs.clone();
        self.sim.mem.journal_rewind_to(snapshot.mem_mark);
        truncate_to(&mut self.sim.exec_log, snapshot.exec_log_len);
        truncate_to(&mut self.sim.output, snapshot.output_len);
        self.sim.regs.pc = snapshot.pc;
    }

    fn forget(&mut self, snapshot: &Self::Snapshot) {
        // The app dropped this snapshot out of its ring buffer: the entries it
        // would have rewound to can never be needed again.
        self.sim.mem.journal_drop_before(snapshot.mem_mark);
    }

    fn changes_since(&self, snapshot: &Self::Snapshot) -> RegisterChanges {
        let x = self.sim.regs.x_values();
        let f = self.sim.regs.f_values();
        let old_x = snapshot.regs.x_values();
        let old_f = snapshot.regs.f_values();
        RegisterChanges {
            x: (0..32).map(|i| x[i] != old_x[i]).collect(),
            f: (0..32).map(|i| f[i] != old_f[i]).collect(),
        }
    }

    fn view(&self, request: &ViewRequest) -> ViewState {
        let (dma_out, dma_inp) = self.dma_availability();
        ViewState {
            listing: self.build_listing(request),
            registers: self.build_registers(request),
            memory: self.build_memory(request),
            exec: self.build_exec(),
            output: self
                .sim
                .output
                .lines()
                .map(|l| RichLine::plain(expand_tabs(l, 8)))
                .collect(),
            prompt: self.prompt(),
            dialect: self.dialect(),
            dma_out_available: dma_out,
            dma_inp_available: dma_inp,
            pc: self.sim.regs.pc,
        }
    }

    fn highlight_source_line(&self, line: &str) -> Vec<Segment> {
        rvs_syntax::highlight_source_line(line)
    }

    fn dialect(&self) -> Dialect {
        match self.sim.dialect {
            rvs_transpile::AssemblyDialect::Rvs => Dialect::Rvs,
            rvs_transpile::AssemblyDialect::Gnu => Dialect::Gnu,
        }
    }

    fn set_dialect(&mut self, dialect: Dialect) {
        self.sim.dialect = match dialect {
            Dialect::Rvs => rvs_transpile::AssemblyDialect::Rvs,
            Dialect::Gnu => rvs_transpile::AssemblyDialect::Gnu,
        };
    }

    fn detect_dialect(&self, source: &str) -> Dialect {
        match rvs_transpile::detect_dialect(source) {
            rvs_transpile::AssemblyDialect::Rvs => Dialect::Rvs,
            rvs_transpile::AssemblyDialect::Gnu => Dialect::Gnu,
        }
    }

    fn peek_mem(&self, addr: u64) -> u64 {
        match self.sim.mem.cells.get(&(addr / 8 * 8)) {
            Some(rvs_memory::MemoryCell::Data { value, .. }) => *value,
            _ => 0,
        }
    }

    fn eval_expr(&self, expr: &str) -> Option<i64> {
        // Registers first (`x1`, `a0`, `t3` ...), then labels, then Tcl `expr`
        // semantics — so `:b loop if a0 == 3` works like a debugger condition.
        let mut expanded = String::with_capacity(expr.len() + 16);
        let mut token = String::new();
        for c in expr.chars() {
            if c.is_alphanumeric() || c == '_' || c == '.' {
                token.push(c);
                continue;
            }
            push_expr_token(&mut expanded, &token, self);
            token.clear();
            expanded.push(c);
        }
        push_expr_token(&mut expanded, &token, self);
        rvs_asm_syntax::slbl(&expanded, &self.sim.sym).and_then(|v| i64::try_from(v).ok())
    }

    fn at_call(&self) -> bool {
        let pc = self.sim.regs.pc;
        match self.sim.mem.cells.get(&pc) {
            Some(rvs_memory::MemoryCell::Instruction(c)) => {
                let op = c.op.to_ascii_uppercase();
                (op == "JAL" || op == "JALR")
                    && c.args
                        .first()
                        .map(|r| r != "x0" && r != "zero")
                        .unwrap_or(false)
            }
            _ => false,
        }
    }

    fn return_address(&self) -> Option<u64> {
        if let Some(frame) = self.sim.convention.call_stack.last() {
            return Some(frame.call_pc.wrapping_add(4));
        }
        let ra = self.sim.regs.read_x(1);
        (ra != 0).then_some(ra)
    }

    fn a7_abi(&self) -> bool {
        self.sim.ecall_abi == rvs_sim::EcallAbi::A7
    }

    fn set_a7_abi(&mut self, on: bool) {
        self.sim.ecall_abi = if on {
            rvs_sim::EcallAbi::A7
        } else {
            rvs_sim::EcallAbi::Rvs
        };
    }

    fn convention_enabled(&self) -> bool {
        self.sim.convention.enabled
    }

    fn set_convention_enabled(&mut self, on: bool) {
        self.sim.convention.enabled = on;
    }

    fn metrics(&self) -> MetricsView {
        let m = &self.sim.metrics;
        MetricsView {
            instructions: m.instructions,
            cycles: m.cycles,
            alu_ops: m.alu_ops,
            branches_total: m.branches_total,
            branches_taken: m.branches_taken,
            jumps: m.jumps,
            loads: m.loads,
            stores: m.stores,
            floats: m.floats,
            ecalls: m.ecalls,
            cpi: (m.cpi() * 100.0).round() as u64,
            taken_rate: (m.branch_taken_rate() * 10.0).round() as u64,
        }
    }

    fn convention(&self) -> ConventionView {
        ConventionView {
            enabled: self.sim.convention.enabled,
            violations: self.sim.convention.violations.clone(),
            depth: self.sim.convention.call_stack.len(),
        }
    }

    fn stack(&self) -> StackView {
        let sp = self.sim.regs.read_x(2);
        StackView {
            frames: self
                .sim
                .get_stack_frames()
                .into_iter()
                .map(|f| FrameView {
                    depth: f.depth,
                    function_name: f.function_name,
                    caller_name: f.caller_name,
                    call_pc: f.call_pc,
                    target_pc: f.target_pc,
                    frame_sp: f.frame_sp,
                    parent_sp: f.parent_sp,
                    frame_size: f.frame_size,
                    slots: f
                        .slots
                        .into_iter()
                        .map(|s| SlotView {
                            address: s.address,
                            value: s.value,
                            annotation: s.annotation,
                        })
                        .collect(),
                })
                .collect(),
            sp,
            aligned: sp.is_multiple_of(16),
        }
    }

    fn pc(&self) -> u64 {
        self.sim.regs.pc
    }

    fn sp(&self) -> u64 {
        self.sim.regs.read_x(2)
    }

    fn reset_pc(&mut self) {
        let pc = self
            .sim
            .sym
            .get("START")
            .and_then(|s| rvs_tcl::parse_int(s))
            .map(|v| v as u64)
            .unwrap_or(0);
        self.sim.regs.pc = pc;
    }

    fn refresh_listing(&mut self) {
        if !self.sim.listing.is_empty() && !self.sim.listing.contains("ERROR:") {
            self.sim.listing = format!("{}\n", rvs_assembler::dump(&mut self.sim));
        }
    }

    fn line_address(&self, line: usize) -> Option<u64> {
        self.sim.line_to_addr.get(&line).copied()
    }

    fn nearest_line_address(&self, line: usize) -> Option<(usize, u64)> {
        if let Some((&l, &addr)) = self.sim.line_to_addr.get(&line).map(|a| (&line, a)) {
            return Some((l, addr));
        }
        if let Some((&l, &addr)) = self.sim.line_to_addr.range(line..).next() {
            return Some((l, addr));
        }
        self.sim
            .line_to_addr
            .iter()
            .next_back()
            .map(|(l, a)| (*l, *a))
    }

    fn symbol_address(&self, name: &str) -> Option<u64> {
        self.sym_address(name)
    }

    fn memory_addresses(&self) -> Vec<u64> {
        self.sim.mem.cells.keys().copied().collect()
    }

    fn input_active(&self) -> bool {
        self.sim.pending.is_some() || !self.sim.entry_type.is_empty()
    }

    fn submit_input(&mut self, value: &str) -> Result<(), String> {
        if self.sim.pending.is_some() {
            let inst_pc = self.sim.pending.as_ref().map(|p| p.inst_pc).unwrap_or(0);
            if let Err(e) = rvs_ecall::submit_ecall_input(&mut self.sim, value) {
                self.sim.regs.pc = inst_pc;
                self.sim.exec_log.push_str(&format!("ERROR: {}\n", e.0));
                return Err(e.0);
            }
        } else if !self.sim.entry_type.is_empty() {
            if let Err(e) = rvs_ecall::submit_dma_input(&mut self.sim, value) {
                self.sim.exec_log.push_str(&format!("ERROR: {}\n", e.0));
                return Err(e.0);
            }
        }
        Ok(())
    }

    fn cancel_input(&mut self) {
        if self.sim.pending.take().is_some() {
            self.sim.output.push_str("Cancelled\n");
            self.sim.exec_log.push_str("ERROR: Cancelled\n");
        }
        self.sim.entry_type.clear();
        self.sim.inp_prompt = false;
    }

    fn append_log(&mut self, line: &str) {
        self.sim.exec_log.push_str(line);
    }

    fn clear_logs(&mut self) {
        self.sim.exec_log.clear();
        self.sim.output.clear();
    }

    fn clear_exec_log(&mut self) {
        self.sim.exec_log.clear();
    }

    fn clear_output(&mut self) {
        self.sim.output.clear();
    }

    fn poll_dma(&mut self, out: bool, inp: bool) {
        self.sim.view.dma_out = out;
        self.sim.view.dma_inp = inp;
        if out {
            rvs_ecall::poll_dma_output(&mut self.sim);
        }
        if inp {
            rvs_ecall::poll_dma_input(&mut self.sim);
        }
    }

    fn dma_availability(&self) -> (bool, bool) {
        (
            self.sim.sym.contains_key("OUTC") && self.sim.sym.contains_key("OUTD"),
            self.sim.sym.contains_key("INPC") && self.sim.sym.contains_key("INPD"),
        )
    }

    fn listing_len(&self) -> usize {
        self.sim.listing.lines().count()
    }

    fn exec_len(&self) -> usize {
        self.sim.exec_log.lines().count()
    }

    fn output_len(&self) -> usize {
        self.sim.output.lines().count()
    }

    fn memory_len(&self) -> usize {
        self.sim.mem.cells.len()
    }

    fn run_batch_file(
        &self,
        asm: &std::path::Path,
        input: &std::path::Path,
        out: &std::path::Path,
        timeout_ms: u64,
    ) -> Result<(), String> {
        rvs_format::run_batch_file(asm, input, out, timeout_ms, rvs_format::BatchOptions::new())
            .map(|_| ())
    }

    fn transpile(&mut self, source: &str, target: Dialect) -> Option<String> {
        // `target` names the destination format; the buffer is assumed to be
        // in the other dialect, matching `:transpile <target>` semantics.
        Some(match target {
            Dialect::Gnu => rvs_transpile::transpile_rvs_to_gnu(source),
            Dialect::Rvs => rvs_transpile::transpile_gnu_to_rvs(source),
        })
    }
}

// ---------------------------------------------------------------------------
// View builders
// ---------------------------------------------------------------------------

impl RiscV {
    fn prompt(&self) -> Option<PromptView> {
        let label = if let Some(p) = &self.sim.pending {
            match p.imm {
                5 => "I?",
                6 => "F?",
                7 => "X?",
                8 => "C?",
                9 => "S?",
                _ => "?",
            }
        } else if !self.sim.entry_type.is_empty() {
            match self.sim.entry_type.as_str() {
                "I" => "I?",
                "F" => "F?",
                "C" => "C?",
                "S" => "S?",
                _ => "?",
            }
        } else {
            return None;
        };
        Some(PromptView {
            label: label.to_string(),
            awaiting: true,
        })
    }

    #[expect(
        clippy::cognitive_complexity,
        reason = "listing formatter handles tabs, errors, highlights, and source-tracking"
    )]
    fn build_listing(&self, req: &ViewRequest) -> Vec<RichLine> {
        let mut lines = Vec::new();
        if self.sim.listing.is_empty() || self.sim.listing.contains("ERROR:") {
            for line_str in self.sim.listing.lines() {
                let clean = expand_tabs(line_str, 8);
                if clean.contains("ERROR:") {
                    lines.push(rich(clean, Tone::Err));
                } else {
                    lines.push(RichLine::new(self.highlight_source_line(&clean)));
                }
            }
            return lines;
        }

        lines.push(rich("ASSEMBLY LISTING", Tone::Heading));
        let mut cols = vec![("  ADDRESS", 20usize)];
        if req.flags.list_bin {
            cols.push(("BIN / HEX CODE", 40));
        }
        if req.flags.list_hex {
            cols.push(("HEX OPERANDS", 23));
        }
        if req.flags.list_int {
            cols.push(("INT OPERANDS", 23));
        }
        if req.flags.list_txt {
            cols.push(("TEXT SOURCE", 34));
        }
        let (hdr, div) = build_table_header(&cols);
        lines.push(hdr);
        lines.push(div);

        for (&addr, cell) in self.sim.mem.cells.iter() {
            let is_pc = addr == self.sim.regs.pc;
            let has_bp = req.breakpoints.contains(&addr);
            let mut segs = Vec::new();
            segs.push(Segment::new(
                if has_bp { "● " } else { "  " },
                if has_bp { Tone::Err } else { Tone::Plain },
            ));
            segs.push(Segment::new(
                format!("0x{addr:016x}"),
                if is_pc {
                    Tone::Pc
                } else if has_bp {
                    Tone::Err
                } else {
                    Tone::InfoPlain
                },
            ));

            match cell {
                MemoryCell::Data {
                    value,
                    case,
                    expr,
                    source_line,
                } => {
                    let val_str = expr
                        .as_deref()
                        .map(str::to_string)
                        .unwrap_or_else(|| rvs_memory::render_value(*value, *case));
                    if req.flags.list_bin {
                        push_sep(&mut segs);
                        let is_err = val_str.contains("ERROR");
                        if req.colorize {
                            segs.push(Segment::new("DD ", Tone::Heading));
                            segs.push(Segment::new(
                                format!("{:<37}", val_str),
                                if is_err { Tone::Err } else { Tone::OkPlain },
                            ));
                        } else {
                            segs.push(Segment::new("DD ", Tone::Secondary));
                            segs.push(Segment::new(
                                format!("{:<37}", val_str),
                                if is_err { Tone::Err } else { Tone::Secondary },
                            ));
                        }
                    }
                    if req.flags.list_hex {
                        push_sep(&mut segs);
                        segs.push(Segment::plain(" ".repeat(23)));
                    }
                    if req.flags.list_int {
                        push_sep(&mut segs);
                        segs.push(Segment::plain(" ".repeat(23)));
                    }
                    if req.flags.list_txt {
                        push_sep(&mut segs);
                        let expanded = expand_tabs(source_line, 8);
                        if req.colorize {
                            segs.extend(self.highlight_source_line(&expanded));
                        } else {
                            let is_err = expanded.contains("ERROR");
                            segs.push(Segment::new(
                                expanded,
                                if is_err { Tone::Err } else { Tone::Secondary },
                            ));
                        }
                    }
                }
                MemoryCell::Instruction(inst) => {
                    let enc = rvs_assembler::encode(&inst.op, &inst.raw_args, addr, &self.sim.sym);
                    let fmt_tag = enc.bin.split_whitespace().next();
                    if req.flags.list_bin {
                        push_sep(&mut segs);
                        let bin_padded = format!("{:<40}", enc.bin);
                        if req.colorize {
                            segs.extend(rvs_syntax::highlight_bin_correlated(&bin_padded));
                        } else {
                            let is_err = bin_padded.contains("ERROR");
                            segs.push(Segment::new(
                                bin_padded,
                                if is_err { Tone::Err } else { Tone::Secondary },
                            ));
                        }
                    }
                    if req.flags.list_hex {
                        push_sep(&mut segs);
                        let new_op = enc.asmx.first().map(String::as_str).unwrap_or("");
                        let new_args: Vec<String> = enc.asmx.iter().skip(1).cloned().collect();
                        let hex_ops = format!("{:<23}", rvs_asm_syntax::fmti(new_op, &new_args));
                        if req.colorize {
                            segs.extend(rvs_syntax::highlight_operands_correlated(
                                &hex_ops, fmt_tag,
                            ));
                        } else {
                            segs.push(Segment::new(hex_ops, Tone::Secondary));
                        }
                    }
                    if req.flags.list_int {
                        push_sep(&mut segs);
                        let words: Vec<&str> = enc.asm.split_whitespace().collect();
                        let int_op = words.first().copied().unwrap_or("");
                        let int_rest: Vec<String> =
                            words.iter().skip(1).map(|s| s.to_string()).collect();
                        let int_ops = format!("{:<23}", rvs_asm_syntax::fmti(int_op, &int_rest));
                        if req.colorize {
                            segs.extend(rvs_syntax::highlight_operands_correlated(
                                &int_ops, fmt_tag,
                            ));
                        } else {
                            segs.push(Segment::new(int_ops, Tone::Secondary));
                        }
                    }
                    if req.flags.list_txt {
                        push_sep(&mut segs);
                        let expanded = expand_tabs(&inst.source_line, 8);
                        if req.colorize {
                            segs.extend(rvs_syntax::highlight_source_line_correlated(
                                &expanded, fmt_tag,
                            ));
                        } else {
                            let is_err = expanded.contains("ERROR");
                            segs.push(Segment::new(
                                expanded,
                                if is_err { Tone::Err } else { Tone::Secondary },
                            ));
                        }
                    }
                }
            }
            lines.push(RichLine::new(segs));
        }

        lines.push(RichLine::plain(""));
        lines.push(rich("SYMBOL TABLE", Tone::Heading));
        let (sym_hdr, sym_div) = build_table_header(&[("ADDRESS", 18), ("SYMBOL", 16)]);
        lines.push(sym_hdr);
        lines.push(sym_div);
        for (name, value) in self.sim.sym.iter() {
            let v = rvs_tcl::parse_int(value).unwrap_or(0) as u64;
            lines.push(RichLine::new(vec![
                Segment::new(format!("0x{v:016x}"), Tone::InfoPlain),
                Segment::new(" │ ", Tone::Faint),
                Segment::new(name.clone(), Tone::Label),
            ]));
        }
        lines
    }

    fn build_registers(&self, req: &ViewRequest) -> Vec<RichLine> {
        let mut lines = Vec::new();
        lines.push(rich("INT Regs", Tone::Heading));
        if !req.flags.reg_int {
            lines.push(rich("---OFF---", Tone::Muted));
        } else {
            for i in 0..32u8 {
                let changed = req.changed_x.get(i as usize).copied().unwrap_or(false);
                let reg = self.sim.regs.x_string(i);
                let val = self.sim.regs.read_x(i);
                let abi = rvs_tcl::fmt_s_pad(rvs_regs::abi_name(i), 4);
                if changed {
                    let mut text = format!("x{:<2} {abi}", i);
                    if req.flags.reg_hex {
                        text.push(' ');
                        text.push_str(&reg);
                    }
                    text.push(' ');
                    text.push_str(&rvs_tcl::fmt_ld_pad(val as i64, 20));
                    lines.push(rich(text, Tone::Ok));
                    continue;
                }
                let mut segs = vec![
                    Segment::new(format!("x{:<2} ", i), Tone::Info),
                    Segment::new(abi, Tone::Label),
                ];
                if req.flags.reg_hex {
                    segs.push(Segment::new(
                        format!(" 0x{:016x}", val),
                        if val == 0 { Tone::Muted } else { Tone::Plain },
                    ));
                }
                segs.push(Segment::plain(" "));
                segs.push(Segment::new(
                    rvs_tcl::fmt_ld_pad(val as i64, 20),
                    if val == 0 { Tone::Muted } else { Tone::OkPlain },
                ));
                lines.push(RichLine::new(segs));
            }
        }

        lines.push(rich("FLP Regs", Tone::Heading));
        if !req.flags.reg_flp {
            lines.push(rich("---OFF---", Tone::Muted));
        } else {
            for i in 0..32u8 {
                let changed = req.changed_f.get(i as usize).copied().unwrap_or(false);
                let bits = self.sim.regs.read_f_bits(i);
                let reg = self.sim.regs.f_string(i);
                let flp = match rvs_tcl::x2f(&reg).and_then(rvs_tcl::fmt_e24) {
                    Some(v) => v,
                    None => rvs_tcl::fmt_s_pad_right(&double_fallback(&reg), 14),
                };
                if changed {
                    let mut text = format!("f{:<2}", i);
                    if req.flags.reg_hex {
                        text.push(' ');
                        text.push_str(&reg);
                    }
                    text.push(' ');
                    text.push_str(&flp);
                    lines.push(rich(text, Tone::Ok));
                    continue;
                }
                let mut segs = vec![Segment::new(format!("f{:<2}", i), Tone::Info)];
                if req.flags.reg_hex {
                    segs.push(Segment::new(
                        format!(" 0x{:016x}", bits),
                        if bits == 0 { Tone::Muted } else { Tone::Plain },
                    ));
                }
                segs.push(Segment::plain(" "));
                segs.push(Segment::new(
                    flp,
                    if bits == 0 {
                        Tone::Muted
                    } else {
                        Tone::WarnPlain
                    },
                ));
                lines.push(RichLine::new(segs));
            }
        }
        lines
    }

    fn build_memory(&self, req: &ViewRequest) -> Vec<RichLine> {
        let mut lines = Vec::new();
        lines.push(rich("MEMORY", Tone::Heading));
        let mut cols = vec![("ADDRESS", 18usize)];
        if req.flags.mem_hex {
            cols.push(("HEXADECIMAL", 18));
        }
        if req.flags.mem_int {
            cols.push(("INTEGER", 20));
        }
        if req.flags.mem_flp {
            cols.push(("FLOAT", 24));
        }
        if req.flags.mem_txt {
            cols.push(("SOURCE TEXT", 34));
        }
        let (hdr, div) = build_table_header(&cols);
        lines.push(hdr);
        lines.push(div);

        let mut data_rows = 0usize;
        for (&addr, cell) in self.sim.mem.cells.iter() {
            let MemoryCell::Data {
                value,
                case,
                expr,
                source_line,
            } = cell
            else {
                continue;
            };
            let hex = expr
                .as_deref()
                .map(str::to_string)
                .unwrap_or_else(|| rvs_memory::render_value(*value, *case));
            data_rows += 1;
            let is_zero = rvs_syntax::is_zero_hex(&hex);
            let mut segs = vec![Segment::new(format!("0x{addr:016x}"), Tone::InfoPlain)];
            if req.flags.mem_hex {
                push_sep(&mut segs);
                segs.push(Segment::new(
                    format!("{:<18}", hex),
                    if is_zero { Tone::Muted } else { Tone::Plain },
                ));
            }
            if req.flags.mem_int {
                push_sep(&mut segs);
                let int_str = match rvs_tcl::fmt_ld_str(&hex) {
                    Some(v) => rvs_tcl::fmt_s_pad(v.trim(), 20),
                    None => rvs_tcl::fmt_s_pad("NOT_AN_INT", 20),
                };
                segs.push(Segment::new(
                    int_str,
                    if is_zero { Tone::Muted } else { Tone::OkPlain },
                ));
            }
            if req.flags.mem_flp {
                push_sep(&mut segs);
                let flp_str = match rvs_tcl::x2f(&hex).and_then(rvs_tcl::fmt_e24) {
                    Some(v) => format!("{:<24}", v.trim()),
                    None => rvs_tcl::fmt_s_pad("NOT_A_FLP", 24),
                };
                segs.push(Segment::new(
                    flp_str,
                    if is_zero {
                        Tone::Muted
                    } else {
                        Tone::WarnPlain
                    },
                ));
            }
            if req.flags.mem_txt {
                push_sep(&mut segs);
                let formatted =
                    rvs_asm_syntax::render_line_f1(&rvs_asm_syntax::split_line(source_line));
                segs.extend(self.highlight_source_line(&formatted));
            }
            lines.push(RichLine::new(segs));
        }
        if data_rows == 0 {
            lines.push(rich(
                "  (no data cells: use DD/DW/DH/DB/DC/DF/DM)",
                Tone::Muted,
            ));
        }
        lines
    }

    fn build_exec(&self) -> Vec<RichLine> {
        let mut lines = Vec::new();
        for line_str in self.sim.exec_log.lines() {
            let clean = expand_tabs(line_str, 8);
            if clean.starts_with("ERROR:") {
                lines.push(rich(clean, Tone::Err));
            } else if clean.starts_with("WARN") {
                lines.push(rich(clean, Tone::Warn));
            } else if clean.starts_with("ebreak") {
                lines.push(rich(clean, Tone::Label));
            } else if clean == "START" || clean == "NEXT" || clean == "RUN" {
                lines.push(rich(clean, Tone::Focus));
            } else if clean.starts_with("0x") && clean.len() >= 18 {
                let mut segs = vec![Segment::new(clean[..18].to_string(), Tone::InfoPlain)];
                segs.extend(self.highlight_inst_operands(&clean[18..]));
                lines.push(RichLine::new(segs));
            } else {
                lines.push(rich(clean, Tone::Secondary));
            }
        }
        lines
    }

    fn highlight_inst_operands(&self, text: &str) -> Vec<Segment> {
        rvs_syntax::highlight_inst_operands(text)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn rich(text: impl Into<String>, tone: Tone) -> RichLine {
    RichLine::new(vec![Segment::new(text, tone)])
}

fn push_sep(segs: &mut Vec<Segment>) {
    segs.push(Segment::new(" │ ", Tone::Faint));
}

fn double_fallback(hex: &str) -> String {
    match rvs_tcl::x2f(hex) {
        Some(v) => rvs_tcl::tcl_double_to_string(v),
        None => String::new(),
    }
}

fn build_table_header(cols: &[(&str, usize)]) -> (RichLine, RichLine) {
    let mut hdr = Vec::new();
    let mut div = String::new();
    for (i, &(title, width)) in cols.iter().enumerate() {
        if i > 0 {
            hdr.push(Segment::new(" │ ", Tone::Faint));
            div.push_str("─┼─");
        }
        let text = if width > 0 {
            format!("{:<width$}", title)
        } else {
            title.to_string()
        };
        hdr.push(Segment::new(text, Tone::Heading));
        let div_width = if width > 0 {
            width
        } else {
            title.len().max(16)
        };
        div.push_str(&"─".repeat(div_width));
    }
    (
        RichLine::new(hdr),
        RichLine::new(vec![Segment::new(div, Tone::Faint)]),
    )
}

/// Everything the CLI hands to a batch run.
#[derive(Clone, Copy, Debug)]
pub struct BatchRequest<'a> {
    /// `None` = detect from the source.
    pub dialect: Option<rvs_machine::Dialect>,
    pub trace: bool,
    pub json: Option<&'a std::path::Path>,
    pub a7: bool,
}

impl Default for BatchRequest<'_> {
    fn default() -> Self {
        Self {
            dialect: None,
            trace: true,
            json: None,
            a7: false,
        }
    }
}

/// CLI batch entry point. Kept here so the TUI binary has no engine dependency.
pub fn run_batch_file(
    asm: &std::path::Path,
    input: &std::path::Path,
    out: &std::path::Path,
    timeout_ms: u64,
    req: BatchRequest<'_>,
) -> Result<rvs_format::BatchReport, String> {
    let opts = rvs_format::BatchOptions {
        dialect: req.dialect.map(|d| match d {
            rvs_machine::Dialect::Rvs => rvs_transpile::AssemblyDialect::Rvs,
            rvs_machine::Dialect::Gnu => rvs_transpile::AssemblyDialect::Gnu,
        }),
        trace: req.trace,
        json: req.json.map(|p| p.to_path_buf()),
        abi: if req.a7 {
            rvs_sim::EcallAbi::A7
        } else {
            rvs_sim::EcallAbi::Rvs
        },
    };
    rvs_format::run_batch_file(asm, input, out, timeout_ms, opts)
}
