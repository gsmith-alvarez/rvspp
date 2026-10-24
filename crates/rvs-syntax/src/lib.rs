//! RISC-V and RVS assembly syntax highlighting and token styling for the TUI.
//!
//! CRITICAL INVARIANT: `highlight_source_line(line: &str) -> Vec<Span<'static>>`
//! MUST preserve every character of `line` in the exact same order and character count
//! (`spans.iter().map(|s| s.content.chars().count()).sum::<usize>() == line.chars().count()`).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

pub const STYLE_FMT: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
pub const STYLE_OP: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
pub const STYLE_ARG1: Style = Style::new().fg(Color::LightBlue).add_modifier(Modifier::BOLD);
pub const STYLE_ARG2: Style = Style::new().fg(Color::LightMagenta).add_modifier(Modifier::BOLD);
pub const STYLE_ARG3_IMM: Style = Style::new().fg(Color::LightGreen).add_modifier(Modifier::BOLD);
pub const STYLE_PUNCT: Style = Style::new().fg(Color::DarkGray);
pub const STYLE_COMMENT: Style = Style::new().fg(Color::DarkGray).add_modifier(Modifier::ITALIC);
pub const STYLE_LABEL: Style = Style::new().fg(Color::LightMagenta).add_modifier(Modifier::BOLD);

pub fn is_zero_hex(hex: &str) -> bool {
    let t = hex.trim();
    t == "0x0000000000000000" || t == "0x0" || t == "0x00000000"
}

/// Splits a line of text into `(leading_ws, opcode, rest)`.
fn split_leading_opcode(text: &str) -> (&str, &str, &str) {
    let ws_len = text.find(|c: char| !c.is_whitespace()).unwrap_or(text.len());
    let (ws, after_ws) = text.split_at(ws_len);
    let op_len = after_ws
        .find(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')')
        .unwrap_or(after_ws.len());
    let (op, rest) = after_ws.split_at(op_len);
    (ws, op, rest)
}

/// Highlight formatted instruction text (e.g. `addi    x1 x0 0x005` or `addi    x1,x0,5`).
/// First word = opcode in bold `Color::Cyan`, registers in `Color::LightBlue` (or `Color::Blue` for zero),
/// immediates/hex in `Color::LightGreen`, punctuation in `Color::DarkGray`,
/// preserving exact character length of `text`.
pub fn highlight_inst_operands(text: &str) -> Vec<Span<'static>> {
    if text.is_empty() {
        return Vec::new();
    }
    let (ws, op, rest) = split_leading_opcode(text);
    let mut spans = Vec::new();
    if !ws.is_empty() {
        spans.push(Span::raw(ws.to_string()));
    }
    if !op.is_empty() {
        spans.push(Span::styled(op.to_string(), STYLE_OP));
    }
    if !rest.is_empty() {
        spans.append(&mut highlight_operands_str(rest));
    }
    spans
}

/// Scans for the first unquoted comment delimiter (`#`, `;`, `//`),
/// returning `(code, comment)` slices of `line`.
fn split_code_and_comment(line: &str) -> (&str, &str) {
    let mut in_str = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut prev_slash = None;

    for (idx, c) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if in_str {
            if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
        } else if in_char {
            if c == '\\' {
                escaped = true;
            } else if c == '\'' {
                in_char = false;
            }
        } else {
            if let Some(slash_idx) = prev_slash.take() {
                if c == '/' {
                    return (&line[..slash_idx], &line[slash_idx..]);
                }
            }

            if c == '"' {
                in_str = true;
            } else if c == '\'' {
                in_char = true;
            } else if c == '#' || c == ';' {
                return (&line[..idx], &line[idx..]);
            } else if c == '/' {
                prev_slash = Some(idx);
            }
        }
    }

    (line, "")
}

#[derive(Clone, Copy)]
enum HighlightMode<'a> {
    Standard,
    Correlated(Option<&'a str>),
}

