//! End-to-end tests modelling the EECS 2021 Lab S1 & Lab E exercises:
//! - Exercise 1: `reverse(int32_t *arr, int totalBytes)` using the stack
//! - Exercise 2 (Section 4): `bigcall(...)` with `.struct 0`, `.space`, `.balign`, `sz = . - svfp`,
//!   and 9 mixed `int32_t`/`int64_t` arguments across `a0..a7` + frame pointer `fp`
//! - Exercise 3 (Section 5): `strlen32(char const *s)`
//! - Exercise 4 (Section 6): `struint(char const *text, bool *ok)` with char literals (`'0'`, `'9'`, `' '`)
//!   and `bool *ok` out-parameter
//! - Lab E: `check_parity` with `.section .data` + `.word` + `jalr zero, 0(ra)`

use rvs_harness::{parse_call_spec, run_single_call, run_test_suite};
use rvs_sim::Simulator;
use rvs_transpile::AssemblyDialect;

const LAB_S1_REVERSE: &str = r#"
# @test reverse([10, 34, 21, 8, 50], 20) -> arg0 == [50, 8, 21, 34, 10]
# @test reverse([1, 2, 3], 12) -> arg0 == [3, 2, 1]

    .global reverse
    .text
reverse:
    addi sp, sp, -20
    addi x30, x0, 0
    add  x31, x0, sp
push_stack:
    add  x12, a0, x30
    lw   x4, 0(x12)
    sw   x4, 0(x31)
    addi x30, x30, 4
    addi x31, x31, 4
    bne  a1, x30, push_stack
    addi x30, x0, 0
pop_stack:
    addi x31, x31, -4
    add  x12, a0, x30
    lw   x4, 0(x31)
    sw   x4, 0(x12)
    addi x30, x30, 4
    bne  x31, sp, pop_stack
exit:
    addi sp, sp, 20
    ret
"#;

const LAB_S1_BIGCALL: &str = r#"
# @test bigcall(101, 102i64, 103, 104i64, 105, 106i64, 107, 108i64, 109) == 945

    .global bigcall

    .struct 0
svfp:   .space 4        # Save register fp/s0
svra:   .space 4        # Save register ra
        .balign 8       # Make sz mod 8
sz      =       . - svfp

    .text
bigcall:
    addi sp, sp, -sz    # Set sp for stack frame
    sw   fp, svfp(sp)   # Save fp/s0
    sw   ra, svra(sp)   # Save ra
    addi fp, sp, +sz    # Set fp = original sp

    add  a0, a0, a1     # + two (lo)
    add  a0, a0, a3     # + three
    add  a0, a0, a4     # + four (lo)
    add  a0, a0, a6     # + five
    add  a0, a0, a7     # + six (lo)
    lw   t0, 4(fp)      # seven
    add  a0, a0, t0
    lw   t0, 8(fp)      # eight (lo)
    add  a0, a0, t0
    lw   t0, 16(fp)     # nine
    add  a0, a0, t0

    lw   ra, svra(sp)   # Restore ra
    lw   fp, svfp(sp)   # Restore fp/s0
    addi sp, sp, +sz    # Restore sp
    ret
"#;

const LAB_S1_STRLEN32: &str = r#"
# @test strlen32("One") == 3
# @test strlen32("Three") == 5
# @test strlen32("Ten four") == 8
# @test strlen32("") == 0
# @test strlen32("This is the end!") == 16

    .global strlen32
    .text
strlen32:
    add  t0, x0, a0     # t0 = char const *ptr
    addi a0, x0, 0      # Zero strlen
loop:
    lb   t1, 0(t0)      # t1 = *ptr
    beqz t1, end        # Branch if Null byte
    addi a0, a0, 1
    addi t0, t0, 1      # ++ptr
    j    loop
end:
    ret
"#;

const LAB_S1_STRUINT: &str = r#"
# @test struint("1234", &bool) == 1234, arg1 == 1
# @test struint(" 123", &bool) == 123, arg1 == 1
# @test struint("0234", &bool) == 234, arg1 == 1
# @test struint("0", &bool) == 0, arg1 == 1
# @test struint("905", &bool) == 905, arg1 == 1
# @test struint("9_05", &bool) == 9, arg1 == 0
# @test struint(" 907, ", &bool) == 907, arg1 == 0
# @test struint("   ", &bool) == 0, arg1 == 0

    .global struint
    .text
