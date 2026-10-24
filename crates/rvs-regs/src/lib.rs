//! Register file with ABI names, mirroring `RVS046.tcl`'s `::regs`,
//! `reg2i`, `regc` and `regp`.
//!
//! Tcl stores every register as an `"0x..."` hex *string*, and different
//! executor procs write that string in lower- or uppercase (`a64` lowercases
//! via `%016lx`, `typeR` uppercases via `%016lX`). The case leaks into
//! reports (`ECALL 2`, DMA `X` output, `regp`), so each register also tracks
//! the hex-digit case of its last writer.

/// Parse a register name (case-insensitive) to its 0..31 index.
///
/// Accepts `x0`..`x31`, `f0`..`f31` and the standard ABI aliases
/// (`zero`, `ra`, `sp`, `gp`, `tp`, `t0`..`t2`, `s0`/`fp`, `s1`,
/// `a0`..`a7`, `s2`..`s11`, `t3`..`t6`), plus the FP ABI aliases
/// (`ft0`..`ft7`, `fs0`..`fs1`, `fa0`..`fa7`, `fs2`..`fs11`, `ft8`..`ft11`).
pub fn parse_reg(s: &str) -> Option<u8> {
    let lower = s.to_ascii_lowercase();
    // xN / fN forms first.
    if lower.len() >= 2 {
        let (prefix, rest) = lower.split_at(1);
        if let Ok(n) = rest.parse::<u8>() {
            if n <= 31 && matches!(prefix, "x" | "f") {
                return Some(n);
            }
        }
    }
    ABI_LOOKUP
        .iter()
        .find(|(name, _)| *name == lower)
        .map(|(_, idx)| *idx)
}

/// ABI display label for an integer register index (matches the first
/// occurrence of `xN` in RVS046's `::regs` table, so `x8` displays as `s0`).
pub fn abi_name(idx: u8) -> &'static str {
    ABI_LIST[idx as usize].1
}

/// Integer ABI aliases in table order (`name -> xN` pairs).
pub const ABI_LIST: [(&str, &str); 32] = [
    ("x0", "zero"),
    ("x1", "ra"),
    ("x2", "sp"),
    ("x3", "gp"),
    ("x4", "tp"),
    ("x5", "t0"),
    ("x6", "t1"),
    ("x7", "t2"),
    ("x8", "s0"),
    ("x9", "s1"),
    ("x10", "a0"),
    ("x11", "a1"),
    ("x12", "a2"),
    ("x13", "a3"),
    ("x14", "a4"),
    ("x15", "a5"),
    ("x16", "a6"),
    ("x17", "a7"),
    ("x18", "s2"),
    ("x19", "s3"),
    ("x20", "s4"),
    ("x21", "s5"),
    ("x22", "s6"),
    ("x23", "s7"),
    ("x24", "s8"),
    ("x25", "s9"),
    ("x26", "s10"),
    ("x27", "s11"),
    ("x28", "t3"),
    ("x29", "t4"),
    ("x30", "t5"),
    ("x31", "t6"),
];

/// Flat lookup for every accepted integer/FP register spelling.
/// FP aliases: `ftN` -> N (0..7), `fs0`/`fs1` -> 8/9, `faN` -> 10..17,
/// `fs2`..`fs11` -> 18..27, `ft8`..`ft11` -> 28..31.
const ABI_LOOKUP: [(&str, u8); 65] = [
    ("zero", 0),
    ("ra", 1),
    ("sp", 2),
    ("gp", 3),
    ("tp", 4),
    ("t0", 5),
    ("t1", 6),
    ("t2", 7),
    ("s0", 8),
    ("fp", 8),
    ("s1", 9),
    ("a0", 10),
    ("a1", 11),
    ("a2", 12),
    ("a3", 13),
    ("a4", 14),
    ("a5", 15),
    ("a6", 16),
    ("a7", 17),
    ("s2", 18),
    ("s3", 19),
    ("s4", 20),
    ("s5", 21),
    ("s6", 22),
    ("s7", 23),
    ("s8", 24),
    ("s9", 25),
    ("s10", 26),
    ("s11", 27),
    ("t3", 28),
    ("t4", 29),
    ("t5", 30),
    ("t6", 31),
    ("ft0", 0),
    ("ft1", 1),
    ("ft2", 2),
    ("ft3", 3),
    ("ft4", 4),
    ("ft5", 5),
    ("ft6", 6),
    ("ft7", 7),
    ("fs0", 8),
    ("fs1", 9),
    ("fa0", 10),
    ("fa1", 11),
    ("fa2", 12),
    ("fa3", 13),
    ("fa4", 14),
    ("fa5", 15),
    ("fa6", 16),
    ("fa7", 17),
    ("fs2", 18),
    ("fs3", 19),
    ("fs4", 20),
    ("fs5", 21),
    ("fs6", 22),
    ("fs7", 23),
    ("fs8", 24),
    ("fs9", 25),
    ("fs10", 26),
    ("fs11", 27),
    ("ft8", 28),
    ("ft9", 29),
    ("ft10", 30),
    ("ft11", 31),
];