/// Highlight a single line of assembly code (excluding any trailing comment).
fn highlight_code_line(code: &str, mode: HighlightMode) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = code;

    // 1. Optional leading whitespace
    let ws_len = rest.find(|c: char| !c.is_whitespace()).unwrap_or(rest.len());
    if ws_len > 0 {
        spans.push(Span::raw(rest[..ws_len].to_string()));
        rest = &rest[ws_len..];
    }
    if rest.is_empty() {
        return spans;
    }

    // 2. Optional label `ident:`
    let ident_len = rest.find(|c: char| !is_ident_char(c)).unwrap_or(rest.len());
    if ident_len > 0 && rest[ident_len..].starts_with(':') {
        let label_end = ident_len + 1; // include ':'
        spans.push(Span::styled(rest[..label_end].to_string(), STYLE_LABEL));
        rest = &rest[label_end..];

        // Whitespace after label
        let ws2_len = rest.find(|c: char| !c.is_whitespace()).unwrap_or(rest.len());
        if ws2_len > 0 {
            spans.push(Span::raw(rest[..ws2_len].to_string()));
            rest = &rest[ws2_len..];
        }
    }
    if rest.is_empty() {
        return spans;
    }

    // 3. Opcode or Directive
    let op_len = rest.find(|c: char| !is_ident_char(c) && c != '.').unwrap_or(rest.len());
    if op_len > 0 {
        let tok = &rest[..op_len];
        let style = if is_directive(tok) {
            STYLE_FMT
        } else if is_instruction(tok) {
            STYLE_OP
        } else {
            match mode {
                HighlightMode::Standard => Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                HighlightMode::Correlated(_) => STYLE_OP,
            }
        };
        spans.push(Span::styled(tok.to_string(), style));
        rest = &rest[op_len..];
    }

    // 4. Operands
    if !rest.is_empty() {
        let mut op_spans = match mode {
            HighlightMode::Standard => highlight_operands_str(rest),
            HighlightMode::Correlated(fmt) => highlight_operand_tokens_correlated(rest, fmt),
        };
        spans.append(&mut op_spans);
    }

    spans
}

/// Highlight a single line of source code preserving exact characters.
pub fn highlight_source_line(line: &str) -> Vec<Span<'static>> {
    if line.is_empty() {
        return Vec::new();
    }
    if line.contains("ERROR:") {
        return vec![Span::styled(
            line.to_string(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )];
    }
    let (code, comment) = split_code_and_comment(line);
    let mut spans = highlight_code_line(code, HighlightMode::Standard);
    if !comment.is_empty() {
        spans.push(Span::styled(comment.to_string(), STYLE_COMMENT));
    }
    spans
}

