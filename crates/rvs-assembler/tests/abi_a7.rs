//! The `a7` syscall convention (`--abi a7` / `:abi a7`).
//!
//! This is what makes a GNU-flavoured program behave like one expects: the
//! selector lives in `a7`, arguments in `a0`/`a1`.

use rvs_assembler::assemble;
use rvs_executor::{run, RunStatus};
use rvs_sim::{EcallAbi, Simulator};

fn run_a7(src: &str, input: &str) -> Simulator {
    let mut sim = Simulator::new();
    sim.ecall_abi = EcallAbi::A7;
    sim.input_lines = Some(input.split('\n').map(|s| s.to_string()).collect());
    assemble(&mut sim, src).expect("assemble");
    let status = run(&mut sim, 2000);
    assert_eq!(status, RunStatus::Completed, "program should halt cleanly");
    sim
}

#[test]
fn prints_int_string_char_and_exits() {
    let src = "\
        addi x10, x0, 42
        addi x17, x0, 1
        ecall
        addi x10, x0, 'A'
        addi x17, x0, 11
        ecall
        addi x17, x0, 10
        ecall
        addi x10, x0, 1
        addi x17, x0, 1
        ecall
";
    let sim = run_a7(src, "");
    assert_eq!(sim.output, "42A");
    assert!(sim.exit_requested);
    assert_eq!(sim.last_error, None);
}

#[test]
fn reads_an_integer() {
    let src = "\
        addi x17, x0, 5
        ecall
        addi x17, x0, 1
        ecall
        addi x17, x0, 10
        ecall
";
    let sim = run_a7(src, "1234");
    assert_eq!(sim.output, "1234");
    assert_eq!(sim.regs.read_x(10), 1234);
}

#[test]
fn sbrk_hands_out_increasing_addresses() {
    let src = "\
        addi x10, x0, 64
        addi x17, x0, 9
        ecall
        addi x18, x10, 0
        addi x10, x0, 32
        addi x17, x0, 9
        ecall
        sub  x19, x10, x18
        addi x17, x0, 10
        ecall
";
    let sim = run_a7(src, "");
    assert_eq!(sim.regs.read_x(19), 64, "second break = first + 64");
}

#[test]
fn unknown_syscall_is_reported_not_fatal() {
    let mut sim = Simulator::new();
    sim.ecall_abi = EcallAbi::A7;
    assemble(&mut sim, "addi x17, x0, 99\necall\nebreak x0, x0, 0\n").expect("assemble");
    run(&mut sim, 100);
    let err = sim.last_error.clone().unwrap_or_default();
    assert!(err.contains("unknown syscall 99"), "got {err:?}");
}

#[test]
fn rvs_abi_is_unchanged_by_default() {
    let mut sim = Simulator::new();
    assert_eq!(sim.ecall_abi, EcallAbi::Rvs);
    assemble(
        &mut sim,
        "addi x1, x0, 7\necall x0, x1, 0\nebreak x0, x0, 0\n",
    )
    .expect("assemble");
    run(&mut sim, 100);
    assert_eq!(sim.output, "7\n");
}
