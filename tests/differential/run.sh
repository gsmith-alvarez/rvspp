#!/usr/bin/env bash
# Differential harness: run every tests/differential/t[1-5].asm through the
# Tcl reference (xvfb) and the Rust rvs binary, then diff the reports.
# t6 exercises the rewrite's new instructions (expected to differ) and is
# only smoke-run.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
fail=0
for name in t1 t2 t3 t4 t5 t7 t8 t9; do
  asm="tests/differential/$name.asm"
  base="tests/differential/$name"
  nix-shell -p tk xvfb-run --run "xvfb-run -a tclsh tests/differential/driver.tcl $asm $base.in.txt /tmp/$name.tcl.out 3000" >/dev/null 2>&1
  cargo run -q --bin rvs -- "$asm" "$base.in.txt" "/tmp/$name.rust.out" 3000 || { echo "$name: RUST-FAIL"; fail=1; continue; }
  if diff -q "/tmp/$name.tcl.out" "/tmp/$name.rust.out" >/dev/null 2>&1; then
    echo "$name: OK"
  else
    echo "$name: DIFF ($(diff "/tmp/$name.tcl.out" "/tmp/$name.rust.out" | grep -c '^[<>]') lines)"
    fail=1
  fi
done
cargo run -q --bin rvs -- tests/differential/t6.asm tests/differential/t6.in.txt /tmp/t6.rust.out 3000 \
  && echo "t6: SMOKE-OK" || { echo "t6: SMOKE-FAIL"; fail=1; }
exit $fail
