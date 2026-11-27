//! Subroutine call harness and `@test` runner for prototyping RISC-V functions.
//!
//! Supports calling assembly routines directly with typed arguments (32-bit and
//! 64-bit integers, `int32_t[]` / `int64_t[]` arrays, NUL-terminated C strings,
//! and out-parameters like `&bool` / `&i32` / `&i64`), marshaling arguments
//! using the RV32 ILP32 C calling convention (matching ESP32-C3 labs), and
//! running comment-embedded or standalone `@test` suites.

use rvs_sim::Simulator;
use rvs_tcl as tcl;
use rvs_transpile::AssemblyDialect;

/// Default 16-byte-aligned stack pointer (< 2^31 so 32-bit `sw`/`lw` round-trips).
pub const DEFAULT_SP: u64 = 0x0010_0000;

/// Return sentinel address (< 2^31 so 32-bit `sw ra`/`lw ra` round-trips cleanly).
pub const RETURN_SENTINEL: u64 = 0x000f_fffc;

/// Base address for scratch buffers (arrays, strings, out-parameters).
pub const SCRATCH_BASE: u64 = 0x0020_0000;

/// One argument passed to a subroutine via `:call` or `@test`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallArg {
    Int32(i32),
    Int64(i64),
    ArrayI32(Vec<i32>),
    ArrayI64(Vec<i64>),
    CString(String),
    OutBool(bool),
    OutI32(i32),
    OutI64(i64),
    Symbol(String),
}

/// Parsed subroutine invocation: `func(arg0, arg1, ...)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallSpec {
    pub func: String,
    pub args: Vec<CallArg>,
}

/// Scratch buffer allocated for a pointer argument so it can be inspected after return.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BufferKind {
    ArrayI32(usize),
    ArrayI64(usize),
    CString,
    OutBool,
    OutI32,
    OutI64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllocatedBuffer {
    pub arg_index: usize,
    pub addr: u64,
    pub kind: BufferKind,
}

/// Metadata recorded when preparing a call, used to inspect outputs on return.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedCall {
    pub func: String,
    pub buffers: Vec<AllocatedBuffer>,
}

/// One expected post-condition in a `@test` directive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Assertion {
    RetEq(i64),
    ArgIntEq(usize, i64),
    ArgArrayI32Eq(usize, Vec<i32>),
    ArgArrayI64Eq(usize, Vec<i64>),
    ArgStringEq(usize, String),
}

/// One parsed `@test` directive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestCase {
    pub line: usize,
    pub raw: String,
    pub call: CallSpec,
    pub assertions: Vec<Assertion>,
}

/// Result of executing a single `@test` case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestCaseOutcome {
    pub line: usize,
    pub raw: String,
    pub passed: bool,
    pub detail: String,
    pub steps: u64,
}

/// Aggregated result of running a `@test` suite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestSuiteReport {
    pub results: Vec<TestCaseOutcome>,
    pub passed: usize,
    pub failed: usize,
    pub formatted: String,
}

// ---------------------------------------------------------------------------
// Parsing: CallSpec & @test directives
// ---------------------------------------------------------------------------

