//! `ECALL` syscalls 0-9, string packing and DMA memory-mapped I/O.

use rvs_memory::{SimError, SimResult};
use rvs_regs::Xlen;
use rvs_sim::{
    imm_raw, read_reg_arg, reg_string_of, write_reg_arg, write_x, EcallAbi, PendingInput, Simulator,
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

/// The little-endian character sequence of a register value.
fn chars_of(value: u64, width: usize) -> String {
    let mut s = String::new();
    for k in 0..width {
        s.push(char::from_u32(((value >> (8 * k)) & 0xff) as u32).unwrap_or('\0'));
    }
    s
}

/// Pack character bytes little-endian into a register-width value.
fn pack_chars(s: &str, width: usize) -> u64 {
    let mut v = 0u64;
    for (k, b) in s.as_bytes().iter().take(width).enumerate() {
        v |= (*b as u64) << (8 * k);
    }
    v
}

fn char_width(sim: &Simulator) -> usize {
    match sim.regs.xlen() {
        Xlen::X32 => 4,
        Xlen::X64 => 8,
    }
}

fn get_c_string(sim: &Simulator, start: u64) -> SimResult<String> {
    let start = sim.regs.normalize(start);
    if sim.regs.xlen() == Xlen::X32 && start.is_multiple_of(8) {
        let mut addr = start;
        loop {
            if sim.mem.get_word_string(addr).chars().count() != 8 {
                break;
            }
            if addr == 0xffff_fff8 {
                return Err(SimError(
                    "ERROR: string crosses RV32 address-space boundary".into(),
                ));
            }
            addr += 8;
        }
    }
    sim.mem.get_c_string(start)
}

fn put_c_string(sim: &mut Simulator, start: u64, text: &str) -> SimResult<String> {
    let start = sim.regs.normalize(start);
    if sim.regs.xlen() == Xlen::X32 {
        let len = text.chars().count() as u64;
        if len > 0x1_0000_0000 - start {
            return Err(SimError(
                "ERROR: string crosses RV32 address-space boundary".into(),
            ));
        }
    }
    sim.mem.put_c_string(start, text)
}

/// `ECALL` dispatch. In the a7 ABI the operands are ignored, exactly like a
/// normal RISC-V program expects.
pub fn ecall(sim: &mut Simulator, args: &[String]) -> Result<bool, SimError> {
    if sim.ecall_abi == EcallAbi::A7 {
        return ecall_a7(sim);
    }
    let rs1a = read_reg_arg(sim, &args[1])?;
    let imm = imm_raw(&args[2])?;
    if imm <= 4 {
        let rs1_string = reg_string_of(sim, &args[1]);
        let res: String = match imm {
            0 => format!("{}", sim.regs.signed(rs1a)),
            1 => match tcl::x2f(&rs1_string) {
                Some(v) => tcl::tcl_double_to_string(v),
                None => String::new(),
            },
            2 => rs1_string,
            3 => tcl::trim_nul(&chars_of(rs1a, char_width(sim))),
            4 => get_c_string(sim, rs1a)?,
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
        let prompt = tcl::trim_nul(&chars_of(rs1a, char_width(sim)));
        sim.output.push_str(&prompt);
        let queue = match &mut sim.input_lines {
            Some(q) => q,
            None => {
                sim.pending = Some(PendingInput {
                    rd_arg: args[0].clone(),
                    rs1_arg: args[1].clone(),
                    imm,
                    inst_pc: sim.regs.normalize(sim.regs.pc.wrapping_sub(4)),
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
                let f = parse_tcl_double(&line).ok_or_else(|| {
                    SimError(format!("expected floating-point number but got \"{line}\""))
                })?;
                write_reg_arg(sim, &args[0], f.to_bits(), false)?;
            }
            7 => {
                let v = expr_value(&line)?;
                write_x(sim, &args[0], v, false)?;
            }
            8 => {
                let value = pack_chars(&line, char_width(sim));
                write_x(sim, &args[0], value, false)?;
            }
            9 => {
                let dest = read_reg_arg(sim, &args[0])?;
                put_c_string(sim, dest, &format!("{line}\0"))?;
            }
            _ => unreachable!(),
        }
        return Ok(false);
    }
    // Immediates outside 0..9 do nothing at all (neither branch matches).
    Ok(false)
}

/// Pop one line of batch input, or `""` when the machine is interactive.
fn next_input_line(sim: &mut Simulator) -> String {
    match &mut sim.input_lines {
        Some(queue) => queue.pop_front().unwrap_or_default(),
        None => String::new(),
    }
}

/// RARS-compatible `a7` syscalls: 1 print int, 4 print string, 5 read int,
/// 8 read string, 9 sbrk, 10/17/93 exit, 11 print char, 34 print hex.
fn ecall_a7(sim: &mut Simulator) -> Result<bool, SimError> {
    let a0 = sim.regs.read_x(10);
    let a1 = sim.regs.read_x(11);
    let a7 = sim.regs.read_x(17);
    match a7 {
        1 => sim.output.push_str(&format!("{}", sim.regs.signed(a0))),
        4 => {
            let s = get_c_string(sim, a0)?;
            sim.output.push_str(&s);
        }
        5 => {
            let line = next_input_line(sim);
            let v = tcl::parse_int(line.trim())
                .ok_or_else(|| SimError(format!("expected integer but got \"{}\"", line.trim())))?;
            sim.regs.write_x(10, tcl::low64(v), false);
        }
        8 => {
            let line = next_input_line(sim);
            if !a0.is_multiple_of(8) {
                return Err(SimError(
                    "ERROR: read-string buffer must be double-word aligned".to_string(),
                ));
            }
            let max = a1 as usize;
            let mut text: String = line.chars().take(max.saturating_sub(1)).collect();
            text.push('\0');
            put_c_string(sim, a0, &text)?;
        }
        9 => {
            let next = sim.heap_break.checked_add(a0.max(8));
            if sim.regs.xlen() == Xlen::X32 && next.is_none_or(|value| value > u32::MAX as u64) {
                return Err(SimError("ERROR: heap exceeds RV32 address space".into()));
            }
            sim.regs.write_x(10, sim.heap_break, false);
            sim.heap_break = sim.heap_break.wrapping_add(a0.max(8));
        }
        10 | 17 | 93 => {
            sim.exit_requested = true;
            sim.exit_code = sim.regs.signed(a0);
        }
        11 => {
            let c = char::from_u32((a0 & 0xff) as u32).unwrap_or('\0');
            sim.output.push(c);
        }
        34 => sim.output.push_str(&format!("0x{a0:x}")),
        other => {
            return Err(SimError(format!(
                "unknown syscall {other} (a7 ABI supports 1,4,5,8,9,10,11,17,34,93)"
            )))
        }
    }
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
        tcl::parse_int(&m11)
            .map(|v| format!("{}", v as u64))
            .unwrap_or_default()
    } else if m01.contains('F') {
        tcl::x2f(&m11)
            .map(tcl::tcl_double_to_string)
            .unwrap_or_default()
    } else if m01.contains('C') {
        if sim.regs.xlen() == Xlen::X32 {
            let value = tcl::parse_int(&m11).map(|v| v as u64).unwrap_or(0);
            tcl::x2s(&tcl::revx(&format!("0x{:08x}", value as u32)))
        } else {
            tcl::x2s(&tcl::revx(&m11))
        }
    } else if m01.contains('S') {
        let addr = tcl::parse_int(&m11).map(|v| v as u64).unwrap_or(0);
        match get_c_string(sim, addr) {
            Ok(s) => s,
            Err(e) => e.0,
        }
    } else if m01.contains('X') {
        m11
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
            let value = pack_chars(line, char_width(sim));
            sim.mem.set_dd_value(0x18, value, 0);
        }
        "S" => {
            let dest = tcl::parse_int(&m11).map(|v| v as u64).unwrap_or(0);
            let res = put_c_string(sim, dest, &format!("{line}\0"))?;
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
    let args = [pending.rd_arg.clone(), pending.rs1_arg.clone()];
    match pending.imm {
        5 => {
            let v = tcl::parse_int(line)
                .ok_or_else(|| SimError(format!("expected integer but got \"{line}\"")))?;
            write_x(sim, &args[0], tcl::low64(v), false)?;
        }
        6 => {
            let f = parse_tcl_double(line).ok_or_else(|| {
                SimError(format!("expected floating-point number but got \"{line}\""))
            })?;
            write_reg_arg(sim, &args[0], f.to_bits(), false)?;
        }
        7 => {
            let v = expr_value(line)?;
            write_x(sim, &args[0], v, false)?;
        }
        8 => {
            let value = pack_chars(line, char_width(sim));
            write_x(sim, &args[0], value, false)?;
        }
        9 => {
            let dest = read_reg_arg(sim, &args[0])?;
            put_c_string(sim, dest, &format!("{line}\0"))?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rv32() -> Simulator {
        let mut sim = Simulator::new();
        sim.regs.set_xlen(Xlen::X32);
        sim
    }

    #[test]
    fn rv32_a7_signed_io_and_heap_boundary() {
        let mut sim = rv32();
        sim.ecall_abi = EcallAbi::A7;
        sim.regs.write_x(10, u32::MAX as u64, false);
        sim.regs.write_x(17, 1, false);
        ecall(&mut sim, &[]).unwrap();
        assert_eq!(sim.output, "-1");
        sim.regs.write_x(17, 93, false);
        ecall(&mut sim, &[]).unwrap();
        assert_eq!(sim.exit_code, -1);

        sim.input_lines = Some(["-2".to_string()].into());
        sim.regs.write_x(17, 5, false);
        ecall(&mut sim, &[]).unwrap();
        assert_eq!(sim.regs.read_x(10), 0xffff_fffe);

        sim.heap_break = 0xffff_fff8;
        sim.regs.write_x(10, 8, false);
        sim.regs.write_x(17, 9, false);
        assert!(ecall(&mut sim, &[]).is_err());
        assert_eq!(sim.heap_break, 0xffff_fff8);
        assert_eq!(sim.regs.read_x(10), 8);
    }

    #[test]
    fn rv32_strings_reject_crossing_and_dma_pointers_normalize() {
        let mut sim = rv32();
        sim.mem.insert_dd(0xffff_fff8, "0", "");
        sim.mem.set_dd_value(0xffff_fff8, 0x4847_4645_4443_4241, 0);
        assert!(get_c_string(&sim, 0xffff_fff8).is_err());
        assert!(put_c_string(&mut sim, 0xffff_fff8, "12345678\0").is_err());
        put_c_string(&mut sim, 0xffff_fff8, "1234567\0").unwrap();
        assert_eq!(get_c_string(&sim, 0xffff_fff8).unwrap(), "1234567");

        sim.entry_type = "S".into();
        sim.mem.insert_dd(0x18, "0", "");
        sim.mem.set_dd_value(0x18, 0x1_0000_0100, 0);
        submit_dma_input(&mut sim, "abc").unwrap();
        assert_eq!(sim.mem.get_c_string(0x100).unwrap(), "abc");
    }

    #[test]
    fn rv32_packed_input_uses_four_bytes_in_both_paths() {
        let mut sim = rv32();
        let args = ["x1".into(), "x0".into(), "8".into()];
        sim.input_lines = Some(["abcdef".to_string()].into());
        ecall(&mut sim, &args).unwrap();
        assert_eq!(sim.regs.read_x(1), 0x6463_6261);

        sim.input_lines = None;
        ecall(&mut sim, &args).unwrap();
        submit_ecall_input(&mut sim, "uvwxyz").unwrap();
        assert_eq!(sim.regs.read_x(1), 0x7877_7675);
        sim.entry_type = "C".into();
        sim.mem.insert_dd(0x18, "0", "");
        submit_dma_input(&mut sim, "abcdef").unwrap();
        assert_eq!(sim.mem.dd_hex(0x18).unwrap(), "0x0000000064636261");
    }
}
