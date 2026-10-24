use std::collections::VecDeque;

use rvs_core::assembler::assemble;
use rvs_core::executor::{run, step, submit_dma_input, poll_dma_output, poll_dma_input, StepResult};
use rvs_core::Simulator;

#[test]
fn test_rv64i_arithmetic_and_word_ops() {
    let mut sim = Simulator::new();
    let src = r#"
        addi x1, x0, 10
        addi x2, x0, 3
        add  x3, x1, x2       # 13
        sub  x4, x1, x2       # 7
        and  x5, x1, x2       # 10 & 3 = 2
        or   x6, x1, x2       # 10 | 3 = 11
        xor  x7, x1, x2       # 10 ^ 3 = 9
        sll  x8, x1, x2       # 10 << 3 = 80
        srl  x9, x8, x2       # 80 >> 3 = 10
        subi x10, x1, 4       # 6
        addw x11, x1, x2      # 13
        addiw x12, x1, -2     # 8
        sllw x13, x1, x2      # 80
        srlw x14, x13, x2     # 10
        ebreak x0, x0, 0
    "#;
    assemble(&mut sim, src).expect("assemble");
    sim.exec_log.push_str("START\n");
    run(&mut sim, 1000);

    assert_eq!(sim.regs.read_x(3), 13);
    assert_eq!(sim.regs.read_x(4), 7);
    assert_eq!(sim.regs.read_x(5), 2);
    assert_eq!(sim.regs.read_x(6), 11);
    assert_eq!(sim.regs.read_x(7), 9);
    assert_eq!(sim.regs.read_x(8), 80);
    assert_eq!(sim.regs.read_x(9), 10);
    assert_eq!(sim.regs.read_x(10), 6);
    assert_eq!(sim.regs.read_x(11), 13);
    assert_eq!(sim.regs.read_x(12), 8);
    assert_eq!(sim.regs.read_x(13), 80);
    assert_eq!(sim.regs.read_x(14), 10);
}

#[test]
fn test_rv64m_multiplication_division() {
    let mut sim = Simulator::new();
    let src = r#"
        addi x1, x0, -7
        addi x2, x0, 3
        mul  x3, x1, x2       # -21
        div  x4, x1, x2       # -3 (Tcl floor)
        rem  x5, x1, x2       # 2 (sign of divisor)
        mulh x6, x1, x2       # high 64 of -21 (sign extended)
        divu x7, x1, x2       # (2^64-7)/3
        mulw x8, x1, x2       # -21 sign-extended 32-bit
        divw x9, x1, x2       # -2
        remw x10, x1, x2      # -1
        ebreak x0, x0, 0
    "#;
    assemble(&mut sim, src).expect("assemble");
    sim.exec_log.push_str("START\n");
    run(&mut sim, 1000);

    assert_eq!(sim.regs.read_x(3) as i64, -21);
    assert_eq!(sim.regs.read_x(4) as i64, -3);
    assert_eq!(sim.regs.read_x(5) as i64, 2);
    assert_eq!(sim.regs.read_x(6) as i64, -1);
    assert_eq!(sim.regs.read_x(8) as i64, -21);
    assert_eq!(sim.regs.read_x(9) as i64, -2);
    assert_eq!(sim.regs.read_x(10) as i64, -1);
}

