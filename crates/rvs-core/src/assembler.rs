//! Two-pass assembler mirroring `RVS046.tcl`'s `lns`, `slbl`, `argsnrm`,
//! `chkadr`, `asm`, `instr`, `fmti` and `dump`.

use std::collections::BTreeMap;

use crate::memory;
use crate::memory::{MemoryCell, tcl_string_range};
use crate::registers::parse_reg;
use crate::tcl;

/// Result of tokenizing one source line (`lns`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub label: String,
    pub op: String,
    /// Raw operand text after the opcode (tabs converted to spaces).
    pub args: String,
    pub comment: String,
}

/// `lns`: split a line into `{label op args comment}`.
pub fn split_line(line: &str) -> Line {
    let mut ln = line.trim().to_string();
    let mut cm = String::new();
    if ln.contains('#') || ln.contains(';') || ln.contains("//") {
        let orig: Vec<char> = ln.chars().collect();
        let mut q1 = false;
        for i in 0..orig.len() {
            let ch = orig[i];
            if ch == '"' {
                q1 = !q1;
            }
            if ch == '#' || ch == ';' || (ch == '/' && orig.get(i + 1) == Some(&'/')) {
                if !q1 {
                    // Tcl reassigns on every unquoted hit; later hits shrink
                    // the working string and can blank the comment.
                    cm = tcl_string_range(&ln, i as i64, i64::MAX);
                    ln = tcl_string_range(&ln, 0, i as i64 - 1);
                }
            }
        }
    }
    let mut label = String::new();
    if let Some(i) = ln.find(':') {
        label = tcl_string_range(&ln, 0, i as i64 - 1);
        ln = tcl_string_range(&ln, i as i64 + 1, i64::MAX).trim().to_string();
    }
    let op = ln
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string();
    let args = tcl_string_range(&ln, op.chars().count() as i64, i64::MAX)
        .trim()
        .to_string();
    Line {
        label,
        op,
        args: args.replace('\t', " "),
        comment: cm,
    }
}

/// `lns` with `f=1`: the listing rendering of a source line.
pub fn render_line_f1(line: &Line) -> String {
    let label = if line.label.is_empty() {
        String::new()
    } else {
        format!("{}:", line.label)
    };
    format!(
        "{} {} {} {}",
        tcl::fmt_s_pad(&label, 5),
        tcl::fmt_s_pad(&line.op, 6),
        tcl::fmt_s_pad(&line.args, 20),
        line.comment
    )
}

/// `fmti`: `format "%-7s %-15s" op rest`.
pub fn fmti(op: &str, args: &[String]) -> String {
    format!(
        "{} {}",
        tcl::fmt_s_pad(op, 7),
        tcl::fmt_s_pad(&args.join(" "), 15)
    )
}

/// `slbl`: substitute char literals and labels, then evaluate with Tcl
/// `expr` semantics. Returns `None` on any error (Tcl's `"err"`).
pub fn slbl(expr: &str, sym: &BTreeMap<String, String>) -> Option<i128> {
    // 'c' -> 0x<hex>
    let mut s = expr.to_string();
    while let Some(i) = s.find('\'') {
        let chars: Vec<char> = s.chars().collect();
        if chars.get(i + 2) != Some(&'\'') {
            return None;
        }
        let c = chars[i + 1];
        let hex = format!("{:02x}", c as u32);
        let mut rebuilt: String = chars[..i].iter().collect();
        rebuilt.push_str("0x");
        rebuilt.push_str(&hex);
        rebuilt.extend(chars[i + 3..].iter());
        s = rebuilt;
    }
    // Space out operators exactly like the reference.
    let mut spaced = format!(" {s} ");
    for (from, to) in [
        ("(", " ( "),
        (")", " ) "),
        ("+", " + "),
        ("-", " - "),
        ("*", " * "),
        ("/", " / "),
        ("%", " % "),
        ("<<", " << "),
        (">>", " >> "),
        ("&", " & "),
        ("|", " || "),
    ] {
        spaced = spaced.replace(from, to);
    }
    // Substitute every known symbol (space-delimited), repeatedly.
    for (name, value) in sym.iter() {
        let pat = format!(" {name} ");
        let rep = format!(" {value} ");
        loop {
            let next = spaced.replace(&pat, &rep);
            if next == spaced {
                break;
            }
            spaced = next;
        }
    }
    tcl_expr(&spaced)
}

