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
use rvs_memory::{AluOp, BranchOp, DecodedInstr, MemoryCell, RmKind, SimError, SimResult, WKind};
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

fn compute_rm(a: u64, b: u64, kind: RmKind) -> SimResult<u64> {
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
    match kind {
        RmKind::Mul | RmKind::MulH | RmKind::MulHSu | RmKind::MulHu => {
            let product = if kind == RmKind::MulHu {
                (a as u128).wrapping_mul(b as u128) as i128
            } else {
                sa.wrapping_mul(sb)
            };
            if high {
                Ok((product >> 64) as u64)
            } else {
                Ok(tcl::low64(product))
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
                Ok(tcl::low64(q))
            } else {
                Ok(tcl::low64(sa - q * sb))
            }
        }
        RmKind::DivU => {
            if sb == 0 {
                return Err(SimError("divide by zero".to_string()));
            }
            Ok(tcl::low64(sa / sb))
        }
        RmKind::RemU => {
            if sb == 0 {
                return Err(SimError("divide by zero".to_string()));
            }
            Ok(tcl::low64(sa % sb))
        }
    }
}

/// `typeRM`: multiply/divide with 128-bit intermediates.
fn type_rm(sim: &mut Simulator, args: &[String], kind: RmKind) -> SimResult<()> {
    let a = read_x_arg(sim, &args[1])?;
    let b = read_x_arg(sim, &args[2])?;
    let value = compute_rm(a, b, kind)?;
    write_x(sim, &args[0], value, false)
}

fn compute_wi(a: u64, imm: i64, kind: WKind) -> u64 {
    let aw = a as u32 as i32;
    match kind {
        WKind::Add => (aw.wrapping_add(imm as i32)) as i64 as u64,
        WKind::Sll => (aw.wrapping_shl(imm as u32 & 31)) as i64 as u64,
        WKind::Srl => (((a as u32).wrapping_shr(imm as u32 & 31)) as i32) as i64 as u64,
        WKind::Sra => (aw >> (imm as u32 & 31)) as i64 as u64,
        _ => 0,
    }
}

/// 32-bit immediate word variants.
fn type_wi(sim: &mut Simulator, args: &[String], kind: WKind) -> SimResult<()> {
    let a = read_x_arg(sim, &args[1])?;
    let imm = imm12(&args[2])?;
    let value = compute_wi(a, imm, kind);
    write_x(sim, &args[0], value, true)
}

#[expect(
    clippy::manual_checked_ops,
    reason = "RVS/Tcl semantics define exact fallbacks for 0 and MIN/-1"
)]
fn compute_w(a: u64, b: u64, kind: WKind) -> u64 {
    let aw = a as u32 as i32;
    let bw = b as u32 as i32;
    match kind {
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
    }
}

