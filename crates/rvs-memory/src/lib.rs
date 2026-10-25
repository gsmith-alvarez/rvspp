//! Sparse segmented memory mirroring `RVS046.tcl`'s `::mem`, `memr`, `memw`,
//! `chkadr`, `getstr1`, `getstr` and `putstr`.
//!
//! Model derived from the reference: every 64-bit cell holds a value whose
//! Tcl rendering is `"0x" + 16 hex digits`, and the byte at address
//! `a8 + k` is `(value >> 8k) & 0xff`, i.e. plain little-endian memory
//! (`memr`/`memw`'s digit-index arithmetic reduces to shifts and masks).
//! Integer loads/stores, characters and IEEE-754 doubles (`f2x`/`x2f`) all
//! agree on that layout. Hex-digit case is preserved per byte because Tcl
//! splices register strings (which may be uppercase) into cells.

use std::collections::{BTreeMap, VecDeque};

use rvs_tcl as tcl;

/// One assembled instruction cell (`{op args source}` in Tcl).
#[derive(Clone, Debug)]
pub struct InstructionCell {
    pub op: String,
    /// Normalized operands (Tcl-facing form, rewritten by `dump`).
    pub args: Vec<String>,
    /// Operands exactly as written, kept so `dump` stays idempotent.
    pub raw_args: Vec<String>,
    pub source_line: String,
    /// Encoder diagnostic for this cell, set by `dump`. A cell with an error
    /// is never executed: the run halts and the message goes to the trace.
    pub encode_error: Option<String>,
}

/// One memory cell.
#[derive(Clone, Debug)]
pub enum MemoryCell {
    /// `DD` data cell. `expr` holds the raw directive operand until `dump`
    /// evaluates it; afterwards cells are pure values. `case` bit `k` marks
    /// byte `k` as written with uppercase hex digits.
    Data {
        value: u64,
        case: u64,
        expr: Option<String>,
        source_line: String,
    },
    Instruction(InstructionCell),
}

impl MemoryCell {
    pub fn is_data(&self) -> bool {
        matches!(self, MemoryCell::Data { .. })
    }

    /// The cell's Tcl string form: `DD 0x................ {source}`.
    pub fn tcl_string(&self) -> String {
        match self {
            MemoryCell::Data {
                value,
                case,
                expr,
                source_line,
            } => {
                let hex = render_value(*value, *case);
                let hex = match expr {
                    Some(e) => e.clone(),
                    None => hex,
                };
                tcl::tcl_list_str(&["DD".to_string(), hex, source_line.clone()])
            }
            MemoryCell::Instruction(c) => {
                tcl::tcl_list_str(&[c.op.clone(), c.args.join(" "), c.source_line.clone()])
            }
        }
    }
}

/// Render a 64-bit value with per-byte hex case (bit `k` of `case` = byte
/// `k` uppercase).
pub fn render_value(value: u64, case: u64) -> String {
    let mut s = String::with_capacity(18);
    s.push_str("0x");
    for p in 0..8usize {
        let byte = (value >> (8 * (7 - p))) & 0xff;
        let k = 7 - p;
        if (case >> k) & 1 == 1 {
            s.push_str(&format!("{byte:02X}"));
        } else {
            s.push_str(&format!("{byte:02x}"));
        }
    }
    s
}

/// Simulation error carrying the exact Tcl-visible message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimError(pub String);

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SimError {}

pub type SimResult<T> = Result<T, SimError>;

pub fn sim_err<T>(msg: impl Into<String>) -> SimResult<T> {
    Err(SimError(msg.into()))
}

/// Tcl `string range s first last`: negative first clamps to 0, `last >= len`
/// clamps to `len-1`, a negative or out-of-order `last` yields nothing.
pub fn tcl_string_range(s: &str, first: i64, last: i64) -> String {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;
    let a = first.max(0);
    if a >= len {
        return String::new();
    }
    if last < 0 || last < a {
        return String::new();
    }
    let b = last.min(len - 1);
    chars[a as usize..=b as usize].iter().collect()
}

