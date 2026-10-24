# RVS++ Roadmap & Development Changelog

A plain-English breakdown of what we've built, why each feature exists, and where RVS++ is headed.

---

## The Backstory

The full "why" lives in the README under "Why We Built RVS++". Short version: the 2019 Tcl/Tk RVS simulator had a proprietary dialect, clunky UI, and painful local setup, so this project rebuilt it in pure Rust with modern ergonomics and 100% autograder parity.

---

## What We've Built (v1.0.0)

### 1. Pure-Rust Core & Autograder Compatibility
- **No runtime dependencies**: Replaced all Tcl/Tk requirements with a standalone Rust application that compiles into a single binary (no Tcl, Tk, X11, Java or browser).
- **Autograder parity**: Headless batch mode (`rvs --batch <asm> <in> <out>`) produces reports that match the original `kkbatch` harness byte-for-byte on the bundled conformance fixtures (t1–t5, t7–t9; t6 exercises added instructions and is smoke-tested only). `tests/differential/run.sh` diffs against the Tcl reference.
- **GNU files are gradeable too**: `--dialect auto|rvs|gnu` picks the dialect for batch; `auto` detects it from directives and pseudo-instructions.
- **Works on the platforms we test**: Linux and macOS, via Cargo or Nix Flakes (`nix run .`). Windows is WSL only; the native Windows path is untested.

### 1a. Hardening (v1.0.1)
- **No panic on bad input**: an unencodable instruction (bad register, out-of-range immediate, wrong operand count) is reported in the listing and halts the run with `ERROR: <diagnostic>` instead of aborting the process. Covered by `crates/rvs-assembler/tests/robustness.rs`.
- **Crash containment in the TUI**: a panic hook restores the terminal and the event loop catches unwinds per iteration, writing the editor buffer to `$XDG_DATA_HOME/rvs/recovery-<ts>.s`.
- **Assembler diagnostics fixed**: the out-of-range immediate message was missing its comma (`[-20484095]`); `fsqrt.d`/`fcvt`/`fmv` (2-operand F2 forms) no longer fail encoding.
- **Unicode-safe transpiler**: lines starting with multi-byte whitespace (U+00A0, U+2003) no longer panic on a char/byte boundary slice.
- **`ld` of negative values** no longer overflows a shift mask in debug builds.

### 1b. Debugger depth (v1.1.0)
- **Step over / step out**: `:next` runs a call and stops at its return address; `:finish` runs until the current frame returns. Both reuse the breakpoint machinery (temporary address + removal).
- **Conditional breakpoints**: `:b loop if x1 == 3` — expressions are expanded with live register values, then evaluated with the Tcl `expr` semantics the assembler already uses for operands.
- **Watchpoints**: `:watch sp|addr|label` stops continuous execution when that cell's value changes; `:cwatch` clears them.
- **Search**: `/` prompts for a pattern (`n`/`N` are taken by stepping, so repeat is `Ctrl+N`/`Ctrl+P`), plus `:search <pattern>`. Motions accept counts (`3j`, `4w`, `5x`).
- **Reverse stepping is now O(1)**: snapshots record registers plus a mark into a memory undo journal instead of cloning memory and the trace. 20k stepped instructions went from 2.6 s to 7 ms; 500k steps rewind in ~100 ms. The journal is trimmed as history rolls over, so memory stays bounded.
- **`a7` syscall ABI**: `--abi a7` / `:abi a7` makes `ecall` behave like RARS/GNU (1 print int, 4 print string, 5 read int, 8 read string, 9 sbrk, 10/17/93 exit, 11 print char, 34 print hex), so a real GNU-style program prints and exits instead of running to the timeout.
- **Batch flags that do not touch parity**: `--no-trace` (10M-instruction loop: 12 s / 800 MB → 3 s / 6 KB), `--json PATH` sidecar, `--fail-on-error` (exit 2).
- **CI**: `.github/workflows/ci.yml` runs three blocking jobs — `lint` (rustfmt + clippy `-D warnings`), `test` on Linux and macOS (release-mode build so every crate and binary must link, then the suite), and `parity` (Tcl harness with apt tcl/tk/xvfb instead of Nix). `.github/workflows/release.yml` builds tagged musl/macOS binaries. The tree was reformatted with rustfmt and every clippy warning fixed to make the lint gate meaningful.

### 2. Clean Process Teardown (No Zombie Spinners)
- **The problem**: In early TUI builds, closing the terminal tab or window caused event loops to spin infinitely on EOF (`read(0) == 0`), quietly eating 100% CPU in the background.
- **The fix**: Added pre-event polling (`libc::poll` checking for `POLLHUP | POLLERR`) and registered POSIX signal handlers (`SIGINT`, `SIGTERM`, `SIGHUP`). Now when you close the tab or press `Ctrl+C`, it exits cleanly in under 1 ms.

### 3. Real Vim Modal Editing
- **Full modal engine**: `Normal`, `Insert` (`i`/`a`/`o`), and `Command` (`:`) modes.
- **Vim muscle memory**: Jump with `h/j/k/l`, `w/b`, `0/$`, and `gg/G`. Cut with `dd`, copy with `yy`, paste with `p`, and undo/redo with `u`/`Ctrl+R`.
- **External editor hand-off**: Hit `Enter` while focused on the Source pane, and your code opens right up in Neovim (`nvim`), Nano, or whatever `$EDITOR` you have set. When you save and quit, RVS++ reloads the buffer instantly.
- **Fullscreen zoom**: Press `z` or `Enter` on any pane to maximize it when you need to focus on a large memory dump or trace log.

