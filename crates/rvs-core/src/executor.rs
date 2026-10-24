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

use crate::memory::SimError;
use crate::memory::SimResult;
use crate::registers::parse_reg;
use crate::tcl;
use crate::Simulator;

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

fn reg_index(arg: &str) -> SimResult<u8> {
    parse_reg(arg).ok_or_else(|| {
        SimError(format!(
            "can't read \"::{}\": no such variable",
            arg.chars().next().unwrap_or('x')
        ))
    })
}

/// Read `::x{idx}` for an argument (the reference indexes the x-file through
/// `reg2i` regardless of the argument's prefix).
fn read_x_arg(sim: &Simulator, arg: &str) -> SimResult<u64> {
    let idx = reg_index(arg)?;
    Ok(sim.regs.read_x(idx))
}

/// Read `::{prefix}{idx}`: the prefix comes from the argument itself, so
/// `f3` reads the FP register and `x3` the integer register.
fn read_reg_arg(sim: &Simulator, arg: &str) -> SimResult<u64> {
    let idx = reg_index(arg)?;
    if arg.to_ascii_lowercase().starts_with('f') {
        Ok(sim.regs.read_f_bits(idx))
    } else {
        Ok(sim.regs.read_x(idx))
    }
}

fn write_x(sim: &mut Simulator, arg: &str, val: u64, upper: bool) -> SimResult<()> {
    let idx = reg_index(arg)?;
    sim.regs.write_x(idx, val, upper);
    Ok(())
}

fn write_reg_arg(sim: &mut Simulator, arg: &str, val: u64, upper: bool) -> SimResult<()> {
    let idx = reg_index(arg)?;
    if arg.to_ascii_lowercase().starts_with('f') {
        sim.regs.write_f_bits(idx, val, upper);
    } else {
        sim.regs.write_x(idx, val, upper);
    }
    Ok(())
}

/// The register's display string (case preserved), as Tcl would hold it.
fn reg_string_of(sim: &Simulator, arg: &str) -> String {
    match parse_reg(arg) {
        Some(idx) if arg.to_ascii_lowercase().starts_with('f') => sim.regs.f_string(idx),
        Some(idx) => sim.regs.x_string(idx),
        None => "0x0000000000000000".to_string(),
    }
}

/// `x12i` over the hex immediate string held in the memory cell.
fn imm12(arg: &str) -> SimResult<i64> {
    let v = tcl::parse_int(arg)
        .ok_or_else(|| SimError(format!("expected integer but got \"{arg}\"")))?;
    Ok(tcl::x12i(v as i64 as u64))
}

/// Raw immediate used by `JALR`, `FLD`, `FSD` (no sign extension: the
/// reference passes the hex string straight into `expr`).
fn imm_raw(arg: &str) -> SimResult<u64> {
    let v = tcl::parse_int(arg)
        .ok_or_else(|| SimError(format!("expected integer but got \"{arg}\"")))?;
    Ok(v as i64 as u64)
}

fn imm20(arg: &str) -> SimResult<i64> {
    let v = tcl::parse_int(arg)
        .ok_or_else(|| SimError(format!("expected integer but got \"{arg}\"")))?;
    Ok(tcl::x20i(v as i64 as u64))
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
            let amount = if unsigned { b as i128 } else { b as i64 as i128 };
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
    let mask = if nbytes == 8 { u64::MAX } else { (1u64 << (nbytes * 8)) - 1 };
    let val = reg_val & mask;
    let upper = sim.regs.x_case(idx);
    sim.mem.write_bytes(addr, nbytes, val, upper)
}