struint:
    add  t6, x0, a0     # t6 = ptr to test
    addi a0, x0, 0      # Accumulator for uint
    addi t2, x0, 0      # Digit count
    addi t4, x0, '0'
    addi t3, x0, '9'
    addi t1, x0, ' '
    addi t0, x0, 10

loop:
    lb   t5, 0(t6)
    beqz t5, nulbyt
    beq  t5, t1, skip
    blt  t5, t4, fail
    bgt  t5, t3, fail
    andi t5, t5, 0x0F
    mul  a0, a0, t0
    add  a0, a0, t5
    addi t2, t2, 1      # Bump digit count
skip:
    addi t6, t6, 1      # ++text ptr
    j    loop

nulbyt:
    beqz t2, fail       # Fail if no digits
    li   t0, 1
    sb   t0, 0(a1)      # ok = true
    ret

fail:
    li   t0, 0
exit:
    sb   t0, 0(a1)      # ok = false
    ret
"#;

#[test]
fn lab_s1_reverse_test_suite_passes() {
    let report =
        run_test_suite(LAB_S1_REVERSE, None, None, 2000, rvs_regs::Xlen::X64).expect("suite");
    assert_eq!(report.failed, 0, "failures:\n{}", report.formatted);
    assert_eq!(report.passed, 2);
}

#[test]
fn lab_s1_bigcall_struct_and_rv32_stack_args_pass() {
    let report =
        run_test_suite(LAB_S1_BIGCALL, None, None, 2000, rvs_regs::Xlen::X64).expect("suite");
    assert_eq!(report.failed, 0, "failures:\n{}", report.formatted);
    assert_eq!(report.passed, 1);
}

#[test]
fn lab_s1_strlen32_test_suite_passes() {
    let report =
        run_test_suite(LAB_S1_STRLEN32, None, None, 2000, rvs_regs::Xlen::X64).expect("suite");
    assert_eq!(report.failed, 0, "failures:\n{}", report.formatted);
    assert_eq!(report.passed, 5);
}

#[test]
fn lab_s1_struint_test_suite_passes() {
    let report =
        run_test_suite(LAB_S1_STRUINT, None, None, 2000, rvs_regs::Xlen::X64).expect("suite");
    assert_eq!(report.failed, 0, "failures:\n{}", report.formatted);
    assert_eq!(report.passed, 8);
}

#[test]
fn single_call_formats_mutated_array_and_out_bool() {
    let mut sim = Simulator::new();
    sim.dialect = AssemblyDialect::Gnu;
    rvs_assembler::assemble(&mut sim, LAB_S1_REVERSE).expect("assemble");
    let spec = parse_call_spec("reverse([10, 34, 21, 8, 50], 20)").expect("parse");
    let out = run_single_call(&mut sim, &spec, 2000).expect("run");
    assert!(
        out.contains("[50, 8, 21, 34, 10]"),
        "expected reversed array in output, got:\n{out}"
    );
}

#[test]
fn lab_e_check_parity_with_data_section_and_jalr_ret() {
    let src = r#"
# @test check_parity() == 0
.section .data
.align 2
number: .word 0xA5A5A5A5
result: .word 0

.section .text
.global check_parity
check_parity:
    la t0, number
    lw t1, 0(t0)
    addi t2, zero, 0
parity_loop:
    beq t1, zero, parity_done
    andi t3, t1, 1
    xor t2, t2, t3
    srli t1, t1, 1
    jal zero, parity_loop
parity_done:
    la t0, result
    sw t2, 0(t0)
    addi a0, t2, 0
    jalr zero, 0(ra)
"#;
    let report = run_test_suite(src, None, None, 2000, rvs_regs::Xlen::X64).expect("suite");
    assert_eq!(report.failed, 0, "failures:\n{}", report.formatted);
    assert_eq!(report.passed, 1);
}
