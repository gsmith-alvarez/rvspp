//! Transpiler totality: any text in, some text out, no panic, no growth.
//!
//! The transpiler runs on every keystroke-adjacent action in the TUI
//! (`:transpile`, dialect switch on load), so it must stay total on garbage.

use proptest::prelude::*;
use rvs_transpile::{detect_dialect, transpile_gnu_to_rvs, transpile_rvs_to_gnu, AssemblyDialect};

#[test]
fn detects_gnu_files() {
    assert_eq!(
        detect_dialect("        .text\nmain: li a0, 1\n"),
        AssemblyDialect::Gnu
    );
    assert_eq!(detect_dialect("main: mv a0, a1\n"), AssemblyDialect::Gnu);
    assert_eq!(detect_dialect(".dword 1, 2\n"), AssemblyDialect::Gnu);
}

#[test]
fn detects_rvs_files() {
    // RVS directives and the bare 3-operand form must not trip the detector.
    assert_eq!(
        detect_dialect("START: DD 1\n        addi x1, x0, 5\n"),
        AssemblyDialect::Rvs
    );
    assert_eq!(
        detect_dialect("        jal x1, sub\n"),
        AssemblyDialect::Rvs
    );
    assert_eq!(detect_dialect(""), AssemblyDialect::Rvs);
}

#[test]
fn gnu_to_rvs_is_idempotent_on_a_realistic_file() {
    let src = "\
        .globl main
        .text
main:   li   a0, 5
        mv   a1, a0
        call fact
        ret
";
    let once = transpile_gnu_to_rvs(src);
    assert_eq!(transpile_gnu_to_rvs(&once), once);
}

proptest! {
    #[test]
    fn transpile_never_panics_and_keeps_line_count(text in "\\PC{0,200}") {
        let to_rvs = transpile_gnu_to_rvs(&text);
        let to_gnu = transpile_rvs_to_gnu(&text);
        // Line count is preserved in both directions, which is what lets the
        // TUI map listing addresses back to source lines.
        prop_assert_eq!(to_rvs.lines().count(), text.lines().count());
        prop_assert_eq!(to_gnu.lines().count(), text.lines().count());
    }

    #[test]
    fn gnu_to_rvs_stabilises_after_one_pass(lines in prop::collection::vec("\\PC{0,40}", 0..12)) {
        let text = lines.join("\n");
        let once = transpile_gnu_to_rvs(&text);
        let twice = transpile_gnu_to_rvs(&once);
        prop_assert_eq!(once, twice);
    }
}
