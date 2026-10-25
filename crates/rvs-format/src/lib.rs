//! Report and pane formatters (`memp`, `regp`) and the `kkbatch` runner.

use std::path::Path;

use rvs_sim::Simulator;
use rvs_tcl as tcl;

/// `memp`: the MEMORY pane text.
pub fn format_memory(sim: &Simulator) -> String {
    let mut res = String::from("MEMORY\n");
    let mut header = String::new();
    header.push_str(&tcl::fmt_s_pad("ADDRESS", 18));
    header.push(' ');
    if sim.view.mem_hex {
        header.push_str(&tcl::fmt_s_pad("HEXADECIMAL", 18));
        header.push(' ');
    }
    if sim.view.mem_int {
        header.push_str(&tcl::fmt_s_pad("INTEGER", 20));
        header.push(' ');
    }
    if sim.view.mem_flp {
        header.push_str(&tcl::fmt_s_pad("FLOAT", 14));
        header.push(' ');
    }
    if sim.view.mem_txt {
        header.push_str(&tcl::fmt_s_pad("SOURCE TEXT", 50));
        header.push(' ');
    }
    res.push_str(&header);
    res.push('\n');
    for (addr, cell) in sim.mem.cells.iter() {
        let (hex, source_line) = match cell {
            rvs_memory::MemoryCell::Data {
                value,
                case,
                expr,
                source_line,
            } => (
                match expr {
                    Some(e) => e.clone(),
                    None => rvs_memory::render_value(*value, *case),
                },
                source_line.clone(),
            ),
            _ => continue,
        };
        let mut line = format!("0x{addr:016x}");
        if sim.view.mem_hex {
            line.push(' ');
            line.push_str(&hex);
        }
        if sim.view.mem_int {
            line.push(' ');
            match tcl::fmt_ld_str(&hex) {
                Some(v) => line.push_str(&tcl::fmt_s_pad(&v, 20)),
                None => line.push_str(&tcl::fmt_s_pad("NOT_AN_INT", 20)),
            }
        }
        if sim.view.mem_flp {
            line.push(' ');
            match tcl::x2f(&hex).and_then(tcl::fmt_e24) {
                Some(v) => line.push_str(&v),
                None => line.push_str(&tcl::fmt_s_pad("NOT_A_FLP", 14)),
            }
        }
        if sim.view.mem_txt {
            line.push(' ');
            line.push_str(&rvs_asm_syntax::render_line_f1(
                &rvs_asm_syntax::split_line(&source_line),
            ));
        }
        res.push_str(&line);
        res.push('\n');
    }
    res
}

/// `regp`: the register pane text.
pub fn format_registers(sim: &Simulator) -> String {
    let mut res = String::from("INT Regs\n");
    if !sim.view.reg_int {
        res.push_str("---OFF---\n");
    }
    for i in 0..32u8 {
        if sim.view.reg_int {
            let reg = sim.regs.x_string(i);
            res.push_str(&format!(
                "x{:<2} {}",
                i,
                tcl::fmt_s_pad(rvs_regs::abi_name(i), 4)
            ));
            if sim.view.reg_hex {
                res.push(' ');
                res.push_str(&reg);
            }
            let signed = sim.regs.read_x(i) as i64;
            res.push(' ');
            res.push_str(&tcl::fmt_ld_pad(signed, 20));
            res.push('\n');
        }
    }
    res.push_str("FLP Regs\n");
    if !sim.view.reg_flp {
        res.push_str("---OFF---\n");
    }
    for i in 0..32u8 {
        if sim.view.reg_flp {
            let reg = sim.regs.f_string(i);
            res.push_str(&format!("f{:<2}", i));
            if sim.view.reg_hex {
                res.push(' ');
                res.push_str(&reg);
            }
            res.push(' ');
            match tcl::x2f(&reg).and_then(tcl::fmt_e24) {
                Some(v) => res.push_str(&v),
                None => res.push_str(&tcl::fmt_s_pad_right(&tcl_double_fallback(&reg), 14)),
            }
            res.push('\n');
        }
    }
    res
}