/// 32-bit word variants added by this rewrite (standard RV64 semantics).
fn type_w(sim: &mut Simulator, args: &[String], kind: WKind) -> SimResult<()> {
    let a = read_x_arg(sim, &args[1])?;
    let b = read_x_arg(sim, &args[2])?;
    let value = compute_w(a, b, kind);
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
    let need = match upper.as_str() {
        "JAL" | "LUI" | "AUIPC" => 2,
        "EBREAK" => 0,
        _ => 3,
    };
    if args.len() < need && rvs_asm_syntax::find_instr(op).is_some() {
        return Err(SimError(format!("MUST HAVE {need} ARGS")));
    }
    let r = match upper.as_str() {
        "ADD" => type_r(sim, args, AluOp::Add, false),
        "SUB" => type_r(sim, args, AluOp::Sub, false),
        "AND" => type_r(sim, args, AluOp::And, false),
        "OR" => type_r(sim, args, AluOp::Or, false),
        "XOR" => type_r(sim, args, AluOp::Xor, false),
        "SLL" => type_r(sim, args, AluOp::Sll, false),
        "SRL" => type_r(sim, args, AluOp::Srl, true),
        "SRA" => type_r(sim, args, AluOp::Sra, false),
        "SLT" => type_r(sim, args, AluOp::Slt, false),
        "SLTU" => type_r(sim, args, AluOp::Slt, true),
        "ADDI" => type_i(sim, args, AluOp::Add, false),
        "SUBI" => type_i(sim, args, AluOp::Sub, false),
        "ANDI" => type_i(sim, args, AluOp::And, false),
        "ORI" => type_i(sim, args, AluOp::Or, false),
        "XORI" => type_i(sim, args, AluOp::Xor, false),
        "SLLI" => type_i(sim, args, AluOp::Sll, false),
        "SRLI" => type_i(sim, args, AluOp::Srl, true),
        "SRAI" => type_i(sim, args, AluOp::Sra, false),
        "SLTI" => type_i(sim, args, AluOp::Slt, false),
        "SLTIU" => type_i(sim, args, AluOp::Slt, true),
        "SD" => smem(sim, args, 8),
        "SW" => smem(sim, args, 4),
        "SH" => smem(sim, args, 2),
        "SB" => smem(sim, args, 1),
        "LD" => lmem(sim, args, 8, false),
        "LW" => lmem(sim, args, 4, false),
        "LWU" => lmem(sim, args, 4, true),
        "LH" => lmem(sim, args, 2, false),
        "LHU" => lmem(sim, args, 2, true),
        "LB" => lmem(sim, args, 1, false),
        "LBU" => lmem(sim, args, 1, true),
        "BEQ" => type_sb(sim, args, BranchOp::Eq, false),
        "BNE" => type_sb(sim, args, BranchOp::Ne, false),
        "BGE" => type_sb(sim, args, BranchOp::Ge, false),
        "BGEU" => type_sb(sim, args, BranchOp::GeU, true),
        "BLT" => type_sb(sim, args, BranchOp::Lt, false),
        "BLTU" => type_sb(sim, args, BranchOp::LtU, true),
        "JAL" => jal(sim, args),
        "JALR" => jalr(sim, args),
        "LUI" => lui(sim, args),
        "AUIPC" => auipc(sim, args),
        "EBREAK" => ebreak(sim),
        "ECALL" => return ecall(sim, args),
        "MUL" => type_rm(sim, args, RmKind::Mul),
        "MULH" => type_rm(sim, args, RmKind::MulH),
        "MULHSU" => type_rm(sim, args, RmKind::MulHSu),
        "MULHU" => type_rm(sim, args, RmKind::MulHu),
        "DIV" => type_rm(sim, args, RmKind::Div),
        "DIVU" => type_rm(sim, args, RmKind::DivU),
        "REM" => type_rm(sim, args, RmKind::Rem),
        "REMU" => type_rm(sim, args, RmKind::RemU),
        "ADDIW" => type_wi(sim, args, WKind::Add),
        "SLLIW" => type_wi(sim, args, WKind::Sll),
        "SRLIW" => type_wi(sim, args, WKind::Srl),
        "SRAIW" => type_wi(sim, args, WKind::Sra),
        "ADDW" => type_w(sim, args, WKind::Add),
        "SUBW" => type_w(sim, args, WKind::Sub),
        "SLLW" => type_w(sim, args, WKind::Sll),
        "SRLW" => type_w(sim, args, WKind::Srl),
        "SRAW" => type_w(sim, args, WKind::Sra),
        "MULW" => type_w(sim, args, WKind::Mul),
        "DIVW" => type_w(sim, args, WKind::Div),
        "DIVUW" => type_w(sim, args, WKind::DivU),
        "REMW" => type_w(sim, args, WKind::Rem),
        "REMUW" => type_w(sim, args, WKind::RemU),
        "FADD.D" => type_fr(sim, args, FOp::Add),
        "FSUB.D" => type_fr(sim, args, FOp::Sub),
        "FMUL.D" => type_fr(sim, args, FOp::Mul),
        "FDIV.D" => type_fr(sim, args, FOp::Div),
        "FEQ.D" => type_fr(sim, args, FOp::Eq),
        "FLT.D" => type_fr(sim, args, FOp::Lt),
        "FLE.D" => type_fr(sim, args, FOp::Le),
        "FMIN.D" => fminmax(sim, args, false),
        "FMAX.D" => fminmax(sim, args, true),
        "FSGNJ.D" => fsgnj(sim, args, SgnMode::J),
        "FSGNJN.D" => fsgnj(sim, args, SgnMode::Jn),
        "FSGNJX.D" => fsgnj(sim, args, SgnMode::Jx),
        "FCVT.L.D" => fcvt_to_int(sim, args, false),
        "FCVT.LU.D" => fcvt_to_int(sim, args, true),
        "FCVT.D.L" => fcvt_to_float(sim, args, false),
        "FCVT.D.LU" => fcvt_to_float(sim, args, true),
        "FMV.X.D" => fmv_x_d(sim, args),
        "FMV.D.X" => fmv_d_x(sim, args),
        "FSQRT.D" => fsqrt(sim, args),
        "FLD" => flmem(sim, args, 8),
        "FSD" => fsmem(sim, args, 8),
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

fn execute_fast(sim: &mut Simulator, dec: DecodedInstr) -> SimResult<bool> {
    match dec {
        DecodedInstr::AluR {
            rd,
            rs1,
            rs2,
            op,
            unsigned,
        } => {
            let a = sim.regs.read_x(rs1);
            let b = sim.regs.read_x(rs2);
            let value = alu(op, a, b, unsigned)?;
            sim.regs.write_x(rd, value, true);
            Ok(false)
        }
        DecodedInstr::AluI {
            rd,
            rs1,
            imm,
            op,
            unsigned,
        } => {
            let a = sim.regs.read_x(rs1);
            let value = if unsigned {
                let imm_u = if imm < 0 {
                    (4096i64 + imm) as u64
                } else {
                    imm as u64
                };
                alu(op, a, imm_u, true)?
            } else {
                alu(op, a, imm as u64, false)?
            };
            sim.regs.write_x(rd, value, true);
            Ok(false)
        }
        DecodedInstr::AluW { rd, rs1, rs2, kind } => {
            let a = sim.regs.read_x(rs1);
            let b = sim.regs.read_x(rs2);
            let value = compute_w(a, b, kind);
            let upper = matches!(
                kind,
                WKind::Add | WKind::Sub | WKind::Sll | WKind::Srl | WKind::Sra | WKind::Mul
            );
            sim.regs.write_x(rd, value, upper);
            Ok(false)
        }
        DecodedInstr::AluWI { rd, rs1, imm, kind } => {
            let a = sim.regs.read_x(rs1);
            let value = compute_wi(a, imm, kind);
            sim.regs.write_x(rd, value, true);
            Ok(false)
        }
        DecodedInstr::AluRM { rd, rs1, rs2, kind } => {
            let a = sim.regs.read_x(rs1);
            let b = sim.regs.read_x(rs2);
            let value = compute_rm(a, b, kind)?;
            sim.regs.write_x(rd, value, false);
            Ok(false)
        }
        DecodedInstr::Branch {
            rs1,
            rs2,
            off,
            op,
            unsigned,
        } => {
            let a = sim.regs.read_x(rs1);
            let b = sim.regs.read_x(rs2);
            let taken = match (op, unsigned) {
                (BranchOp::Eq, _) => a == b,
                (BranchOp::Ne, _) => a != b,
                (BranchOp::Ge, false) => a as i64 >= b as i64,
                (BranchOp::Ge, true) | (BranchOp::GeU, _) => a >= b,
                (BranchOp::Lt, false) => (a as i64) < (b as i64),
                (BranchOp::Lt, true) | (BranchOp::LtU, _) => a < b,
            };
            if taken {
                sim.regs.pc = sim.regs.pc.wrapping_add((off << 1) as u64).wrapping_sub(4);
            }
            Ok(false)
        }
        DecodedInstr::Load {
            rd,
            rs1,
            offset,
            nbytes,
            unsigned,
        } => {
            let base = sim.regs.read_x(rs1);
            let addr = base.wrapping_add(offset as u64);
            let bits = sim.mem.read_bytes(addr, nbytes as usize)?;
            let value = if !unsigned && nbytes < 8 && (bits >> (nbytes * 8 - 1)) & 1 == 1 {
                bits | !((1u64 << (nbytes * 8)) - 1)
            } else {
                bits
            };
            sim.regs.write_x(rd, value, false);
            Ok(false)
        }
        DecodedInstr::Store {
            rs1,
            rs2,
            offset,
            nbytes,
        } => {
            let base = sim.regs.read_x(rs2);
            let addr = base.wrapping_add(offset as u64);
            let reg_val = sim.regs.read_x(rs1);
            let mask = if nbytes == 8 {
                u64::MAX
            } else {
                (1u64 << (nbytes * 8)) - 1
            };
            let val = reg_val & mask;
            let upper = sim.regs.x_case(rs1);
            sim.mem.write_bytes(addr, nbytes as usize, val, upper)?;
            Ok(false)
        }
        DecodedInstr::Jal { rd, off } => {
            let ret = sim.regs.pc;
            sim.regs.write_x(rd, ret, true);
            sim.regs.pc = sim.regs.pc.wrapping_add((off << 1) as u64).wrapping_sub(4);
            Ok(false)
        }
        DecodedInstr::Jalr { rd, rs1, imm } => {
            let ret = sim.regs.pc;
            sim.regs.write_x(rd, ret, true);
            let base = sim.regs.read_x(rs1);
            sim.regs.pc = base.wrapping_add(imm);
            Ok(false)
        }
        DecodedInstr::Lui { rd, imm } => {
            let value = (imm << 12) as u64;
            sim.regs.write_x(rd, value, false);
            Ok(false)
        }
        DecodedInstr::Auipc { rd, imm } => {
            let value = sim.regs.pc.wrapping_add((imm << 12) as u64);
            sim.regs.write_x(rd, value, false);
            Ok(false)
        }
        DecodedInstr::Ebreak => ebreak(sim).map(|_| false),
        DecodedInstr::Ecall { .. } | DecodedInstr::Other => unreachable!(),
    }
}

/// `step`: fetch, trace, and execute one instruction.
#[expect(
    clippy::cognitive_complexity,
    reason = "instruction cycle dispatcher handles tracing, fast/slow path, conventions, and metrics"
)]
pub fn step(sim: &mut Simulator) -> StepResult {
    let inst_pc = sim.regs.pc;
    sim.regs.pc = inst_pc.wrapping_add(4);

    let (decoded, is_err) = match sim.mem.cells.get(&inst_pc) {
        Some(MemoryCell::Instruction(c)) => {
            if let Some(msg) = &c.encode_error {
                (None, Some(msg.clone()))
            } else {
                (
                    c.decoded
                        .or_else(|| rvs_sim::decode_instruction(&c.op, &c.args)),
                    None,
                )
            }
        }
        Some(MemoryCell::Data { .. }) => (None, None),
        None => {
            sim.exec_log
                .push_str(&format!("ERROR: 0x{inst_pc:016x}: NO INSTRUCTION\n"));
            sim.regs.pc = inst_pc;
            return StepResult::Halted;
        }
    };

    if let Some(msg) = is_err {
        sim.exec_log.push_str(&format!("ERROR: {msg}\n"));
        sim.last_error.get_or_insert(msg);
        sim.regs.pc = inst_pc;
        return StepResult::Halted;
    }

    if sim.trace {
        if let Some(cell) = sim.mem.cells.get(&inst_pc) {
            match cell {
                MemoryCell::Instruction(c) => {
                    let mut trace =
                        format!("0x{inst_pc:016x} {}", rvs_asm_syntax::fmti(&c.op, &c.args));
                    if sim.view.exec_txt && !c.source_line.is_empty() {
                        trace.push(' ');
                        trace.push_str(&rvs_asm_syntax::render_line_f1(
                            &rvs_asm_syntax::split_line(&c.source_line),
                        ));
                    }
                    trace.push('\n');
                    sim.exec_log.push_str(&trace);
                }
                MemoryCell::Data {
                    value,
                    case,
                    expr,
                    source_line,
                } => {
                    let arg = expr
                        .as_deref()
                        .map(str::to_string)
                        .unwrap_or_else(|| rvs_memory::render_value(*value, *case));
                    let mut trace =
                        format!("0x{inst_pc:016x} {}", rvs_asm_syntax::fmti("DD", &[arg]));
                    if sim.view.exec_txt && !source_line.is_empty() {
                        trace.push(' ');
                        trace.push_str(&rvs_asm_syntax::render_line_f1(
                            &rvs_asm_syntax::split_line(source_line),
                        ));
                    }
                    trace.push('\n');
                    sim.exec_log.push_str(&trace);
                }
            }
        }
    }

    let use_fast = match decoded {
        Some(DecodedInstr::Ecall { .. } | DecodedInstr::Other) | None => false,
        Some(_) => true,
    };

    let step_res = if use_fast {
        let dec = decoded.expect("use_fast ensures decoded is Some");
        match execute_fast(sim, dec) {
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
        }
    } else {
        let (op, args) = match sim.mem.cells.get(&inst_pc) {
            Some(MemoryCell::Instruction(c)) => (c.op.clone(), c.args.clone()),
            Some(MemoryCell::Data {
                value, case, expr, ..
            }) => (
                "DD".to_string(),
                vec![expr
                    .clone()
                    .unwrap_or_else(|| rvs_memory::render_value(*value, *case))],
            ),
            None => unreachable!(),
        };
        match execute(sim, &op, &args) {
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
        }
    };

    if sim.exit_requested {
        return StepResult::Halted;
    }

    if sim.convention.enabled {
        let is_call = match decoded {
            Some(DecodedInstr::Jal { rd, .. } | DecodedInstr::Jalr { rd, .. }) => rd != 0,
            _ => false,
        };
        let is_ret = matches!(decoded, Some(DecodedInstr::Jalr { rd: 0, rs1: 1, .. }));
        if is_call {
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
        if is_ret {
            for w in sim.convention.on_return(inst_pc, &sim.regs) {
                sim.exec_log.push_str(&format!("WARN [Convention]: {w}\n"));
            }
        }
    }

    sim.metrics.instructions += 1;
    sim.metrics.cycles += 1;
    match decoded {
        Some(DecodedInstr::Branch { .. }) => {
            sim.metrics.branches_total += 1;
            if sim.regs.pc != inst_pc.wrapping_add(4) {
                sim.metrics.branches_taken += 1;
                sim.metrics.cycles += 1;
            }
        }
        Some(DecodedInstr::Jal { .. } | DecodedInstr::Jalr { .. }) => {
            sim.metrics.jumps += 1;
        }
        Some(DecodedInstr::Load { .. }) => {
            sim.metrics.loads += 1;
            sim.metrics.cycles += 1;
        }
        Some(DecodedInstr::Store { .. }) => {
            sim.metrics.stores += 1;
        }
        Some(DecodedInstr::AluRM { .. }) => {
            sim.metrics.alu_ops += 1;
            sim.metrics.cycles += 2;
        }
        Some(DecodedInstr::AluW { kind, .. } | DecodedInstr::AluWI { kind, .. }) => {
            sim.metrics.alu_ops += 1;
            if matches!(
                kind,
                WKind::Mul | WKind::Div | WKind::DivU | WKind::Rem | WKind::RemU
            ) {
                sim.metrics.cycles += 2;
            }
        }
        Some(DecodedInstr::Ecall { .. }) => {
            sim.metrics.ecalls += 1;
            sim.metrics.cycles += 4;
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