/// Split a comma-separated list respecting `""`, `''`, `[]`, and `()`.
fn split_top_level(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth_sq = 0usize;
    let mut depth_par = 0usize;
    let mut in_str = false;
    let mut in_char = false;
    let mut escaped = false;
    for ch in s.chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
            continue;
        }
        if (in_str || in_char) && ch == '\\' {
            cur.push(ch);
            escaped = true;
            continue;
        }
        if ch == '"' && !in_char {
            in_str = !in_str;
            cur.push(ch);
        } else if ch == '\'' && !in_str {
            in_char = !in_char;
            cur.push(ch);
        } else if !in_str && !in_char {
            match ch {
                '[' => {
                    depth_sq += 1;
                    cur.push(ch);
                }
                ']' => {
                    depth_sq = depth_sq.saturating_sub(1);
                    cur.push(ch);
                }
                '(' => {
                    depth_par += 1;
                    cur.push(ch);
                }
                ')' => {
                    depth_par = depth_par.saturating_sub(1);
                    cur.push(ch);
                }
                c if c == sep && depth_sq == 0 && depth_par == 0 => {
                    out.push(cur.trim().to_string());
                    cur.clear();
                }
                _ => cur.push(ch),
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() || !out.is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn unescape_quoted(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('0') => out.push('\0'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('\'') => out.push('\''),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn parse_i64_literal(s: &str) -> Option<i64> {
    let t = s.trim();
    if t.eq_ignore_ascii_case("true") {
        return Some(1);
    }
    if t.eq_ignore_ascii_case("false") {
        return Some(0);
    }
    if t.starts_with('\'') && t.ends_with('\'') && t.len() >= 3 {
        let inner = unescape_quoted(&t[1..t.len() - 1]);
        let mut chars = inner.chars();
        let ch = chars.next()?;
        if chars.next().is_none() {
            return Some(ch as i64);
        }
        return None;
    }
    tcl::parse_int(t).map(|v| v as i64)
}

fn parse_int_list<T>(inner: &str, conv: fn(i64) -> T) -> Result<Vec<T>, String> {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let mut vals = Vec::new();
    for item in split_top_level(trimmed, ',') {
        let v = parse_i64_literal(&item)
            .ok_or_else(|| format!("invalid integer element \"{item}\""))?;
        vals.push(conv(v));
    }
    Ok(vals)
}

fn parse_arg(raw: &str) -> Result<CallArg, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Err("empty argument".to_string());
    }
    // Quoted C string: "..."
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        return Ok(CallArg::CString(unescape_quoted(&t[1..t.len() - 1])));
    }
    // 64-bit array: i64[...]
    if let Some(rest) = t
        .strip_prefix("i64[")
        .or_else(|| t.strip_prefix("int64["))
        .and_then(|r| r.strip_suffix(']'))
    {
        return Ok(CallArg::ArrayI64(parse_int_list(rest, |v| v)?));
    }
    // 32-bit array: [...] or i32[...]
    if let Some(rest) = t
        .strip_prefix('[')
        .or_else(|| t.strip_prefix("i32["))
        .or_else(|| t.strip_prefix("int32["))
        .and_then(|r| r.strip_suffix(']'))
    {
        return Ok(CallArg::ArrayI32(parse_int_list(rest, |v| v as i32)?));
    }
    // Out-parameters: &bool, &i32, &i64, &bool(1), &i32(5), &i64(5)
    if let Some(rest) = t.strip_prefix('&') {
        let (ty, init) = match rest.trim().split_once('(') {
            Some((ty, val_part)) => {
                let s = val_part
                    .strip_suffix(')')
                    .ok_or_else(|| format!("unclosed out-param \"{t}\""))?;
                let v = parse_i64_literal(s)
                    .ok_or_else(|| format!("invalid out-param initializer \"{s}\""))?;
                (ty.trim(), v)
            }
            None => (rest.trim(), 0),
        };
        return match ty.to_ascii_lowercase().as_str() {
            "bool" | "u8" => Ok(CallArg::OutBool(init != 0)),
            "i32" | "int" | "int32" => Ok(CallArg::OutI32(init as i32)),
            "i64" | "int64" => Ok(CallArg::OutI64(init)),
            other => Err(format!(
                "unknown out-param \"&{other}\" (expected &bool, &i32, or &i64)"
            )),
        };
    }
    // Explicit 64-bit integer: 102i64 or i64(102)
    if let Some(num) = t
        .strip_suffix("i64")
        .or_else(|| t.strip_suffix("I64"))
        .or_else(|| t.strip_suffix("u64"))
        .or_else(|| t.strip_suffix("U64"))
    {
        let v = parse_i64_literal(num).ok_or_else(|| format!("invalid 64-bit integer \"{t}\""))?;
        return Ok(CallArg::Int64(v));
    }
    if let Some(inner) = t
        .strip_prefix("i64(")
        .or_else(|| t.strip_prefix("int64("))
        .and_then(|r| r.strip_suffix(')'))
    {
        let v =
            parse_i64_literal(inner).ok_or_else(|| format!("invalid 64-bit integer \"{t}\""))?;
        return Ok(CallArg::Int64(v));
    }
    // Plain integer / char / bool
    if let Some(v) = parse_i64_literal(t) {
        return Ok(CallArg::Int32(v as i32));
    }
    // Bare symbol name
    if t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Ok(CallArg::Symbol(t.to_string()));
    }
    Err(format!("cannot parse call argument \"{t}\""))
}

