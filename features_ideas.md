# Deferred Features & Future Roadmap

1. **Standard GNU Mode vs. RVS Dialect Mode & Two-Way Transpiler** (Deferred)
   - **Context**: Computer organization courses (e.g. EECS 2021) currently require students to write explicit, non-pseudo 3-operand instructions directly in the canonical RVS dialect without relying on pseudo-instructions.
   - **Planned Capability**:
     - A toggleable mode: `GNU Mode` vs. `RVS Dialect Mode`.
     - In `GNU Mode`: support standard GNU assembler directives (`.text`, `.data`, `.globl`, `.word`, `.byte`, `.string`, `.align`, etc.) and standard RISC-V pseudo-instructions (`li`, `la`, `mv`, `not`, `neg`, `ret`, `jr`, `j`, `call`, `nop`, `beqz`, `bnez`, `bgt`, `ble`, `bgtu`, `bleu`).
     - A built-in two-way transpiler/converter: command to translate standard GNU RISC-V assembly into canonical RVS dialect (and vice-versa) directly inside the editor.

2. **Call Stack & Stack Frame Visualizer** (Deferred)
   - Visual inspection of active stack frames between `$sp` and base frame pointers (`fp`/`s0`), tracking saved register boundaries and local variable allocations across function calls.