/// Sparse memory addressed by 64-bit cell addresses.
///
/// When the undo journal is enabled (`start_journal`), every mutation records
/// the cell's previous contents. `RiscV::snapshot` then only has to remember a
/// mark, which is what makes reverse stepping O(registers) instead of O(memory).
#[derive(Clone, Debug, Default)]
pub struct Memory {
    pub cells: BTreeMap<u64, MemoryCell>,
    journal: Option<VecDeque<(u64, Option<MemoryCell>)>>,
    journal_dropped: u64,
}

impl Memory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.cells.clear();
        self.reset_journal();
    }

    // ----- undo journal -------------------------------------------------

    /// Begin recording previous cell contents (called after assembly).
    pub fn start_journal(&mut self) {
        self.journal = Some(VecDeque::new());
        self.journal_dropped = 0;
    }

    /// Discard the journal (fresh compile, batch mode).
    pub fn reset_journal(&mut self) {
        self.journal = None;
        self.journal_dropped = 0;
    }

    /// Absolute position in the journal, for snapshotting.
    pub fn journal_mark(&self) -> u64 {
        self.journal_dropped + self.journal.as_ref().map_or(0, |j| j.len() as u64)
    }

    fn record(&mut self, a8: u64) {
        if let Some(journal) = &mut self.journal {
            let previous = self.cells.get(&a8).cloned();
            journal.push_back((a8, previous));
        }
    }

    /// Undo every journal entry after `mark`, newest first.
    pub fn journal_rewind_to(&mut self, mark: u64) {
        let Some(journal) = &mut self.journal else {
            return;
        };
        while self.journal_dropped + journal.len() as u64 > mark {
            let Some((addr, previous)) = journal.pop_back() else {
                break;
            };
            match previous {
                Some(cell) => {
                    self.cells.insert(addr, cell);
                }
                None => {
                    self.cells.remove(&addr);
                }
            }
        }
    }

    /// Forget entries older than `mark` (a history snapshot was dropped).
    pub fn journal_drop_before(&mut self, mark: u64) {
        let Some(journal) = &mut self.journal else {
            return;
        };
        while self.journal_dropped < mark {
            if journal.pop_front().is_none() {
                break;
            }
            self.journal_dropped += 1;
        }
    }

    /// `chkadr`: conflict test used by the assembler.
    ///
    /// Data (`is_data`): conflicts when either half of the doubleword slot
    /// holds any cell. Instruction: conflicts when the exact 4-byte slot is
    /// taken or when the containing doubleword holds data.
    pub fn check_addr_conflict(&self, addr: u64, is_data: bool) -> bool {
        if is_data {
            self.cells.contains_key(&(addr / 8 * 8)) || self.cells.contains_key(&(addr / 8 * 8 + 4))
        } else {
            self.cells.contains_key(&(addr / 4 * 4))
                || matches!(
                    self.cells.get(&(addr / 8 * 8)),
                    Some(MemoryCell::Data { .. })
                )
        }
    }

    fn addr_str(addr: u64) -> String {
        format!("0x{addr:016x}")
    }

    /// The `#code` error shared by `memr`/`memw`: Tcl interpolates
    /// `::mem($a8)` into the message, which itself fails when `a8` is empty.
    fn code_error(&self, prefix: &str, addr: u64, a8: u64, len: usize) -> SimError {
        match self.cells.get(&a8) {
            Some(cell) => SimError(format!(
                "{prefix}: #code adr={} len={len} mem({})={}",
                Self::addr_str(addr),
                Self::addr_str(a8),
                cell.tcl_string()
            )),
            None => SimError(format!(
                "can't read \"::mem({})\": no such element in array",
                Self::addr_str(a8)
            )),
        }
    }

    /// `memr`: read `len` (1, 2, 4 or 8) bytes as a little-endian value.
    pub fn read_bytes(&self, addr: u64, len: usize) -> SimResult<u64> {
        if !matches!(len, 1 | 2 | 4 | 8) {
            return sim_err(format!(
                "memr: #len problem adr={} len={len}",
                Self::addr_str(addr)
            ));
        }
        if addr / len as u64 * len as u64 != addr {
            return sim_err(format!(
                "memr: #alignment problem adr={} len={len}",
                Self::addr_str(addr)
            ));
        }
        let a8 = addr / 8 * 8;
        let r8 = (addr % 8) as u32;
        if self.cells.contains_key(&(a8 + 4)) {
            return Err(self.code_error("memr", addr, a8, len));
        }
        let value = match self.cells.get(&a8) {
            None => 0,
            Some(MemoryCell::Data { value, .. }) => *value,
            Some(other) => {
                return sim_err(format!(
                    "memr: #not data adr={} len={len} mem({})={}",
                    Self::addr_str(addr),
                    Self::addr_str(a8),
                    other.tcl_string()
                ))
            }
        };
        let mask = if len == 8 {
            u64::MAX
        } else {
            (1u64 << (len * 8)) - 1
        };
        Ok((value >> (8 * r8)) & mask)
    }

    /// `memw`: write `len` (1, 2, 4 or 8) bytes of `val` (little-endian).
    /// `upper` marks the written bytes' hex case, exactly like Tcl splicing
    /// a register string into the cell.
    pub fn write_bytes(&mut self, addr: u64, len: usize, val: u64, upper: bool) -> SimResult<()> {
        if !matches!(len, 1 | 2 | 4 | 8) {
            return sim_err(format!(
                "memw: #len problem adr={} len={len}",
                Self::addr_str(addr)
            ));
        }
        if addr / len as u64 * len as u64 != addr {
            return sim_err(format!(
                "memw: #alignment problem adr={} len={len}",
                Self::addr_str(addr)
            ));
        }
        let a8 = addr / 8 * 8;
        let r8 = (addr % 8) as u32;
        if self.cells.contains_key(&(a8 + 4)) {
            return Err(self.code_error("memw", addr, a8, len));
        }
        self.record(a8);
        self.cells.entry(a8).or_insert_with(|| MemoryCell::Data {
            value: 0,
            case: 0,
            expr: None,
            source_line: String::new(),
        });
        let mask = if len == 8 {
            u64::MAX
        } else {
            (1u64 << (len * 8)) - 1
        };
        let clear = !(mask << (8 * r8));
        match self.cells.get_mut(&a8) {
            Some(MemoryCell::Data {
                value, case, expr, ..
            }) => {
                if let Some(e) = expr {
                    // Tcl splices into the raw operand text; cells carrying an
                    // unevaluated expression are never written at runtime, so
                    // this path only mirrors the defensive behavior.
                    let _ = e;
                }
                *expr = None;
                *value = (*value & clear) | ((val & mask) << (8 * r8));
                let case_mask = mask << (8 * r8);
                if upper {
                    *case |= case_mask;
                } else {
                    *case &= !case_mask;
                }
                Ok(())
            }
            Some(cell) => sim_err(format!(
                "memw: #not data adr={} len={len} mem({})={}",
                Self::addr_str(addr),
                Self::addr_str(a8),
                cell.tcl_string()
            )),
            None => unreachable!(),
        }
    }

    /// Characters of one doubleword, little-endian, cut before the first
    /// NUL (`getstr`); empty when the cell is missing or not data.
    pub fn get_word_string(&self, addr: u64) -> String {
        match self.cells.get(&addr) {
            Some(MemoryCell::Data { value, .. }) => {
                let mut s = String::new();
                for k in 0..8usize {
                    let b = (value >> (8 * k)) & 0xff;
                    s.push(char::from_u32(b as u32).unwrap_or('\0'));
                }
                match s.find('\0') {
                    Some(f) => s[..f].to_string(),
                    None => s,
                }
            }
            _ => String::new(),
        }
    }

    /// `getstr1`: NUL-terminated string starting at a doubleword boundary.
    pub fn get_c_string(&self, start: u64) -> SimResult<String> {
        if !start.is_multiple_of(8) {
            return sim_err("ERROR: source string must start at double-word boundary");
        }
        let mut a = start;
        let mut r = String::new();
        loop {
            let s = self.get_word_string(a);
            let done = s.chars().count() != 8;
            r.push_str(&s);
            if done {
                break;
            }
            a += 8;
        }
        Ok(r)
    }

    /// `putstr`: write a NUL-terminated string at a doubleword boundary.
    /// Returns the `"addr:hex"` summary; the overwrite error is returned as
    /// data (Tcl callers print it).
    pub fn put_c_string(&mut self, start: u64, s: &str) -> SimResult<String> {
        if !start.is_multiple_of(8) {
            return sim_err("ERROR: destination string must start at double-word boundary");
        }
        let chars: Vec<String> = s.chars().map(|c| c.to_string()).collect();
        let hex = args2hex(&chars, ArgsFmt::A1).map_err(|e| SimError(e.0))?;
        let mut a = start;
        for dw in &hex {
            let cell_ok = match self.cells.get(&a) {
                None => true,
                Some(MemoryCell::Data { .. }) => true,
                Some(_) => false,
            };
            if !cell_ok {
                return Ok(format!(
                    "ERROR: attempt to overwrite code at {}",
                    Self::addr_str(a)
                ));
            }
            let value = tcl::parse_int(dw).unwrap_or(0) as u64;
            self.record(a);
            self.cells.insert(
                a,
                MemoryCell::Data {
                    value,
                    case: 0,
                    expr: None,
                    source_line: String::new(),
                },
            );
            a += 8;
        }
        Ok(format!(
            "{}:{hex2}",
            Self::addr_str(start),
            hex2 = hex.join(",")
        ))
    }

    /// Whether the cell at `addr` exists and is a `DD` data cell.
    pub fn is_dd(&self, addr: u64) -> bool {
        matches!(self.cells.get(&addr), Some(MemoryCell::Data { .. }))
    }

    /// The data cell's rendered hex string at `addr` (with `0x` prefix).
    pub fn dd_hex(&self, addr: u64) -> Option<String> {
        match self.cells.get(&addr) {
            Some(MemoryCell::Data {
                value, case, expr, ..
            }) => Some(match expr {
                Some(e) => e.clone(),
                None => render_value(*value, *case),
            }),
            _ => None,
        }
    }

    /// Overwrite a data cell's value/case, keeping its source line.
    pub fn set_dd_value(&mut self, addr: u64, value: u64, case: u64) {
        self.record(addr);
        if let Some(MemoryCell::Data {
            value: v,
            case: c,
            expr,
            ..
        }) = self.cells.get_mut(&addr)
        {
            *v = value;
            *c = case;
            *expr = None;
        }
    }

    /// The raw directive operand still awaiting `dump` evaluation.
    pub fn dd_expr(&self, addr: u64) -> Option<String> {
        match self.cells.get(&addr) {
            Some(MemoryCell::Data { expr, .. }) => expr.clone(),
            _ => None,
        }
    }

    /// Insert a data cell like the assembler's `DD` branch does.
    pub fn insert_dd(&mut self, addr: u64, expr: &str, source_line: &str) {
        self.record(addr);
        self.cells.insert(
            addr,
            MemoryCell::Data {
                value: 0,
                case: 0,
                expr: Some(expr.to_string()),
                source_line: source_line.to_string(),
            },
        );
    }

    /// Insert an instruction cell like the assembler does.
    pub fn insert_instruction(
        &mut self,
        addr: u64,
        op: &str,
        args: Vec<String>,
        source_line: &str,
    ) {
        self.record(addr);
        self.cells.insert(
            addr,
            MemoryCell::Instruction(InstructionCell {
                op: op.to_string(),
                raw_args: args.clone(),
                args,
                source_line: source_line.to_string(),
                encode_error: None,
            }),
        );
    }
}