fn highlight_operands_str(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = text;

    while !rest.is_empty() {
        let c = rest.chars().next().unwrap();
        let c_len = c.len_utf8();

        // Whitespace
        if c.is_whitespace() {
            let ws_len = rest.find(|ch: char| !ch.is_whitespace()).unwrap_or(rest.len());
            spans.push(Span::raw(rest[..ws_len].to_string()));
            rest = &rest[ws_len..];
            continue;
        }

        // String literal "..."
        if c == '"' {
            let mut end = c_len;
            let mut chars = rest[c_len..].char_indices();
            let mut closed = false;
            while let Some((idx, ch)) = chars.next() {
                if ch == '\\' {
                    chars.next(); // skip escaped char
                } else if ch == '"' {
                    end = c_len + idx + 1;
                    closed = true;
                    break;
                }
            }
            if !closed {
                end = rest.len();
            }
            spans.push(Span::styled(rest[..end].to_string(), Style::default().fg(Color::LightYellow)));
            rest = &rest[end..];
            continue;
        }

        // Char literal 'c'
        if c == '\'' {
            let mut end = c_len;
            let mut chars = rest[c_len..].char_indices();
            let mut closed = false;
            while let Some((idx, ch)) = chars.next() {
                if ch == '\\' {
                    chars.next(); // skip escaped char
                } else if ch == '\'' {
                    end = c_len + idx + 1;
                    closed = true;
                    break;
                }
            }
            if !closed {
                end = rest.len();
            }
            spans.push(Span::styled(rest[..end].to_string(), Style::default().fg(Color::LightYellow)));
            rest = &rest[end..];
            continue;
        }

        // Punctuation
        if c == ',' || c == '(' || c == ')' || c == '{' || c == '}' {
            spans.push(Span::styled(c.to_string(), STYLE_PUNCT));
            rest = &rest[c_len..];
            continue;
        }

        // Shift operators << or >>
        if rest.starts_with("<<") || rest.starts_with(">>") {
            spans.push(Span::styled(rest[..2].to_string(), STYLE_PUNCT));
            rest = &rest[2..];
            continue;
        }

        // Math operators (+, -, *, /)
        // Check if '-' is start of a negative number: e.g. preceded by whitespace or comma or '('
        if c == '-' && rest[c_len..].chars().next().map(|ch| ch.is_ascii_digit()).unwrap_or(false) {
            let num_len = rest[c_len..]
                .find(|ch: char| !is_ident_char(ch))
                .map(|idx| c_len + idx)
                .unwrap_or(rest.len());
            spans.push(Span::styled(rest[..num_len].to_string(), Style::default().fg(Color::LightGreen)));
            rest = &rest[num_len..];
            continue;
        }

        if c == '+' || c == '-' || c == '*' || c == '/' || c == '%' || c == '&' || c == '|' || c == '^' || c == '~' {
            spans.push(Span::styled(c.to_string(), STYLE_PUNCT));
            rest = &rest[c_len..];
            continue;
        }

        // Identifiers, hex literals, registers, numbers
        if is_ident_char(c) || c == '$' {
            let tok_len = rest
                .find(|ch: char| !is_ident_char(ch) && ch != '$')
                .unwrap_or(rest.len());
            let tok = &rest[..tok_len];

            // Check if register
            if is_register(tok) {
                let lower = tok.to_ascii_lowercase();
                let style = if lower == "x0" || lower == "zero" {
                    Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::LightBlue)
                };
                spans.push(Span::styled(tok.to_string(), style));
            } else if is_numeric(tok) {
                spans.push(Span::styled(tok.to_string(), Style::default().fg(Color::LightGreen)));
            } else {
                // Label reference or symbol name
                spans.push(Span::styled(tok.to_string(), Style::default().fg(Color::LightMagenta)));
            }
            rest = &rest[tok_len..];
            continue;
        }

        // Fallback for any other character
        spans.push(Span::raw(c.to_string()));
        rest = &rest[c_len..];
    }

    spans
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_numeric(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if s.starts_with("0x") || s.starts_with("0X") {
        return s.len() > 2 && s[2..].chars().all(|c| c.is_ascii_hexdigit());
    }
    if s.starts_with("0b") || s.starts_with("0B") {
        return s.len() > 2 && s[2..].chars().all(|c| c == '0' || c == '1');
    }
    s.chars().all(|c| c.is_ascii_digit())
}

pub fn is_register(tok: &str) -> bool {
    rvs_regs::parse_reg(tok).is_some()
}

fn is_directive(tok: &str) -> bool {
    let upper = tok.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "ORG"
            | "EQU"
            | "DD"
            | "DW"
            | "DH"
            | "DB"
            | "DC"
            | "DF"
            | "DM"
            | ".TEXT"
            | ".DATA"
            | ".GLOBL"
            | ".GLOBAL"
            | ".WORD"
            | ".DWORD"
            | ".BYTE"
            | ".HALF"
            | ".ASCIZ"
            | ".STRING"
            | ".SPACE"
            | ".ZERO"
            | ".ALIGN"
            | ".BSS"
            | ".RODATA"
    )
}

fn is_instruction(tok: &str) -> bool {
    if rvs_asm_syntax::INSTR_TABLE.iter().any(|d| d.op.eq_ignore_ascii_case(tok)) {
        return true;
    }
    let lower = tok.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "nop"
            | "li"
            | "la"
            | "mv"
            | "not"
            | "neg"
            | "negw"
            | "sext.w"
            | "seqz"
            | "snez"
            | "sltz"
            | "sgtz"
            | "beqz"
            | "bnez"
            | "blez"
            | "bgez"
            | "bltz"
            | "bgtz"
            | "bgt"
            | "ble"
            | "bgtu"
            | "bleu"
            | "j"
            | "jr"
            | "ret"
            | "call"
            | "tail"
            | "fmv.d"
            | "fneg.d"
            | "fabs.d"
    )
}

