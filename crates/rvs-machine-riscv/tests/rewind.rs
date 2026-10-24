//! Reverse stepping must restore registers, memory and the trace exactly.
//!
//! Snapshots are deltas (a register copy + a mark into the memory undo
//! journal), so these tests pin the equivalence with a fresh forward run.

use rvs_machine::{LoadOutcome, Machine};
use rvs_machine_riscv::RiscV;

/// A loop that stores one cell per iteration, so history contains both
/// register and memory changes.
const PROGRAM: &str = "\
        addi x5, x0, 0
        addi x4, x0, 0x100
loop:   addi x5, x5, 1
        sd   x5, 0(x4)
        addi x4, x4, 8
        addi x6, x0, 0x100
        blt  x5, x6, loop
        ebreak x0, x0, 0
";

fn loaded() -> RiscV {
    let mut m = RiscV::new();
    assert!(matches!(m.load(PROGRAM), LoadOutcome::Ok));
    m
}

fn fingerprint(m: &RiscV) -> (Vec<u64>, Vec<(u64, u64)>, usize, u64) {
    let sim = m.sim();
    let x: Vec<u64> = (0..32).map(|i| sim.regs.read_x(i)).collect();
    let mem: Vec<(u64, u64)> = sim
        .mem
        .cells
        .iter()
        .filter_map(|(a, c)| match c {
            rvs_memory::MemoryCell::Data { value, .. } => Some((*a, *value)),
            _ => None,
        })
        .collect();
    (x, mem, sim.exec_log.len(), sim.regs.pc)
}

#[test]
fn rewind_matches_a_fresh_run() {
    let mut m = loaded();
    let mut marks = Vec::new();
    for _ in 0..6 {
        marks.push(m.snapshot());
        m.step();
    }
    let after_six = fingerprint(&m);

    // Rewind to the third snapshot, then re-execute the same three steps.
    m.restore(&marks[2].clone());
    let mut reference = loaded();
    for _ in 0..2 {
        reference.step();
    }
    assert_eq!(
        fingerprint(&m),
        fingerprint(&reference),
        "restore must equal a fresh run of the same length"
    );

    for _ in 0..4 {
        m.step();
    }
    assert_eq!(
        fingerprint(&m),
        after_six,
        "replay must land on the same state"
    );
}

#[test]
fn rewind_past_a_memory_write_removes_the_cell() {
    let mut m = loaded();
    let before = m.snapshot();
    // addi, addi, addi (loop), sd
    for _ in 0..4 {
        m.step();
    }
    let data_cells = |m: &RiscV| {
        m.sim()
            .mem
            .cells
            .values()
            .filter(|c| matches!(c, rvs_memory::MemoryCell::Data { .. }))
            .count()
    };
    assert!(
        data_cells(&m) > 0,
        "the program should have stored something"
    );
    m.restore(&before);
    assert_eq!(
        data_cells(&m),
        0,
        "stores executed after the snapshot must be forgotten"
    );
    assert!(m.sim().exec_log.contains("START") || m.sim().exec_log.is_empty());
}

#[test]
fn dropping_snapshots_keeps_later_rewinds_valid() {
    let mut m = loaded();
    let mut live = vec![m.snapshot()];
    // 200 steps of history with only the last 128 kept, as the TUI does.
    for _ in 0..200 {
        m.step();
        live.push(m.snapshot());
        if live.len() > 128 {
            let dropped = live.remove(0);
            m.forget(&dropped);
        }
    }
    let oldest = live[0].clone();
    let steps_taken = 200 + 1 - live.len(); // snapshots live are "before step N"
    m.restore(&oldest);

    let mut reference = loaded();
    for _ in 0..steps_taken {
        reference.step();
    }
    assert_eq!(fingerprint(&m), fingerprint(&reference));
}
