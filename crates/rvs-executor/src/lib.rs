//! Instruction execution mirroring `RVS046.tcl`'s `step`, the uppercase
//! executor procs (`typeR`, `typeI`, `Smem`, `Lmem`, `typeSB`, `JAL`,
//! `JALR`, `LUI`, `AUIPC`, `EBREAK`, `ECALL`, `typeRM`, `typeFR`, the float
//! ops, `FLmem`, `FSmem`) plus the `Output` / `Input` / `EntryInputCmd`
//! DMA polling flows.
//!
//! Register hex strings preserve the reference's lower/uppercase writer
//! conventions (`%016lx` vs `%016lX`), which leak into `ECALL 2`, DMA `X`
//! output and the register pane.

use std::time::{Duration, Instant};

use rvs_ecall::ecall;
use rvs_fpu::{
    fcvt_to_float, fcvt_to_int, flmem, fminmax, fmv_d_x, fmv_x_d, fsgnj, fsmem, fsqrt, type_fr,
    FOp, SgnMode,
};
use rvs_memory::SimError;
use rvs_memory::SimResult;
use rvs_sim::{imm12, imm20, imm_raw, read_x_arg, reg_index, write_x, Simulator};
use rvs_tcl as tcl;

/// Result of one `step` call.
#[derive(Debug)]
pub enum StepResult {
    /// Instruction executed; trace already appended.
    Continued,
    /// `ebreak` or an execution error: message appended to the execution
    /// log and the PC rewound to the reference position.
    Halted,
    /// An `ECALL 5..9` needs interactive input; the prompt is on the output.
    WaitingForInput,
}

/// Result of a full `RunCmd`-style run.
#[derive(Debug, PartialEq, Eq)]
pub enum RunStatus {
    Completed,
    TimedOut,
    WaitingForInput,
}

/// Pending `ECALL 5..9` request kept while the UI gathers a line.
#[derive(Debug, Clone)]
pub struct PendingInput {
    pub rd_arg: String,
    pub rs1_arg: String,
    pub imm: u64,
    pub inst_pc: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AluOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
    Sll,
    Srl,
    Sra,
    Slt,
}

/// Shared ALU with Tcl bignum semantics: signed operands are `i64`
/// (`format %ld`), unsigned are raw u64; results truncate to 64 bits.
fn alu(op: AluOp, a: u64, b: u64, unsigned: bool) -> SimResult<u64> {
    let result: i128 = match (op, unsigned) {
        (AluOp::Add, false) => (a as i64 as i128) + (b as i64 as i128),
        (AluOp::Sub, false) => (a as i64 as i128) - (b as i64 as i128),
        (AluOp::And, false) => (a as i64 as i128) & (b as i64 as i128),
        (AluOp::Or, false) => (a as i64 as i128) | (b as i64 as i128),
        (AluOp::Xor, false) => (a as i64 as i128) ^ (b as i64 as i128),
        (AluOp::And, true) => (a as i128) & (b as i128),
        (AluOp::Or, true) => (a as i128) | (b as i128),
        (AluOp::Xor, true) => (a as i128) ^ (b as i128),
        (AluOp::Add, true) => a as i128 + b as i128,
        (AluOp::Sll, _) => {
            let amount = if unsigned {
                b as i128
            } else {
                b as i64 as i128
            };
            if amount < 0 {
                return Err(SimError("negative shift argument".to_string()));
            }
            if amount >= 1 << 40 {
                return Err(SimError("integer value too large to represent".to_string()));
            }
            if amount > 200 {
                0
            } else {
                (a as i128) << amount
            }
        }
        (AluOp::Srl, _) => {
            let amount = if unsigned { b } else { b as i64 as u64 };
            if amount >= 64 {
                0
            } else {
                i128::from(a >> amount)
            }
        }
        (AluOp::Sra, false) => {
            let amount = b as i64 as i128;
            if amount < 0 {
                return Err(SimError("negative shift argument".to_string()));
            }
            if amount >= 64 {
                if (a as i64) < 0 {
                    -1
                } else {
                    0
                }
            } else {
                (a as i64 as i128) >> amount
            }
        }
        (AluOp::Slt, false) => i128::from((a as i64) < (b as i64)),
        (AluOp::Slt, true) | (AluOp::Sub, true) | (AluOp::Sra, true) => i128::from(a < b),
    };
    Ok(tcl::low64(result))
}

