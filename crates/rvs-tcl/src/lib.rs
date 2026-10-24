//! Tcl-compatible numeric and text semantics used by RVS046.tcl.
//!
//! Every helper here replicates an observable behavior of Tcl 8.6 that the
//! reference simulator depends on (integer parsing rules, floor division,
//! formatting widths, shortest-round-trip double rendering, ...).

/// Tcl integer literal parsing as used by `expr`, `binary format` and
/// `format %ld`: optional sign, `0x`/`0X` hex, `0b`/`0B` binary, `0o`/`0O`
/// octal, leading-zero octal, otherwise decimal. Returns `i128` so callers
/// can detect 64-bit overflow where Tcl would keep a bignum.
pub fn parse_int(s: &str) -> Option<i128> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let (neg, rest) = match t.as_bytes()[0] {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    if rest.is_empty() {
        return None;
    }
    let lower = rest.to_ascii_lowercase();
    let (radix, digits) = if let Some(d) = lower.strip_prefix("0x") {
        (16, d)
    } else if let Some(d) = lower.strip_prefix("0b") {
        (2, d)
    } else if let Some(d) = lower.strip_prefix("0o") {
        (8, d)
    } else if lower.len() > 1 && lower.starts_with('0') {
        (8, &lower[1..])
    } else {
        (10, &lower[..])
    };
    if digits.is_empty() {
        return None;
    }
    // Tcl permits underscores as digit separators.
    let cleaned: String = digits.chars().filter(|c| *c != '_').collect();
    if cleaned.is_empty() {
        return None;
    }
    let mag = i128::from_str_radix(&cleaned, radix).ok()?;
    Some(if neg { -mag } else { mag })
}

/// True when `s` parses as an integer under Tcl's `string is integer` rules
/// (used by slbl result checks and by `instr` for immediates).
pub fn is_integer(s: &str) -> bool {
    parse_int(s).is_some()
}

/// True when the value fits Tcl `string is wideinteger`: Tcl 8.6 accepts
/// any value in [-2^63, 2^64-1] (hex literals beyond i64::MAX stay wide).
pub fn is_wide_integer(s: &str) -> bool {
    match parse_int(s) {
        Some(v) => v >= i64::MIN as i128 && v <= u64::MAX as i128,
        None => false,
    }
}

/// Tcl `expr` integer division: rounds toward negative infinity.
pub fn tcl_div(a: i64, b: i64) -> Option<i64> {
    if b == 0 {
        return None;
    }
    let q = a.wrapping_div(b);
    if (a % b != 0) && ((a < 0) != (b < 0)) {
        Some(q - 1)
    } else {
        Some(q)
    }
}

/// Tcl `expr` integer remainder: result carries the sign of the divisor.
pub fn tcl_rem(a: i64, b: i64) -> Option<i64> {
    if b == 0 {
        return None;
    }
    Some(a.wrapping_sub(tcl_div(a, b)?.wrapping_mul(b)))
}

/// `a64`: Tcl `format "0x%016lx" [expr $s]` over a 64-bit value.
pub fn a64(v: u64) -> String {
    format!("0x{v:016x}")
}

/// `i2x`: `format 0x%016lx`.
pub fn i2x(v: i64) -> String {
    a64(v as u64)
}

/// `x12i`: 12-bit value to signed.
pub fn x12i(x: u64) -> i64 {
    if x > 2047 {
        x as i64 - 4096
    } else {
        x as i64
    }
}

/// `x20i`: 20-bit value to signed.
pub fn x20i(x: u64) -> i64 {
    if x > (1u64 << 19) - 1 {
        x as i64 - (1i64 << 20)
    } else {
        x as i64
    }
}

/// `i12x` rendering: `0x` + last three hex digits of `%03x` (works for
/// negative values through Tcl's unsigned formatting).
pub fn i12x_hex(v: i64) -> String {
    let digits = format!("{v:03x}");
    format!("0x{}", &digits[digits.len() - 3..])
}

/// `i20x` rendering: `0x` + last five hex digits of `%05x`.
pub fn i20x_hex(v: i64) -> String {
    let digits = format!("{v:05x}");
    format!("0x{}", &digits[digits.len() - 5..])
}

/// `format "%ld"` of a hex-or-decimal string: signed 64-bit.
pub fn fmt_ld_str(s: &str) -> Option<String> {
    let v = parse_int(s)?;
    Some((v as i64).to_string())
}

