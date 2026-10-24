# RVS Project Roadmap & Changelog

This document provides a comprehensive technical overview of all completed development milestones, architectural enhancements, and long-term research horizons for **RVS (RISC-V Visual Simulator)**.

---

## Project Vision & Architecture
`rvs` is an ultra-fast, standalone RISC-V visual simulator and assembly debugger written in pure Rust (RV64IMFD). It modernizes and supersedes the legacy 2019 Tcl/Tk educational simulator (`RVS046.tcl`), preserving 100% computational and autograding parity while delivering:
- A responsive, zero-latency Terminal User Interface (TUI) powered by Ratatui.
- Vim modal text editing with buffer undo/redo history.
- Zero external C/X11/Tcl dependencies for effortless cross-platform deployment across Linux, macOS, and Windows (WSL).
- First-class Nix Flake integration alongside standard Cargo builds.

---

## Completed Milestones

```
  ┌────────────────────────────────────────────────────────────────────────┐
  │                           MILESTONES COMPLETE                          │
  ├────────────────────────────────────────────────────────────────────────┤
  │ [x] Phase 1: Pure-Rust Core & Assembly Parity (RV64IMFD)               │
  │ [x] Phase 2: Terminal Disconnect & Zero-Zombie Process Teardown        │
  │ [x] Phase 3: Vim Modal Navigation & TUI Architectural Refactoring      │
  │ [x] Phase 4: Correlated Bitfield & Operand Colorizer ([CLR])           │
  │ [x] Phase 5: Interactive Breakpoints & Reverse Execution (Step Back)   │
  │ [x] Phase 6: Hardware Profiling & Calling Convention Compliance Checker│
  │ [x] Phase 7: Standard GNU Mode & In-Editor Two-Way Transpiler          │
  │ [x] Phase 8: Call Stack & Stack Frame Memory Hierarchy Visualizer      │
  └────────────────────────────────────────────────────────────────────────┘
```

---

### Phase 1: Pure-Rust Core & Reference Parity
- **Full RV64IMFD Instruction Set**: Complete arithmetic, logic, branch, jump, memory load/store, integer multiplication/division (`M`), and single/double-precision floating point (`F`/`D`) operations.
- **Two-Pass Assembler**: Direct support for original RVS directives (`DD`, `DW`, `DH`, `DB`, `DC`, `DF`, `DM`, `ORG`), symbol resolution, labels, and instruction encodings.
- **Memory-Mapped I/O (DMA)**: Automatic DMA detection and simulation at fixed low addresses (`OUTC` at `0x0`, `OUTD` at `0x8`, `INPC` at `0x10`, `INPD` at `0x18`).
- **Syscalls & Headless Batch Parity**: `ECALL` syscalls 0–9 and full CLI headless batch execution mode (`rvs -b <asm> <in> <out> [timeout]`) outputting byte-for-byte identical reports to the legacy `kkbatch` autograding harness.
- **Standalone Binary & Nix Packaging**: Fully reproducible Nix flake (`nix run .`) and standalone Cargo release build without runtime shared library dependencies.

### Phase 2: Terminal Disconnect & Zero-Zombie Process Teardown
- **Root Cause Resolution**: Resolved an issue where closing terminal tabs or windows caused Crossterm's event loop to spin indefinitely on `read(0) == 0`.
- **Low-Level Polling (`libc::poll`)**: Implemented pre-event polling checking for `POLLHUP | POLLERR` on `stdin` file descriptor 0, exiting cleanly in $\le 1$ ms upon connection loss.
- **Signal Handling**: Integrated POSIX signal handlers (`SIGINT`, `SIGTERM`, `SIGHUP`) and instant `Ctrl+C` termination without leaving orphan processes spinning.

### Phase 3: Vim Modal Navigation & TUI Refactoring
- **Vim Modal Editing Engine**:
  - Full modal state machine: `Normal`, `Insert` (`i`/`a`/`I`/`A`/`o`/`O`), `Command` (`:`), `Input`, and `Running`.
  - Vim cursor motions (`h`/`j`/`k`/`l`, `w`/`b`, `0`/`$`, `gg`/`G`, `Ctrl+D`/`Ctrl+U`).
  - Text mutations (`x`, `dd`, `yy`, `p`/`P`) and multi-level Undo/Redo (`u`, `Ctrl+R`).
- **Seamless External Editor Integration**: Pressing `Enter` on the `Source` pane opens the file in `$EDITOR` (`nvim`, `vim`, or `nano`) with automatic buffer reload upon exit.
- **Pane Layout & Navigation**:
  - Fullscreen Zoom toggle (`z`, `Enter`, `:zoom`) across all 6 panes.
  - Forward and reverse pane cycling (`Tab` / `Shift+Tab`).
  - Yazi-style interactive file explorer modal (`Ctrl+O` or `:e`).
  - Hit-test layout deduplication and zero-copy syntax highlighting.

### Phase 4: Correlated Bitfield & Operand Colorizer (`[CLR]`)
- **Semantic Machine Code Visualization**:
  - Correlates 32-bit binary instruction bitfields with their corresponding assembly opcode, destination register, source registers, and immediate values.
  - Distinct theme colors applied uniformly across binary columns and disassembled text.
- **Controls**: Toggled via key `2` in `Listing` pane, toolbar button `[CLR]`, or `:color [on|off]`.