/// Tcl `expr` evaluation over integers (i128 bignum semantics; `/` floors,
/// `%` takes the divisor's sign, `|` was mapped to logical-or by `slbl`).
pub fn tcl_expr(text: &str) -> Option<i128> {
    #[derive(Debug, Clone, PartialEq)]
    enum Tok {
        Num(i128),
        Op(String),
        LParen,
        RParen,
    }
    let chars: Vec<char> = text.chars().collect();
    let mut toks: Vec<Tok> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '(' {
            toks.push(Tok::LParen);
            i += 1;
            continue;
        }
        if c == ')' {
            toks.push(Tok::RParen);
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            toks.push(Tok::Num(tcl::parse_int(&word)?));
            continue;
        }
        let rest: String = chars[i..].iter().collect();
        let mut matched = None;
        for op in [
            "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "+", "-", "*", "/", "%", "&", "<",
            ">",
        ] {
            if rest.starts_with(op) {
                matched = Some(op);
                break;
            }
        }
        match matched {
            Some(op) => {
                toks.push(Tok::Op(op.to_string()));
                i += op.len();
            }
            None => return None,
        }
    }
    if toks.is_empty() {
        return None;
    }

    // Tcl operator precedence: || < && < ==/!= < relational < & < shifts
    // < additive < multiplicative < unary. A bare `|` never survives `slbl`.
    fn bin<'a>(toks: &'a [Tok], pos: &mut usize, ops: &[&str]) -> Option<String> {
        if let Some(Tok::Op(op)) = toks.get(*pos) {
            if ops.contains(&op.as_str()) {
                let o = op.clone();
                *pos += 1;
                return Some(o);
            }
        }
        None
    }
    fn shift(v: i128, op: &str, amount: i128) -> Option<i128> {
        if amount < 0 {
            return None;
        }
        if op == "<<" {
            if amount > 200 {
                return None; // Tcl raises "integer value too large to represent"
            }
            v.checked_shl(amount as u32)
        } else if amount > 127 {
            Some(if v < 0 { -1 } else { 0 })
        } else {
            Some(v >> amount)
        }
    }
    fn parse_atom(toks: &[Tok], pos: &mut usize) -> Option<i128> {
        match toks.get(*pos)? {
            Tok::Num(n) => {
                let v = *n;
                *pos += 1;
                Some(v)
            }
            Tok::LParen => {
                *pos += 1;
                let v = parse_or(toks, pos)?;
                match toks.get(*pos) {
                    Some(Tok::RParen) => {
                        *pos += 1;
                        Some(v)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
    fn parse_unary(toks: &[Tok], pos: &mut usize) -> Option<i128> {
        if let Some(Tok::Op(op)) = toks.get(*pos) {
            if op == "-" {
                *pos += 1;
                return parse_unary(toks, pos).map(|v| -v);
            }
            if op == "+" {
                *pos += 1;
                return parse_unary(toks, pos);
            }
        }
        parse_atom(toks, pos)
    }
    fn parse_mul(toks: &[Tok], pos: &mut usize) -> Option<i128> {
        let mut left = parse_unary(toks, pos)?;
        loop {
            match bin(toks, pos, &["*", "/", "%"]) {
                Some(op) => {
                    let right = parse_unary(toks, pos)?;
                    left = match op.as_str() {
                        "*" => left.checked_mul(right)?,
                        "/" => {
                            if right == 0 {
                                return None;
                            }
                            let q = left / right;
                            let q = if left % right != 0 && ((left < 0) != (right < 0)) {
                                q - 1
                            } else {
                                q
                            };
                            q
                        }
                        _ => {
                            if right == 0 {
                                return None;
                            }
                            let q = left / right;
                            let q = if left % right != 0 && ((left < 0) != (right < 0)) {
                                q - 1
                            } else {
                                q
                            };
                            left - q * right
                        }
                    };
                }
                None => break,
            }
        }
        Some(left)
    }
    fn parse_add(toks: &[Tok], pos: &mut usize) -> Option<i128> {
        let mut left = parse_mul(toks, pos)?;
        loop {
            match bin(toks, pos, &["+", "-"]) {
                Some(op) => {
                    let right = parse_mul(toks, pos)?;
                    left = if op == "+" {
                        left.checked_add(right)?
                    } else {
                        left.checked_sub(right)?
                    };
                }
                None => break,
            }
        }
        Some(left)
    }
    fn parse_shift(toks: &[Tok], pos: &mut usize) -> Option<i128> {
        let mut left = parse_add(toks, pos)?;
        loop {
            match bin(toks, pos, &["<<", ">>"]) {
                Some(op) => {
                    let right = parse_add(toks, pos)?;
                    left = shift(left, &op, right)?;
                }
                None => break,
            }
        }
        Some(left)
    }
    fn parse_bitand(toks: &[Tok], pos: &mut usize) -> Option<i128> {
        let mut left = parse_shift(toks, pos)?;
        while bin(toks, pos, &["&"]).is_some() {
            let right = parse_shift(toks, pos)?;
            left &= right;
        }
        Some(left)
    }
    fn parse_rel(toks: &[Tok], pos: &mut usize) -> Option<i128> {
        let mut left = parse_bitand(toks, pos)?;
        loop {
            match bin(toks, pos, &["<", "<=", ">", ">="]) {
                Some(op) => {
                    let right = parse_bitand(toks, pos)?;
                    left = match op.as_str() {
                        "<" => i128::from(left < right),
                        "<=" => i128::from(left <= right),
                        ">" => i128::from(left > right),
                        _ => i128::from(left >= right),
                    };
                }
                None => break,
            }
        }
        Some(left)
    }
    fn parse_eq(toks: &[Tok], pos: &mut usize) -> Option<i128> {
        let mut left = parse_rel(toks, pos)?;
        loop {
            match bin(toks, pos, &["==", "!="]) {
                Some(op) => {
                    let right = parse_rel(toks, pos)?;
                    left = i128::from(if op == "==" { left == right } else { left != right });
                }
                None => break,
            }
        }
        Some(left)
    }
    fn parse_and(toks: &[Tok], pos: &mut usize) -> Option<i128> {
        let mut left = parse_eq(toks, pos)?;
        while bin(toks, pos, &["&&"]).is_some() {
            let right = parse_eq(toks, pos)?;
            left = i128::from(left != 0 && right != 0);
        }
        Some(left)
    }
    fn parse_or(toks: &[Tok], pos: &mut usize) -> Option<i128> {
        let mut left = parse_and(toks, pos)?;
        while bin(toks, pos, &["||"]).is_some() {
            let right = parse_and(toks, pos)?;
            left = i128::from(left != 0 || right != 0);
        }
        Some(left)
    }
    let mut pos = 0usize;
    let value = parse_or(&toks, &mut pos)?;
    if pos != toks.len() {
        return None;
    }
    Some(value)
}

/// `argsnrm`: `rd,imm(rs1)` -> `{rd rs1 imm}` (and keep 2-arg forms for
/// U/UJ ops).
pub fn normalize_args(args: &str, op: &str) -> Vec<String> {
    let trimmed = args.trim_matches(|c| c == '{' || c == '}' || c == ' ');
    let parts: Vec<String> = trimmed
        .split(',')
        .map(|p| p.trim_matches(|c| c == '{' || c == '}' || c == ' ').to_string())
        .collect();
    let arg0 = parts.first().cloned().unwrap_or_default();
    let arg1 = parts.get(1).cloned().unwrap_or_default();
    let arg2 = parts.get(2).cloned().unwrap_or_default();
    if !arg2.is_empty() {
        return vec![arg0, arg1, arg2];
    }
    let mut out = vec![arg0.clone(), arg1.clone()];
    if op != "auipc" && op != "jal" && op != "lui" {
        if let Some(p1) = arg1.rfind('(') {
            let arg11 = tcl_string_range(&arg1, 0, p1 as i64 - 1);
            let arg12 = tcl_string_range(&arg1, p1 as i64, i64::MAX);
            let arg2 = arg12.replace(['(', ')'], " ").trim().to_string();
            out = vec![arg0, arg2, arg11];
        }
    }
    out
}

/// Instruction encoding formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstrFmt {
    R,
    I,
    S,
    Sb,
    U,
    Uj,
    Fi,
    Fs,
    Fr,
    F2,
}

pub struct InstrDesc {
    pub op: &'static str,
    pub fmt: InstrFmt,
    pub opcode: u32,
    pub f3: Option<u32>,
    pub f7: Option<u32>,
}

/// `::instrl` plus the additions this rewrite provides (`subi`, `remu`
/// encoding and the RV64M word ops).
pub const INSTR_TABLE: &[InstrDesc] = &[
    d("lb", InstrFmt::I, 0x3, Some(0x0), None),
    d("lh", InstrFmt::I, 0x3, Some(0x1), None),
    d("lw", InstrFmt::I, 0x3, Some(0x2), None),
    d("ld", InstrFmt::I, 0x3, Some(0x3), None),
    d("lbu", InstrFmt::I, 0x3, Some(0x4), None),
    d("lhu", InstrFmt::I, 0x3, Some(0x5), None),
    d("lwu", InstrFmt::I, 0x3, Some(0x6), None),
    d("fence", InstrFmt::I, 0xf, Some(0x0), None),
    d("fence.i", InstrFmt::I, 0xf, Some(0x1), None),
    d("addi", InstrFmt::I, 0x13, Some(0x0), None),
    d("slli", InstrFmt::I, 0x13, Some(0x1), Some(0x0)),
    d("slti", InstrFmt::I, 0x13, Some(0x2), None),
    d("sltiu", InstrFmt::I, 0x13, Some(0x3), None),
    d("xori", InstrFmt::I, 0x13, Some(0x4), None),
    d("srli", InstrFmt::I, 0x13, Some(0x5), Some(0x0)),
    d("srai", InstrFmt::I, 0x13, Some(0x5), Some(0x20)),
    d("ori", InstrFmt::I, 0x13, Some(0x6), None),
    d("andi", InstrFmt::I, 0x13, Some(0x7), None),
    d("auipc", InstrFmt::U, 0x17, None, None),
    d("addiw", InstrFmt::I, 0x1b, Some(0x0), None),
    d("slliw", InstrFmt::I, 0x1b, Some(0x1), Some(0x0)),
    d("srliw", InstrFmt::I, 0x1b, Some(0x5), Some(0x0)),
    d("sraiw", InstrFmt::I, 0x1b, Some(0x5), Some(0x20)),
    d("sb", InstrFmt::S, 0x23, Some(0x0), None),
    d("sh", InstrFmt::S, 0x23, Some(0x1), None),
    d("sw", InstrFmt::S, 0x23, Some(0x2), None),
    d("sd", InstrFmt::S, 0x23, Some(0x3), None),
    d("add", InstrFmt::R, 0x33, Some(0x0), Some(0x0)),
    d("sub", InstrFmt::R, 0x33, Some(0x0), Some(0x20)),
    d("sll", InstrFmt::R, 0x33, Some(0x1), Some(0x0)),
    d("slt", InstrFmt::R, 0x33, Some(0x2), Some(0x0)),
    d("sltu", InstrFmt::R, 0x33, Some(0x3), Some(0x0)),
    d("xor", InstrFmt::R, 0x33, Some(0x4), Some(0x0)),
    d("srl", InstrFmt::R, 0x33, Some(0x5), Some(0x0)),
    d("sra", InstrFmt::R, 0x33, Some(0x5), Some(0x20)),
    d("or", InstrFmt::R, 0x33, Some(0x6), Some(0x0)),
    d("and", InstrFmt::R, 0x33, Some(0x7), Some(0x0)),
    d("lui", InstrFmt::U, 0x37, None, None),
    d("addw", InstrFmt::R, 0x3b, Some(0x0), Some(0x0)),
    d("subw", InstrFmt::R, 0x3b, Some(0x0), Some(0x20)),
    d("sllw", InstrFmt::R, 0x3b, Some(0x1), Some(0x0)),
    d("srlw", InstrFmt::R, 0x3b, Some(0x5), Some(0x0)),
    d("sraw", InstrFmt::R, 0x3b, Some(0x5), Some(0x20)),
    d("mulw", InstrFmt::R, 0x3b, Some(0x0), Some(0x1)),
    d("divw", InstrFmt::R, 0x3b, Some(0x4), Some(0x1)),
    d("divuw", InstrFmt::R, 0x3b, Some(0x5), Some(0x1)),
    d("remw", InstrFmt::R, 0x3b, Some(0x6), Some(0x1)),
    d("remuw", InstrFmt::R, 0x3b, Some(0x7), Some(0x1)),
    d("beq", InstrFmt::Sb, 0x63, Some(0x0), None),
    d("bne", InstrFmt::Sb, 0x63, Some(0x1), None),
    d("blt", InstrFmt::Sb, 0x63, Some(0x4), None),
    d("bge", InstrFmt::Sb, 0x63, Some(0x5), None),
    d("bltu", InstrFmt::Sb, 0x63, Some(0x6), None),
    d("bgeu", InstrFmt::Sb, 0x63, Some(0x7), None),
    d("jalr", InstrFmt::I, 0x67, Some(0x0), None),
    d("jal", InstrFmt::Uj, 0x6f, None, None),
    d("ecall", InstrFmt::I, 0x73, Some(0x0), Some(0x0)),
    d("ebreak", InstrFmt::I, 0x73, Some(0x0), Some(0x1)),
    d("csrrw", InstrFmt::I, 0x73, Some(0x1), None),
    d("csrrs", InstrFmt::I, 0x73, Some(0x2), None),
    d("csrrc", InstrFmt::I, 0x73, Some(0x3), None),
    d("csrrwi", InstrFmt::I, 0x73, Some(0x4), None),
    d("csrrsi", InstrFmt::I, 0x73, Some(0x5), None),
    d("csrrci", InstrFmt::I, 0x73, Some(0x6), None),
    d("subi", InstrFmt::I, 0x13, Some(0x0), None),
    d("mul", InstrFmt::R, 0x33, Some(0x0), Some(0x1)),
    d("mulh", InstrFmt::R, 0x33, Some(0x1), Some(0x1)),
    d("mulhsu", InstrFmt::R, 0x33, Some(0x2), Some(0x1)),
    d("mulhu", InstrFmt::R, 0x33, Some(0x3), Some(0x1)),
    d("div", InstrFmt::R, 0x33, Some(0x4), Some(0x1)),
    d("divu", InstrFmt::R, 0x33, Some(0x5), Some(0x1)),
    d("rem", InstrFmt::R, 0x33, Some(0x6), Some(0x1)),
    d("remu", InstrFmt::R, 0x33, Some(0x7), Some(0x1)),
    d("fld", InstrFmt::Fi, 0x7, Some(0x3), None),
    d("fsd", InstrFmt::Fs, 0x27, Some(0x3), None),
    d("fadd.d", InstrFmt::Fr, 0x53, Some(0x0), Some(0x1)),
    d("fsub.d", InstrFmt::Fr, 0x53, Some(0x0), Some(0x5)),
    d("fmul.d", InstrFmt::Fr, 0x53, Some(0x0), Some(0x9)),
    d("fdiv.d", InstrFmt::Fr, 0x53, Some(0x0), Some(0xd)),
    d("fsqrt.d", InstrFmt::F2, 0x53, Some(0x0), Some(0b0101101)),
    d("fsgnj.d", InstrFmt::Fr, 0x53, Some(0b000), Some(0b0010001)),
    d("fsgnjn.d", InstrFmt::Fr, 0x53, Some(0b001), Some(0b0010001)),
    d("fsgnjx.d", InstrFmt::Fr, 0x53, Some(0b010), Some(0b0010001)),
    d("fmin.d", InstrFmt::Fr, 0x53, Some(0b000), Some(0b0010101)),
    d("fmax.d", InstrFmt::Fr, 0x53, Some(0b010), Some(0b0010101)),
    d("feq.d", InstrFmt::Fr, 0x53, Some(0b010), Some(0b1010001)),
    d("flt.d", InstrFmt::Fr, 0x53, Some(0b001), Some(0b1010001)),
    d("fle.d", InstrFmt::Fr, 0x53, Some(0b000), Some(0b1010001)),
    d("fcvt.l.d", InstrFmt::F2, 0x53, Some(0x0), Some(0b1100001)),
    d("fcvt.lu.d", InstrFmt::F2, 0x53, Some(0x0), Some(0b1100001)),
    d("fmv.x.d", InstrFmt::F2, 0x53, Some(0x0), Some(0b1110001)),
    d("fcvt.d.l", InstrFmt::F2, 0x53, Some(0x0), Some(0b1101001)),
    d("fcvt.d.lu", InstrFmt::F2, 0x53, Some(0x0), Some(0b1101001)),
    d("fmv.d.x", InstrFmt::F2, 0x53, Some(0x0), Some(0b1111001)),
];

const fn d(op: &'static str, fmt: InstrFmt, opcode: u32, f3: Option<u32>, f7: Option<u32>) -> InstrDesc {
    InstrDesc { op, fmt, opcode, f3, f7 }
}

pub fn find_instr(op: &str) -> Option<&'static InstrDesc> {
    INSTR_TABLE.iter().find(|dd| dd.op == op.to_ascii_lowercase())
}

/// One encoded instruction, exactly the four values `instr` returns.
#[derive(Clone, Debug, Default)]
pub struct Encoded {
    pub bin: String,
    pub asmx: Vec<String>,
    pub asm: String,
    pub error: Option<String>,
}

fn reg_or_err(arg: &str, role: &str, fp_only: bool, reject_fp: bool) -> Result<u8, String> {
    match parse_reg(arg) {
        None => Err(format!("ERROR: {role} NOT A REGISTER")),
        Some(idx) => {
            let is_f = arg.to_ascii_lowercase().starts_with('f');
            if fp_only && !is_f {
                Err(format!("ERROR: {role} NOT AN FP REGISTER"))
            } else if reject_fp && is_f {
                Err(format!("ERROR: {role} NOT A REGISTER"))
            } else {
                Ok(idx)
            }
        }
    }
}

/// `instr`: encode one instruction at `pc` with `sym` for label resolution.
pub fn encode(op: &str, args: &[String], pc: u64, sym: &BTreeMap<String, String>) -> Encoded {
    let desc = match find_instr(op) {
        Some(dd) => dd,
        None => {
            return Encoded {
                bin: "ERROR: INSTR CODE NOT RECOGNIZED".to_string(),
                asmx: {
                    let mut v = vec![op.to_string()];
                    v.extend(args.iter().cloned());
                    v
                },
                asm: format!("{op} {}", args.join(" ")),
                error: Some("ERROR: INSTR CODE NOT RECOGNIZED".to_string()),
            }
        }
    };
    let mut e = Encoded::default();
    e.asm = match desc.fmt {
        InstrFmt::I | InstrFmt::Fi => {
            if desc.op == "ld" || desc.op == "fld" {
                format!("{op} {},{}({})", args[0], args[2], args[1])
            } else {
                format!("{op} {},{},{}", args[0], args[1], args[2])
            }
        }
        InstrFmt::S | InstrFmt::Fs => format!("{op} {},{}({})", args[0], args[2], args[1]),
        InstrFmt::Sb => format!("{op} {},{},{}", args[0], args[1], args[2]),
        InstrFmt::R | InstrFmt::Fr => format!("{op} {},{},{}", args[0], args[1], args[2]),
        InstrFmt::U | InstrFmt::Uj => format!("{op} {},{}", args[0], args[1]),
        InstrFmt::F2 => {
            let rs2: u8 = match desc.op {
                "fcvt.d.l" | "fcvt.l.d" => 2,
                "fcvt.d.lu" | "fcvt.lu.d" => 3,
                _ => 0,
            };
            format!("{op} {},{},f{}", args[0], args[1], rs2)
        }
    };
    e.asmx = vec![op.to_string()];
    let slbl_imm = |raw: &str| -> Result<i64, String> {
        match slbl(raw, sym) {
            None => Err("ERROR: imm NOT AN INTEGER".to_string()),
            Some(v) => {
                if v < i64::MIN as i128 || v > i64::MAX as i128 {
                    Err("ERROR: imm NOT AN INTEGER".to_string())
                } else {
                    Ok(v as i64)
                }
            }
        }
    };
    let imm12_ok = |imm: i64| -> Result<(), String> {
        if imm < -(1i64 << 11) || imm > (1i64 << 12) - 1 {
            Err(format!(
                "ERROR: imm OUT OF RANGE [{}{}]",
                -(1i64 << 11),
                (1i64 << 12) - 1
            ))
        } else {
            Ok(())
        }
    };
    match desc.fmt {
        InstrFmt::R => {
            e.asmx = vec![op.to_string(), "rd,rs1,rs2".to_string()];
            if args.len() != 3 {
                e.error = Some("ERROR: MUST HAVE 3 ARGS".to_string());
            } else if let Err(msg) = reg_or_err(&args[0], "rd", false, true)
                .and_then(|rd| reg_or_err(&args[1], "rs1", false, true).map(|r1| (rd, r1)))
                .and_then(|(rd, r1)| reg_or_err(&args[2], "rs2", false, true).map(|r2| (rd, r1, r2)))
            {
                e.error = Some(msg);
            } else {
                let (rd, rs1, rs2) = (
                    parse_reg(&args[0]).unwrap(),
                    parse_reg(&args[1]).unwrap(),
                    parse_reg(&args[2]).unwrap(),
                );
                e.asmx = vec![
                    op.to_string(),
                    format!("x{rd}"),
                    format!("x{rs1}"),
                    format!("x{rs2}"),
                ];
                e.bin = format!(
                    "R  {:07b} {:05b} {:05b} {:03b} {:05b} {:07b}",
                    desc.f7.unwrap_or(0),
                    rs2,
                    rs1,
                    desc.f3.unwrap_or(0),
                    rd,
                    desc.opcode
                );
            }
        }
        InstrFmt::I => {
            e.asmx = vec![op.to_string(), "rd,rs1,imm".to_string()];
            if args.len() != 3 {
                e.error = Some("ERROR: MUST HAVE 3 ARGS".to_string());
            } else {
                let is_ecall = desc.op == "ecall";
                let rd = reg_or_err(&args[0], "rd", false, !is_ecall);
                let rs1 = reg_or_err(&args[1], "rs1", false, !is_ecall);
                match (rd, rs1, slbl_imm(&args[2])) {
                    (Err(m), _, _) | (_, Err(m), _) | (_, _, Err(m)) => e.error = Some(m),
                    (Ok(rd), Ok(rs1), Ok(imm)) => {
                        if let Err(m) = imm12_ok(imm) {
                            e.error = Some(m);
                        } else {
                            let rdxp = if args[0].to_ascii_lowercase().starts_with('f') { "f" } else { "x" };
                            let rs1xp = if args[1].to_ascii_lowercase().starts_with('f') { "f" } else { "x" };
                            e.asmx = vec![
                                op.to_string(),
                                format!("{rdxp}{rd}"),
                                format!("{rs1xp}{rs1}"),
                                tcl::i12x_hex(imm),
                            ];
                            e.bin = format!(
                                "I  {}  {:05b} {:03b} {:05b} {:07b}",
                                tcl::fmt_bits(imm, 12),
                                rs1,
                                desc.f3.unwrap_or(0),
                                rd,
                                desc.opcode
                            );
                            e.asm = if desc.op == "ld" {
                                format!("{op} {},{}({})", args[0], imm, args[1])
                            } else {
                                format!("{op} {},{},{}", args[0], args[1], imm)
                            };
                        }
                    }
                }
            }
        }
        InstrFmt::Fi => {
            e.asmx = vec![op.to_string(), "fd,rs1,imm".to_string()];
            if args.len() != 3 {
                e.error = Some("ERROR: MUST HAVE 3 ARGS".to_string());
            } else {
                let rd = reg_or_err(&args[0], "fd", true, false);
                let rs1 = reg_or_err(&args[1], "rs1", false, false);
                match (rd, rs1, slbl_imm(&args[2])) {
                    (Err(m), _, _) | (_, Err(m), _) | (_, _, Err(m)) => e.error = Some(m),
                    (Ok(rd), Ok(rs1), Ok(imm)) => {
                        if let Err(m) = imm12_ok(imm) {
                            e.error = Some(m);
                        } else {
                            e.asmx = vec![
                                op.to_string(),
                                format!("f{rd}"),
                                format!("x{rs1}"),
                                tcl::i12x_hex(imm),
                            ];
                            e.bin = format!(
                                "FI {}  {:05b} {:03b} {:05b} {:07b}",
                                tcl::fmt_bits(imm, 12),
                                rs1,
                                desc.f3.unwrap_or(0),
                                rd,
                                desc.opcode
                            );
                            e.asm = format!("{op} {},{}({})", args[0], imm, args[1]);
                        }
                    }
                }
            }
        }
        InstrFmt::S | InstrFmt::Fs => {
            let is_f = desc.fmt == InstrFmt::Fs;
            let (r2role, r1role) = if is_f { ("fs2", "rs1") } else { ("rs2", "rs1") };
            e.asmx = vec![op.to_string(), format!("{r2role},{r1role},imm").to_string()];
            if args.len() != 3 {
                e.error = Some("ERROR: MUST HAVE 3 ARGS".to_string());
            } else {
                let rs2 = reg_or_err(&args[0], r2role, is_f, !is_f);
                let rs1 = reg_or_err(&args[1], r1role, false, is_f);
                match (rs2, rs1, slbl_imm(&args[2])) {
                    (Err(m), _, _) | (_, Err(m), _) | (_, _, Err(m)) => e.error = Some(m),
                    (Ok(rs2), Ok(rs1), Ok(imm)) => {
                        if let Err(m) = imm12_ok(imm) {
                            e.error = Some(m);
                        } else {
                            let r2p = if is_f { "f" } else { "x" };
                            e.asmx = vec![
                                op.to_string(),
                                format!("{r2p}{rs2}"),
                                format!("x{rs1}"),
                                tcl::i12x_hex(imm),
                            ];
                            let immb = tcl::fmt_bits(imm, 12);
                            let hi = tcl_string_range(&immb, 0, 6);
                            let lo = tcl_string_range(&immb, 7, 11);
                            e.bin = format!(
                                "{prefix} {hi} {:05b} {:05b} {:03b} {lo} {:07b}",
                                rs2,
                                rs1,
                                desc.f3.unwrap_or(0),
                                desc.opcode,
                                prefix = if is_f { "FS" } else { "S " }
                            );
                            e.asm = format!("{op} {},{}({})", args[0], imm, args[1]);
                        }
                    }
                }
            }
        }
        InstrFmt::Sb => {
            e.asmx = vec![op.to_string(), "rs1,rs2,imm".to_string()];
            if args.len() != 3 {
                e.error = Some("ERROR: MUST HAVE 3 ARGS".to_string());
            } else {
                let rs1 = reg_or_err(&args[0], "rs1", false, true);
                let rs2 = reg_or_err(&args[1], "rs2", false, true);
                match (rs1, rs2, slbl_imm(&args[2])) {
                    (Err(m), _, _) | (_, Err(m), _) | (_, _, Err(m)) => e.error = Some(m),
                    (Ok(rs1), Ok(rs2), Ok(target)) => {
                        if target < 0 {
                            e.error = Some("ERROR: imm IS NEGATIVE".to_string());
                        } else if target % 4 != 0 {
                            e.error = Some("ERROR: imm NOT WORD-ALIGNED".to_string());
                        } else {
                            let offset = target as i64 - pc as i64;
                            if offset < -(1i64 << 12) || offset > (1i64 << 12) - 1 {
                                e.error = Some(format!(
                                    "ERROR: imm OUT OF RANGE [PC{},PC+{}]",
                                    -(1i64 << 12),
                                    (1i64 << 12) - 1
                                ));
                            } else {
                                let imm = offset / 2;
                                e.asmx = vec![
                                    op.to_string(),
                                    format!("x{rs1}"),
                                    format!("x{rs2}"),
                                    tcl::i12x_hex(imm),
                                ];
                                let immb = tcl::fmt_bits(imm, 13);
                                e.bin = format!(
                                    "SB {}{} {:05b} {:05b} {:03b} {}{} {:07b}",
                                    immb.get(0..1).unwrap_or("0"),
                                    tcl_string_range(&immb, 2, 7),
                                    rs2,
                                    rs1,
                                    desc.f3.unwrap_or(0),
                                    tcl_string_range(&immb, 8, 11),
                                    tcl_string_range(&immb, 1, 1),
                                    desc.opcode
                                );
                                e.asm = format!("{op} {},{},{}", args[0], args[1], imm);
                            }
                        }
                    }
                }
            }
        }
        InstrFmt::U | InstrFmt::Uj => {
            let is_uj = desc.fmt == InstrFmt::Uj;
            e.asmx = vec![op.to_string(), "rd,imm".to_string()];
            if args.len() != 2 {
                e.error = Some("ERROR: MUST HAVE 2 ARGS".to_string());
            } else {
                let rd = reg_or_err(&args[0], "rd", false, true);
                match (rd, slbl_imm(&args[1])) {
                    (Err(m), _) | (_, Err(m)) => e.error = Some(m),
                    (Ok(rd), Ok(target)) => {
                        let hexw = if is_uj { 21 } else { 20 };
                        let pw = hexw - 1;
                        let raw_imm = if is_uj {
                            if target < 0 {
                                e.error = Some("ERROR: imm IS NEGATIVE".to_string());
                                return finish(e);
                            }
                            if target % 4 != 0 {
                                e.error = Some("ERROR: imm NOT WORD-ALIGNED".to_string());
                                return finish(e);
                            }
                            let offset = target as i64 - pc as i64;
                            if offset < -(1i64 << pw) || offset > (1i64 << pw) - 1 {
                                e.error = Some(format!(
                                    "ERROR: imm OUT OF RANGE [PC{},PC+{}]",
                                    -(1i64 << pw),
                                    (1i64 << pw) - 1
                                ));
                                return finish(e);
                            }
                            offset / 2
                        } else if target < -(1i64 << pw) || target > (1i64 << (pw + 1)) - 1 {
                            e.error = Some(format!(
                                "ERROR: imm OUT OF RANGE [{},{}]",
                                -(1i64 << pw),
                                (1i64 << (pw + 1)) - 1
                            ));
                            return finish(e);
                        } else {
                            target
                        };
                        e.asmx = vec![
                            op.to_string(),
                            format!("x{rd}"),
                            tcl::i20x_hex(raw_imm),
                        ];
                        let digits = if is_uj { 21 } else { 20 };
                        let immb = tcl::fmt_bits(raw_imm, digits);
                        e.bin = if is_uj {
                            format!(
                                "UJ {}{}{}{} {:05b} {:07b}",
                                tcl_string_range(&immb, 0, 0),
                                tcl_string_range(&immb, 10, 19),
                                tcl_string_range(&immb, 9, 9),
                                tcl_string_range(&immb, 1, 8),
                                rd,
                                desc.opcode
                            )
                        } else {
                            format!("U  {immb} {:05b} {:07b}", rd, desc.opcode)
                        };
                        e.asm = format!("{op} {},{}", args[0], raw_imm);
                    }
                }
            }
        }
        InstrFmt::F2 => {
            e.asmx = vec![op.to_string(), "fd,fs1,fs2".to_string()];
            let rs2: u8 = match desc.op {
                "fcvt.d.l" | "fcvt.l.d" => 2,
                "fcvt.d.lu" | "fcvt.lu.d" => 3,
                _ => 0,
            };
            if args.len() != 2 {
                e.error = Some("ERROR: MUST HAVE 2 ARGS".to_string());
            } else {
                let int_rd = !matches!(desc.op, "fcvt.d.l" | "fcvt.d.lu" | "fmv.d.x" | "fsqrt.d");
                let result = if int_rd {
                    reg_or_err(&args[0], "rd", false, true)
                        .and_then(|rd| reg_or_err(&args[1], "fs1", true, false).map(|r| (rd, r)))
                } else {
                    reg_or_err(&args[0], "fd", true, false)
                        .and_then(|rd| {
                            if desc.op == "fsqrt.d" {
                                reg_or_err(&args[1], "rs1", true, false).map(|r| (rd, r))
                            } else {
                                reg_or_err(&args[1], "rs1", false, true).map(|r| (rd, r))
                            }
                        })
                };
                match result {
                    Err(m) => e.error = Some(m),
                    Ok((rd, rs1)) => {
                        e.asmx = vec![
                            op.to_string(),
                            format!("{}{rd}", args[0].chars().next().unwrap_or('x')),
                            format!("{}{rs1}", args[1].chars().next().unwrap_or('x')),
                            format!("f{rs2}"),
                        ];
                        e.bin = format!(
                            "FR {:07b} {:05b} {:05b} {:03b} {:05b} {:07b}",
                            desc.f7.unwrap_or(0),
                            rs2,
                            rs1,
                            desc.f3.unwrap_or(0),
                            rd,
                            desc.opcode
                        );
                    }
                }
            }
        }
        InstrFmt::Fr => {
            e.asmx = vec![op.to_string(), "fd,fs1,fs2".to_string()];
            if args.len() != 3 {
                e.error = Some("ERROR: MUST HAVE 3 ARGS".to_string());
            } else {
                let int_rd = matches!(desc.op, "feq.d" | "flt.d" | "fle.d");
                let rd = reg_or_err(&args[0], if int_rd { "rd" } else { "fd" }, !int_rd, int_rd);
                let rs1 = reg_or_err(&args[1], "fs1", true, false);
                let rs2 = reg_or_err(&args[2], "fs2", true, false);
                match (rd, rs1, rs2) {
                    (Err(m), _, _) | (_, Err(m), _) | (_, _, Err(m)) => e.error = Some(m),
                    (Ok(rd), Ok(rs1), Ok(rs2)) => {
                        let rdp = if int_rd { "x" } else { "f" };
                        e.asmx = vec![
                            op.to_string(),
                            format!("{rdp}{rd}"),
                            format!("f{rs1}"),
                            format!("f{rs2}"),
                        ];
                        e.bin = format!(
                            "FR {:07b} {:05b} {:05b} {:03b} {:05b} {:07b}",
                            desc.f7.unwrap_or(0),
                            rs2,
                            rs1,
                            desc.f3.unwrap_or(0),
                            rd,
                            desc.opcode
                        );
                    }
                }
            }
        }
    }
    finish(e)
}

fn finish(mut e: Encoded) -> Encoded {
    if e.bin.is_empty() {
        let msg = e.error.clone().unwrap_or_default();
        e.bin = msg;
    }
    e
}

/// `asm`: first pass — build `::sym` and `::mem` from the source text.
/// Returns the listing (`dump`) on success.
pub fn assemble(sim: &mut crate::Simulator, body: &str) -> Result<String, crate::AsmError> {
    sim.mem.clear();
    sim.sym.clear();
    sim.regs.clear();
    sim.regs.pc = 0;
    sim.inp_prompt = false;
    let mut res = String::from("\n");
    macro_rules! fail {
        ($msg:expr) => {{
            return Err(crate::AsmError { progress: res, message: $msg.to_string() });
        }};
    }
    for line in body.split('\n') {
        let parsed = split_line(line);
        res.push_str(line);
        res.push('\n');
        let label = parsed.label.clone();
        if !label.is_empty() {
            if !sim.sym.contains_key(&label) || label == "START" {
                sim.sym.insert(label.clone(), tcl::a64(sim.regs.pc));
            } else {
                fail!("redefined label");
            }
        }
        if !label.is_empty()
            && (!tcl::is_alpha_first(&label) || !tcl::is_wordchars(&label))
        {
            fail!("Label name must be alphanumeric starting with a letter");
        }
        if parsed.op.is_empty() {
            continue;
        }
        let op = parsed.op.clone();
        let lineorg = line.to_string();
        let mut args = parsed
            .args
            .trim_matches(|c| c == '{' || c == '}')
            .to_string();

        // Directives that fold into DD.
        if op.eq_ignore_ascii_case("DC") {
            if !args.starts_with('"') || !args.ends_with('"') || args.len() < 2 {
                fail!("DC operand must be enclosed in double quotes");
            }
            let inner = &args[1..args.len() - 1];
            let chars: Vec<String> = tcl_subst(inner)
                .chars()
                .map(|c| c.to_string())
                .collect();
            args = match memory::args2hex(&chars, memory::ArgsFmt::A1) {
                Ok(a) => a.join(","),
                Err(e) => fail!(e.0),
            };
        } else if op.eq_ignore_ascii_case("DW") {
            let items: Vec<String> = args.split(',').map(|s| s.to_string()).collect();
            args = match memory::args2hex(&items, memory::ArgsFmt::I1) {
                Ok(a) => a.join(","),
                Err(e) => fail!(e.0),
            };
        } else if op.eq_ignore_ascii_case("DH") {
            let items: Vec<String> = args.split(',').map(|s| s.to_string()).collect();
            args = match memory::args2hex(&items, memory::ArgsFmt::S1) {
                Ok(a) => a.join(","),
                Err(e) => fail!(e.0),
            };
        } else if op.eq_ignore_ascii_case("DB") {
            let items: Vec<String> = args.split(',').map(|s| s.to_string()).collect();
            args = match memory::args2hex(&items, memory::ArgsFmt::C1) {
                Ok(a) => a.join(","),
                Err(e) => fail!(e.0),
            };
        } else if op.eq_ignore_ascii_case("DF") {
            let items: Vec<String> = args.split(',').map(|s| s.to_string()).collect();
            args = match memory::args2hex(&items, memory::ArgsFmt::Q1) {
                Ok(a) => a.join(","),
                Err(e) => fail!(e.0),
            };
        } else if op.eq_ignore_ascii_case("DM") {
            let count = match tcl::parse_int(&args) {
                Some(c) if c >= 0 => c as usize,
                _ => fail!(format!("expected integer but got \"{args}\"")),
            };
            let zeros: Vec<String> = vec!["0".to_string(); count];
            args = zeros.join(",");
        }

        if op.eq_ignore_ascii_case("DD") || args_are_dd(&op) {
            let mut first = true;
            for raw_arg in args.split(',') {
                let arg = raw_arg.trim().to_string();
                if arg.is_empty() {
                    fail!("Missing operand");
                }
                if sim.regs.pc % 8 != 0 {
                    sim.regs.pc = sim.regs.pc / 8 * 8 + 8;
                    if !label.is_empty() {
                        sim.sym.insert(label.clone(), tcl::a64(sim.regs.pc));
                    }
                }
                if sim.mem.check_addr_conflict(sim.regs.pc, true) {
                    fail!("Overwrite attempt");
                }
                let source = if first { lineorg.clone() } else { String::new() };
                sim.mem.insert_dd(sim.regs.pc, &arg, &source);
                first = false;
                sim.regs.pc = sim.regs.pc.wrapping_add(8);
            }
            continue;
        }
        if op.eq_ignore_ascii_case("ORG") {
            let value = match slbl(&args, &sim.sym).filter(|v| {
                *v >= i64::MIN as i128 && *v <= i64::MAX as i128
            }) {
                Some(v) => v,
                None => fail!("Wrong ORG value/expr (no forward labels allowed)"),
            };
            sim.regs.pc = value as i64 as u64;
            if !label.is_empty() {
                sim.sym.insert(label.clone(), tcl::a64(sim.regs.pc));
            }
            continue;
        }
        if op.eq_ignore_ascii_case("EQU") {
            if label.is_empty() {
                fail!("EQU must have a label");
            }
            let value = match slbl(&args, &sim.sym).filter(|v| {
                *v >= i64::MIN as i128 && *v <= i64::MAX as i128
            }) {
                Some(v) => v,
                None => fail!("Wrong EQU value/expr (no forward labels allowed)"),
            };
            sim.sym.insert(label.clone(), tcl::a64(value as i64 as u64));
            continue;
        }
        if sim.regs.pc % 4 != 0 {
            sim.regs.pc = sim.regs.pc / 4 * 4 + 4;
            if !label.is_empty() {
                sim.sym.insert(label.clone(), tcl::a64(sim.regs.pc));
            }
        }
        if sim.mem.check_addr_conflict(sim.regs.pc, false) {
            fail!("Overwrite attempt");
        }
        if !sim.sym.contains_key("START") {
            sim.sym.insert("START".to_string(), tcl::a64(sim.regs.pc));
        }
        let mut arg_text = args.as_str();
        let lowered = op.to_ascii_lowercase();
        if (lowered == "ecall" || lowered == "ebreak") && arg_text.trim().is_empty() {
            arg_text = "x0,x0,0";
        }
        let norm = normalize_args(arg_text, &op);
        sim.mem
            .insert_instruction(sim.regs.pc, &op, norm, &lineorg);
        sim.regs.pc = sim.regs.pc.wrapping_add(4);
    }
    if !sim.sym.contains_key("START") {
        sim.sym
            .insert("START".to_string(), crate::Simulator::START_DEFAULT.to_string());
    }
    let listing = dump(sim);
    sim.regs.pc = sim
        .sym
        .get("START")
        .and_then(|s| tcl::parse_int(s))
        .map(|v| v as i64 as u64)
        .unwrap_or(0);
    Ok(listing)
}

/// Ops that were rewritten to `DD` by the directive handlers above.
fn args_are_dd(op: &str) -> bool {
    let l = op.to_ascii_lowercase();
    matches!(l.as_str(), "dc" | "dw" | "dh" | "db" | "df" | "dm")
}

/// `subst -nocommands -novariables`: backslash escapes only.
fn tcl_subst(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c != '\\' {
            out.push(c);
            i += 1;
            continue;
        }
        if i + 1 >= chars.len() {
            out.push('\\');
            break;
        }
        let n = chars[i + 1];
        match n {
            'n' => {
                out.push('\n');
                i += 2;
            }
            'r' => {
                out.push('\r');
                i += 2;
            }
            't' => {
                out.push('\t');
                i += 2;
            }
            'a' => {
                out.push('\u{7}');
                i += 2;
            }
            'b' => {
                out.push('\u{8}');
                i += 2;
            }
            'f' => {
                out.push('\u{c}');
                i += 2;
            }
            'v' => {
                out.push('\u{b}');
                i += 2;
            }
            '\\' | '"' | '$' | '[' => {
                out.push(n);
                i += 2;
            }
            'x' => {
                let mut digits = String::new();
                let mut j = i + 2;
                while j < chars.len() && digits.len() < 2 && chars[j].is_ascii_hexdigit() {
                    digits.push(chars[j]);
                    j += 1;
                }
                if digits.is_empty() {
                    out.push('x');
                    i += 2;
                } else {
                    let v = u32::from_str_radix(&digits, 16).unwrap_or(0);
                    if let Some(ch) = char::from_u32(v) {
                        out.push(ch);
                    }
                    i = j;
                }
            }
            'u' => {
                let mut digits = String::new();
                let mut j = i + 2;
                while j < chars.len() && digits.len() < 4 && chars[j].is_ascii_hexdigit() {
                    digits.push(chars[j]);
                    j += 1;
                }
                if digits.is_empty() {
                    out.push('u');
                    i += 2;
                } else {
                    let v = u32::from_str_radix(&digits, 16).unwrap_or(0);
                    if let Some(ch) = char::from_u32(v) {
                        out.push(ch);
                    }
                    i = j;
                }
            }
            '0'..='7' => {
                let mut digits = String::new();
                let mut j = i + 1;
                while j < chars.len() && digits.len() < 3 && ('0'..='7').contains(&chars[j]) {
                    digits.push(chars[j]);
                    j += 1;
                }
                let v = u32::from_str_radix(&digits, 8).unwrap_or(0);
                if let Some(ch) = char::from_u32(v & 0xff) {
                    out.push(ch);
                }
                i = j;
            }
            '\n' => {
                // Backslash-newline is replaced by a single space.
                out.push(' ');
                i += 2;
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
            }
            _ => {
                out.push(n);
                i += 2;
            }
        }
    }
    out
}

/// `dump`: second pass — evaluate `DD` values, encode instructions into the
/// `asmx` form used for execution, and render the listing.
pub fn dump(sim: &mut crate::Simulator) -> String {
    let mut res = String::from("ASSEMBLY LISTING\n");
    let mut header = String::new();
    header.push_str(&tcl::fmt_s_pad("ADDRESS", 18));
    header.push(' ');
    if sim.view.list_bin {
        header.push_str(&tcl::fmt_s_pad("BIN/HEX CODE", 40));
        header.push(' ');
    }
    if sim.view.list_hex {
        header.push_str(&tcl::fmt_s_pad("        HEX OPERANDS", 25));
        header.push(' ');
    }
    if sim.view.list_int {
        header.push_str(&tcl::fmt_s_pad("        INT OPERANDS", 25));
        header.push(' ');
    }
    if sim.view.list_txt {
        header.push_str(&tcl::fmt_s_pad("      TEXT SOURCE", 50));
        header.push(' ');
    }
    res.push_str(&header);
    res.push('\n');

    let addresses: Vec<u64> = sim.mem.cells.keys().copied().collect();
    for addr in addresses {
        let addr_hex = format!("0x{addr:016x}");
        match sim.mem.cells.get(&addr).cloned() {
            Some(MemoryCell::Data {
                value,
                case,
                expr,
                source_line,
            }) => {
                let raw = expr.clone().unwrap_or_else(|| crate::memory::render_value(value, case));
                let (evaluated, new_value, err) = match slbl(&raw, &sim.sym) {
                    Some(v) if v >= i64::MIN as i128 && v <= u64::MAX as i128 => {
                        (format!("0x{:016x}", v as u64), v as u64, false)
                    }
                    _ => ("ERROR: NOT A NUMBER".to_string(), 0, true),
                };
                if err {
                    // Tcl stores the error text as the cell's value; reads of
                    // such a cell degrade to string ops in the reference.
                    sim.mem.set_dd_value(addr, 0, 0);
                    if let Some(crate::memory::MemoryCell::Data { expr, .. }) =
                        sim.mem.cells.get_mut(&addr)
                    {
                        *expr = Some(evaluated.clone());
                    }
                } else {
                    sim.mem.set_dd_value(addr, new_value, 0);
                }
                res.push_str(&addr_hex);
                if sim.view.list_bin {
                    res.push_str(&format!(" DD {evaluated}{}", " ".repeat(19)));
                }
                if sim.view.list_hex {
                    res.push(' ');
                    res.push_str(&tcl::fmt_s_pad("", 25));
                }
                if sim.view.list_int {
                    res.push(' ');
                    res.push_str(&tcl::fmt_s_pad("", 25));
                }
                if sim.view.list_txt {
                    res.push(' ');
                    res.push_str(&render_line_f1(&split_line(&source_line)));
                }
                res.push('\n');
            }
            Some(MemoryCell::Instruction(cell)) => {
                let enc = encode(&cell.op, &cell.args, addr, &sim.sym);
                let new_op = enc.asmx.first().cloned().unwrap_or_default();
                let new_args: Vec<String> = enc.asmx.iter().skip(1).cloned().collect();
                sim.mem.insert_instruction(addr, &new_op, new_args.clone(), &cell.source_line);
                res.push_str(&addr_hex);
                if sim.view.list_bin {
                    res.push(' ');
                    res.push_str(&tcl::fmt_s_pad(&enc.bin, 40));
                }
                if sim.view.list_hex {
                    res.push(' ');
                    res.push_str(&tcl::fmt_s_pad(
                        &fmti(enc.asmx.first().map(String::as_str).unwrap_or(""), &new_args),
                        25,
                    ));
                }
                if sim.view.list_int {
                    let words: Vec<&str> = enc.asm.split_whitespace().collect();
                    let int_op = words.first().copied().unwrap_or("");
                    let int_rest: Vec<String> = words.iter().skip(1).map(|s| s.to_string()).collect();
                    res.push(' ');
                    res.push_str(&tcl::fmt_s_pad(&fmti(int_op, &int_rest), 25));
                }
                if sim.view.list_txt {
                    res.push(' ');
                    res.push_str(&render_line_f1(&split_line(&cell.source_line)));
                }
                res.push('\n');
            }
            None => {}
        }
    }
    res.push_str("\nSYMBOL TABLE\n");
    for (name, value) in sim.sym.iter() {
        let v = tcl::parse_int(value).unwrap_or(0) as u64;
        res.push_str(&format!("0x{v:016x} {}\n", tcl::fmt_s_pad(name, 10)));
    }
    res
}
