//! Two-way transpiler between Standard GNU RISC-V assembly and RVS Dialect assembly.
//!
//! GNU Mode supports:
//! - Standard GNU directives: `.word`, `.dword`, `.half`, `.byte`, `.float`, `.asciz`, `.string`, `.ascii`, `.zero`, `.space`, `.org`
//! - Ignoring non-code sections and metadata: `.text`, `.data`, `.globl`, `.global`, `.section`, `.align`, `.file`, `.ident`, etc.
//! - Canonical RISC-V pseudo-instructions: `li`, `la`, `mv`, `ret`, `j`, `jr`, `call`, `tail`, `nop`,
//!   `not`, `neg`, `negw`, `sext.w`, `seqz`, `snez`, `sltz`, `sgtz`, `beqz`, `bnez`, `blez`, `bgez`, `bltz`, `bgtz`,
//!   `bgt`, `ble`, `bgtu`, `bleu`, `fmv.s`, `fabs.s`, `fneg.s`, `fmv.d`, `fabs.d`, `fneg.d`.

/// Assembly dialect selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AssemblyDialect {
    #[default]
    Rvs,
    Gnu,
}

impl AssemblyDialect {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rvs => "RVS",
            Self::Gnu => "GNU",
        }
    }

    pub fn toggle(&self) -> Self {
        match self {
            Self::Rvs => Self::Gnu,
            Self::Gnu => Self::Rvs,
        }
    }
}

/// Directives that only exist in GNU assembly (`.org` is excluded because the
/// RVS dialect uses a bare `ORG`).
const GNU_ONLY_DIRECTIVES: &[&str] = &[
    ".text",
    ".data",
    ".bss",
    ".rodata",
    ".section",
    ".globl",
    ".global",
    ".asciz",
    ".string",
    ".ascii",
    ".word",
    ".dword",
    ".half",
    ".byte",
    ".float",
    ".double",
    ".zero",
    ".space",
    ".align",
    ".balign",
    ".p2align",
    ".file",
    ".ident",
    ".type",
    ".size",
    ".option",
    ".attribute",
];

/// Pseudo-instructions the RVS assembler does not know, so their presence
/// means the file was written for GNU mode.
const GNU_ONLY_PSEUDO: &[&str] = &[
    "li", "la", "mv", "call", "tail", "ret", "jr", "nop", "not", "neg", "negw", "sext.w", "seqz",
    "snez", "sltz", "sgtz", "beqz", "bnez", "blez", "bgez", "bltz", "bgtz", "bgt", "ble", "bgtu",
    "bleu", "j",
];

/// Guess the dialect of a source file: any GNU-only directive or
/// pseudo-instruction means GNU mode, otherwise the canonical RVS dialect.
pub fn detect_dialect(source: &str) -> AssemblyDialect {
    for line in source.lines() {
        let (_, _, body, _) = parse_line_parts(line);
        let mut words = body.split_whitespace();
        let Some(op) = words.next() else { continue };
        let lower = op.to_ascii_lowercase();
        if GNU_ONLY_DIRECTIVES.contains(&lower.as_str()) {
            return AssemblyDialect::Gnu;
        }
        // `jal`/`j` share the mnemonic across dialects; only the unambiguous
        // pseudo-instructions are trustable.
        if GNU_ONLY_PSEUDO.contains(&lower.as_str()) {
            return AssemblyDialect::Gnu;
        }
    }
    AssemblyDialect::Rvs
}

/// Splits a raw assembly line into:
/// - `indent`: leading whitespace
/// - `label`: optional label including colon (e.g. `main:`)
/// - `body`: instruction / directive with arguments
/// - `comment`: comment including comment character (e.g. `# ...`)
pub fn parse_line_parts(raw: &str) -> (String, String, String, String) {
    // Byte length, not char count: non-ASCII whitespace (U+00A0, U+2003, ...)
    // is multi-byte, and slicing at a char index would split a code point.
    let indent_len = raw.len() - raw.trim_start_matches(|c: char| c.is_whitespace()).len();
    let indent = raw[..indent_len].to_string();
    let trimmed = raw[indent_len..].trim_end();

    if trimmed.is_empty() {
        return (indent, String::new(), String::new(), String::new());
    }

    // Extract comment if present
    let mut comment_start = None;
    let mut in_quote = false;
    let chars: Vec<(usize, char)> = trimmed.char_indices().collect();
    for i in 0..chars.len() {
        let (byte_idx, ch) = chars[i];
        if ch == '"' {
            in_quote = !in_quote;
        } else if !in_quote
            && (ch == '#'
                || ch == ';'
                || (ch == '/' && i + 1 < chars.len() && chars[i + 1].1 == '/'))
        {
            comment_start = Some(byte_idx);
            break;
        }
    }

    let (non_comment, comment) = match comment_start {
        Some(idx) => (trimmed[..idx].trim_end(), trimmed[idx..].to_string()),
        None => (trimmed, String::new()),
    };

    if non_comment.is_empty() {
        return (indent, String::new(), String::new(), comment);
    }

    // Extract label if present
    let mut label = String::new();
    let body;
    if let Some(col_idx) = non_comment.find(':') {
        label = non_comment[..=col_idx].trim().to_string();
        body = non_comment[col_idx + 1..].trim().to_string();
    } else {
        body = non_comment.trim().to_string();
    }

    (indent, label, body, comment)
}