/// `format "%ld"` of a Tcl unsigned/bignum value truncated to a wide.
pub fn fmt_ld(v: i128) -> String {
    (v as i64).to_string()
}

/// `format "%-Nld"` (left-justified signed decimal, width N).
pub fn fmt_ld_pad(v: i64, w: usize) -> String {
    let s = v.to_string();
    if s.len() >= w {
        s
    } else {
        format!("{s}{}", " ".repeat(w - s.len()))
    }
}

/// `format "%-Ns"` (left-justified string).
pub fn fmt_s_pad(s: &str, w: usize) -> String {
    if s.len() >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - s.len()))
    }
}

/// Low 64 bits of a Tcl bignum, as unsigned.
pub fn low64(v: i128) -> u64 {
    (v as u128 & u64::MAX as u128) as u64
}

/// `format %0N$b` of a possibly negative wide followed by taking the last
/// `digits` characters: the low `digits` bits of the 64-bit two's complement.
pub fn fmt_bits(v: i64, digits: usize) -> String {
    let full = format!("{:064b}", v as u64);
    full[full.len() - digits..].to_string()
}

/// `format %05x` / `%03x` of a possibly negative wide: full-width two's
/// complement hex, last `digits` characters.
pub fn fmt_hex_tail(v: i64, digits: usize) -> String {
    let full = format!("{:016x}", v as u64);
    full[full.len() - digits..].to_string()
}

/// `format %02X`-style uppercase 16-digit hex (register writes via `%016lX`).
pub fn fmt_hex_upper16(v: u64) -> String {
    format!("0x{:016X}", v)
}

/// Tcl double rendering (`x2f` string results, ECALL 1, FLP panes):
/// shortest significant-digit representation that round-trips, then rendered
/// fixed for decimal exponents -4..16 and scientific otherwise, always
/// containing a decimal point or exponent, `Inf`/`-Inf`/`NaN` named.
pub fn tcl_double_to_string(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Inf".to_string()
        } else {
            "-Inf".to_string()
        };
    }
    if v == 0.0 {
        return if v.is_sign_negative() {
            "-0.0".to_string()
        } else {
            "0.0".to_string()
        };
    }
    let neg = v < 0.0;
    let av = v.abs();
    // Find the smallest precision whose scientific form round-trips, then
    // trim trailing zero digits that stay round-tripping.
    let mut digits = String::new();
    let mut exp10: i32 = 0;
    for p in 1..=17usize {
        let s = format!("{av:.*e}", p - 1);
        if s.parse::<f64>() == Ok(av) {
            let (mant, e) = s.split_once('e').expect("e-notation");
            exp10 = e.parse::<i32>().expect("exponent");
            digits = mant.chars().filter(|c| c.is_ascii_digit()).collect();
            while digits.len() > 1 && digits.ends_with('0') {
                let candidate = &digits[..digits.len() - 1];
                if render_digits(candidate, exp10).parse::<f64>() == Ok(av) {
                    digits = candidate.to_string();
                } else {
                    break;
                }
            }
            break;
        }
    }
    let body = render_digits(&digits, exp10);
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

fn render_digits(digits: &str, exp10: i32) -> String {
    if (-4..17).contains(&exp10) {
        if exp10 >= 0 {
            let e = exp10 as usize;
            if digits.len() > e + 1 {
                let (int_part, frac_part) = digits.split_at(e + 1);
                format!("{int_part}.{frac_part}")
            } else {
                let mut int_part = digits.to_string();
                for _ in digits.len()..=e {
                    int_part.push('0');
                }
                format!("{int_part}.0")
            }
        } else {
            let zeros = (-exp10 - 1) as usize;
            format!("0.{}{digits}", "0".repeat(zeros))
        }
    } else {
        let sign = if exp10 < 0 { '-' } else { '+' };
        let mag = exp10.abs();
        if digits.len() == 1 {
            format!("{digits}e{sign}{mag}")
        } else {
            let (head, tail) = digits.split_at(1);
            format!("{head}.{tail}e{sign}{mag}")
        }
    }
}