/// `typeR`: `rd = rs1 op rs2` on signed (default) or unsigned operands.
fn type_r(sim: &mut Simulator, args: &[String], op: AluOp, unsigned: bool) -> SimResult<()> {
    let a = read_x_arg(sim, &args[1])?;
    let b = read_x_arg(sim, &args[2])?;
    let value = alu(op, a, b, unsigned)?;
    write_x(sim, &args[0], value, true)
}

/// `typeI`: `rd = rs1 op imm` with `x12i` (signed) or 12-bit-unsigned imm.
fn type_i(sim: &mut Simulator, args: &[String], op: AluOp, unsigned: bool) -> SimResult<()> {
    let a = read_x_arg(sim, &args[1])?;
    let imm_signed = imm12(&args[2])?;
    let value = if unsigned {
        // u=1 path: negative immediates become their 12-bit unsigned form.
        let imm_u = if imm_signed < 0 {
            (4096i64 + imm_signed) as u64
        } else {
            imm_signed as u64
        };
        alu(op, a, imm_u, true)?
    } else {
        alu(op, a, imm_signed as u64, false)?
    };
    write_x(sim, &args[0], value, true)
}

/// `Smem`: store the low bytes of `rs1` at `x[rs2] + sext(imm)`.
fn smem(sim: &mut Simulator, args: &[String], nbytes: usize) -> SimResult<()> {
    let base = read_x_arg(sim, &args[1])?;
    let addr = base.wrapping_add(imm12(&args[2])? as u64);
    let idx = reg_index(&args[0])?;
    let reg_val = sim.regs.read_x(idx);
    let mask = if nbytes == 8 {
        u64::MAX
    } else {
        (1u64 << (nbytes * 8)) - 1
    };
    let val = reg_val & mask;
    let upper = sim.regs.x_case(idx);
    sim.mem.write_bytes(addr, nbytes, val, upper)
}