/// regp's catch path prints the raw `x2f` string (`NaN`/`Inf`) in a
/// right-justified 14-wide field.
fn tcl_double_fallback(hex: &str) -> String {
    match tcl::x2f(hex) {
        Some(v) => tcl::tcl_double_to_string(v),
        None => String::new(),
    }
}

/// What a batch run did, for the CLI's exit status and the JSON sidecar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BatchReport {
    pub assembled: bool,
    pub steps: u64,
    pub timed_out: bool,
    /// First execution error, if the run halted on one.
    pub error: Option<String>,
}

impl BatchReport {
    pub fn failed(&self) -> bool {
        !self.assembled || self.error.is_some()
    }
}

/// Options for a batch run. Defaults preserve the reference report exactly.
#[derive(Clone, Debug, Default)]
pub struct BatchOptions {
    /// `None` = detect from the source.
    pub dialect: Option<rvs_transpile::AssemblyDialect>,
    /// Append the per-instruction trace (the reference always does).
    pub trace: bool,
    /// Optional machine-readable sidecar (`--json`).
    pub json: Option<std::path::PathBuf>,
    /// `ecall` convention: the course dialect (default) or a7.
    pub abi: rvs_sim::EcallAbi,
}

impl BatchOptions {
    /// Reference-equivalent defaults: trace on, dialect detected, no sidecar.
    pub fn new() -> Self {
        Self {
            dialect: None,
            trace: true,
            json: None,
            abi: rvs_sim::EcallAbi::Rvs,
        }
    }
}

/// `kkbatch`: compile, run with timeout, and write the exact report.
#[expect(
    clippy::needless_pass_by_value,
    reason = "public API consumes owned options struct"
)]
pub fn run_batch_file(
    asm_path: &Path,
    in_path: &Path,
    out_path: &Path,
    timeout_ms: u64,
    opts: BatchOptions,
) -> Result<BatchReport, String> {
    let stars = format!("\n{}\n", "*".repeat(57));
    let mut sim = Simulator::new();
    let mut data = String::new();

    data.push_str(&format!("{stars}Load {}{stars}", asm_path.display()));
    let source = match std::fs::read_to_string(asm_path) {
        Ok(s) => s,
        Err(e) => return Err(format!("ERROR: {e}")),
    };
    sim.source = source.clone();
    sim.dialect = opts
        .dialect
        .unwrap_or_else(|| rvs_transpile::detect_dialect(&source));
    sim.trace = opts.trace;
    sim.ecall_abi = opts.abi;
    data.push_str(&source);

    let input = match std::fs::read_to_string(in_path) {
        Ok(s) => s,
        Err(e) => return Err(format!("ERROR: {e}")),
    };
    sim.input_lines = Some(std::collections::VecDeque::from(
        input.split('\n').map(|s| s.to_string()).collect::<Vec<_>>(),
    ));

    data.push_str(&format!("{stars}Compile{stars}"));
    let mut asm_error: Option<String> = None;
    match rvs_assembler::assemble(&mut sim, &source) {
        Ok(listing) => {
            sim.listing = format!("{listing}\n");
        }
        Err(err) => {
            // The reference unconditionally resets START after a failed compile.
            sim.sym
                .insert("START".to_string(), Simulator::START_DEFAULT.to_string());
            sim.regs.pc = tcl::parse_int(Simulator::START_DEFAULT).unwrap_or(0) as u64;
            sim.listing = format!("{}ERROR: {}\n\n", err.progress, err.message);
            asm_error = Some(err.message);
        }
    }
    data.push_str(&sim.listing);
    data.push_str(&stars);
    data.push_str(&format_memory(&sim));

    data.push_str(&format!("{stars}Run{stars}"));
    // RunCmd 1: restart from START.
    sim.regs.pc = sim
        .sym
        .get("START")
        .and_then(|s| tcl::parse_int(s))
        .map(|v| v as u64)
        .unwrap_or(0);
    sim.exec_log.push_str("START\n");
    sim.inp_prompt = false;
    let status = rvs_executor::run(&mut sim, timeout_ms);
    data.push_str(&sim.exec_log);
    data.push_str(&stars);
    data.push_str(&format_memory(&sim));
    data.push_str(&stars);
    data.push_str(&format_registers(&sim));
    data.push_str(&stars);
    data.push_str(&sim.output);

    std::fs::write(out_path, data.as_bytes()).map_err(|e| format!("ERROR: {e}"))?;
    let report = BatchReport {
        assembled: asm_error.is_none(),
        steps: sim.metrics.instructions,
        timed_out: status == rvs_executor::RunStatus::TimedOut,
        error: sim.last_error.clone().or(asm_error),
    };
    if let Some(path) = &opts.json {
        write_json_report(path, &sim, &report, asm_path, timeout_ms)?;
    }
    Ok(report)
}