/// Parse `func(arg1, arg2, ...)` or bare `func` (zero args).
pub fn parse_call_spec(text: &str) -> Result<CallSpec, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("usage: call <func>(<args...>)".to_string());
    }
    let Some(open) = trimmed.find('(') else {
        if trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Ok(CallSpec {
                func: trimmed.to_string(),
                args: Vec::new(),
            });
        }
        return Err(format!("invalid function name \"{trimmed}\""));
    };
    let func = trimmed[..open].trim();
    if func.is_empty() || !func.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!("invalid function name \"{func}\""));
    }
    let rest = trimmed[open + 1..].trim();
    let inner = rest
        .strip_suffix(')')
        .ok_or_else(|| format!("missing closing ')' in \"{trimmed}\""))?;
    let mut args = Vec::new();
    if !inner.trim().is_empty() {
        for raw in split_top_level(inner, ',') {
            args.push(parse_arg(&raw)?);
        }
    }
    Ok(CallSpec {
        func: func.to_string(),
        args,
    })
}

fn find_matching_paren(s: &str, open_idx: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_str = false;
    let mut in_char = false;
    let mut escaped = false;
    for (idx, ch) in s[open_idx..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if (in_str || in_char) && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' && !in_char {
            in_str = !in_str;
        } else if ch == '\'' && !in_str {
            in_char = !in_char;
        } else if !in_str && !in_char {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    return Some(open_idx + idx);
                }
            }
        }
    }
    None
}

fn parse_assertion(raw: &str) -> Result<Assertion, String> {
    let (lhs, rhs) = raw
        .split_once("==")
        .ok_or_else(|| format!("assertion must use '==': \"{raw}\""))?;
    let lhs = lhs.trim().to_ascii_lowercase();
    let rhs = rhs.trim();
    if lhs == "ret" || lhs == "a0" || lhs == "return" {
        let v = parse_i64_literal(rhs)
            .ok_or_else(|| format!("expected integer return value, got \"{rhs}\""))?;
        return Ok(Assertion::RetEq(v));
    }
    if let Some(num) = lhs.strip_prefix("arg") {
        let idx = num
            .parse::<usize>()
            .map_err(|_| format!("invalid argument index \"{lhs}\""))?;
        if let Some(inner) = rhs
            .strip_prefix("i64[")
            .or_else(|| rhs.strip_prefix("int64["))
            .and_then(|r| r.strip_suffix(']'))
        {
            return Ok(Assertion::ArgArrayI64Eq(idx, parse_int_list(inner, |v| v)?));
        }
        if let Some(inner) = rhs
            .strip_prefix('[')
            .or_else(|| rhs.strip_prefix("i32["))
            .and_then(|r| r.strip_suffix(']'))
        {
            return Ok(Assertion::ArgArrayI32Eq(
                idx,
                parse_int_list(inner, |v| v as i32)?,
            ));
        }
        if rhs.starts_with('"') && rhs.ends_with('"') && rhs.len() >= 2 {
            return Ok(Assertion::ArgStringEq(
                idx,
                unescape_quoted(&rhs[1..rhs.len() - 1]),
            ));
        }
        let v = parse_i64_literal(rhs)
            .ok_or_else(|| format!("expected value for {lhs}, got \"{rhs}\""))?;
        return Ok(Assertion::ArgIntEq(idx, v));
    }
    Err(format!(
        "unknown assertion target \"{lhs}\" (expected ret, a0, or argN)"
    ))
}

