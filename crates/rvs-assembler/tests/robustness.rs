//! Malformed input must never panic.
//!
//! Every case here used to abort the process (`index out of bounds`) somewhere
//! between `dump()` and `step()`. The contract is now: assembly stays total,
//! an unencodable instruction carries a diagnostic, and the run reports it.

use rvs_assembler::assemble;
use rvs_executor::{run, step, StepResult};
use rvs_sim::Simulator;

const MALFORMED: &[&str] = &[
    "addi x1",
    "add",
    "addi x1, x0, 4096",
    "addi x1, x0, 1000000",
    "lui x1",
    "jal x1",
    "ld x1, 0(x2",
    "addi x1, f2, 1",
    "blt x1, x2, -8",
    "beq x1, x2, 7",
    "j x1",
    "jr x1",
    "foo x1, x2, x3",
    "ecall",
    "ebreak",
    ".dd 1/0",
    "dc \"abc",
    "dm -1",
    "DW 1,",
    "addi x1, x0, 3\naddi x1, x0, 4",
    "fsqrt.d f1",
    "fsqrt.d f1, f2, f3",
    "fadd.d f1, f2",
    "mulh x1, x2",
];

#[test]
fn malformed_source_never_panics() {
    for src in MALFORMED {
        let mut sim = Simulator::new();
        // A compile error is fine; a panic is not.
        if assemble(&mut sim, src).is_ok() {
            sim.exec_log.push_str("START\n");
            let mut steps = 0;
            while steps < 64 {
                match step(&mut sim) {
                    StepResult::Continued => steps += 1,
                    _ => break,
                }
            }
        }
    }
}

#[test]
fn out_of_range_immediate_is_reported_not_executed() {
    let mut sim = Simulator::new();
    assemble(&mut sim, "addi x1, x0, 4096\nebreak x0, x0, 0\n").expect("assemble");
    assert_eq!(sim.regs.read_x(1), 0);
    sim.exec_log.push_str("START\n");
    run(&mut sim, 1000);
    // The instruction never ran: x1 keeps its old value and the trace says why.
    assert_eq!(
        sim.regs.read_x(1),
        0,
        "unencodable instruction was executed"
    );
    assert!(
        sim.exec_log.contains("OUT OF RANGE"),
        "trace should carry the encoder diagnostic, got:\n{}",
        sim.exec_log
    );
}

#[test]
fn good_program_still_runs() {
    let mut sim = Simulator::new();
    assemble(
        &mut sim,
        "addi x1, x0, 41\naddi x1, x1, 1\nebreak x0, x0, 0\n",
    )
    .expect("assemble");
    run(&mut sim, 1000);
    assert_eq!(sim.regs.read_x(1), 42);
}

#[test]
fn negative_doubleword_load_round_trips() {
    // `ld` of a value with bit 63 set used to overflow a shift mask in debug
    // builds.
    let mut sim = Simulator::new();
    let src = "\
        addi x3, x0, 0x100\n\
        addi x1, x0, -5\n\
        sd   x1, 0(x3)\n\
        ld   x2, 0(x3)\n\
        ebreak x0, x0, 0\n";
    assemble(&mut sim, src).expect("assemble");
    run(&mut sim, 1000);
    assert_eq!(sim.regs.read_x(2), u64::MAX - 4);
}

#[test]
fn rv32_availability_and_placement() {
    use rvs_regs::Xlen;
    for op in [
        "ld", "sd", "lwu", "addiw", "slliw", "srliw", "sraiw", "addw",
        "subw", "sllw", "srlw", "sraw", "mulw", "divw", "divuw", "remw",
        "remuw", "fcvt.l.d", "fcvt.lu.d", "fcvt.d.l", "fcvt.d.lu",
        "fmv.x.d", "fmv.d.x",
    ] {
        let mut sim = Simulator::new();
        sim.regs.set_xlen(Xlen::X32);
        let listing = assemble(&mut sim, &format!("{op} x1,x2,x3")).unwrap();
        assert!(listing.contains("instruction unavailable in RV32"), "{op}");
        assert!(matches!(step(&mut sim), StepResult::Halted), "{op}");
        assert!(sim.exec_log.contains("instruction unavailable in RV32"), "{op}");
        assert_eq!(sim.regs.read_x(1), 0);
    }
    for source in ["org 0x100000000", "org -1", "org 0xfffffffc\nDD 1", "org 0xffffffff\nadd x1,x2,x3"] {
        let mut sim = Simulator::new();
        sim.regs.set_xlen(Xlen::X32);
        assert!(assemble(&mut sim, source).is_err(), "{source}");
    }
    for (source, valid) in [("slli x1,x2,31", true), ("slli x1,x2,32", false), ("fld f1,0(x2)", true), ("fsd f1,0(x2)", true)] {
        let mut sim = Simulator::new();
        sim.regs.set_xlen(Xlen::X32);
        let listing = assemble(&mut sim, source).unwrap();
        assert_eq!(!listing.contains("ERROR:"), valid, "{source}: {listing}");
    }
}

#[test]
fn rv32_gnu_li_loads_full_width_literals() {
    for (literal, expected) in [
        ("2048", 2048),
        ("0x80000000", 0x8000_0000),
        ("0xffffffff", 0xffff_ffff),
        ("-2147483648", 0x8000_0000),
        ("-2049", 0xffff_f7ff),
    ] {
        let mut sim = Simulator::new();
        sim.regs.set_xlen(rvs_regs::Xlen::X32);
        sim.dialect = rvs_transpile::AssemblyDialect::Gnu;
        let listing = assemble(&mut sim, &format!("li x1, {literal}")).unwrap();
        assert!(!listing.contains("ERROR:"), "{literal}: {listing}");
        assert!(matches!(step(&mut sim), StepResult::Continued));
        assert!(matches!(step(&mut sim), StepResult::Continued));
        assert_eq!(sim.regs.read_x(1), expected, "{literal}");
    }
    let mut sim = Simulator::new();
    sim.regs.set_xlen(rvs_regs::Xlen::X32);
    sim.dialect = rvs_transpile::AssemblyDialect::Gnu;
    assert!(assemble(&mut sim, "li x1, 4294967296").unwrap().contains("ERROR:"));
}