/// Splits an instruction or directive body into `(opcode, args)`.
pub fn split_opcode_args(body: &str) -> (&str, &str) {
    let trimmed = body.trim();
    if let Some(idx) = trimmed.find(char::is_whitespace) {
        (&trimmed[..idx], trimmed[idx..].trim())
    } else {
        (trimmed, "")
    }
}

/// Parses comma-separated arguments, trimming each.
pub fn parse_args(args_str: &str) -> Vec<String> {
    if args_str.trim().is_empty() {
        return Vec::new();
    }
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for ch in args_str.chars() {
        if ch == '"' {
            in_quote = !in_quote;
            cur.push(ch);
        } else if ch == ',' && !in_quote {
            args.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() || !args.is_empty() {
        args.push(cur.trim().to_string());
    }
    args
}

/// Transpiles GNU assembly source into canonical RVS dialect assembly.
pub fn transpile_gnu_to_rvs(source: &str) -> String {
    let mut out = String::with_capacity(source.len() + 128);

    for line in source.lines() {
        let (indent, label, body, comment) = parse_line_parts(line);

        if body.is_empty() {
            // Line is blank, comment-only, or bare label
            let mut l = indent;
            if !label.is_empty() {
                l.push_str(&label);
                if !comment.is_empty() {
                    l.push(' ');
                }
            }
            l.push_str(&comment);
            out.push_str(&l);
            out.push('\n');
            continue;
        }

        let (op, args_str) = split_opcode_args(&body);
        let op_lower = op.to_lowercase();
        let args = parse_args(args_str);

        // Check GNU directives
        if op.starts_with('.') {
            match op_lower.as_str() {
                // Stripped / ignored declarations
                ".text" | ".data" | ".rodata" | ".bss" => {
                    let mut l = indent;
                    if !label.is_empty() {
                        l.push_str(&label);
                        l.push(' ');
                    }
                    l.push_str(&format!("# {}", op));
                    if !args_str.is_empty() {
                        l.push(' ');
                        l.push_str(args_str);
                    }
                    if !comment.is_empty() {
                        l.push(' ');
                        l.push_str(&comment);
                    }
                    out.push_str(&l);
                    out.push('\n');
                    continue;
                }
                ".globl" | ".global" | ".file" | ".ident" | ".type" | ".size" | ".section"
                | ".align" | ".p2align" | ".balign" | ".attribute" | ".option" => {
                    let mut l = indent;
                    if !label.is_empty() {
                        l.push_str(&label);
                        l.push(' ');
                    }
                    l.push_str(&format!("# {} {}", op, args_str));
                    if !comment.is_empty() {
                        l.push(' ');
                        l.push_str(&comment);
                    }
                    out.push_str(&l);
                    out.push('\n');
                    continue;
                }
                // Memory definitions
                ".word" => {
                    emit_line(&mut out, &indent, &label, "DW", args_str, &comment);
                    continue;
                }
                ".dword" | ".2byte" => {
                    emit_line(&mut out, &indent, &label, "DD", args_str, &comment);
                    continue;
                }
                ".half" => {
                    emit_line(&mut out, &indent, &label, "DH", args_str, &comment);
                    continue;
                }
                ".byte" => {
                    emit_line(&mut out, &indent, &label, "DB", args_str, &comment);
                    continue;
                }
                ".float" => {
                    emit_line(&mut out, &indent, &label, "DF", args_str, &comment);
                    continue;
                }
                ".double" => {
                    emit_line(&mut out, &indent, &label, "DD", args_str, &comment);
                    continue;
                }
                ".asciz" | ".string" | ".ascii" => {
                    emit_line(&mut out, &indent, &label, "DC", args_str, &comment);
                    continue;
                }
                ".zero" | ".space" => {
                    let n: i64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(8);
                    let words = (n + 7) / 8;
                    emit_line(
                        &mut out,
                        &indent,
                        &label,
                        "DM",
                        &words.to_string(),
                        &comment,
                    );
                    continue;
                }
                ".org" => {
                    emit_line(&mut out, &indent, &label, "ORG", args_str, &comment);
                    continue;
                }
                _ => {}
            }
        }

        // Pseudo-instructions expansion
        match op_lower.as_str() {
            "nop" => {
                emit_line(&mut out, &indent, &label, "addi", "x0, x0, 0", &comment);
            }
            "mv" if args.len() == 2 => {
                let exp = format!("{}, {}, 0", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "addi", &exp, &comment);
            }
            "not" if args.len() == 2 => {
                let exp = format!("{}, {}, -1", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "xori", &exp, &comment);
            }
            "neg" if args.len() == 2 => {
                let exp = format!("{}, x0, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "sub", &exp, &comment);
            }
            "negw" if args.len() == 2 => {
                let exp = format!("{}, x0, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "subw", &exp, &comment);
            }
            "sext.w" if args.len() == 2 => {
                let exp = format!("{}, {}, 0", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "addiw", &exp, &comment);
            }
            "seqz" if args.len() == 2 => {
                let exp = format!("{}, {}, 1", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "sltiu", &exp, &comment);
            }
            "snez" if args.len() == 2 => {
                let exp = format!("{}, x0, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "sltu", &exp, &comment);
            }
            "sltz" if args.len() == 2 => {
                let exp = format!("{}, {}, x0", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "slt", &exp, &comment);
            }
            "sgtz" if args.len() == 2 => {
                let exp = format!("{}, x0, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "slt", &exp, &comment);
            }
            "beqz" if args.len() == 2 => {
                let exp = format!("{}, x0, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "beq", &exp, &comment);
            }
            "bnez" if args.len() == 2 => {
                let exp = format!("{}, x0, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "bne", &exp, &comment);
            }
            "blez" if args.len() == 2 => {
                let exp = format!("x0, {}, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "bge", &exp, &comment);
            }
            "bgez" if args.len() == 2 => {
                let exp = format!("{}, x0, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "bge", &exp, &comment);
            }
            "bltz" if args.len() == 2 => {
                let exp = format!("{}, x0, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "blt", &exp, &comment);
            }
            "bgtz" if args.len() == 2 => {
                let exp = format!("x0, {}, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "blt", &exp, &comment);
            }
            "bgt" if args.len() == 3 => {
                let exp = format!("{}, {}, {}", args[1], args[0], args[2]);
                emit_line(&mut out, &indent, &label, "blt", &exp, &comment);
            }
            "ble" if args.len() == 3 => {
                let exp = format!("{}, {}, {}", args[1], args[0], args[2]);
                emit_line(&mut out, &indent, &label, "bge", &exp, &comment);
            }
            "bgtu" if args.len() == 3 => {
                let exp = format!("{}, {}, {}", args[1], args[0], args[2]);
                emit_line(&mut out, &indent, &label, "bltu", &exp, &comment);
            }
            "bleu" if args.len() == 3 => {
                let exp = format!("{}, {}, {}", args[1], args[0], args[2]);
                emit_line(&mut out, &indent, &label, "bgeu", &exp, &comment);
            }
            "j" if args.len() == 1 => {
                let exp = format!("x0, {}", args[0]);
                emit_line(&mut out, &indent, &label, "jal", &exp, &comment);
            }
            "jr" if args.len() == 1 => {
                let exp = format!("x0, {}, 0", args[0]);
                emit_line(&mut out, &indent, &label, "jalr", &exp, &comment);
            }
            "ret" => {
                emit_line(&mut out, &indent, &label, "jalr", "x0, ra, 0", &comment);
            }
            "call" if args.len() == 1 => {
                let exp = format!("ra, {}", args[0]);
                emit_line(&mut out, &indent, &label, "jal", &exp, &comment);
            }
            "tail" if args.len() == 1 => {
                let exp = format!("x0, {}", args[0]);
                emit_line(&mut out, &indent, &label, "jal", &exp, &comment);
            }
            "li" if args.len() == 2 => {
                let rd = &args[0];
                let imm_str = &args[1];
                let parsed_val = parse_int_imm(imm_str);

                match parsed_val {
                    Some(val) if (-2048..=2047).contains(&val) => {
                        let exp = format!("{}, x0, {}", rd, val);
                        emit_line(&mut out, &indent, &label, "addi", &exp, &comment);
                    }
                    Some(val) if (-2147483648..=2147483647).contains(&val) => {
                        // 32-bit immediate load via lui + addiw
                        let lower = ((val & 0xfff) as i16) as i64;
                        let mut upper = (val - lower) >> 12;
                        if upper > 0xfffff {
                            upper &= 0xfffff;
                        }
                        let exp1 = format!("{}, 0x{:x}", rd, upper);
                        let exp2 = format!("{}, {}, {}", rd, rd, lower);
                        emit_line(&mut out, &indent, &label, "lui", &exp1, "");
                        emit_line(&mut out, &indent, "", "addiw", &exp2, &comment);
                    }
                    _ => {
                        // Symbolic or standard immediate
                        let exp = format!("{}, x0, {}", rd, imm_str);
                        emit_line(&mut out, &indent, &label, "addi", &exp, &comment);
                    }
                }
            }
            "la" if args.len() == 2 => {
                let exp = format!("{}, x0, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "addi", &exp, &comment);
            }
            "fmv.s" if args.len() == 2 => {
                let exp = format!("{}, {}, {}", args[0], args[1], args[1]);
                emit_line(&mut out, &indent, &label, "fsgnj.s", &exp, &comment);
            }
            "fabs.s" if args.len() == 2 => {
                let exp = format!("{}, {}, {}", args[0], args[1], args[1]);
                emit_line(&mut out, &indent, &label, "fsgnjx.s", &exp, &comment);
            }
            "fneg.s" if args.len() == 2 => {
                let exp = format!("{}, {}, {}", args[0], args[1], args[1]);
                emit_line(&mut out, &indent, &label, "fsgnjn.s", &exp, &comment);
            }
            "fmv.d" if args.len() == 2 => {
                let exp = format!("{}, {}, {}", args[0], args[1], args[1]);
                emit_line(&mut out, &indent, &label, "fsgnj.d", &exp, &comment);
            }
            "fabs.d" if args.len() == 2 => {
                let exp = format!("{}, {}, {}", args[0], args[1], args[1]);
                emit_line(&mut out, &indent, &label, "fsgnjx.d", &exp, &comment);
            }
            "fneg.d" if args.len() == 2 => {
                let exp = format!("{}, {}, {}", args[0], args[1], args[1]);
                emit_line(&mut out, &indent, &label, "fsgnjn.d", &exp, &comment);
            }
            _ => {
                // Pass instruction through verbatim
                emit_line(&mut out, &indent, &label, op, args_str, &comment);
            }
        }
    }

    out
}

/// Transpiles canonical RVS dialect assembly into idiomatic GNU RISC-V assembly.
pub fn transpile_rvs_to_gnu(source: &str) -> String {
    let mut out = String::with_capacity(source.len() + 128);

    for line in source.lines() {
        let (indent, label, body, comment) = parse_line_parts(line);

        if body.is_empty() {
            let mut l = indent;
            if !label.is_empty() {
                l.push_str(&label);
                if !comment.is_empty() {
                    l.push(' ');
                }
            }
            l.push_str(&comment);
            out.push_str(&l);
            out.push('\n');
            continue;
        }

        let (op, args_str) = split_opcode_args(&body);
        let op_upper = op.to_uppercase();
        let op_lower = op.to_lowercase();
        let args = parse_args(args_str);

        // Directives
        match op_upper.as_str() {
            "DD" => {
                emit_line(&mut out, &indent, &label, ".dword", args_str, &comment);
                continue;
            }
            "DW" => {
                emit_line(&mut out, &indent, &label, ".word", args_str, &comment);
                continue;
            }
            "DH" => {
                emit_line(&mut out, &indent, &label, ".half", args_str, &comment);
                continue;
            }
            "DB" => {
                emit_line(&mut out, &indent, &label, ".byte", args_str, &comment);
                continue;
            }
            "DF" => {
                emit_line(&mut out, &indent, &label, ".float", args_str, &comment);
                continue;
            }
            "DC" => {
                emit_line(&mut out, &indent, &label, ".asciz", args_str, &comment);
                continue;
            }
            "DM" => {
                let words: i64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(1);
                emit_line(
                    &mut out,
                    &indent,
                    &label,
                    ".zero",
                    &(words * 8).to_string(),
                    &comment,
                );
                continue;
            }
            "ORG" => {
                emit_line(&mut out, &indent, &label, ".org", args_str, &comment);
                continue;
            }
            _ => {}
        }

        // Canonical instruction patterns compressed to pseudo-ops
        match op_lower.as_str() {
            "addi" if args.len() == 3 => {
                let rd = &args[0];
                let rs = &args[1];
                let imm = &args[2];

                if is_zero_reg(rd) && is_zero_reg(rs) && imm == "0" {
                    emit_line(&mut out, &indent, &label, "nop", "", &comment);
                } else if !is_zero_reg(rd) && is_zero_reg(rs) {
                    let exp = format!("{}, {}", rd, imm);
                    emit_line(&mut out, &indent, &label, "li", &exp, &comment);
                } else if !is_zero_reg(rd) && !is_zero_reg(rs) && imm == "0" {
                    let exp = format!("{}, {}", rd, rs);
                    emit_line(&mut out, &indent, &label, "mv", &exp, &comment);
                } else {
                    emit_line(&mut out, &indent, &label, op, args_str, &comment);
                }
            }
            "addiw" if args.len() == 3 => {
                let rd = &args[0];
                let rs = &args[1];
                let imm = &args[2];
                if !is_zero_reg(rd) && !is_zero_reg(rs) && imm == "0" {
                    let exp = format!("{}, {}", rd, rs);
                    emit_line(&mut out, &indent, &label, "sext.w", &exp, &comment);
                } else {
                    emit_line(&mut out, &indent, &label, op, args_str, &comment);
                }
            }
            "jalr" if args.len() == 3 => {
                let rd = &args[0];
                let rs = &args[1];
                let imm = &args[2];
                if is_zero_reg(rd) && is_ra_reg(rs) && imm == "0" {
                    emit_line(&mut out, &indent, &label, "ret", "", &comment);
                } else if is_zero_reg(rd) && imm == "0" {
                    emit_line(&mut out, &indent, &label, "jr", rs, &comment);
                } else {
                    emit_line(&mut out, &indent, &label, op, args_str, &comment);
                }
            }
            "jal" if args.len() == 2 => {
                let rd = &args[0];
                let target = &args[1];
                if is_zero_reg(rd) {
                    emit_line(&mut out, &indent, &label, "j", target, &comment);
                } else if is_ra_reg(rd) {
                    emit_line(&mut out, &indent, &label, "call", target, &comment);
                } else {
                    emit_line(&mut out, &indent, &label, op, args_str, &comment);
                }
            }
            "beq" if args.len() == 3 && is_zero_reg(&args[1]) => {
                let exp = format!("{}, {}", args[0], args[2]);
                emit_line(&mut out, &indent, &label, "beqz", &exp, &comment);
            }
            "bne" if args.len() == 3 && is_zero_reg(&args[1]) => {
                let exp = format!("{}, {}", args[0], args[2]);
                emit_line(&mut out, &indent, &label, "bnez", &exp, &comment);
            }
            "bge" if args.len() == 3 => {
                if is_zero_reg(&args[0]) {
                    let exp = format!("{}, {}", args[1], args[2]);
                    emit_line(&mut out, &indent, &label, "blez", &exp, &comment);
                } else if is_zero_reg(&args[1]) {
                    let exp = format!("{}, {}", args[0], args[2]);
                    emit_line(&mut out, &indent, &label, "bgez", &exp, &comment);
                } else {
                    emit_line(&mut out, &indent, &label, op, args_str, &comment);
                }
            }
            "blt" if args.len() == 3 => {
                if is_zero_reg(&args[0]) {
                    let exp = format!("{}, {}", args[1], args[2]);
                    emit_line(&mut out, &indent, &label, "bgtz", &exp, &comment);
                } else if is_zero_reg(&args[1]) {
                    let exp = format!("{}, {}", args[0], args[2]);
                    emit_line(&mut out, &indent, &label, "bltz", &exp, &comment);
                } else {
                    emit_line(&mut out, &indent, &label, op, args_str, &comment);
                }
            }
            "xori" if args.len() == 3 && (args[2] == "-1" || args[2] == "0xffffffffffffffff") => {
                let exp = format!("{}, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "not", &exp, &comment);
            }
            "sub" if args.len() == 3 && is_zero_reg(&args[1]) => {
                let exp = format!("{}, {}", args[0], args[2]);
                emit_line(&mut out, &indent, &label, "neg", &exp, &comment);
            }
            "subw" if args.len() == 3 && is_zero_reg(&args[1]) => {
                let exp = format!("{}, {}", args[0], args[2]);
                emit_line(&mut out, &indent, &label, "negw", &exp, &comment);
            }
            "sltiu" if args.len() == 3 && args[2] == "1" => {
                let exp = format!("{}, {}", args[0], args[1]);
                emit_line(&mut out, &indent, &label, "seqz", &exp, &comment);
            }
            "sltu" if args.len() == 3 && is_zero_reg(&args[1]) => {
                let exp = format!("{}, {}", args[0], args[2]);
                emit_line(&mut out, &indent, &label, "snez", &exp, &comment);
            }
            "slt" if args.len() == 3 => {
                if is_zero_reg(&args[2]) {
                    let exp = format!("{}, {}", args[0], args[1]);
                    emit_line(&mut out, &indent, &label, "sltz", &exp, &comment);
                } else if is_zero_reg(&args[1]) {
                    let exp = format!("{}, {}", args[0], args[2]);
                    emit_line(&mut out, &indent, &label, "sgtz", &exp, &comment);
                } else {
                    emit_line(&mut out, &indent, &label, op, args_str, &comment);
                }
            }
            _ => {
                emit_line(&mut out, &indent, &label, op, args_str, &comment);
            }
        }
    }

    out
}

fn emit_line(out: &mut String, indent: &str, label: &str, op: &str, args: &str, comment: &str) {
    let mut l = indent.to_string();
    if !label.is_empty() {
        l.push_str(label);
        l.push(' ');
    }
    l.push_str(op);
    if !args.is_empty() {
        l.push(' ');
        l.push_str(args);
    }
    if !comment.is_empty() {
        if !op.is_empty() || !args.is_empty() {
            l.push('\t');
        }
        l.push_str(comment);
    }
    out.push_str(&l);
    out.push('\n');
}

fn is_zero_reg(reg: &str) -> bool {
    let r = reg.trim().to_lowercase();
    r == "x0" || r == "zero"
}

fn is_ra_reg(reg: &str) -> bool {
    let r = reg.trim().to_lowercase();
    r == "x1" || r == "ra"
}

fn parse_int_imm(s: &str) -> Option<i64> {
    let trimmed = s.trim();
    if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        i64::from_str_radix(&trimmed[2..], 16).ok()
    } else if trimmed.starts_with("0b") || trimmed.starts_with("0B") {
        i64::from_str_radix(&trimmed[2..], 2).ok()
    } else if trimmed.starts_with("0o") || trimmed.starts_with("0O") {
        i64::from_str_radix(&trimmed[2..], 8).ok()
    } else {
        trimmed.parse::<i64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transpile_gnu_to_rvs() {
        let gnu = r#"
.globl main
.text
main:
    li   a0, 42
    mv   a1, a0
    nop
    ret
"#;
        let rvs = transpile_gnu_to_rvs(gnu);
        assert!(rvs.contains("addi a0, x0, 42"));
        assert!(rvs.contains("addi a1, a0, 0"));
        assert!(rvs.contains("addi x0, x0, 0"));
        assert!(rvs.contains("jalr x0, ra, 0"));
    }

    #[test]
    fn test_transpile_rvs_to_gnu() {
        let rvs = r#"
main:
    addi a0, x0, 42
    addi a1, a0, 0
    addi x0, x0, 0
    jalr x0, ra, 0
"#;
        let gnu = transpile_rvs_to_gnu(rvs);
        assert!(gnu.contains("li a0, 42"));
        assert!(gnu.contains("mv a1, a0"));
        assert!(gnu.contains("nop"));
        assert!(gnu.contains("ret"));
    }

    #[test]
    fn test_branch_pseudos() {
        let gnu = "beqz t0, label\nbgt t0, t1, label\nj label\ncall my_func";
        let rvs = transpile_gnu_to_rvs(gnu);
        assert!(rvs.contains("beq t0, x0, label"));
        assert!(rvs.contains("blt t1, t0, label"));
        assert!(rvs.contains("jal x0, label"));
        assert!(rvs.contains("jal ra, my_func"));
    }
}