/// Extract all `@test ...` directives from assembly comments or a driver file.
///
/// Supported forms:
/// - `# @test func(args...) == 42`
/// - `# @test func(args...) == 42, arg1 == 1`
/// - `# @test func(args...) -> arg0 == [50, 8, 21, 34, 10]`
/// - `@test func(args...) == 42` (in standalone `.rvst` driver files)
pub fn parse_test_directives(source: &str) -> Vec<Result<TestCase, String>> {
    let mut cases = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();
        let rest = if let Some(after) = trimmed.strip_prefix("@test") {
            after
        } else if let Some(after_hash) = trimmed
            .strip_prefix('#')
            .or_else(|| trimmed.strip_prefix("//"))
            .or_else(|| trimmed.strip_prefix(';'))
        {
            let inner = after_hash.trim_start();
            let Some(after) = inner.strip_prefix("@test") else {
                continue;
            };
            after
        } else {
            continue;
        };
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            continue;
        }
        let body = rest.trim();
        if body.is_empty() {
            continue;
        }
        cases.push(parse_test_body(line_no, body));
    }
    cases
}

fn parse_test_body(line: usize, body: &str) -> Result<TestCase, String> {
    let (call_str, tail) = if let Some(open) = body.find('(') {
        let close = find_matching_paren(body, open)
            .ok_or_else(|| format!("line {line}: unclosed '(' in @test"))?;
        (&body[..=close], body[close + 1..].trim())
    } else if let Some((c, _)) = body.split_once("->") {
        (c.trim(), &body[c.len()..])
    } else if let Some((c, _)) = body.split_once("==") {
        (c.trim(), &body[c.len()..])
    } else {
        (body, "")
    };
    let call = parse_call_spec(call_str).map_err(|e| format!("line {line}: {e}"))?;
    let mut assertions = Vec::new();
    let tail = tail.trim();
    if let Some(after_arrow) = tail.strip_prefix("->") {
        for part in split_top_level(after_arrow.trim(), ',') {
            if !part.is_empty() {
                assertions.push(parse_assertion(&part).map_err(|e| format!("line {line}: {e}"))?);
            }
        }
    } else if let Some(after_eq) = tail.strip_prefix("==") {
        let parts = split_top_level(after_eq.trim(), ',');
        if let Some(first) = parts.first() {
            let v = parse_i64_literal(first)
                .ok_or_else(|| format!("line {line}: invalid expected return value \"{first}\""))?;
            assertions.push(Assertion::RetEq(v));
        }
        for extra in parts.iter().skip(1) {
            if !extra.is_empty() {
                assertions.push(parse_assertion(extra).map_err(|e| format!("line {line}: {e}"))?);
            }
        }
    } else if !tail.is_empty() {
        return Err(format!("line {line}: unexpected trailing text \"{tail}\""));
    }
    Ok(TestCase {
        line,
        raw: body.to_string(),
        call,
        assertions,
    })
}

// ---------------------------------------------------------------------------
// ABI marshaling, call preparation & execution
// ---------------------------------------------------------------------------

fn align_up(addr: u64, align: u64) -> u64 {
    if align <= 1 {
        return addr;
    }
    let rem = addr % align;
    if rem == 0 {
        addr
    } else {
        addr + (align - rem)
    }
}

fn write_cstring(sim: &mut Simulator, addr: u64, text: &str) -> Result<u64, String> {
    let mut cur = addr;
    for b in text.as_bytes() {
        sim.mem
            .write_bytes(cur, 1, u64::from(*b), false)
            .map_err(|e| e.0)?;
        cur += 1;
    }
    sim.mem.write_bytes(cur, 1, 0, false).map_err(|e| e.0)?;
    Ok(cur + 1)
}