#[derive(Clone, Debug, Default)]
pub struct RegisterFile {
    x: [u64; 32],
    f: [u64; 32],
    /// `true` when the register's hex digits were last written uppercase.
    x_upper: [bool; 32],
    f_upper: [bool; 32],
    pub pc: u64,
}

impl RegisterFile {
    pub fn new() -> Self {
        Self::default()
    }

    /// `regc`: zero all registers and the PC.
    pub fn clear(&mut self) {
        self.x = [0; 32];
        self.f = [0; 32];
        self.x_upper = [false; 32];
        self.f_upper = [false; 32];
        self.pc = 0;
    }

    /// Current integer register values.
    pub fn x_values(&self) -> [u64; 32] {
        self.x
    }

    /// Current FP register values (raw bits).
    pub fn f_values(&self) -> [u64; 32] {
        self.f
    }

    /// Read an integer register (`x0` always reads 0).
    pub fn read_x(&self, idx: u8) -> u64 {
        if idx == 0 {
            0
        } else {
            self.x[idx as usize]
        }
    }

    /// Write an integer register (`x0` writes are ignored, like Tcl's
    /// `reg2i` resetting `::x0` before every access).
    pub fn write_x(&mut self, idx: u8, val: u64, upper: bool) {
        if idx != 0 {
            self.x[idx as usize] = val;
            self.x_upper[idx as usize] = upper;
        }
    }

    /// Read raw FP register bits.
    pub fn read_f_bits(&self, idx: u8) -> u64 {
        self.f[idx as usize]
    }

    /// Write raw FP register bits with hex-case metadata.
    pub fn write_f_bits(&mut self, idx: u8, bits: u64, upper: bool) {
        self.f[idx as usize] = bits;
        self.f_upper[idx as usize] = upper;
    }

    /// Read an FP register as `f64` (`f64::from_bits`).
    pub fn read_f64(&self, idx: u8) -> f64 {
        f64::from_bits(self.f[idx as usize])
    }

    /// Write an FP register from an `f64`.
    pub fn write_f64(&mut self, idx: u8, val: f64, upper: bool) {
        self.f[idx as usize] = val.to_bits();
        self.f_upper[idx as usize] = upper;
    }

    /// Whether the register's hex digits were last written uppercase.
    pub fn x_case(&self, idx: u8) -> bool {
        self.x_upper[idx as usize]
    }

    /// Whether the FP register's hex digits were last written uppercase.
    pub fn f_case(&self, idx: u8) -> bool {
        self.f_upper[idx as usize]
    }

    /// The register's display string, exactly as Tcl would hold it
    /// (`0x` + 16 hex digits with the writer's case).
    pub fn x_string(&self, idx: u8) -> String {
        let v = self.read_x(idx);
        if self.x_upper[idx as usize] {
            format!("0x{v:016X}")
        } else {
            format!("0x{v:016x}")
        }
    }

    /// The register's display string. FP registers hold the `f2x` digit
    /// order (little-endian bytes), i.e. the swapped bit pattern.
    pub fn f_string(&self, idx: u8) -> String {
        let v = self.f[idx as usize];
        if self.f_upper[idx as usize] {
            format!("0x{v:016X}")
        } else {
            format!("0x{v:016x}")
        }
    }

    pub fn pc_string(&self) -> String {
        format!("0x{:016x}", self.pc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_register_names() {
        for i in 0..32u8 {
            assert_eq!(parse_reg(&format!("x{i}")), Some(i));
            assert_eq!(parse_reg(&format!("X{i}")), Some(i));
            assert_eq!(parse_reg(&format!("f{i}")), Some(i));
        }
        assert_eq!(parse_reg("zero"), Some(0));
        assert_eq!(parse_reg("ra"), Some(1));
        assert_eq!(parse_reg("fp"), Some(8));
        assert_eq!(parse_reg("s0"), Some(8));
        assert_eq!(parse_reg("a7"), Some(17));
        assert_eq!(parse_reg("t6"), Some(31));
        assert_eq!(parse_reg("ft3"), Some(3));
        assert_eq!(parse_reg("fa5"), Some(15));
        assert_eq!(parse_reg("fs11"), Some(27));
        assert_eq!(parse_reg("ft11"), Some(31));
        assert_eq!(parse_reg("q5"), None);
        assert_eq!(parse_reg("x32"), None);
    }

    #[test]
    fn x0_is_hardwired() {
        let mut rf = RegisterFile::new();
        rf.write_x(0, 42, false);
        assert_eq!(rf.read_x(0), 0);
        assert_eq!(rf.x_string(0), "0x0000000000000000");
        rf.write_x(1, 0x0a, true);
        assert_eq!(rf.x_string(1), "0x000000000000000A");
        rf.write_f64(2, 2.0, false);
        assert_eq!(rf.f_string(2), "0x4000000000000000");
        assert_eq!(rf.read_f64(2), 2.0);
    }

    #[test]
    fn abi_names_match_reference_table() {
        assert_eq!(abi_name(0), "zero");
        assert_eq!(abi_name(8), "s0");
        assert_eq!(abi_name(31), "t6");
    }
}