/// Highlight the binary code column (`enc.bin`) correlated with the instruction bitfield roles.
pub fn highlight_bin_correlated(bin_padded: &str) -> Vec<Span<'static>> {
    if bin_padded.is_empty() {
        return Vec::new();
    }
    if bin_padded.contains("ERROR") {
        return vec![Span::styled(
            bin_padded.to_string(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )];
    }

    let mut tokens: Vec<(usize, usize, &str)> = Vec::new();
    let mut start = None;
    for (idx, c) in bin_padded.char_indices() {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                tokens.push((s, idx, &bin_padded[s..idx]));
            }
        } else if start.is_none() {
            start = Some(idx);
        }
    }
    if let Some(s) = start {
        tokens.push((s, bin_padded.len(), &bin_padded[s..]));
    }

    if tokens.is_empty() {
        return vec![Span::raw(bin_padded.to_string())];
    }

    let fmt_tag = tokens[0].2;
    let mut spans = Vec::new();
    let mut curr_idx = 0;

    for (tok_idx, (start, end, text)) in tokens.into_iter().enumerate() {
        if start > curr_idx {
            spans.push(Span::raw(bin_padded[curr_idx..start].to_string()));
        }
        curr_idx = end;

        let style = if tok_idx == 0 {
            STYLE_FMT
        } else {
            match fmt_tag {
                "R" | "FR" | "F2" => match tok_idx {
                    2 => STYLE_ARG3_IMM,
                    3 => STYLE_ARG2,
                    5 => STYLE_ARG1,
                    _ => STYLE_OP,
                },
                "I" | "FI" => match tok_idx {
                    1 => STYLE_ARG3_IMM,
                    2 => STYLE_ARG2,
                    4 => STYLE_ARG1,
                    _ => STYLE_OP,
                },
                "S" | "FS" => match tok_idx {
                    1 | 5 => STYLE_ARG3_IMM,
                    2 => STYLE_ARG1,
                    3 => STYLE_ARG2,
                    _ => STYLE_OP,
                },
                "SB" => match tok_idx {
                    1 | 5 => STYLE_ARG3_IMM,
                    2 => STYLE_ARG2,
                    3 => STYLE_ARG1,
                    _ => STYLE_OP,
                },
                "U" | "UJ" => match tok_idx {
                    1 => STYLE_ARG3_IMM,
                    2 => STYLE_ARG1,
                    _ => STYLE_OP,
                },
                _ => STYLE_OP,
            }
        };

        spans.push(Span::styled(text.to_string(), style));
    }

    if curr_idx < bin_padded.len() {
        spans.push(Span::raw(bin_padded[curr_idx..].to_string()));
    }

    spans
}

/// Highlights operand text (for HEX OPERANDS and INT OPERANDS) correlated with bitfield roles.
pub fn highlight_operands_correlated(text: &str, fmt: Option<&str>) -> Vec<Span<'static>> {
    if text.is_empty() {
        return Vec::new();
    }
    if text.contains("ERROR") {
        return vec![Span::styled(
            text.to_string(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )];
    }
    let (ws, op, rest) = split_leading_opcode(text);
    let mut spans = Vec::new();
    if !ws.is_empty() {
        spans.push(Span::raw(ws.to_string()));
    }
    if !op.is_empty() {
        spans.push(Span::styled(op.to_string(), STYLE_OP));
    }
    if !rest.is_empty() {
        spans.append(&mut highlight_operand_tokens_correlated(rest, fmt));
    }
    spans
}

