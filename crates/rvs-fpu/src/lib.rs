//! Floating-point (RV64FD) instruction handlers.

use rvs_memory::{SimError, SimResult};
use rvs_sim::{imm_raw, read_reg_arg, read_x_arg, reg_index, write_x, Simulator};
use rvs_tcl as tcl;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FOp {
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
pub fn freg_value(sim: &Simulator, arg: &str) -> SimResult<Option<f64>> {
    let idx = reg_index(arg)?;
    Ok(tcl::x2f(&sim.regs.f_string(idx)))
}

pub fn fp_operand_err(op: &str) -> SimError {
    SimError(format!(
        "can't use non-numeric floating-point value as operand of \"{op}\""
    ))
}

/// `typeFR`: float ALU; comparison results go to the integer register file.
pub fn type_fr(sim: &mut Simulator, args: &[String], op: FOp) -> SimResult<()> {
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
pub fn fminmax(sim: &mut Simulator, args: &[String], is_max: bool) -> SimResult<()> {
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
pub enum SgnMode {
    J,
    Jn,
    Jx,
}

/// Sign-injection family implemented exactly like the reference (via
/// `abs`/comparisons, so `+0.0`/`-0.0` corners follow Tcl, not IEEE).
pub fn fsgnj(sim: &mut Simulator, args: &[String], mode: SgnMode) -> SimResult<()> {
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
pub fn fcvt_to_int(sim: &mut Simulator, args: &[String], unsigned: bool) -> SimResult<()> {
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
pub fn fcvt_to_float(sim: &mut Simulator, args: &[String], unsigned: bool) -> SimResult<()> {
    let raw = read_reg_arg(sim, &args[1])?;
    let value = if unsigned {
        raw as f64
    } else {
        raw as i64 as f64
    };
    let idx = reg_index(&args[0])?;
    sim.regs.write_f64(idx, value, false);
    Ok(())
}

/// `FMV.X.D`: the x register receives `i2x` of the FP register *string*, so
/// it holds the byte-swapped bit pattern (the digit value).
pub fn fmv_x_d(sim: &mut Simulator, args: &[String]) -> SimResult<()> {
    let idx = reg_index(&args[1])?;
    let digits = sim.regs.read_f_bits(idx);
    write_x(sim, &args[0], digits, false)
}

/// `FMV.D.X`: the FP register receives `i2x` of the integer register's
/// value, so its bits are the byte-swap of that value.
pub fn fmv_d_x(sim: &mut Simulator, args: &[String]) -> SimResult<()> {
    let value = read_reg_arg(sim, &args[1])?;
    let idx = reg_index(&args[0])?;
    sim.regs.write_f_bits(idx, value, false);
    Ok(())
}

/// `FSQRT.D`; NaN and negative operands raise Tcl's errors.
pub fn fsqrt(sim: &mut Simulator, args: &[String]) -> SimResult<()> {
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
pub fn flmem(sim: &mut Simulator, args: &[String], nbytes: usize) -> SimResult<()> {
    let base = read_x_arg(sim, &args[1])?;
    let addr = base.wrapping_add(imm_raw(&args[2])?);
    let cell = sim.mem.read_bytes(addr, nbytes)?;
    let idx = reg_index(&args[0])?;
    sim.regs.write_f_bits(idx, cell, false);
    Ok(())
}

/// `FSmem` / `FSD`: store FP bits; the cell receives the swapped digits.
pub fn fsmem(sim: &mut Simulator, args: &[String], nbytes: usize) -> SimResult<()> {
    let base = read_x_arg(sim, &args[1])?;
    let addr = base.wrapping_add(imm_raw(&args[2])?);
    let idx = reg_index(&args[0])?;
    let bits = sim.regs.read_f_bits(idx);
    let mask = if nbytes == 8 {
        u64::MAX
    } else {
        (1u64 << (nbytes * 8)) - 1
    };
    let val = bits & mask;
    sim.mem.write_bytes(addr, nbytes, val, sim.regs.f_case(idx))
}
