# Implemented Features & Future Roadmap

All major Tier 1, Tier 2, and previously deferred features have been implemented in **RVS++ v1.0.0**:

## Completed Features
- **Vim Modal Engine & TUI Navigation**: Normal, Insert (`i`/`a`/`o`), Command (`:`), Input, and Running modes with full keybindings, buffer undo/redo, and external `$EDITOR` support.
- **Correlated Bitfield & Operand Colorizer (`[CLR]`)**: Semantic visual linkage between binary instruction bitfields, mnemonics, and register operands.
- **Interactive Source & Listing Breakpoints (`● `)**: Set/toggle via `B`, `:b <line|addr|label|pc>`, `:cb`.
- **"Step Back" Reverse Step (`Prev`)**: 128-state ring buffer snapshotting registers, memory, and execution log.
- **Run Speed Throttling & Live Visual Animation (`+` / `-`, `[SPD:...]`)**: `1Hz`, `5Hz`, `20Hz`, and `Max` modes with live pane updates.
- **Memory Navigation**: One-click jump to `$sp` (`[SP]`), base `0x0`, or arbitrary symbols/addresses (`:mem`).
- **Dynamic Hardware Counters & Metrics (`:stats`)**: Instruction counts, cycle model, live average CPI, branch prediction accuracy (taken rate %), memory traffic, and instruction mix breakdown.
- **Calling Convention Compliance Checker (`:cc`)**: Live 16-byte stack alignment checks and saved register preservation verification across function calls/returns.
- **Standard GNU Mode vs. RVS Dialect Mode (`:mode [gnu|rvs]`)**: Direct assembly of standard GNU directives (`.word`, `.dword`, `.half`, `.byte`, `.float`, `.asciz`, `.string`, `.zero`, etc.) and canonical pseudo-instructions (`li`, `la`, `mv`, `ret`, `call`, `j`, `nop`, `beqz`, `bgt`, etc.).
- **Two-Way In-Editor Transpiler (`:transpile [gnu|rvs]`)**: In-place buffer translation between canonical educational 3-operand RVS assembly and standard idiomatic GNU RISC-V assembly.
- **Call Stack & Stack Frame Visualizer (`:stack`, `[STACK]`)**: Interactive modal inspecting active call frames, stack slot memory words, return addresses, and saved registers.

## Long-Term Future Ideas (Optional Exploration)
1. **Cache Simulation & Memory Hierarchy Visualizer**:
   - Configurable L1 instruction and data cache model (direct mapped, 2-way/4-way set associative).
   - Hit/miss rates, cache block replacement animations, and write-back/write-through indicators.
2. **5-Stage Pipeline Hazard & Forwarding Diagram**:
   - Visual representation of IF, ID, EX, MEM, WB pipeline registers.
   - Highlighting data hazards, load-use delays, branch stalls, and forwarding paths.