/// Highlights the operand portion of an instruction string.
fn highlight_operand_tokens_correlated(text: &str, fmt: Option<&str>) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let has_paren = text.contains('(');
    let mut operand_idx = 0;
    let mut inside_paren = false;
    let mut rest = text;

    while !rest.is_empty() {
        let first_char = rest.chars().next().unwrap();
        if first_char.is_whitespace() {
            let ws_len = rest.find(|c: char| !c.is_whitespace()).unwrap_or(rest.len());
            spans.push(Span::raw(rest[..ws_len].to_string()));
            rest = &rest[ws_len..];
            continue;
        }
        if first_char == ',' {
            spans.push(Span::styled(",".to_string(), STYLE_PUNCT));
            rest = &rest[1..];
            continue;
        }
        if first_char == '(' {
            inside_paren = true;
            spans.push(Span::styled("(".to_string(), STYLE_PUNCT));
            rest = &rest[1..];
            continue;
        }
        if first_char == ')' {
            inside_paren = false;
            spans.push(Span::styled(")".to_string(), STYLE_PUNCT));
            rest = &rest[1..];
            continue;
        }

        let tok_len = rest
            .find(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')')
            .unwrap_or(rest.len());
        let tok = &rest[..tok_len];

        let style = if has_paren {
            if inside_paren {
                STYLE_ARG2 // base reg (rs1)
            } else if operand_idx == 0 {
                STYLE_ARG1 // rd or rs2
            } else {
                STYLE_ARG3_IMM // offset
            }
        } else if fmt == Some("U") || fmt == Some("UJ") {
            if operand_idx == 0 {
                STYLE_ARG1 // rd
            } else {
                STYLE_ARG3_IMM // imm
            }
        } else {
            match operand_idx {
                0 => STYLE_ARG1,
                1 => STYLE_ARG2,
                _ => STYLE_ARG3_IMM,
            }
        };

        operand_idx += 1;
        spans.push(Span::styled(tok.to_string(), style));
        rest = &rest[tok_len..];
    }

    spans
}