### Phase 5: Interactive Breakpoints & Reverse Execution
- **Source & Listing Breakpoints**:
  - Prominent red `● ` indicator and bold red line numbers in both `Source` editor and `Listing` table.
  - Toggle via `B` (Source cursor line), `b` (Listing PC), `:b <line>`, `:b pc`, `:b <addr|label>`, or `:cb` (clear all).
  - Resumes execution cleanly without infinite re-triggering.
- **Step Back / Reverse Execution (`Prev`)**:
  - 128-state ring buffer storing full simulator snapshots (`regs`, `mem`, `exec_log`, `output`, `pc`).
  - Rewind 1 instruction backward with `p`, `P`, `N`, or `:prev`.
- **Execution Speed Throttling & Live Animation**:
  - Speed presets: `1Hz`, `5Hz`, `20Hz`, and `Max` (default).
  - Stepping with live visual animation across all 6 panes.
  - Adjust via `+` / `-`, `[` / `]`, `[SPD:...]` toolbar button, or `:speed <1|5|20|max>`.
- **Memory Navigation**:
  - One-click jump to stack pointer (`[SP]` button or `:mem sp`).
  - Jump to base `0x0` (`[0x0]` button) or arbitrary address/label (`:mem <addr|symbol>`).

### Phase 6: Hardware Profiling & Calling Convention Compliance Checker
- **Dynamic Hardware Counters & Metrics (`:stats`, `[STATS]`)**:
  - **Dynamic CPI Model**: Cycle-accurate estimation (ALU: 1 cycle, branch penalty: +1, load-use latency: +1, multiply/divide: +2, syscalls: +4). Live average CPI display.
  - **Branch Prediction Statistics**: Total branch instructions, branches taken, and taken percentage rate (`%`).
  - **Memory Access Traffic**: Total loads and stores retired.
  - **Instruction Mix Breakdown**: Categorized counts for ALU, Memory, Jumps, Floating-Point, and Syscalls.
- **Calling Convention Compliance Checker (`:cc`, `[CC]`)**:
  - **16-Byte Stack Alignment**: Actively verifies that `$sp` satisfies `sp % 16 == 0` upon every function call (`jal ra`, `jalr ra`).
  - **Callee-Saved Register Preservation**: Shadow-tracks all 12 integer saved registers (`s0`–`s11` / `x8`, `x9`, `x18`–`x27`) and float saved registers upon function entry. Verifies that all registers and `$sp` are restored upon return (`jalr zero, ra, 0`).
  - **Real-Time Diagnostics**: Emits bold yellow `WARN [Convention]` warnings in `Exec` trace log and lists violations in `:stats`.

### Phase 7: Standard GNU Mode & Two-Way In-Editor Transpiler
- **Standard GNU Dialect Compilation (`:mode [gnu|rvs]`)**:
  - Direct assembly of code written using GNU directives (`.word`, `.dword`, `.half`, `.byte`, `.float`, `.asciz`, `.string`, `.ascii`, `.zero`, `.space`, `.org`).
  - Graceful stripping of linker sections and metadata (`.text`, `.data`, `.rodata`, `.bss`, `.globl`, `.global`, `.section`, `.align`, `.file`, `.ident`, `.type`, `.size`).
  - Full lowering of 25+ canonical RISC-V pseudo-instructions: `li`, `la`, `mv`, `ret`, `call`, `tail`, `j`, `jr`, `nop`, `not`, `neg`, `negw`, `sext.w`, `seqz`, `snez`, `sltz`, `sgtz`, `beqz`, `bnez`, `blez`, `bgez`, `bltz`, `bgtz`, `bgt`, `ble`, `bgtu`, `bleu`, `fmv.s`, `fabs.s`, `fneg.s`, etc.
- **In-Editor Two-Way Transpiler (`:transpile [gnu|rvs]`)**:
  - `:transpile rvs`: Converts GNU assembly with pseudo-ops into strict, explicit 3-operand RVS dialect directly inside the editor buffer.
  - `:transpile gnu`: Compresses canonical 3-operand RVS code back into clean, idiomatic GNU assembly with standard pseudo-ops.

### Phase 8: Call Stack & Stack Frame Memory Visualizer
- **Interactive Stack Inspector (`:stack`, `[STACK]`)**:
  - Modal overlay rendering active function call frames in reverse hierarchical order.
  - Resolves target function names and caller addresses from the symbol table (`sym`).
  - Calculates frame bounds (`frame_sp` to `parent_sp`) and inspects memory double-words, annotating return addresses (`Saved ra`), callee-saved registers (`Saved s0`..`s11`), and local variables/arguments.
  - Displays live 16-byte stack pointer alignment status (`16-byte ALIGNED ✓` or `MISALIGNED ✗`).

---

## Future Research & Long-Term Horizons

The following capabilities represent potential directions for advanced computer architecture coursework and educational experimentation:

1. **Cache Simulation & Memory Hierarchy Visualizer**:
   - Configurable L1 Instruction and Data cache models (direct-mapped, 2-way, 4-way set-associative).
   - Real-time hit/miss rates, cache block replacement animations (LRU/FIFO), and write-back/write-through policies.
2. **5-Stage Pipeline Hazard & Forwarding Diagram**:
   - Visual inspection of IF, ID, EX, MEM, and WB pipeline registers.
   - Highlighting data hazards, load-use interlocks, branch delay stalls, and forwarding path activations.
3. **Multi-Hart / SMP Simulation**:
   - Multi-threaded RISC-V execution modeling with atomic memory operations (RV64A extension: `lr.d`/`sc.d`, `amoswap`, `amoadd`).