/// Escape a string for the JSON sidecar (no `serde` dependency).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Machine-readable summary written by `rvs --json <path>`.
///
/// Deliberately a sidecar: the human report stays byte-identical to `kkbatch`,
/// so the differential harness keeps its meaning.
pub fn write_json_report(
    path: &Path,
    sim: &Simulator,
    report: &BatchReport,
    asm_path: &Path,
    timeout_ms: u64,
) -> Result<(), String> {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"program\": \"{}\",\n",
        json_escape(&asm_path.display().to_string())
    ));
    out.push_str(&format!("  \"assembled\": {},\n", report.assembled));
    out.push_str(&format!("  \"steps\": {},\n", report.steps));
    out.push_str(&format!("  \"timeout_ms\": {timeout_ms},\n"));
    out.push_str(&format!("  \"timed_out\": {},\n", report.timed_out));
    out.push_str(&format!(
        "  \"error\": {},\n",
        match &report.error {
            Some(e) => format!("\"{}\"", json_escape(e)),
            None => "null".to_string(),
        }
    ));
    out.push_str(&format!("  \"cycles\": {},\n", sim.metrics.cycles));
    out.push_str(&format!("  \"cpi\": {:.4},\n", sim.metrics.cpi()));
    out.push_str(&format!(
        "  \"output\": \"{}\",\n",
        json_escape(&sim.output)
    ));
    out.push_str("  \"x\": [");
    for i in 0..32u8 {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("{}", sim.regs.read_x(i)));
    }
    out.push_str("],\n  \"f_bits\": [");
    for i in 0..32u8 {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("{}", sim.regs.read_f_bits(i)));
    }
    out.push_str("],\n  \"memory\": [");
    let mut first = true;
    for (addr, cell) in sim.mem.cells.iter() {
        if let rvs_memory::MemoryCell::Data { value, .. } = cell {
            if !first {
                out.push(',');
            }
            out.push_str(&format!("[{addr},{value}]"));
            first = false;
        }
    }
    out.push_str("]\n}\n");
    std::fs::write(path, out).map_err(|e| format!("ERROR: {e}"))
}

/// Formats a listing-relative view of the current (listing content is stored on the simulator itself).
pub fn listing_of(sim: &Simulator) -> String {
    sim.listing.clone()
}

#[allow(unused_imports)]
use rvs_memory::SimError as _SimErrorUnused;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_pane_shape() {
        let mut sim = Simulator::new();
        sim.mem.insert_dd(0, "0x0000000000000005", "DD 5");
        let text = format_memory(&sim);
        assert!(text.starts_with("MEMORY\n"));
        assert!(text.contains("0x0000000000000000 0x0000000000000005"));
    }
    #[test]
    fn registers_pane_hex_toggle() {
        let mut sim = Simulator::new();
        sim.view.reg_hex = true;
        let with_hex = format_registers(&sim);
        assert!(with_hex.contains("x0  zero 0x0000000000000000"));
        assert!(with_hex.contains("f0  0x0000000000000000"));

        sim.view.reg_hex = false;
        let without_hex = format_registers(&sim);
        assert!(!without_hex.contains("0x"));
        assert!(without_hex.contains("x0  zero 0                   "));
        assert!(without_hex.contains("f0    0.0000000000000000E+00"));
    }
}