#[test]
fn test_rv64d_floating_point() {
    let mut sim = Simulator::new();
    let src = r#"
        fld f1, flt1(x0)      # 3.5
        fld f2, flt2(x0)      # 1.5
        fadd.d f3, f1, f2     # 5.0
        fsub.d f4, f1, f2     # 2.0
        fmul.d f5, f1, f2     # 5.25
        fdiv.d f6, f1, f2     # 2.3333333333333335
        fmin.d f7, f1, f2     # 1.5
        fmax.d f8, f1, f2     # 3.5
        feq.d  x1, f1, f2     # 0
        flt.d  x2, f2, f1     # 1
        fle.d  x3, f1, f1     # 1
        fcvt.l.d x4, f1       # 4 (rounded 3.5)
        fcvt.d.l f9, x4       # 4.0
        fmv.x.d x5, f1        # bits
        fmv.d.x f10, x5       # bits restored
        ebreak x0, x0, 0
        ORG 128
flt1:   DF 3.5
flt2:   DF 1.5
    "#;
    assemble(&mut sim, src).expect("assemble");
    sim.exec_log.push_str("START\n");
    run(&mut sim, 1000);

    assert_eq!(sim.regs.read_f64(3), 5.0);
    assert_eq!(sim.regs.read_f64(4), 2.0);
    assert_eq!(sim.regs.read_f64(5), 5.25);
    assert_eq!(sim.regs.read_f64(7), 1.5);
    assert_eq!(sim.regs.read_f64(8), 3.5);
    assert_eq!(sim.regs.read_x(1), 0);
    assert_eq!(sim.regs.read_x(2), 1);
    assert_eq!(sim.regs.read_x(3), 1);
    assert_eq!(sim.regs.read_x(4), 4);
    assert_eq!(sim.regs.read_f64(9), 4.0);
    assert_eq!(sim.regs.read_f64(10), 3.5);
}

#[test]
fn test_ecall_syscalls_and_io() {
    let mut sim = Simulator::new();
    sim.input_lines = Some(VecDeque::from(vec![
        "99".to_string(),
        "3.1415".to_string(),
        "0x1234".to_string(),
        "world".to_string(),
    ]));

    let src = r#"
        # prompt and read integer
        ld x1, p1(x0)
        ecall x2, x1, 5       # input int -> x2
        ecall x0, x2, 0       # print int with newline
        # prompt and read float
        ld x3, p2(x0)
        ecall f1, x3, 6       # input float -> f1
        ecall x0, f1, 1       # print float
        # prompt and read hex
        ecall x4, x1, 7       # input hex
        ecall x0, x4, 2       # print hex
        # prompt and read string to memory
        addi x5, x0, 256
        ecall x5, x1, 9       # input string to 256
        ecall x0, x5, 4       # print string at 256
        ebreak x0, x0, 0
        ORG 128
p1:     DC "int: "
p2:     DC "flt: "
        ORG 256
buf:    DM 4
    "#;
    assemble(&mut sim, src).expect("assemble");
    sim.exec_log.push_str("START\n");
    run(&mut sim, 1000);

    assert_eq!(sim.regs.read_x(2), 99);
    assert_eq!(sim.regs.read_f64(1), 3.1415);
    assert_eq!(sim.regs.read_x(4), 0x1234);
    assert!(sim.output.contains("int: 99\n99\n"));
    assert!(sim.output.contains("3.1415\n"));
    assert!(sim.output.contains("0x0000000000001234\n"));
    assert!(sim.output.contains("world\n"));
}

#[test]
fn test_dma_memory_mapped_io() {
    let mut sim = Simulator::new();
    sim.view.dma_out = true;
    sim.view.dma_inp = true;

    let src = r#"
        ORG 0
OUTC:   DD 0
OUTD:   DD 0
INPC:   DD 0
INPD:   DD 0
        ORG 64
start:  # DMA output of integer 123
        addi x1, x0, 123
        sd   x1, OUTD(x0)
        addi x2, x0, 0x49     # 'I'
        sb   x2, OUTC(x0)
        # DMA input request for integer
        sb   x2, INPC(x0)
        ebreak x0, x0, 0
    "#;
    assemble(&mut sim, src).expect("assemble");
    sim.exec_log.push_str("START\n");

    // Step until ebreak
    while let StepResult::Continued = step(&mut sim) {}

    // Poll DMA output
    poll_dma_output(&mut sim);
    assert!(sim.output.contains("123\n"));
    assert_eq!(sim.mem.read_bytes(0, 8).unwrap(), 0); // OUTC cleared to 0

    // Poll DMA input
    poll_dma_input(&mut sim);
    assert_eq!(sim.entry_type, "I");
    assert!(sim.output.contains("\nI?"));

    // Submit DMA input
    submit_dma_input(&mut sim, "789").expect("submit");
    assert_eq!(sim.mem.read_bytes(0x18, 8).unwrap(), 789); // INPD holds 789
    assert_eq!(sim.mem.read_bytes(0x10, 8).unwrap(), 0);   // INPC cleared to 0
    assert!(sim.output.contains("789\n"));
}