fn read_cstring(sim: &Simulator, addr: u64) -> String {
    let mut bytes = Vec::new();
    for offset in 0..4096u64 {
        let b = sim.mem.read_bytes(addr + offset, 1).unwrap_or(0) as u8;
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Prepare `sim` to execute `spec` using the RV32 ILP32 C calling convention.
///
/// Resets registers, sets `sp = DEFAULT_SP`, `ra = RETURN_SENTINEL`,
/// `pc = &func`, allocates arrays/strings/out-parameters in scratch memory,
/// populates `a0..a7` + stack overflow words at `0(sp), 4(sp), ...`, and
/// registers a call frame with the convention checker if enabled.
pub fn prepare_call(sim: &mut Simulator, spec: &CallSpec) -> Result<PreparedCall, String> {
    let entry_pc = sim
        .sym
        .get(&spec.func)
        .and_then(|s| tcl::parse_int(s))
        .map(|v| v as u64)
        .ok_or_else(|| format!("symbol \"{}\" not found (compile first)", spec.func))?;

    sim.regs.clear();
    let entry_pc = sim.regs.normalize(entry_pc);
    sim.regs.pc = entry_pc;
    sim.regs.write_x(1, RETURN_SENTINEL, false); // ra
    sim.regs.write_x(2, DEFAULT_SP, false); // sp
    sim.return_sentinel = Some(RETURN_SENTINEL);
    sim.exit_requested = false;
    sim.last_error = None;
    sim.brk = false;
    sim.pending = None;
    sim.convention.reset();
    sim.metrics.reset();

    let mut scratch = SCRATCH_BASE;
    let mut buffers = Vec::new();
    // Each RV32 argument slot is 32 bits; sign-extended to 64 bits for integer
    // slots or zero-extended for pointers/split halves so 64-bit ALU ops behave
    // naturally.
    let mut slots: Vec<u64> = Vec::new();

    for (arg_idx, arg) in spec.args.iter().enumerate() {
        match arg {
            CallArg::Int32(v) => {
                slots.push(*v as i64 as u64);
            }
            CallArg::Int64(v) => {
                let bits = *v as u64;
                let lo = (bits & 0xffff_ffff) as u32;
                let hi = ((bits >> 32) & 0xffff_ffff) as u32;
                slots.push(u64::from(lo));
                slots.push(u64::from(hi));
            }
            CallArg::ArrayI32(items) => {
                scratch = align_up(scratch, 8);
                let base = scratch;
                for (i, &val) in items.iter().enumerate() {
                    sim.mem
                        .write_bytes(base + (i as u64) * 4, 4, (val as u32) as u64, false)
                        .map_err(|e| e.0)?;
                }
                scratch = align_up(base + (items.len() as u64).max(1) * 4, 8);
                buffers.push(AllocatedBuffer {
                    arg_index: arg_idx,
                    addr: base,
                    kind: BufferKind::ArrayI32(items.len()),
                });
                slots.push(base);
            }
            CallArg::ArrayI64(items) => {
                scratch = align_up(scratch, 8);
                let base = scratch;
                for (i, &val) in items.iter().enumerate() {
                    sim.mem
                        .write_bytes(base + (i as u64) * 8, 8, val as u64, false)
                        .map_err(|e| e.0)?;
                }
                scratch = align_up(base + (items.len() as u64).max(1) * 8, 8);
                buffers.push(AllocatedBuffer {
                    arg_index: arg_idx,
                    addr: base,
                    kind: BufferKind::ArrayI64(items.len()),
                });
                slots.push(base);
            }
            CallArg::CString(text) => {
                scratch = align_up(scratch, 8);
                let base = scratch;
                let end = write_cstring(sim, base, text)?;
                scratch = align_up(end, 8);
                buffers.push(AllocatedBuffer {
                    arg_index: arg_idx,
                    addr: base,
                    kind: BufferKind::CString,
                });
                slots.push(base);
            }
            CallArg::OutBool(init) => {
                scratch = alloc_out(
                    sim,
                    scratch,
                    8,
                    u64::from(*init),
                    arg_idx,
                    BufferKind::OutBool,
                    &mut buffers,
                    &mut slots,
                )?;
            }
            CallArg::OutI32(init) => {
                scratch = alloc_out(
                    sim,
                    scratch,
                    4,
                    (*init as u32) as u64,
                    arg_idx,
                    BufferKind::OutI32,
                    &mut buffers,
                    &mut slots,
                )?;
            }
            CallArg::OutI64(init) => {
                scratch = alloc_out(
                    sim,
                    scratch,
                    8,
                    *init as u64,
                    arg_idx,
                    BufferKind::OutI64,
                    &mut buffers,
                    &mut slots,
                )?;
            }
            CallArg::Symbol(name) => {
                let addr = sim
                    .sym
                    .get(name)
                    .and_then(|s| tcl::parse_int(s))
                    .map(|v| v as u64)
                    .ok_or_else(|| format!("symbol \"{name}\" not found"))?;
                slots.push(sim.regs.normalize(addr));
            }
        }
    }

    for (i, &val) in slots.iter().enumerate() {
        if i < 8 {
            sim.regs.write_x(10 + i as u8, val, false); // a0..a7
        } else {
            let stack_off = ((i - 8) as u64) * 4;
            sim.mem
                .write_bytes(DEFAULT_SP + stack_off, 4, val & 0xffff_ffff, false)
                .map_err(|e| e.0)?;
        }
    }

    if sim.convention.enabled {
        let _ = sim.convention.on_call(
            RETURN_SENTINEL.wrapping_sub(4),
            entry_pc,
            "<harness>".to_string(),
            spec.func.clone(),
            &sim.regs,
        );
    }

    Ok(PreparedCall {
        func: spec.func.clone(),
        buffers,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "internal helper allocates one scratch out-parameter cell"
)]
fn alloc_out(
    sim: &mut Simulator,
    scratch: u64,
    width: usize,
    val: u64,
    arg_index: usize,
    kind: BufferKind,
    buffers: &mut Vec<AllocatedBuffer>,
    slots: &mut Vec<u64>,
) -> Result<u64, String> {
    let base = align_up(scratch, 8);
    sim.mem
        .write_bytes(base, width, val, false)
        .map_err(|e| e.0)?;
    buffers.push(AllocatedBuffer {
        arg_index,
        addr: base,
        kind,
    });
    slots.push(base);
    Ok(base + 8)
}

fn read_i32_array(sim: &Simulator, addr: u64, len: usize) -> Vec<i32> {
    (0..len)
        .map(|i| sim.mem.read_bytes(addr + (i as u64) * 4, 4).unwrap_or(0) as u32 as i32)
        .collect()
}

fn read_i64_array(sim: &Simulator, addr: u64, len: usize) -> Vec<i64> {
    (0..len)
        .map(|i| sim.mem.read_bytes(addr + (i as u64) * 8, 8).unwrap_or(0) as i64)
        .collect()
}

/// Format the return value (`a0`) and any scratch buffers after a call finishes.
pub fn format_call_result(sim: &Simulator, prepared: &PreparedCall) -> String {
    let a0 = sim.regs.read_x(10);
    let mut lines = vec![format!(
        "{}() => a0 = {} (0x{a0:x})",
        prepared.func, sim.regs.signed(a0)
    )];
    for buf in &prepared.buffers {
        let (idx, addr) = (buf.arg_index, buf.addr);
        let detail = match buf.kind {
            BufferKind::ArrayI32(len) => format!(
                "([i32; {len}] @ 0x{addr:x}) = {:?}",
                read_i32_array(sim, addr, len)
            ),
            BufferKind::ArrayI64(len) => format!(
                "([i64; {len}] @ 0x{addr:x}) = {:?}",
                read_i64_array(sim, addr, len)
            ),
            BufferKind::CString => format!("(str @ 0x{addr:x}) = \"{}\"", read_cstring(sim, addr)),
            BufferKind::OutBool => {
                let b = sim.mem.read_bytes(addr, 1).unwrap_or(0);
                format!(
                    "(*bool @ 0x{addr:x}) = {b} ({})",
                    if b != 0 { "true" } else { "false" }
                )
            }
            BufferKind::OutI32 => format!(
                "(*i32 @ 0x{addr:x}) = {}",
                sim.mem.read_bytes(addr, 4).unwrap_or(0) as u32 as i32
            ),
            BufferKind::OutI64 => format!(
                "(*i64 @ 0x{addr:x}) = {}",
                sim.mem.read_bytes(addr, 8).unwrap_or(0) as i64
            ),
        };
        lines.push(format!("  arg{idx} {detail}"));
    }
    lines.join("\n")
}

/// Prepare and run a single call to completion on an already-assembled `sim`.
pub fn run_single_call(
    sim: &mut Simulator,
    spec: &CallSpec,
    timeout_ms: u64,
) -> Result<String, String> {
    let prepared = prepare_call(sim, spec)?;
    sim.exec_log.push_str(&format!("CALL {}()\n", spec.func));
    let status = rvs_executor::run(sim, timeout_ms);
    if status == rvs_executor::RunStatus::TimedOut {
        return Err(format!("{}() timed out after {timeout_ms}ms", spec.func));
    }
    if let Some(err) = &sim.last_error {
        return Err(format!("{}() faulted: {err}", spec.func));
    }
    Ok(format_call_result(sim, &prepared))
}

fn check_assertion(
    sim: &Simulator,
    prepared: &PreparedCall,
    assertion: &Assertion,
) -> Result<(), String> {
    match assertion {
        Assertion::RetEq(expected) => {
            let actual = sim.regs.signed(sim.regs.read_x(10));
            if actual == *expected || (actual as i32 as i64) == *expected {
                Ok(())
            } else {
                Err(format!("expected ret == {expected}, got {actual}"))
            }
        }
        Assertion::ArgIntEq(idx, expected) => {
            let buf = prepared
                .buffers
                .iter()
                .find(|b| b.arg_index == *idx)
                .ok_or_else(|| format!("arg{idx} is not an inspectable pointer"))?;
            let actual = match buf.kind {
                BufferKind::OutBool => sim.mem.read_bytes(buf.addr, 1).unwrap_or(0) as i64,
                BufferKind::OutI32 => {
                    sim.mem.read_bytes(buf.addr, 4).unwrap_or(0) as u32 as i32 as i64
                }
                BufferKind::OutI64 => sim.mem.read_bytes(buf.addr, 8).unwrap_or(0) as i64,
                _ => return Err(format!("arg{idx} is not a scalar out-parameter")),
            };
            if actual == *expected {
                Ok(())
            } else {
                Err(format!("expected arg{idx} == {expected}, got {actual}"))
            }
        }
        Assertion::ArgArrayI32Eq(idx, expected) => {
            let buf = prepared
                .buffers
                .iter()
                .find(|b| b.arg_index == *idx)
                .ok_or_else(|| format!("arg{idx} is not an array buffer"))?;
            let BufferKind::ArrayI32(len) = buf.kind else {
                return Err(format!("arg{idx} is not an i32 array"));
            };
            let actual: Vec<i32> = (0..len)
                .map(|i| {
                    sim.mem
                        .read_bytes(buf.addr + (i as u64) * 4, 4)
                        .unwrap_or(0) as u32 as i32
                })
                .collect();
            if &actual == expected {
                Ok(())
            } else {
                Err(format!("expected arg{idx} == {expected:?}, got {actual:?}"))
            }
        }
        Assertion::ArgArrayI64Eq(idx, expected) => {
            let buf = prepared
                .buffers
                .iter()
                .find(|b| b.arg_index == *idx)
                .ok_or_else(|| format!("arg{idx} is not an array buffer"))?;
            let BufferKind::ArrayI64(len) = buf.kind else {
                return Err(format!("arg{idx} is not an i64 array"));
            };
            let actual: Vec<i64> = (0..len)
                .map(|i| {
                    sim.mem
                        .read_bytes(buf.addr + (i as u64) * 8, 8)
                        .unwrap_or(0) as i64
                })
                .collect();
            if &actual == expected {
                Ok(())
            } else {
                Err(format!("expected arg{idx} == {expected:?}, got {actual:?}"))
            }
        }
        Assertion::ArgStringEq(idx, expected) => {
            let buf = prepared
                .buffers
                .iter()
                .find(|b| b.arg_index == *idx)
                .ok_or_else(|| format!("arg{idx} is not a string buffer"))?;
            let actual = read_cstring(sim, buf.addr);
            if &actual == expected {
                Ok(())
            } else {
                Err(format!(
                    "expected arg{idx} == \"{expected}\", got \"{actual}\""
                ))
            }
        }
    }
}

/// Assemble `source` and execute every `@test` directive found in `extra_tests`
/// (or in `source` when `extra_tests` is `None`).
pub fn run_test_suite(
    source: &str,
    extra_tests: Option<&str>,
    dialect: Option<AssemblyDialect>,
    timeout_ms: u64,
) -> Result<TestSuiteReport, String> {
    let test_text = extra_tests.unwrap_or(source);
    let parsed = parse_test_directives(test_text);
    if parsed.is_empty() {
        return Err("no @test directives found (add `# @test func(...) == expected`)".to_string());
    }

    let resolved_dialect = dialect.unwrap_or_else(|| rvs_transpile::detect_dialect(source));
    let mut results = Vec::with_capacity(parsed.len());
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut formatted = String::new();
    let mut record = |line: usize, raw: String, ok: bool, detail: String, steps: u64| {
        if ok {
            passed += 1;
            formatted.push_str(&format!("PASS [L{line}] {raw} ({detail})\n"));
        } else {
            failed += 1;
            formatted.push_str(&format!("FAIL [L{line}] {raw} -> {detail}\n"));
        }
        results.push(TestCaseOutcome {
            line,
            raw,
            passed: ok,
            detail,
            steps,
        });
    };

    for entry in parsed {
        let tc = match entry {
            Ok(tc) => tc,
            Err(err) => {
                record(0, err.clone(), false, err, 0);
                continue;
            }
        };

        let mut sim = Simulator::new();
        sim.dialect = resolved_dialect;
        sim.trace = false;
        if let Err(err) = rvs_assembler::assemble(&mut sim, source) {
            return Err(format!("assembly failed: {}", err.message));
        }

        let prepared = match prepare_call(&mut sim, &tc.call) {
            Ok(p) => p,
            Err(err) => {
                record(tc.line, tc.raw, false, err, 0);
                continue;
            }
        };

        let status = rvs_executor::run(&mut sim, timeout_ms);
        let steps = sim.metrics.instructions;
        let failure = if status == rvs_executor::RunStatus::TimedOut {
            Some(format!("timed out after {timeout_ms}ms"))
        } else if let Some(err) = &sim.last_error {
            Some(format!("runtime error: {err}"))
        } else {
            let errs: Vec<String> = tc
                .assertions
                .iter()
                .filter_map(|a| check_assertion(&sim, &prepared, a).err())
                .collect();
            (!errs.is_empty()).then(|| errs.join("; "))
        };

        match failure {
            None => record(tc.line, tc.raw, true, format!("{steps} step(s)"), steps),
            Some(detail) => record(tc.line, tc.raw, false, detail, steps),
        }
    }

    formatted.push_str(&format!(
        "Summary: {passed}/{} passed ({failed} failed)\n",
        passed + failed
    ));
    Ok(TestSuiteReport {
        results,
        passed,
        failed,
        formatted,
    })
}