/// `Lmem`: load and sign-extend unless `unsigned`.
fn lmem(sim: &mut Simulator, args: &[String], nbytes: usize, unsigned: bool) -> SimResult<()> {
    let base = read_x_arg(sim, &args[1])?;
    let addr = base.wrapping_add(imm12(&args[2])? as u64);
    let bits = sim.mem.read_bytes(addr, nbytes)?;
    // Sign-extend sub-word loads; a doubleword is already 64 bits wide (a
    // `1 << 64` mask would overflow).
    let value = if !unsigned && nbytes < 8 && (bits >> (nbytes * 8 - 1)) & 1 == 1 {
        bits | !((1u64 << (nbytes * 8)) - 1)
    } else {
        bits
    };
    write_x(sim, &args[0], value, false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BranchOp {
    Eq,
    Ne,
    Ge,
    GeU,
    Lt,
    LtU,
}

/// `typeSB`: conditional branch; `imm` is `offset/2`.
fn type_sb(sim: &mut Simulator, args: &[String], op: BranchOp, unsigned: bool) -> SimResult<()> {
    let a = read_x_arg(sim, &args[0])?;
    let b = read_x_arg(sim, &args[1])?;
    let taken = match (op, unsigned) {
        (BranchOp::Eq, _) => a == b,
        (BranchOp::Ne, _) => a != b,
        (BranchOp::Ge, false) => a as i64 >= b as i64,
        (BranchOp::Ge, true) | (BranchOp::GeU, _) => a >= b,
        (BranchOp::Lt, false) => (a as i64) < (b as i64),
        (BranchOp::Lt, true) | (BranchOp::LtU, _) => a < b,
    };
    if taken {
        let off = imm12(&args[2])?;
        sim.regs.pc = sim.regs.pc.wrapping_add((off << 1) as u64).wrapping_sub(4);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RmKind {
    Mul,
    MulH,
    MulHSu,
    MulHu,
    Div,
    DivU,
    Rem,
    RemU,
}

/// `typeRM`: multiply/divide with 128-bit intermediates. `DIV`/`REM` use
/// Tcl's floor division and divisor-sign remainder.
fn type_rm(sim: &mut Simulator, args: &[String], kind: RmKind) -> SimResult<()> {
    let a = read_x_arg(sim, &args[1])?;
    let b = read_x_arg(sim, &args[2])?;
    let (signed_a, signed_b, high) = match kind {
        RmKind::Mul => (true, true, false),
        RmKind::MulH => (true, true, true),
        RmKind::MulHSu => (true, false, true),
        RmKind::MulHu => (false, false, true),
        RmKind::Div | RmKind::Rem => (true, true, false),
        RmKind::DivU | RmKind::RemU => (false, false, false),
    };
    let sa = if signed_a {
        a as i64 as i128
    } else {
        a as i128
    };
    let sb = if signed_b {
        b as i64 as i128
    } else {
        b as i128
    };
    let value: u64 = match kind {
        RmKind::Mul | RmKind::MulH | RmKind::MulHSu | RmKind::MulHu => {
            // MULHU's unsigned product reaches 128 bits; i128 is signed so
            // compute that case in u128.
            let product = if kind == RmKind::MulHu {
                (a as u128).wrapping_mul(b as u128) as i128
            } else {
                sa.wrapping_mul(sb)
            };
            if high {
                (product >> 64) as u64
            } else {
                tcl::low64(product)
            }
        }
        RmKind::Div | RmKind::Rem => {
            if sb == 0 {
                return Err(SimError("divide by zero".to_string()));
            }
            let q = sa / sb;
            let q = if sa % sb != 0 && ((sa < 0) != (sb < 0)) {
                q - 1
            } else {
                q
            };
            if kind == RmKind::Div {
                tcl::low64(q)
            } else {
                tcl::low64(sa - q * sb)
            }
        }
        RmKind::DivU => {
            if sb == 0 {
                return Err(SimError("divide by zero".to_string()));
            }
            tcl::low64(sa / sb)
        }
        RmKind::RemU => {
            if sb == 0 {
                return Err(SimError("divide by zero".to_string()));
            }
            tcl::low64(sa % sb)
        }
    };
    write_x(sim, &args[0], value, false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WKind {
    Add,
    Sub,
    Sll,
    Srl,
    Sra,
    Mul,
    Div,
    DivU,
    Rem,
    RemU,
}

/// 32-bit immediate word variants added by this rewrite.
fn type_wi(sim: &mut Simulator, args: &[String], kind: WKind) -> SimResult<()> {
    let a = read_x_arg(sim, &args[1])?;
    let imm = imm12(&args[2])?;
    let aw = a as u32 as i32;
    let value: u64 = match kind {
        WKind::Add => (aw.wrapping_add(imm as i32)) as i64 as u64,
        WKind::Sll => (aw.wrapping_shl(imm as u32 & 31)) as i64 as u64,
        WKind::Srl => (((a as u32).wrapping_shr(imm as u32 & 31)) as i32) as i64 as u64,
        WKind::Sra => (aw >> (imm as u32 & 31)) as i64 as u64,
        _ => unreachable!(),
    };
    write_x(sim, &args[0], value, true)
}

/// 32-bit word variants added by this rewrite (standard RV64 semantics).
/// ALU-family ops inherit the uppercase writer convention, `*div` the
/// lowercase `typeRM` convention.
// Division by zero and MIN/-1 have two *different* Tcl fallbacks (-1 and MIN),
// so `checked_div` cannot express this; the explicit zero check stays.
#[allow(clippy::manual_checked_ops)]
fn type_w(sim: &mut Simulator, args: &[String], kind: WKind) -> SimResult<()> {
    let a = read_x_arg(sim, &args[1])?;
    let b = read_x_arg(sim, &args[2])?;
    let aw = a as u32 as i32;
    let bw = b as u32 as i32;
    let value: u64 = match kind {
        WKind::Add => (aw.wrapping_add(bw)) as i64 as u64,
        WKind::Sub => (aw.wrapping_sub(bw)) as i64 as u64,
        WKind::Sll => (aw.wrapping_shl(bw as u32 & 31)) as i64 as u64,
        WKind::Srl => (((a as u32).wrapping_shr(bw as u32 & 31)) as i32) as i64 as u64,
        WKind::Sra => (aw >> (bw as u32 & 31)) as i64 as u64,
        WKind::Mul => (aw.wrapping_mul(bw)) as i64 as u64,
        WKind::Div => {
            if bw == 0 {
                -1i64 as u64
            } else if aw == i32::MIN && bw == -1 {
                i32::MIN as i64 as u64
            } else {
                (aw / bw) as i64 as u64
            }
        }
        WKind::DivU => {
            if b as u32 == 0 {
                u64::MAX
            } else {
                ((a as u32) / (b as u32)) as i32 as i64 as u64
            }
        }
        WKind::Rem => {
            if bw == 0 {
                aw as i64 as u64
            } else if aw == i32::MIN && bw == -1 {
                0
            } else {
                (aw % bw) as i64 as u64
            }
        }
        WKind::RemU => {
            if b as u32 == 0 {
                a as u32 as i32 as i64 as u64
            } else {
                ((a as u32) % (b as u32)) as i32 as i64 as u64
            }
        }
    };
    let upper = matches!(
        kind,
        WKind::Add | WKind::Sub | WKind::Sll | WKind::Srl | WKind::Sra | WKind::Mul
    );
    write_x(sim, &args[0], value, upper)
}

/// `EBREAK`: advance past the instruction and halt with `ebreak\n`.
fn ebreak(sim: &mut Simulator) -> SimResult<()> {
    sim.regs.pc = sim.regs.pc.wrapping_add(4);
    Err(SimError("ebreak\n".to_string()))
}

type ExecResult = Result<bool, SimError>;

fn execute(sim: &mut Simulator, op: &str, args: &[String]) -> ExecResult {
    let upper = op.to_ascii_uppercase();
    let s: Vec<String> = args.to_vec();
    // A cell whose operands were rewritten to an error placeholder (or a
    // malformed source line) must surface as a clean trace error, never as an
    // out-of-bounds index.
    let need = match upper.as_str() {
        "JAL" | "LUI" | "AUIPC" => 2,
        "EBREAK" => 0,
        _ => 3,
    };
    // Only known mnemonics get the arity check; an unknown one must reach the
    // default arm so the message stays `invalid command name` like the
    // reference's `eval`.
    if s.len() < need && rvs_asm_syntax::find_instr(op).is_some() {
        return Err(SimError(format!("MUST HAVE {need} ARGS")));
    }
    let r = match upper.as_str() {
        "ADD" => type_r(sim, &s, AluOp::Add, false),
        "SUB" => type_r(sim, &s, AluOp::Sub, false),
        "AND" => type_r(sim, &s, AluOp::And, false),
        "OR" => type_r(sim, &s, AluOp::Or, false),
        "XOR" => type_r(sim, &s, AluOp::Xor, false),
        "SLL" => type_r(sim, &s, AluOp::Sll, false),
        "SRL" => type_r(sim, &s, AluOp::Srl, true),
        "SRA" => type_r(sim, &s, AluOp::Sra, false),
        "SLT" => type_r(sim, &s, AluOp::Slt, false),
        "SLTU" => type_r(sim, &s, AluOp::Slt, true),
        "ADDI" => type_i(sim, &s, AluOp::Add, false),
        "SUBI" => type_i(sim, &s, AluOp::Sub, false),
        "ANDI" => type_i(sim, &s, AluOp::And, false),
        "ORI" => type_i(sim, &s, AluOp::Or, false),
        "XORI" => type_i(sim, &s, AluOp::Xor, false),
        "SLLI" => type_i(sim, &s, AluOp::Sll, false),
        "SRLI" => type_i(sim, &s, AluOp::Srl, true),
        "SRAI" => type_i(sim, &s, AluOp::Sra, false),
        "SLTI" => type_i(sim, &s, AluOp::Slt, false),
        "SLTIU" => type_i(sim, &s, AluOp::Slt, true),
        "SD" => smem(sim, &s, 8),
        "SW" => smem(sim, &s, 4),
        "SH" => smem(sim, &s, 2),
        "SB" => smem(sim, &s, 1),
        "LD" => lmem(sim, &s, 8, false),
        "LW" => lmem(sim, &s, 4, false),
        "LWU" => lmem(sim, &s, 4, true),
        "LH" => lmem(sim, &s, 2, false),
        "LHU" => lmem(sim, &s, 2, true),
        "LB" => lmem(sim, &s, 1, false),
        "LBU" => lmem(sim, &s, 1, true),
        "BEQ" => type_sb(sim, &s, BranchOp::Eq, false),
        "BNE" => type_sb(sim, &s, BranchOp::Ne, false),
        "BGE" => type_sb(sim, &s, BranchOp::Ge, false),
        "BGEU" => type_sb(sim, &s, BranchOp::GeU, true),
        "BLT" => type_sb(sim, &s, BranchOp::Lt, false),
        "BLTU" => type_sb(sim, &s, BranchOp::LtU, true),
        "JAL" => jal(sim, &s),
        "JALR" => jalr(sim, &s),
        "LUI" => lui(sim, &s),
        "AUIPC" => auipc(sim, &s),
        "EBREAK" => ebreak(sim),
        "ECALL" => return ecall(sim, &s),
        "MUL" => type_rm(sim, &s, RmKind::Mul),
        "MULH" => type_rm(sim, &s, RmKind::MulH),
        "MULHSU" => type_rm(sim, &s, RmKind::MulHSu),
        "MULHU" => type_rm(sim, &s, RmKind::MulHu),
        "DIV" => type_rm(sim, &s, RmKind::Div),
        "DIVU" => type_rm(sim, &s, RmKind::DivU),
        "REM" => type_rm(sim, &s, RmKind::Rem),
        "REMU" => type_rm(sim, &s, RmKind::RemU),
        "ADDIW" => type_wi(sim, &s, WKind::Add),
        "SLLIW" => type_wi(sim, &s, WKind::Sll),
        "SRLIW" => type_wi(sim, &s, WKind::Srl),
        "SRAIW" => type_wi(sim, &s, WKind::Sra),
        "ADDW" => type_w(sim, &s, WKind::Add),
        "SUBW" => type_w(sim, &s, WKind::Sub),
        "SLLW" => type_w(sim, &s, WKind::Sll),
        "SRLW" => type_w(sim, &s, WKind::Srl),
        "SRAW" => type_w(sim, &s, WKind::Sra),
        "MULW" => type_w(sim, &s, WKind::Mul),
        "DIVW" => type_w(sim, &s, WKind::Div),
        "DIVUW" => type_w(sim, &s, WKind::DivU),
        "REMW" => type_w(sim, &s, WKind::Rem),
        "REMUW" => type_w(sim, &s, WKind::RemU),
        "FADD.D" => type_fr(sim, &s, FOp::Add),
        "FSUB.D" => type_fr(sim, &s, FOp::Sub),
        "FMUL.D" => type_fr(sim, &s, FOp::Mul),
        "FDIV.D" => type_fr(sim, &s, FOp::Div),
        "FEQ.D" => type_fr(sim, &s, FOp::Eq),
        "FLT.D" => type_fr(sim, &s, FOp::Lt),
        "FLE.D" => type_fr(sim, &s, FOp::Le),
        "FMIN.D" => fminmax(sim, &s, false),
        "FMAX.D" => fminmax(sim, &s, true),
        "FSGNJ.D" => fsgnj(sim, &s, SgnMode::J),
        "FSGNJN.D" => fsgnj(sim, &s, SgnMode::Jn),
        "FSGNJX.D" => fsgnj(sim, &s, SgnMode::Jx),
        "FCVT.L.D" => fcvt_to_int(sim, &s, false),
        "FCVT.LU.D" => fcvt_to_int(sim, &s, true),
        "FCVT.D.L" => fcvt_to_float(sim, &s, false),
        "FCVT.D.LU" => fcvt_to_float(sim, &s, true),
        "FMV.X.D" => fmv_x_d(sim, &s),
        "FMV.D.X" => fmv_d_x(sim, &s),
        "FSQRT.D" => fsqrt(sim, &s),
        "FLD" => flmem(sim, &s, 8),
        "FSD" => fsmem(sim, &s, 8),
        _ => Err(SimError(format!("invalid command name \"{upper}\""))),
    };
    r.map(|_| false)
}

fn jal(sim: &mut Simulator, args: &[String]) -> SimResult<()> {
    let ret = sim.regs.pc;
    write_x(sim, &args[0], ret, true)?;
    let off = imm20(&args[1])?;
    sim.regs.pc = sim.regs.pc.wrapping_add((off << 1) as u64).wrapping_sub(4);
    Ok(())
}

fn jalr(sim: &mut Simulator, args: &[String]) -> SimResult<()> {
    let ret = sim.regs.pc;
    write_x(sim, &args[0], ret, true)?;
    let base = read_x_arg(sim, &args[1])?;
    let imm = imm_raw(&args[2])?;
    sim.regs.pc = base.wrapping_add(imm);
    Ok(())
}

fn lui(sim: &mut Simulator, args: &[String]) -> SimResult<()> {
    let imm = imm20(&args[1])?;
    let value = (imm << 12) as u64;
    write_x(sim, &args[0], value, false)
}

fn auipc(sim: &mut Simulator, args: &[String]) -> SimResult<()> {
    let imm = imm20(&args[1])?;
    let value = sim.regs.pc.wrapping_add((imm << 12) as u64);
    write_x(sim, &args[0], value, false)
}

/// `step`: fetch, trace, and execute one instruction.
pub fn step(sim: &mut Simulator) -> StepResult {
    let inst_pc = sim.regs.pc;
    sim.regs.pc = inst_pc.wrapping_add(4);
    let cell = match sim.mem.cells.get(&inst_pc) {
        Some(c) => c.clone(),
        None => {
            sim.exec_log
                .push_str(&format!("ERROR: 0x{inst_pc:016x}: NO INSTRUCTION\n"));
            sim.regs.pc = inst_pc;
            return StepResult::Halted;
        }
    };
    let (op, args, source) = match &cell {
        rvs_memory::MemoryCell::Instruction(c) => {
            (c.op.clone(), c.args.clone(), c.source_line.clone())
        }
        rvs_memory::MemoryCell::Data {
            value,
            case,
            expr,
            source_line,
        } => (
            "DD".to_string(),
            vec![expr
                .clone()
                .unwrap_or_else(|| rvs_memory::render_value(*value, *case))],
            source_line.clone(),
        ),
    };
    // Trace line: address + hex operands (+ source when TXT is enabled).
    // Tracing is optional: `--no-trace` skips the per-instruction formatting,
    // which is what dominates batch throughput on long runs.
    if sim.trace {
        let mut trace = format!("0x{inst_pc:016x} {}", rvs_asm_syntax::fmti(&op, &args));
        if sim.view.exec_txt && !source.is_empty() {
            trace.push(' ');
            trace.push_str(&rvs_asm_syntax::render_line_f1(
                &rvs_asm_syntax::split_line(&source),
            ));
        }
        trace.push('\n');
        sim.exec_log.push_str(&trace);
    }
    // An instruction whose encoding failed is not executable. Report the
    // encoder diagnostic and halt where the reference's `eval` would error.
    if let rvs_memory::MemoryCell::Instruction(c) = &cell {
        if let Some(msg) = &c.encode_error {
            sim.exec_log.push_str(&format!("ERROR: {msg}\n"));
            sim.last_error.get_or_insert_with(|| msg.clone());
            sim.regs.pc = inst_pc;
            return StepResult::Halted;
        }
    }
    let op_upper = op.to_ascii_uppercase();
    let is_call = match op_upper.as_str() {
        "JAL" => args
            .first()
            .map(|r| r != "x0" && r != "zero")
            .unwrap_or(false),
        "JALR" => args
            .first()
            .map(|r| r != "x0" && r != "zero")
            .unwrap_or(false),
        _ => false,
    };

    let is_ret = op_upper == "JALR"
        && args
            .first()
            .map(|r| r == "x0" || r == "zero")
            .unwrap_or(false)
        && args.get(1).map(|r| r == "x1" || r == "ra").unwrap_or(false);

    let step_res = match execute(sim, &op, &args) {
        Ok(false) => StepResult::Continued,
        Ok(true) => StepResult::WaitingForInput,
        Err(e) => {
            let is_ebreak = e.0 == "ebreak\n";
            if is_ebreak {
                sim.exec_log.push_str("ebreak\n");
                sim.regs.pc = sim.regs.pc.wrapping_sub(4);
            } else {
                sim.exec_log.push_str(&format!("ERROR: {}\n", e.0));
                sim.last_error.get_or_insert_with(|| e.0.clone());
                sim.regs.pc = inst_pc;
            }
            StepResult::Halted
        }
    };
    // `exit` in the a7 ABI ends the run where the reference's `ebreak` would.
    if sim.exit_requested {
        return StepResult::Halted;
    }
    if is_call && sim.convention.enabled {
        let target_pc = sim.regs.pc;
        let caller_name = sim.find_symbol_name(inst_pc);
        let function_name = sim.find_symbol_name(target_pc);
        if let Some(w) =
            sim.convention
                .on_call(inst_pc, target_pc, caller_name, function_name, &sim.regs)
        {
            sim.exec_log.push_str(&format!("WARN [Convention]: {w}\n"));
        }
    }

    if is_ret && sim.convention.enabled {
        for w in sim.convention.on_return(inst_pc, &sim.regs) {
            sim.exec_log.push_str(&format!("WARN [Convention]: {w}\n"));
        }
    }

    // Update execution metrics
    sim.metrics.instructions += 1;
    sim.metrics.cycles += 1;
    match op_upper.as_str() {
        "BEQ" | "BNE" | "BLT" | "BGE" | "BLTU" | "BGEU" => {
            sim.metrics.branches_total += 1;
            if sim.regs.pc != inst_pc.wrapping_add(4) {
                sim.metrics.branches_taken += 1;
                sim.metrics.cycles += 1;
            }
        }
        "JAL" | "JALR" => {
            sim.metrics.jumps += 1;
        }
        "LD" | "LW" | "LH" | "LB" | "LWU" | "LHU" | "LBU" | "FLD" => {
            sim.metrics.loads += 1;
            sim.metrics.cycles += 1;
        }
        "SD" | "SW" | "SH" | "SB" | "FSD" => {
            sim.metrics.stores += 1;
        }
        "MUL" | "MULH" | "MULHSU" | "MULHU" | "DIV" | "DIVU" | "REM" | "REMU" | "MULW" | "DIVW"
        | "DIVUW" | "REMW" | "REMUW" => {
            sim.metrics.alu_ops += 1;
            sim.metrics.cycles += 2;
        }
        "ECALL" => {
            sim.metrics.ecalls += 1;
            sim.metrics.cycles += 4;
        }
        _ if op_upper.starts_with('F') => {
            sim.metrics.floats += 1;
        }
        _ => {
            sim.metrics.alu_ops += 1;
        }
    }

    step_res
}

/// `RunCmd` body: loop steps until halt, timeout, or an input request.
/// The caller appends the `START`/`RUN` marker and sets the PC.
pub fn run(sim: &mut Simulator, timeout_ms: u64) -> RunStatus {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    sim.last_error = None;
    let mut halted = false;
    loop {
        if sim.brk {
            break;
        }
        if Instant::now() >= deadline {
            sim.brk = true;
        }
        match step(sim) {
            StepResult::Continued => {}
            StepResult::Halted => halted = true,
            StepResult::WaitingForInput => return RunStatus::WaitingForInput,
        }
        if sim.brk {
            sim.exec_log.push_str("ERROR: Stopped");
            break;
        }
        if halted {
            break;
        }
    }
    if sim.brk {
        sim.brk = false;
        RunStatus::TimedOut
    } else {
        RunStatus::Completed
    }
}