/// Highlights a source line in the `Listing` `TEXT SOURCE` column with correlated bitfield styles.
pub fn highlight_source_line_correlated(line: &str, fmt: Option<&str>) -> Vec<Span<'static>> {
    if line.is_empty() {
        return Vec::new();
    }
    if line.contains("ERROR:") {
        return vec![Span::styled(
            line.to_string(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )];
    }
    let (code, comment) = split_code_and_comment(line);
    let mut spans = highlight_code_line(code, HighlightMode::Correlated(fmt));
    if !comment.is_empty() {
        spans.push(Span::styled(comment.to_string(), STYLE_COMMENT));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_highlighting_preserves_exact_char_widths() {
        let assert_preserves = |spans: &[Span], original: &str| {
            let reconstructed: String = spans.iter().map(|s| s.content.as_ref()).collect();
            assert_eq!(reconstructed, original, "Mismatch reconstructing {:?}", original);
            let char_len: usize = spans.iter().map(|s| s.content.chars().count()).sum();
            assert_eq!(char_len, original.chars().count(), "Mismatch char count for {:?}", original);
        };

        // Edge cases covering escaped quotes, comments (#, ;, //) inside and outside strings,
        // labels, directives, imm(rs1) operands, ERROR: lines, tabs, and empty/whitespace lines.
        let source_cases = [
            "START:  addi x1, x0, 5    # initialize x1",
            "loop:   beq  x1, zero, done ; exit loop",
            "        DD   0x123456789abcdef0 // data",
            "        ORG  0x0000",
            "        EQU  MAX, 100",
            "msg:    DC   \"Hello, \\\"World\\\"! # not comment\" // trailing comment",
            "quoted: DC   ' ; still not a comment ' ; actual comment",
            "        sd   x3, -16(sp)",
            "        addi x2, x1, -12",
            "\t\taddi\tx3,\tx0,\t0x10",
            "ERROR: unexpected token at line 4",
            "",
            "   ",
        ];
        for line in source_cases {
            assert_preserves(&highlight_source_line(line), line);
        }

        let operand_cases = [
            "addi    x1 x0 0x005",
            "addi    x1,x0,5",
            "ld      x2, 8(x3)",
            "sd      x3, -16(sp)",
            "fadd.d  f1, f2, f3",
            "ebreak",
            "",
        ];
        for op in operand_cases {
            assert_preserves(&highlight_inst_operands(op), op);
        }

        let bin_correlated_cases = [
            "R  0000000 00010 00001 000 00011 0110011",
            "I  000000000101  00000 000 00001 0010011",
            "S  0000000 00011 00000 011 01000 0100011",
            "SB 0000000 00011 00001 000 00000 1100011",
            "U  00000000000000000001 00001 0110111",
            "UJ 00000000000000000000 00001 1101111",
            "ERROR: invalid opcode",
            "",
            "   ",
        ];
        for bin in bin_correlated_cases {
            assert_preserves(&highlight_bin_correlated(bin), bin);
        }

        let operands_correlated_cases = [
            ("addi    x1 x0 0x005    ", Some("I")),
            ("addi    x1,x0,5        ", Some("I")),
            ("ld      x1,8(x0)       ", Some("I")),
            ("sd      x3,-16(sp)     ", Some("S")),
            ("beq     x1,x3,label    ", Some("SB")),
            ("lui     x1,0x1         ", Some("U")),
            ("jal     x1,0x0         ", Some("UJ")),
            ("ERROR: unknown instruction", None),
            ("", Some("I")),
        ];
        for (ops, fmt) in operands_correlated_cases {
            assert_preserves(&highlight_operands_correlated(ops, fmt), ops);
        }

        let source_correlated_cases = [
            ("START:  addi x1, x0, 5    # init with \"quotes\"", Some("I")),
            ("        add  x3, x1, x2   // comment", Some("R")),
            ("        sd   x3, -8(sp)   ; comment", Some("S")),
            ("        beq  x1, x3, done", Some("SB")),
            ("        lui  x1, 100", Some("U")),
            ("ERROR: syntax error at line 12", None),
            ("", Some("I")),
        ];
        for (src, fmt) in source_correlated_cases {
            assert_preserves(&highlight_source_line_correlated(src, fmt), src);
        }
    }

    #[test]
    fn correlated_highlighting_maps_bitfield_roles_to_operands() {
        let semantic = |spans: &[Span]| -> Vec<(String, Style)> {
            spans
                .iter()
                .filter_map(|s| {
                    let t = s.content.trim();
                    if t.is_empty() || t == "," || t == "(" || t == ")" {
                        None
                    } else {
                        Some((t.to_string(), s.style))
                    }
                })
                .collect()
        };

        // 1. R-format: add rd, rs1, rs2
        // Binary tokens: [fmt, funct7, rs2, rs1, funct3, rd, opcode]
        let bin_r = highlight_bin_correlated("R  0000000 00010 00001 000 00011 0110011");
        let b_r = semantic(&bin_r);
        assert_eq!(b_r[0].1, STYLE_FMT); // R
        assert_eq!(b_r[1].1, STYLE_OP); // funct7
        assert_eq!(b_r[2].1, STYLE_ARG3_IMM); // rs2
        assert_eq!(b_r[3].1, STYLE_ARG2); // rs1
        assert_eq!(b_r[4].1, STYLE_OP); // funct3
        assert_eq!(b_r[5].1, STYLE_ARG1); // rd
        assert_eq!(b_r[6].1, STYLE_OP); // opcode

        let ops_r = highlight_operands_correlated("add     x3, x1, x2", Some("R"));
        let o_r = semantic(&ops_r);
        assert_eq!(o_r[0].1, STYLE_OP); // add
        assert_eq!(o_r[1].1, STYLE_ARG1); // x3 (rd)
        assert_eq!(o_r[2].1, STYLE_ARG2); // x1 (rs1)
        assert_eq!(o_r[3].1, STYLE_ARG3_IMM); // x2 (rs2)
        // Check role correspondence
        assert_eq!(b_r[5].1, o_r[1].1); // rd
        assert_eq!(b_r[3].1, o_r[2].1); // rs1
        assert_eq!(b_r[2].1, o_r[3].1); // rs2

        // 2. I-format: addi rd, rs1, imm
        // Binary tokens: [fmt, imm, rs1, funct3, rd, opcode]
        let bin_i = highlight_bin_correlated("I  000000000101  00000 000 00001 0010011");
        let b_i = semantic(&bin_i);
        assert_eq!(b_i[0].1, STYLE_FMT); // I
        assert_eq!(b_i[1].1, STYLE_ARG3_IMM); // imm
        assert_eq!(b_i[2].1, STYLE_ARG2); // rs1
        assert_eq!(b_i[3].1, STYLE_OP); // funct3
        assert_eq!(b_i[4].1, STYLE_ARG1); // rd
        assert_eq!(b_i[5].1, STYLE_OP); // opcode

        let ops_i = highlight_operands_correlated("addi    x1, x0, 5", Some("I"));
        let o_i = semantic(&ops_i);
        assert_eq!(o_i[0].1, STYLE_OP); // addi
        assert_eq!(o_i[1].1, STYLE_ARG1); // x1 (rd)
        assert_eq!(o_i[2].1, STYLE_ARG2); // x0 (rs1)
        assert_eq!(o_i[3].1, STYLE_ARG3_IMM); // 5 (imm)
        // Check role correspondence
        assert_eq!(b_i[4].1, o_i[1].1); // rd
        assert_eq!(b_i[2].1, o_i[2].1); // rs1
        assert_eq!(b_i[1].1, o_i[3].1); // imm

        // 3. S-format: sd rs2, imm(rs1)
        // Binary tokens: [fmt, imm[11:5], rs2, rs1, funct3, imm[4:0], opcode]
        // Note: rs2 is operand 1 in asm syntax, but bits [24:20] in binary
        let bin_s = highlight_bin_correlated("S  0000000 00011 00000 011 01000 0100011");
        let b_s = semantic(&bin_s);
        assert_eq!(b_s[0].1, STYLE_FMT); // S
        assert_eq!(b_s[1].1, STYLE_ARG3_IMM); // imm[11:5]
        assert_eq!(b_s[2].1, STYLE_ARG1); // rs2 (bits [24:20])
        assert_eq!(b_s[3].1, STYLE_ARG2); // rs1 (bits [19:15])
        assert_eq!(b_s[4].1, STYLE_OP); // funct3
        assert_eq!(b_s[5].1, STYLE_ARG3_IMM); // imm[4:0]
        assert_eq!(b_s[6].1, STYLE_OP); // opcode

        let ops_s = highlight_operands_correlated("sd      x3, 8(x0)", Some("S"));
        let o_s = semantic(&ops_s);
        assert_eq!(o_s[0].1, STYLE_OP); // sd
        assert_eq!(o_s[1].1, STYLE_ARG1); // x3 (rs2)
        assert_eq!(o_s[2].1, STYLE_ARG3_IMM); // 8 (imm)
        assert_eq!(o_s[3].1, STYLE_ARG2); // x0 (rs1 base)
        // Check role correspondence: asm rs2 (arg1) matches binary rs2, rs1 matches rs1, imm matches imm
        assert_eq!(b_s[2].1, o_s[1].1); // rs2
        assert_eq!(b_s[3].1, o_s[3].1); // rs1
        assert_eq!(b_s[1].1, o_s[2].1); // imm[11:5] == imm
        assert_eq!(b_s[5].1, o_s[2].1); // imm[4:0] == imm

        // 4. SB-format: beq rs1, rs2, off
        // Binary tokens: [fmt, imm[12|10:5], rs2, rs1, funct3, imm[4:1|11], opcode]
        // Note: rs1 is operand 1 in asm, bits [19:15] in binary; rs2 is operand 2 in asm, bits [24:20] in binary
        let bin_sb = highlight_bin_correlated("SB 0000000 00011 00001 000 00000 1100011");
        let b_sb = semantic(&bin_sb);
        assert_eq!(b_sb[0].1, STYLE_FMT); // SB
        assert_eq!(b_sb[1].1, STYLE_ARG3_IMM); // imm[12|10:5]
        assert_eq!(b_sb[2].1, STYLE_ARG2); // rs2 (bits [24:20])
        assert_eq!(b_sb[3].1, STYLE_ARG1); // rs1 (bits [19:15])
        assert_eq!(b_sb[4].1, STYLE_OP); // funct3
        assert_eq!(b_sb[5].1, STYLE_ARG3_IMM); // imm[4:1|11]
        assert_eq!(b_sb[6].1, STYLE_OP); // opcode

        let ops_sb = highlight_operands_correlated("beq     x1, x3, 0", Some("SB"));
        let o_sb = semantic(&ops_sb);
        assert_eq!(o_sb[0].1, STYLE_OP); // beq
        assert_eq!(o_sb[1].1, STYLE_ARG1); // x1 (rs1)
        assert_eq!(o_sb[2].1, STYLE_ARG2); // x3 (rs2)
        assert_eq!(o_sb[3].1, STYLE_ARG3_IMM); // 0 (imm offset)
        // Check role correspondence
        assert_eq!(b_sb[3].1, o_sb[1].1); // rs1
        assert_eq!(b_sb[2].1, o_sb[2].1); // rs2
        assert_eq!(b_sb[1].1, o_sb[3].1); // imm
        assert_eq!(b_sb[5].1, o_sb[3].1); // imm
    }
}