/// `args2hex`: replicate the reference's digit-string packing: each
/// argument's `binary format` hex is prepended, 16 zero digits are prepended
/// when the length is not a multiple of 16, and the string is split into
/// 16-digit doubleword groups from the end.
pub fn args2hex(args: &[String], fmt: ArgsFmt) -> Result<Vec<String>, Args2HexError> {
    let mut hex = String::new();
    for arg in args {
        let s = match fmt {
            ArgsFmt::A1 => format!("{:02x}", arg.as_bytes().first().copied().unwrap_or(0)),
            ArgsFmt::C1 => {
                let v = tcl::parse_int(arg)
                    .ok_or_else(|| Args2HexError(format!("expected integer but got \"{arg}\"")))?;
                format!("{:02x}", (v as i128 & 0xff) as u8)
            }
            ArgsFmt::S1 => {
                let v = tcl::parse_int(arg)
                    .ok_or_else(|| Args2HexError(format!("expected integer but got \"{arg}\"")))?;
                format!("{:04x}", (v as i128 & 0xffff) as u16)
            }
            ArgsFmt::I1 => {
                let v = tcl::parse_int(arg)
                    .ok_or_else(|| Args2HexError(format!("expected integer but got \"{arg}\"")))?;
                format!("{:08x}", (v as i128 & 0xffff_ffff) as u32)
            }
            ArgsFmt::Q1 => {
                let v = arg
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .or_else(|| tcl::parse_int(arg).map(|i| i as f64));
                let v = v.ok_or_else(|| {
                    Args2HexError(format!("expected floating-point number but got \"{arg}\""))
                })?;
                format!("{:016x}", v.to_bits())
            }
        };
        hex = format!("{s}{hex}");
    }
    if !hex.chars().count().is_multiple_of(16) {
        hex = format!("0000000000000000{hex}");
    }
    let chars: Vec<char> = hex.chars().collect();
    let n = chars.len();
    if n == 0 {
        // DC "" yields the literal "0x" in the reference.
        return Ok(vec!["0x".to_string()]);
    }
    let mut out = format!("0x{}", chars[n - 16..].iter().collect::<String>());
    let mut i = n as i64 - 32;
    while i >= 0 {
        out.push_str(&format!(
            ",0x{}",
            chars[i as usize..(i + 16) as usize]
                .iter()
                .collect::<String>()
        ));
        i -= 16;
    }
    Ok(out.split(',').map(|s| s.to_string()).collect())
}

