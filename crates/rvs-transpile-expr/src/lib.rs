//! Expression handling for GNU assembly transpilation.

use std::collections::BTreeMap;

/// Parse a decimal or prefixed unsigned-base integer literal.
pub fn parse_int_imm(s: &str) -> Option<i64> {
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

fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Split `sym = expr` when `lhs` is a bare identifier and `=` is not part of
/// `==`, `!=`, `<=`, or `>=`.
pub fn split_assignment(body: &str) -> Option<(&str, &str)> {
    let eq_idx = body.find('=')?;
    let lhs = body[..eq_idx].trim();
    let rest = &body[eq_idx + 1..];
    if rest.starts_with('=') || lhs.ends_with(['!', '<', '>']) {
        return None;
    }
    let rhs = rest.trim();
    if is_valid_ident(lhs) && !rhs.is_empty() {
        Some((lhs, rhs))
    } else {
        None
    }
}

/// Substitute `.` (when in a `.struct` block) and known struct symbols in an
/// expression so it can be evaluated or emitted as an `EQU` operand.
pub fn resolve_struct_expr(
    expr: &str,
    struct_loc: Option<i64>,
    syms: &BTreeMap<String, i64>,
) -> String {
    let mut out = String::with_capacity(expr.len() + 8);
    let mut chars = expr.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '.' {
            let next_is_ident = chars
                .peek()
                .map(|n| n.is_ascii_alphanumeric() || *n == '_')
                .unwrap_or(false);
            if !next_is_ident {
                if let Some(loc) = struct_loc {
                    out.push_str(&loc.to_string());
                    continue;
                }
            }
            out.push(c);
        } else if c.is_ascii_alphabetic() || c == '_' {
            let mut ident = String::new();
            ident.push(c);
            while let Some(&n) = chars.peek() {
                if n.is_ascii_alphanumeric() || n == '_' {
                    ident.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            if let Some(val) = syms.get(&ident) {
                out.push_str(&val.to_string());
            } else {
                out.push_str(&ident);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Evaluate a simple integer expression (`+`, `-`, `*`, `/`, unary `+/-`, parens)
/// used in `.struct` / `.equ` / `=` assignments.
pub fn eval_simple_expr(expr: &str) -> Option<i64> {
    #[derive(Clone, Copy)]
    enum Tok {
        Num(i64),
        Op(char),
    }
    let mut toks = Vec::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if matches!(c, '+' | '-' | '*' | '/' | '(' | ')') {
            toks.push(Tok::Op(c));
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            toks.push(Tok::Num(parse_int_imm(&word)?));
        } else {
            return None;
        }
    }
    fn add(toks: &[Tok], p: &mut usize) -> Option<i64> {
        let mut v = mul(toks, p)?;
        while let Some(Tok::Op(op @ ('+' | '-'))) = toks.get(*p) {
            *p += 1;
            let r = mul(toks, p)?;
            v = if *op == '+' {
                v.checked_add(r)?
            } else {
                v.checked_sub(r)?
            };
        }
        Some(v)
    }
    fn mul(toks: &[Tok], p: &mut usize) -> Option<i64> {
        let mut v = unary(toks, p)?;
        while let Some(Tok::Op(op @ ('*' | '/'))) = toks.get(*p) {
            *p += 1;
            let r = unary(toks, p)?;
            v = if *op == '*' {
                v.checked_mul(r)?
            } else {
                v.checked_div(r)?
            };
        }
        Some(v)
    }
    fn unary(toks: &[Tok], p: &mut usize) -> Option<i64> {
        match toks.get(*p)? {
            Tok::Op('-') => {
                *p += 1;
                unary(toks, p)?.checked_neg()
            }
            Tok::Op('+') => {
                *p += 1;
                unary(toks, p)
            }
            Tok::Op('(') => {
                *p += 1;
                let v = add(toks, p)?;
                matches!(toks.get(*p), Some(Tok::Op(')'))).then(|| {
                    *p += 1;
                    v
                })
            }
            Tok::Num(n) => {
                *p += 1;
                Some(*n)
            }
            _ => None,
        }
    }
    let mut p = 0;
    let val = add(&toks, &mut p)?;
    (p == toks.len()).then_some(val)
}
