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
- **GNU Mode & Transpiler (`:mode`, `:transpile`)**: Assemble standard GNU pseudo-instructions and directives directly, or convert between GNU and canonical RVS dialect in-place.
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