/// `format "%24.16E"`: C-style scientific, 16 fractional digits, uppercase,
/// two-digit exponent; `INF`/`-INF` for infinities; errors on NaN like Tcl.
pub fn fmt_e24(v: f64) -> Option<String> {
    if v.is_nan() {
        return None;
    }
    if v.is_infinite() {
        let core = if v > 0.0 { "INF" } else { "-INF" };
        return Some(fmt_s_pad_right(core, 24));
    }
    let s = format!("{v:.16E}");
    // Rust renders the exponent without zero padding and lowercase 'e';
    // normalize to C printf's `E+XX` form.
    let (mant, e) = s.split_once('E')?;
    let (sign, mag) = if let Some(m) = e.strip_prefix('-') {
        ('-', m)
    } else {
        ('+', e.trim_start_matches('+'))
    };
    let mag = if mag.len() < 2 {
        format!("0{mag}")
    } else {
        mag.to_string()
    };
    Some(fmt_s_pad_right(&format!("{mant}E{sign}{mag}"), 24))
}

/// `format "%14s"` right-justified.
pub fn fmt_s_pad_right(s: &str, w: usize) -> String {
    if s.len() >= w {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(w - s.len()))
    }
}

/// `revx`: reverse a hex string pairwise; re-homes a trailing `0x` prefix.
pub fn revx(x: &str) -> String {
    let bytes: Vec<char> = x.chars().collect();
    let mut res = String::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let pair: String = bytes[i..(i + 2).min(bytes.len())].iter().collect();
        res = format!("{pair}{res}");
        i += 2;
    }
    if res.ends_with("0x") {
        let body: String = res.chars().take(res.len() - 2).collect();
        format!("0x{body}")
    } else {
        res
    }
}

/// `s2x`: characters to hex, first char high byte, zero-padded to 8 bytes.
pub fn s2x(c: &str) -> String {
    let mut hex = String::new();
    for b in c.as_bytes() {
        hex.push_str(&format!("{b:02x}"));
    }
    hex.push_str("0000000000000000");
    format!("0x{}", &hex[..16])
}

