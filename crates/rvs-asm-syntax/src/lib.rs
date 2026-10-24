//! RVS assembly line parsing, expression evaluation and the instruction
//! table, mirroring `RVS046.tcl`'s `lns`, `slbl`, `argsnrm`, `chkadr`,
//! `instr` and `fmti`.

use std::collections::BTreeMap;

use rvs_memory::tcl_string_range;
use rvs_tcl as tcl;

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
