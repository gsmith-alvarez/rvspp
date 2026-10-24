//! `ECALL` syscalls 0-9, string packing and DMA memory-mapped I/O.

use rvs_memory::{SimError, SimResult};
use rvs_sim::{
    imm_raw, read_reg_arg, reg_string_of, write_reg_arg, write_x, PendingInput,
    Simulator,
};
use rvs_tcl as tcl;

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
    match rvs_asm_syntax::tcl_expr(s) {
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
pub fn ecall(sim: &mut Simulator, args: &[String]) -> Result<bool, SimError> {
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