/// `Lmem`: load and sign-extend unless `unsigned`.
fn lmem(sim: &mut Simulator, args: &[String], nbytes: usize, unsigned: bool) -> SimResult<()> {
    let base = read_x_arg(sim, &args[1])?;
    let addr = base.wrapping_add(imm12(&args[2])? as u64);
    let bits = sim.mem.read_bytes(addr, nbytes)?;
    let value = if !unsigned && (bits >> (nbytes * 8 - 1)) & 1 == 1 {
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
        sim.regs.pc = sim
            .regs
            .pc
            .wrapping_add((off << 1) as u64)
            .wrapping_sub(4);
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
    let sa = if signed_a { a as i64 as i128 } else { a as i128 };
    let sb = if signed_b { b as i64 as i128 } else { b as i128 };
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
            let q = if sa % sb != 0 && ((sa < 0) != (sb < 0)) { q - 1 } else { q };
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Lt,
    Le,
}

/// The double value of an FP-register argument (`x2f` of its hex string);
/// `None` reproduces Tcl's empty-operand error.
fn freg_value(sim: &Simulator, arg: &str) -> SimResult<Option<f64>> {
    let idx = reg_index(arg)?;
    Ok(tcl::x2f(&sim.regs.f_string(idx)))
}

fn fp_operand_err(op: &str) -> SimError {
    SimError(format!(
        "can't use non-numeric floating-point value as operand of \"{op}\""
    ))
}

/// `typeFR`: float ALU; comparison results go to the integer register file.
fn type_fr(sim: &mut Simulator, args: &[String], op: FOp) -> SimResult<()> {
    let op_char = match op {
        FOp::Add => "+",
        FOp::Sub => "-",
        FOp::Mul => "*",
        FOp::Div => "/",
        FOp::Eq => "==",
        FOp::Lt => "<",
        FOp::Le => "<=",
    };
    let a = freg_value(sim, &args[1])?.ok_or_else(|| fp_operand_err(op_char))?;
    let b = freg_value(sim, &args[2])?.ok_or_else(|| fp_operand_err(op_char))?;
    if (a.is_nan() || b.is_nan()) && !matches!(op, FOp::Eq | FOp::Lt | FOp::Le) {
        return Err(fp_operand_err(op_char));
    }
    if a.is_nan() || b.is_nan() {
        write_x(sim, &args[0], 0, true)?;
        return Ok(());
    }
    let result: f64 = match op {
        FOp::Add | FOp::Sub | FOp::Mul => {
            let r = match op {
                FOp::Add => a + b,
                FOp::Sub => a - b,
                _ => a * b,
            };
            if r.is_nan() {
                return Err(SimError(
                    "domain error: argument not in valid range".to_string(),
                ));
            }
            r
        }
        FOp::Div => {
            if b == 0.0 && a == 0.0 {
                return Err(SimError(
                    "domain error: argument not in valid range".to_string(),
                ));
            }
            a / b
        }
        FOp::Eq => f64::from(a == b),
        FOp::Lt => f64::from(a < b),
        FOp::Le => f64::from(a <= b),
    };
    let rd_is_f = args[0].to_ascii_lowercase().starts_with('f');
    if rd_is_f {
        let idx = reg_index(&args[0])?;
        sim.regs.write_f64(idx, result, false);
    } else {
        write_x(sim, &args[0], result as u64, true)?;
    }
    Ok(())
}

/// `FMIN.D` / `FMAX.D` via Tcl's `min`/`max` (NaN raises an error).
fn fminmax(sim: &mut Simulator, args: &[String], is_max: bool) -> SimResult<()> {
    let a = freg_value(sim, &args[1])?
        .filter(|v| !v.is_nan())
        .ok_or_else(|| SimError("floating point value is Not a Number".to_string()))?;
    let b = freg_value(sim, &args[2])?
        .filter(|v| !v.is_nan())
        .ok_or_else(|| SimError("floating point value is Not a Number".to_string()))?;
    let result = if is_max { a.max(b) } else { a.min(b) };
    let idx = reg_index(&args[0])?;
    sim.regs.write_f64(idx, result, false);
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SgnMode {
    J,
    Jn,
    Jx,
}

/// Sign-injection family implemented exactly like the reference (via
/// `abs`/comparisons, so `+0.0`/`-0.0` corners follow Tcl, not IEEE).
fn fsgnj(sim: &mut Simulator, args: &[String], mode: SgnMode) -> SimResult<()> {
    let a = freg_value(sim, &args[1])?
        .filter(|v| !v.is_nan())
        .ok_or_else(|| SimError("floating point value is Not a Number".to_string()))?;
    let b = freg_value(sim, &args[2])?
        .filter(|v| !v.is_nan())
        .ok_or_else(|| SimError("floating point value is Not a Number".to_string()))?;
    let mut result = b.abs();
    match mode {
        SgnMode::J => {
            if a < 0.0 {
                result = -result.abs();
            }
        }
        SgnMode::Jn => {
            if a > 0.0 {
                result = -result.abs();
            }
        }
        SgnMode::Jx => {
            let same = (a > 0.0 && b > 0.0) || (a < 0.0 && b < 0.0);
            result = if same { result.abs() } else { -result.abs() };
        }
    }
    let idx = reg_index(&args[0])?;
    sim.regs.write_f64(idx, result, false);
    Ok(())
}

/// `FCVT.L.D` / `FCVT.LU.D`: round to nearest (half away from zero).
fn fcvt_to_int(sim: &mut Simulator, args: &[String], unsigned: bool) -> SimResult<()> {
    let idx = reg_index(&args[1])?;
    let raw = sim.regs.f_string(idx);
    let v = tcl::x2f(&raw).ok_or_else(|| SimError(String::new()))?;
    if v.is_nan() {
        return Err(SimError("floating point value is Not a Number".to_string()));
    }
    let rounded = v.round() as i128;
    let (lo, hi) = if unsigned {
        (0i128, (1u128 << 64) as i128 - 1)
    } else {
        (i64::MIN as i128, i64::MAX as i128)
    };
    if rounded < lo || rounded > hi {
        return Err(SimError("ERROR: value out of range".to_string()));
    }
    write_x(sim, &args[0], tcl::low64(rounded), false)
}

/// `FCVT.D.L` / `FCVT.D.LU`: integer register to double.
fn fcvt_to_float(sim: &mut Simulator, args: &[String], unsigned: bool) -> SimResult<()> {
    let raw = read_reg_arg(sim, &args[1])?;
    let value = if unsigned { raw as f64 } else { raw as i64 as f64 };
    let idx = reg_index(&args[0])?;
    sim.regs.write_f64(idx, value, false);
    Ok(())
}

/// `FMV.X.D`: the x register receives `i2x` of the FP register *string*, so
/// it holds the byte-swapped bit pattern (the digit value).
fn fmv_x_d(sim: &mut Simulator, args: &[String]) -> SimResult<()> {
    let idx = reg_index(&args[1])?;
    let digits = sim.regs.read_f_bits(idx);
    write_x(sim, &args[0], digits, false)
}

/// `FMV.D.X`: the FP register receives `i2x` of the integer register's
/// value, so its bits are the byte-swap of that value.
fn fmv_d_x(sim: &mut Simulator, args: &[String]) -> SimResult<()> {
    let value = read_reg_arg(sim, &args[1])?;
    let idx = reg_index(&args[0])?;
    sim.regs.write_f_bits(idx, value, false);
    Ok(())
}

/// `FSQRT.D`; NaN and negative operands raise Tcl's errors.
fn fsqrt(sim: &mut Simulator, args: &[String]) -> SimResult<()> {
    let idx = reg_index(&args[1])?;
    let raw = sim.regs.f_string(idx);
    let v = tcl::x2f(&raw).ok_or_else(|| SimError(String::new()))?;
    if v.is_nan() {
        return Err(SimError("floating point value is Not a Number".to_string()));
    }
    if v < 0.0 {
        return Err(SimError(
            "domain error: argument not in valid range".to_string(),
        ));
    }
    let idx_rd = reg_index(&args[0])?;
    sim.regs.write_f64(idx_rd, v.sqrt(), false);
    Ok(())
}

/// `FLmem` / `FLD`: load a doubleword into an FP register. The FP register
/// string is `a64("0x" + digits)`, so its bits are the byte-swapped cell.
fn flmem(sim: &mut Simulator, args: &[String], nbytes: usize) -> SimResult<()> {
    let base = read_x_arg(sim, &args[1])?;
    let addr = base.wrapping_add(imm_raw(&args[2])?);
    let cell = sim.mem.read_bytes(addr, nbytes)?;
    let idx = reg_index(&args[0])?;
    sim.regs.write_f_bits(idx, cell, false);
    Ok(())
}

/// `FSmem` / `FSD`: store FP bits; the cell receives the swapped digits.
fn fsmem(sim: &mut Simulator, args: &[String], nbytes: usize) -> SimResult<()> {
    let base = read_x_arg(sim, &args[1])?;
    let addr = base.wrapping_add(imm_raw(&args[2])?);
    let idx = reg_index(&args[0])?;
    let bits = sim.regs.read_f_bits(idx);
    let mask = if nbytes == 8 { u64::MAX } else { (1u64 << (nbytes * 8)) - 1 };
    let val = bits & mask;
    sim.mem
        .write_bytes(addr, nbytes, val, sim.regs.f_case(idx))
}

/// `EBREAK`: advance past the instruction and halt with `ebreak\n`.
fn ebreak(sim: &mut Simulator) -> SimResult<()> {
    sim.regs.pc = sim.regs.pc.wrapping_add(4);
    Err(SimError("ebreak\n".to_string()))
}

/// Parse a Tcl double for float inputs (`f2x` accepts hex integers too).
fn parse_tcl_double(s: &str) -> Option<f64> {
    if let Ok(v) = s.trim().parse::<f64>() {
        return Some(v);
    }
    tcl::parse_int(s).map(|v| v as f64)
}

/// Evaluate a raw expression the way `a64` does (`expr` without labels).
fn expr_value(s: &str) -> SimResult<u64> {
    if s.trim().is_empty() {
        return Err(SimError(format!("expected integer but got \"{s}\"")));
    }
    match crate::assembler::tcl_expr(s) {
        Some(v) => Ok(tcl::low64(v)),
        None => Err(SimError(format!("invalid bareword \"{s}\""))),
    }
}

/// The little-endian character sequence of a 64-bit value (what
/// `x2s(revx(...))` produces in the reference).
fn chars_of(value: u64) -> String {
    let mut s = String::new();
    for k in 0..8usize {
        s.push(char::from_u32(((value >> (8 * k)) & 0xff) as u32).unwrap_or('\0'));
    }
    s
}

/// Pack the first eight bytes of `s` little-endian (`revx(s2x(...))`).
fn pack_chars(s: &str) -> u64 {
    let mut v = 0u64;
    for (k, b) in s.as_bytes().iter().take(8).enumerate() {
        v |= (*b as u64) << (8 * k);
    }
    v
}

/// `ECALL`: syscalls 0-4 output, 5-9 input.
fn ecall(sim: &mut Simulator, args: &[String]) -> Result<bool, SimError> {
    let rs1a = read_reg_arg(sim, &args[1])?;
    let imm = imm_raw(&args[2])?;
    if imm <= 4 {
        let rs1_string = reg_string_of(sim, &args[1]);
        let res: String = match imm {
            0 => format!("{}", rs1a as i64),
            1 => match tcl::x2f(&rs1_string) {
                Some(v) => tcl::tcl_double_to_string(v),
                None => String::new(),
            },
            2 => rs1_string,
            3 => tcl::trim_nul(&chars_of(rs1a)),
            4 => sim.mem.get_c_string(rs1a)?,
            _ => unreachable!(),
        };
        if args[0] == "x0" {
            sim.output.push_str(&res);
            sim.output.push('\n');
        } else {
            sim.output.push_str(&res);
        }
        return Ok(false);
    }
    if (5..=9).contains(&imm) {
        let prompt = tcl::trim_nul(&chars_of(rs1a));
        sim.output.push_str(&prompt);
        let queue = match &mut sim.input_lines {
            Some(q) => q,
            None => {
                sim.pending = Some(PendingInput {
                    rd_arg: args[0].clone(),
                    rs1_arg: args[1].clone(),
                    imm,
                    inst_pc: sim.regs.pc.wrapping_sub(4),
                });
                return Ok(true);
            }
        };
        let line = queue.pop_front().unwrap_or_default();
        sim.output.push_str(&line);
        sim.output.push('\n');
        match imm {
            5 => {
                let v = tcl::parse_int(&line)
                    .ok_or_else(|| SimError(format!("expected integer but got \"{line}\"")))?;
                write_x(sim, &args[0], tcl::low64(v), false)?;
            }
            6 => {
                let f = parse_tcl_double(&line)
                    .ok_or_else(|| SimError(format!("expected floating-point number but got \"{line}\"")))?;
                write_reg_arg(sim, &args[0], f.to_bits(), false)?;
            }
            7 => {
                let v = expr_value(&line)?;
                write_x(sim, &args[0], v, false)?;
            }
            8 => {
                write_x(sim, &args[0], pack_chars(&line), false)?;
            }
            9 => {
                let dest = read_reg_arg(sim, &args[0])?;
                sim.mem.put_c_string(dest, &format!("{line}\0"))?;
            }
            _ => unreachable!(),
        }
        return Ok(false);
    }
    // Immediates outside 0..9 do nothing at all (neither branch matches).
    Ok(false)
}

/// One `Output` poll tick: serve the DMA OUT control/data registers.
pub fn poll_dma_output(sim: &mut Simulator) {
    if !sim.view.dma_out || !sim.mem.is_dd(0) {
        return;
    }
    let ctrl = match sim.mem.dd_hex(0) {
        Some(c) => c,
        None => return,
    };
    if ctrl == "0x0000000000000000" {
        return;
    }
    let m01 = tcl::x2s(&ctrl);
    let m11 = match sim.mem.dd_hex(8) {
        Some(v) => v,
        None => return, // Tcl's callback would error out silently
    };
    let res: String = if m01.contains('I') {
        tcl::parse_int(&m11).map(|v| format!("{}", v as u64)).unwrap_or_default()
    } else if m01.contains('F') {
        tcl::x2f(&m11)
            .map(tcl::tcl_double_to_string)
            .unwrap_or_default()
    } else if m01.contains('C') {
        tcl::x2s(&tcl::revx(&m11))
    } else if m01.contains('S') {
        let addr = tcl::parse_int(&m11).map(|v| v as u64).unwrap_or(0);
        match sim.mem.get_c_string(addr) {
            Ok(s) => s,
            Err(e) => e.0,
        }
    } else if m01.contains('X') {
        m11.clone()
    } else {
        format!("WRONG CONTROL:{m01}")
    };
    sim.output.push_str(&res);
    sim.output.push('\n');
    sim.mem.set_dd_value(0, 0, 0);
}

/// One `Input` poll tick: detect a DMA input request and show its prompt.
pub fn poll_dma_input(sim: &mut Simulator) {
    if !sim.view.dma_inp || !sim.mem.is_dd(0x10) {
        return;
    }
    let ctrl = match sim.mem.dd_hex(0x10) {
        Some(c) => c,
        None => return,
    };
    if ctrl == "0x0000000000000000" {
        return;
    }
    let m01 = tcl::x2s(&ctrl);
    let dtype = if m01.contains('I') {
        "I"
    } else if m01.contains('F') {
        "F"
    } else if m01.contains('C') {
        "C"
    } else if m01.contains('S') {
        "S"
    } else {
        ""
    };
    sim.entry_type = dtype.to_string();
    if !sim.inp_prompt && !dtype.is_empty() {
        sim.output.push_str(&format!("\n{dtype}?"));
        sim.inp_prompt = true;
    }
}

/// `EntryInputCmd`: deliver a typed line to a pending DMA input request.
pub fn submit_dma_input(sim: &mut Simulator, line: &str) -> SimResult<()> {
    let dtype = sim.entry_type.clone();
    let m11 = sim.mem.dd_hex(0x18).unwrap_or_default();
    match dtype.as_str() {
        "I" => {
            let v = match tcl::parse_int(line) {
                Some(v) => v,
                None => {
                    sim.output
                        .push_str(&format!("{line}<-ERROR: NOT AN INTEGER\n"));
                    sim.inp_prompt = false;
                    return Ok(());
                }
            };
            sim.mem.set_dd_value(0x18, v as u64, 0);
        }
        "F" => {
            let f = match parse_tcl_double(line) {
                Some(f) => f,
                None => {
                    sim.output
                        .push_str(&format!("{line}<-ERROR: NOT A FLOAT\n"));
                    sim.inp_prompt = false;
                    return Ok(());
                }
            };
            sim.mem.set_dd_value(0x18, f.to_bits(), 0);
        }
        "C" => {
            sim.mem.set_dd_value(0x18, pack_chars(&line), 0);
        }
        "S" => {
            let dest = tcl::parse_int(&m11).map(|v| v as u64).unwrap_or(0);
            let res = sim.mem.put_c_string(dest, &format!("{line}\0"))?;
            sim.output.push_str(&format!("{line}\n{res}\n"));
            sim.inp_prompt = false;
            sim.entry_type.clear();
            return Ok(());
        }
        _ => {}
    }
    sim.mem.set_dd_value(0x10, 0, 0);
    sim.output.push_str(line);
    sim.output.push('\n');
    sim.inp_prompt = false;
    sim.entry_type.clear();
    Ok(())
}

/// Deliver a typed line to a pending `ECALL` input request.
pub fn submit_ecall_input(sim: &mut Simulator, line: &str) -> SimResult<()> {
    let pending = match sim.pending.take() {
        Some(p) => p,
        None => return Ok(()),
    };
    sim.output.push_str(line);
    sim.output.push('\n');
    let args = vec![pending.rd_arg.clone(), pending.rs1_arg.clone()];
    match pending.imm {
        5 => {
            let v = tcl::parse_int(line)
                .ok_or_else(|| SimError(format!("expected integer but got \"{line}\"")))?;
            write_x(sim, &args[0], tcl::low64(v), false)?;
        }
        6 => {
            let f = parse_tcl_double(line)
                .ok_or_else(|| SimError(format!("expected floating-point number but got \"{line}\"")))?;
            write_reg_arg(sim, &args[0], f.to_bits(), false)?;
        }
        7 => {
            let v = expr_value(line)?;
            write_x(sim, &args[0], v, false)?;
        }
        8 => {
            write_x(sim, &args[0], pack_chars(&line), false)?;
        }
        9 => {
            let dest = read_reg_arg(sim, &args[0])?;
            sim.mem.put_c_string(dest, &format!("{line}\0"))?;
        }
        _ => {}
    }
    Ok(())
}

/// Dispatch `string toupper op` to the executor procs. Returns `true` when
/// the instruction now waits for interactive input.
type ExecResult = Result<bool, SimError>;

fn execute(sim: &mut Simulator, op: &str, args: &[String]) -> ExecResult {
    let upper = op.to_ascii_uppercase();
    let s: Vec<String> = args.to_vec();
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
        "ECALL" => return ecall(sim, &s).map(|needs| needs),
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
    sim.regs.pc = sim
        .regs
        .pc
        .wrapping_add((off << 1) as u64)
        .wrapping_sub(4);
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
        crate::memory::MemoryCell::Instruction(c) => {
            (c.op.clone(), c.args.clone(), c.source_line.clone())
        }
        crate::memory::MemoryCell::Data {
            value,
            case,
            expr,
            source_line,
        } => (
            "DD".to_string(),
            vec![expr
                .clone()
                .unwrap_or_else(|| crate::memory::render_value(*value, *case))],
            source_line.clone(),
        ),
    };
    // Trace line: address + hex operands (+ source when TXT is enabled).
    let mut trace = format!(
        "0x{inst_pc:016x} {}",
        crate::assembler::fmti(&op, &args)
    );
    if sim.view.exec_txt && !source.is_empty() {
        trace.push(' ');
        trace.push_str(&crate::assembler::render_line_f1(&crate::assembler::split_line(
            &source,
        )));
    }
    trace.push('\n');
    sim.exec_log.push_str(&trace);
    let op_upper = op.to_ascii_uppercase();
    let is_call = match op_upper.as_str() {
        "JAL" => args.first().map(|r| r != "x0" && r != "zero").unwrap_or(false),
        "JALR" => args.first().map(|r| r != "x0" && r != "zero").unwrap_or(false),
        _ => false,
    };

    let is_ret = op_upper == "JALR"
        && args.first().map(|r| r == "x0" || r == "zero").unwrap_or(false)
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
                sim.regs.pc = inst_pc;
            }
            StepResult::Halted
        }
    };
    if is_call && sim.convention.enabled {
        let target_pc = sim.regs.pc;
        let caller_name = sim.find_symbol_name(inst_pc);
        let function_name = sim.find_symbol_name(target_pc);
        if let Some(w) = sim.convention.on_call(inst_pc, target_pc, caller_name, function_name, &sim.regs) {
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
        "MUL" | "MULH" | "MULHSU" | "MULHU" | "DIV" | "DIVU" | "REM" | "REMU"
        | "MULW" | "DIVW" | "DIVUW" | "REMW" | "REMUW" => {
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
