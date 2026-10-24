use rvs_assembler::{assemble, dump};
use rvs_sim::Simulator;

/// `dump` rewrites instruction cells into their normalized (Tcl-facing)
/// operand form. Re-dumping must re-use the operands as written, otherwise
/// every label-referencing branch/jump reports `ERROR: imm NOT WORD-ALIGNED`
/// on the second pass.
#[test]
fn dump_is_idempotent_for_label_branches() {
    let mut sim = Simulator::new();
    let src = "\
start:  addi x1, x0, 1
        jal  x0, done
        addi x2, x0, 2
done:   bge  x1, x2, start
        ebreak x0, x0, 0
";
    assemble(&mut sim, src).expect("assembles");

    let first = dump(&mut sim);
    let second = dump(&mut sim);

    assert_eq!(first, second);
    assert!(!second.contains("ERROR"), "unexpected error in listing:\n{second}");
}

#[test]
fn label_branch_encodings_survive_repeated_dumps() {
    let mut sim = Simulator::new();
    let src = "\
start:  addi x1, x0, 1
        jal  x0, done
        addi x2, x0, 2
done:   bge  x1, x2, start
        ebreak x0, x0, 0
";
    assemble(&mut sim, src).expect("assembles");
    let first = dump(&mut sim);
    let second = dump(&mut sim);

    let bin_of = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|l| l.starts_with("0x"))
            .map(|l| l.split_whitespace().take(2).collect::<Vec<_>>().join(" "))
            .collect()
    };
    assert_eq!(bin_of(&first), bin_of(&second));
    // jal x0, done -> the forward jump must stay a resolved UJ encoding.
    assert!(second.contains("UJ 00000000010000000000 00000 1101111"));
}