/// `args2hex` packing formats (Tcl `binary format` templates).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgsFmt {
    /// `a1`: one raw byte per argument (characters).
    A1,
    /// `c1`: one 8-bit integer per argument.
    C1,
    /// `S1`: one 16-bit big-endian integer per argument.
    S1,
    /// `I1`: one 32-bit big-endian integer per argument.
    I1,
    /// `Q1`: one little-endian IEEE-754 double per argument.
    Q1,
}

/// Error wrapper carrying Tcl's parse failure text.
#[derive(Debug)]
pub struct Args2HexError(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_with_dd(addr: u64, value: u64) -> Memory {
        let mut m = Memory::new();
        m.cells.insert(
            addr,
            MemoryCell::Data {
                value,
                case: 0,
                expr: None,
                source_line: "line".to_string(),
            },
        );
        m
    }

    #[test]
    fn read_write_splices_bytes() {
        let m = mem_with_dd(0, 0x0000_0000_0403_0201);
        assert_eq!(m.read_bytes(0, 4).unwrap(), 0x0403_0201);
        assert_eq!(m.read_bytes(4, 2).unwrap(), 0);
        assert_eq!(m.read_bytes(7, 1).unwrap(), 0x00);
        assert_eq!(m.read_bytes(1, 1).unwrap(), 0x02);
        assert_eq!(
            m.read_bytes(2, 4).unwrap_err().0,
            "memr: #alignment problem adr=0x0000000000000002 len=4"
        );
        // reading unwritten memory yields zeros
        let m2 = Memory::new();
        assert_eq!(m2.read_bytes(0x100, 8).unwrap(), 0);
    }

    #[test]
    fn code_conflicts() {
        let mut m = Memory::new();
        m.insert_instruction(
            4,
            "add",
            vec!["x1".into(), "x2".into(), "x3".into()],
            "add x1,x2,x3",
        );
        // mem(a8) is empty, so Tcl's message construction itself fails
        let err = m.read_bytes(0, 8).unwrap_err().0;
        assert_eq!(
            err,
            "can't read \"::mem(0x0000000000000000)\": no such element in array"
        );
        // with a data cell at a8 the message embeds its list rendering
        m.cells.insert(
            0,
            MemoryCell::Data {
                value: 1,
                case: 0,
                expr: None,
                source_line: String::new(),
            },
        );
        let err = m.read_bytes(0, 8).unwrap_err().0;
        assert!(
            err.starts_with("memr: #code adr=0x0000000000000000 len=8 mem(0x0000000000000000)=DD ")
        );
        let mut m2 = Memory::new();
        m2.cells.insert(
            8,
            MemoryCell::Data {
                value: 1,
                case: 0,
                expr: None,
                source_line: String::new(),
            },
        );
        assert_eq!(m2.read_bytes(8, 8).unwrap(), 1);
        m2.write_bytes(12, 1, 0xff, false).unwrap();
        assert_eq!(m2.dd_hex(8).unwrap(), "0x000000ff00000001");
    }

    #[test]
    fn strings_roundtrip() {
        let mut m = Memory::new();
        let res = m.put_c_string(0, "abc\0").unwrap();
        assert_eq!(res, "0x0000000000000000:0x0000000000636261");
        assert_eq!(m.dd_hex(0).unwrap(), "0x0000000000636261");
        assert_eq!(m.get_c_string(0).unwrap(), "abc");
        assert!(m
            .get_c_string(4)
            .unwrap_err()
            .0
            .contains("must start at double-word boundary"));
    }

    #[test]
    fn args2hex_matches_reference_packing() {
        // DC "abc": chars packed little-endian into one doubleword
        let args: Vec<String> = "abc".chars().map(|c| c.to_string()).collect();
        let r = args2hex(&args, ArgsFmt::A1).unwrap();
        assert_eq!(r, vec!["0x0000000000636261".to_string()]);
        // DW 1,2: two 32-bit fields in one doubleword
        let r = args2hex(&["1".to_string(), "2".to_string()], ArgsFmt::I1).unwrap();
        assert_eq!(r, vec!["0x0000000200000001".to_string()]);
        // DF 2.0 little-endian bits
        let r = args2hex(&["2.0".to_string()], ArgsFmt::Q1).unwrap();
        assert_eq!(r, vec!["0x4000000000000000".to_string()]);
        // long strings span multiple doublewords in address order
        let args: Vec<String> = "abcdefghij".chars().map(|c| c.to_string()).collect();
        let r = args2hex(&args, ArgsFmt::A1).unwrap();
        assert_eq!(
            r,
            vec![
                "0x6867666564636261".to_string(),
                "0x0000000000006a69".to_string()
            ]
        );
    }

    #[test]
    fn journal_rewinds_memory_exactly() {
        let mut m = Memory::new();
        m.start_journal();
        let mark_empty = m.journal_mark();
        m.insert_dd(0x100, "1", "DD 1");
        let mark_one = m.journal_mark();
        m.set_dd_value(0x100, 7, 0);
        let mark_two = m.journal_mark();
        m.set_dd_value(0x100, 9, 0);
        assert_eq!(m.journal_mark(), mark_two + 1);

        m.journal_rewind_to(mark_two);
        assert_eq!(m.dd_hex(0x100).as_deref(), Some("0x0000000000000007"));
        m.journal_rewind_to(mark_one);
        assert!(m.cells.contains_key(&0x100));
        m.journal_rewind_to(mark_empty);
        assert!(!m.cells.contains_key(&0x100), "insert must be undone");
    }

    #[test]
    fn journal_drop_before_releases_old_entries() {
        let mut m = Memory::new();
        m.start_journal();
        m.insert_dd(0x100, "1", "");
        m.set_dd_value(0x100, 2, 0);
        let mark = m.journal_mark();
        m.set_dd_value(0x100, 3, 0);
        m.journal_drop_before(mark);
        // The pinned prefix is gone, but everything after the mark still rewinds.
        m.journal_rewind_to(mark);
        assert_eq!(m.dd_hex(0x100).as_deref(), Some("0x0000000000000002"));
    }

    #[test]
    fn journal_is_inert_until_started() {
        let mut m = Memory::new();
        m.insert_dd(0x100, "1", "");
        assert_eq!(m.journal_mark(), 0);
        m.journal_rewind_to(0);
        assert!(m.cells.contains_key(&0x100));
    }

    #[test]
    fn tcl_string_range_matches() {
        assert_eq!(tcl_string_range("abcdef", 0, -1), "");
        assert_eq!(tcl_string_range("abcdef", -1, 1), "ab");
        assert_eq!(tcl_string_range("abcdef", 2, 99), "cdef");
        assert_eq!(tcl_string_range("abcdef", -99, -1), "");
        assert_eq!(tcl_string_range("abcdef", 5, 2), "");
    }
}
