# RVS++ Feature Notes & Backlog

A quick overview of what's shipped in RVS++ v1.0.0 and what we might explore in the future.

---

## Shipped in v1.0.0

- **Vim Modal Editing**: Real Normal, Insert, and Command modes with cursor motions, word jumps, line deletion/yanking, and multi-level undo/redo.
- **External Editor Hook (`Enter`)**: Pop directly into Neovim, Vim, or Nano and reload cleanly on exit.
- **Interactive Breakpoints (`● `)**: Drop breakpoints on any line or PC (`B` / `:b`), with automatic pause on run.
- **Reverse Execution (`Prev`)**: Step backward by 1 instruction (`p` / `P` / `N`) using a 128-state history ring buffer.
- **Speed Control & Slow-Mo**: Step continuously at `1Hz`, `5Hz`, `20Hz`, or uncapped `Max` with live visual animation across all panes.
- **Calling Convention Linter (`:cc`)**: Live warnings for 16-byte stack misalignment and unpreserved callee-saved registers (`s0`–`s11`).
- **Call Stack Visualizer (`:stack`)**: Interactive modal inspecting active function frames, return addresses, and stack memory slots.
- **Dynamic Metrics (`:stats`)**: Hardware counters, live CPI estimation, branch prediction stats, and memory traffic breakdown.
- **GNU Mode & Transpiler (`:mode`, `:transpile`)**: Assemble GNU-style pseudo-instructions and directives, or convert between GNU and canonical RVS dialect in-place. Syntax-level only — `ecall` keeps RVS semantics unless `--abi a7` is selected, and batch needs `--dialect gnu` (or `auto`) to grade such a file.
- **Crash containment (v1.0.1)**: malformed instructions are reported and halt the run instead of panicking; a TUI panic restores the terminal and saves the buffer to `$XDG_DATA_HOME/rvs/recovery-*.s`.
- **Debugger depth (v1.1.0)**: `:next`/`:finish`, conditional breakpoints (`:b loop if x1 == 3`), watchpoints (`:watch sp`), `/` search with `Ctrl+N`/`Ctrl+P`, counted motions.
- **Cheap reverse stepping (v1.1.0)**: history is registers + a memory-journal mark, so rewinding no longer copies the world (20k steps: 2.6 s → 7 ms).
- **`a7` syscall ABI (v1.1.0)**: `--abi a7` makes RARS/GNU-style `ecall` programs run and exit.
- **Memory Navigation**: One-click jumps to the active stack pointer (`[SP]`), base data (`[0x0]`), or arbitrary symbols (`:mem`).
- **Bitfield Colorizer (`[CLR]`)**: Semantic visual linkage matching raw binary machine words to assembly mnemonics and operands.

---

## Future Backlog Ideas

These are potential enhancements for advanced computer architecture courses:

### 1. Cache Simulation & Memory Hierarchy Visualizer
- **Concept**: A visual cache simulator modeling L1 Instruction and Data caches.
- **Details**: Configurable cache size, block size, and associativity (direct-mapped, 2-way, 4-way set-associative).
- **Visuals**: Displays live hit/miss rates, cache line tags/valid bits, and block replacement animations (LRU/FIFO).

### 2. 5-Stage Pipeline Hazard Diagram
- **Concept**: Visualizing classic RISC pipeline stages (IF, ID, EX, MEM, WB).
- **Details**: Highlights data hazards (RAW dependencies), load-use stalls, branch penalty bubbles, and forwarding path bypasses.