### 4. Correlated Machine Code Colorizer (`[CLR]`)
- Reading 32-bit binary instruction encodings in raw hexadecimal can give anyone a headache.
- Toggling `[CLR]` (or pressing `2` in the Listing pane) color-codes instruction bitfields and links them directly to their corresponding mnemonic, destination register, source registers, and immediate values in the disassembled text.

### 5. Time-Travel Debugging (Step Back) & Breakpoints
- **Breakpoints (`● `)**: Press `B` on any source line or run `:b <line|addr|label>` to drop a bright red breakpoint marker. Running (`r`) pauses automatically right before executing that instruction.
- **Step Back (`p` / `Prev`)**: Stepped one instruction too far and clobbered your answer? Press `p`, `P`, or `N` to step backward. RVS++ continuously snapshots state into a 128-entry history ring buffer, letting you rewind registers, memory, and logs with zero fuss.
- **Slow-motion animation**: Cycle speeds from `1Hz`, `5Hz`, `20Hz`, to `Max` (`+` / `-`). In slow-mo, continuous run animates instruction-by-instruction across all 6 panes, so you can watch algorithms unfold in real time.
- **Quick memory jumps**: Click `[SP]` to snap memory view right to the active stack, or `[0x0]` to jump back to base data.

### 6. Calling Convention Checker (`:cc`)
- **The problem**: In assembly labs, students routinely lose marks for subtle convention mistakes: forgetting that `$sp` must stay 16-byte aligned, or overwriting saved registers (`s0`–`s11`) inside a function without restoring them before `ret`.
- **The fix**: An active calling convention linter. On every `jal ra`, it verifies 16-byte alignment. On function return (`jalr zero, ra, 0`), it checks that all callee-saved registers match the entry state. Violations flash as clear `WARN [Convention]` warnings in the Exec log.

### 7. Call Stack Visualizer (`:stack`)
- Click `[STACK]` or run `:stack` to bring up a visual hierarchy of all active function calls.
- Shows function names, call sites, frame boundaries, and parses double-word stack slots to tell you exactly where your saved return address (`Saved ra`) and callee-saved registers are stored in memory.

### 8. Standard GNU Mode & Two-Way Transpiler (`:mode`, `:transpile`)
- **Dialect toggle**: Switch between strict educational RVS syntax and GNU *syntactic* style (`:mode gnu`). GNU mode translates standard directives (`.word`, `.dword`, `.asciz`, `.zero`) and 25+ pseudo-instructions (`li`, `mv`, `ret`, `call`, `j`, `nop`, `beqz`, `bgt`, etc.) into the RVS dialect. It does not change the runtime ABI: `ecall` still uses `rd, rs1, imm` (no `a7` dispatch), numeric branch operands are still absolute targets, and there is no libc or linker.
- **Two-way transpiler**: Click `[TRANSPILE]` or run `:transpile rvs` to expand pseudo-ops into canonical 3-operand code for homework submissions, or `:transpile gnu` to tidy canonical code back into clean GNU syntax.

---

## Known Limitations (as of v1.1.0)

These are deliberate or accepted, and they define the project's scope:

- **The dialect is not standard RISC-V.** Branch operands are absolute targets, memory is a sparse map of 8-byte cells, syscalls are `ecall rd, rs1, imm` (0–9) and ignore `a7`. Code written for a real toolchain will assemble and then behave differently.
- **Instruction cells execute from parsed operands**; the encoded 32-bit word in the listing is display-only and never decoded.
- **Unsupported ISA surface**: A (atomics), C (compressed), CSR semantics (`csrrw`/`csrrs`/… encode but are rejected at execution), `fence`/`fence.i`, traps, exceptions, privilege modes, paging, interrupts, timers, multiple harts.
- **No toolchain integration**: no ELF/relocations/linker, no `.include` or macros, no C or DWARF debugging, one file at a time.
- **GNU mode is syntax-level**: directives and pseudo-instructions are translated, but `ecall` keeps RVS semantics unless `--abi a7` is selected, numeric branch operands are absolute, and there is no libc or linker.
- **The `a7` ABI covers ten syscalls** (1, 4, 5, 8, 9, 10, 11, 17, 34, 93) — no allocator beyond `sbrk`, no file or environment syscalls.
- **Batch throughput is trace-bound** (≈0.8M instructions/s with the trace on, 800 MB for 10M instructions); `--no-trace` is 4× faster and 100k× smaller, but then the `Run` section is empty by design.
- **No published crates, no coverage tooling.** CI exists (tests + parity + release builds) but is unproven until it runs on GitHub.

## Future Ideas (Under Consideration)

If there's interest down the road, here are some cool things we could explore:

1. **Cache Simulation & Visualizer**:
   - Modeling configurable L1 Instruction and Data caches (direct-mapped vs. 2-way/4-way associative).
   - Displaying live hit/miss rates and block replacement animations.
2. **5-Stage Pipeline Hazard Diagram**:
   - Visualizing IF, ID, EX, MEM, and WB stages to teach data hazards, load-use interlocks, and branch delays.
3. **Multi-Hart / Concurrency Modeling**:
   - Simulating multi-threaded RISC-V execution with atomic memory operations (the RV64A extension).