/// `x2s`: hex digits (no prefix needed) to raw bytes as a lossy string.
pub fn x2s(x: &str) -> String {
    let t = x.strip_prefix("0x").unwrap_or(x);
    let mut bytes = Vec::new();
    let chars: Vec<char> = t.chars().collect();
    let mut i = 0usize;
    while i + 1 < chars.len() {
        let hi = chars[i].to_digit(16);
        let lo = chars[i + 1].to_digit(16);
        if let (Some(hi), Some(lo)) = (hi, lo) {
            bytes.push(((hi << 4) | lo) as u8);
        }
        i += 2;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// `f2x`: double to hex of its little-endian bytes.
pub fn f2x(v: f64) -> String {
    let bits = v.to_bits();
    format!("0x{:016x}", bits)
}

/// `x2f`: hex digits are the byte sequence read big-endian by `binary scan`
/// Q... in practice the digit string IS the IEEE-754 bit pattern; fewer than
/// 16 digits yields the empty string like `binary scan Q` with short input.
pub fn x2f(x: &str) -> Option<f64> {
    let t = x.strip_prefix("0x").unwrap_or(x);
    let chars: Vec<char> = t.chars().collect();
    if chars.len() < 16 || !chars.len().is_multiple_of(2) {
        return None;
    }
    let digits: String = chars[..16].iter().collect();
    let bits = u64::from_str_radix(&digits, 16).ok()?;
    Some(f64::from_bits(bits))
}

/// `string is double` approximation: hex strings count as doubles in Tcl.
pub fn is_double(s: &str) -> bool {
    if s.trim().is_empty() {
        return false;
    }
    if parse_int(s).is_some() {
        return true;
    }
    s.trim().parse::<f64>().is_ok()
}

/// `string is alpha` (first char a letter).
pub fn is_alpha_first(s: &str) -> bool {
    s.chars()
        .next()
        .map(|c| c.is_ascii_alphabetic())
        .unwrap_or(false)
}

/// `string is wordchar` over the whole string (alphanumeric or `_`).
pub fn is_wordchars(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `string trim` with an explicit cut set.
pub fn trim_chars(s: &str, cut: &[char]) -> String {
    s.trim_matches(|c| cut.contains(&c)).to_string()
}

/// Render a Tcl list the way `$list` interpolation would (minimal quoting:
/// bare words, brace-quoting for words without braces, backslash-escapes
/// otherwise). Used only for error messages that embed list values.
pub fn tcl_list_str(elements: &[String]) -> String {
    elements
        .iter()
        .map(|e| {
            let needs = e.is_empty() || e.chars().any(|c| " \t\n\r{}[]\"\\$;".contains(c));
            if !needs {
                e.clone()
            } else if !e.contains('{')
                && !e.contains('}')
                && !e.contains('\\')
                && !e.contains('[')
                && !e.contains(']')
            {
                format!("{{{e}}}")
            } else {
                let mut out = String::new();
                for c in e.chars() {
                    match c {
                        '{' | '}' | '[' | ']' | '"' | '\\' | '$' | ';' | ' ' | '\t' => {
                            out.push('\\');
                            out.push(c);
                        }
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        _ => out.push(c),
                    }
                }
                out
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Remove NUL characters from both ends (`string trim s \0`).
pub fn trim_nul(s: &str) -> String {
    s.trim_matches('\0').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tcl_integers() {
        assert_eq!(parse_int("10"), Some(10));
        assert_eq!(parse_int("010"), Some(8));
        assert_eq!(parse_int("0x10"), Some(16));
        assert_eq!(parse_int("0b101"), Some(5));
        assert_eq!(parse_int("-7"), Some(-7));
        assert_eq!(parse_int("+5"), Some(5));
        assert_eq!(parse_int(""), None);
        assert_eq!(parse_int("5x"), None);
    }

    #[test]
    fn tcl_division_floors() {
        assert_eq!(tcl_div(-7, 2), Some(-4));
        assert_eq!(tcl_div(7, -2), Some(-4));
        assert_eq!(tcl_rem(-7, 2), Some(1));
        assert_eq!(tcl_rem(7, -2), Some(-1));
        assert_eq!(tcl_div(1, 0), None);
    }

    #[test]
    fn doubles_match_tcl() {
        assert_eq!(tcl_double_to_string(2.0), "2.0");
        assert_eq!(tcl_double_to_string(1e20), "1e+20");
        assert_eq!(tcl_double_to_string(1e-20), "1e-20");
        assert_eq!(tcl_double_to_string(0.1), "0.1");
        assert_eq!(tcl_double_to_string(1.0 / 3.0), "0.3333333333333333");
        assert_eq!(
            tcl_double_to_string(123456789012345678.0),
            "1.2345678901234568e+17"
        );
        assert_eq!(tcl_double_to_string(1e16), "10000000000000000.0");
        assert_eq!(tcl_double_to_string(1e15), "1000000000000000.0");
        assert_eq!(tcl_double_to_string(-0.0), "-0.0");
        assert_eq!(tcl_double_to_string(0.0001), "0.0001");
        assert_eq!(tcl_double_to_string(1e-5), "1e-5");
        assert_eq!(tcl_double_to_string(1234.5678), "1234.5678");
    }

    #[test]
    fn hex_helpers() {
        assert_eq!(revx("0000000000000041"), "4100000000000000");
        assert_eq!(s2x("AB"), "0x4142000000000000");
        assert_eq!(f2x(2.0), "0x4000000000000000");
        assert_eq!(x2f("0x4000000000000000"), Some(2.0));
        assert_eq!(fmt_e24(2.0), Some("  2.0000000000000000E+00".to_string()));
        assert_eq!(fmt_e24(0.1), Some("  1.0000000000000001E-01".to_string()));
        assert_eq!(fmt_e24(-0.0), Some(" -0.0000000000000000E+00".to_string()));
        assert_eq!(
            fmt_e24(f64::INFINITY),
            Some("                     INF".to_string())
        );
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn parse_int_roundtrips_a64(v: u64) {
            prop_assert_eq!(parse_int(&a64(v)), Some(v as i128));
        }

        #[test]
        fn parse_int_roundtrips_i2x(v: i64) {
            prop_assert_eq!(parse_int(&i2x(v)), Some((v as u64) as i128));
        }

        #[test]
        fn twelve_bit_sign_roundtrips(v in -2048i64..=2047) {
            let hex = i12x_hex(v);
            prop_assert_eq!(x12i(parse_int(&hex).unwrap() as u64), v);
        }

        #[test]
        fn twenty_bit_sign_roundtrips(v in -524_288i64..=524_287) {
            let hex = i20x_hex(v);
            prop_assert_eq!(x20i(parse_int(&hex).unwrap() as u64), v);
        }

        #[test]
        fn underscores_are_digit_separators(v: u32) {
            let digits = v.to_string();
            let len = digits.len();
            let grouped: String = digits
                .chars()
                .enumerate()
                .flat_map(|(i, c)| {
                    if i > 0 && (len - i) % 3 == 0 {
                        vec!['_', c]
                    } else {
                        vec![c]
                    }
                })
                .collect();
            prop_assert_eq!(parse_int(&grouped), Some(v as i128));
        }
    }
}
