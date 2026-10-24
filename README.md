# RVS++ (RISC-V Visual Simulator++)

[![Vibe-Ware](https://img.shields.io/badge/status-vibe--ware-magenta.svg)](#vibe-ware-notice)

> **Vibe-Ware Notice**: This project is **vibe-ware** — rapidly designed and iterated with AI-assisted engineering, refactored into a clean production Rust architecture, and verified for full behavioral and assembly parity against the original RVS reference simulator.

[![Release](https://img.shields.io/badge/version-1.0.0-cyan.svg)](https://github.com/gsmith-alvarez/rvspp/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

An interactive, fast terminal-based RISC-V simulator and debugger (RV64IMFD) written in pure Rust.

RVS++ takes the classic 2019 educational simulator (`RVS046.tcl`) used in university computer architecture courses and rebuilds it for modern development: real Vim modal editing, reversible execution, live calling convention checking, call stack inspection, and zero external dependencies.

<img width="1908" height="1125" alt="screenshot_20261023_222058-region" src="https://github.com/user-attachments/assets/fa067b96-4a4c-4501-a2c2-fd79ea00e767" />

### Listing Pane
<img width="1898" height="1103" alt="screenshot_20261023_222112-region" src="https://github.com/user-attachments/assets/63ce30bb-47a9-40b7-a8e8-8d9c219952a7" />


### Exec Pane
<img width="950" height="1105" alt="screenshot_20261023_222127-region" src="https://github.com/user-attachments/assets/55c05bc1-a36b-4000-85b8-39bda571daad" />

### Mem Pane:
<img width="1910" height="370" alt="screenshot_20261023_222205-region" src="https://github.com/user-attachments/assets/cb4c17d9-b5bf-4f6b-9414-f3ca913e177c" />

### Transpiling:
<img width="1904" height="1122" alt="image" src="https://github.com/user-attachments/assets/98ae92b9-0e35-49b7-a206-5bae67db977f" />

---

## Why We Built RVS++

If you've taken a Computer Organization course, you've probably had to use **RVS v0.46** (an educational simulator written in 2019 in Tcl/Tk).

The goal was simple: practice writing RISC-V assembly and get comfortable with computer architecture concepts. But actually using RVS day-to-day was frustrating:

- **Can't use standard code**: When practicing problems or looking up examples online, everything uses standard RISC-V pseudo-instructions (`li`, `mv`, `ret`, `call`) and GNU directives (`.word`, `.dword`). Lectures teach you these concepts, but RVS outright rejects them with syntax errors. You're forced into a rigid, non-standard dialect (`DD`, `DW`, `DC`, raw `addi x0, ...`).
- **Clunky UI & horizontal scrollbars**: The Listing and Memory views weren't real aligned tables; they were just manually spaced text. You were constantly wrestling with awkward horizontal scrollbars just to see what an instruction was doing.
- **Painful to run locally**: Getting RVS to run on a personal laptop is like pulling teeth. It was only designed for university lab machines, and its legacy Tcl/Tk dependencies conflict with almost every modern operating system (macOS, modern Linux distributions).
- **Missing basic editor features**: The built-in editor didn't even support standard editing shortcuts like `Ctrl+Backspace`.
- **Zero debugging safety nets**: Stepping past a bug meant restarting the entire program from scratch. Forgetting 16-byte stack pointer alignment or clobbering saved registers (`s0`–`s11`) would silently cost you marks on autograders with zero diagnostic feedback.

After using Nix to finally wrestle RVS onto my machine, I thought of all those vibe-coded Rust tools people make. A lot of them are slop, but honestly, even that slop was better than this. So why not make a simulator that runs on modern hardware without fighting your OS, gives you decent ergonomics like Vim keys, and actually lets you work on your labs?

Like C++ to C, **RVS++** takes that familiar simulator and gives it a ground-up modernization:

- **Runs everywhere instantly**: Single self-contained binary in pure Rust. No Tcl, no Tk, no X11 required. Runs natively on Linux, macOS, and Windows (WSL).
- **Feels like a real editor**: Vim modal editing (`Normal`, `Insert`, `Command`), full motion keys (`h/j/k/l`, `w/b`, `0/$`, `gg/G`), `dd`/`yy`/`p`, multi-level undo/redo, `Ctrl+Backspace` / `Ctrl+W`, and an `$EDITOR` shortcut (`Enter`) to hand off to Neovim.
- **Proper aligned tables**: Clean tabular views with vertical dividers and correlated bitfield-to-operand color coding (`[CLR]`), so you never have to fight horizontal scrollbars.
- **Actual debugging tools**: Interactive breakpoints with line markers (`● `), reverse stepping (`p` to rewind time from a 128-state ring buffer), and adjustable run speeds (`1Hz` up to `Max`) with live multi-pane animation.
- **Catches assignment bugs early**: An automated calling convention linter (`:cc`) that alerts you if `$sp` isn't 16-byte aligned or if a function modifies callee-saved registers (`s0`–`s11`) without restoring them.
- **Call stack visualizer**: Press `[STACK]` or run `:stack` to see your active call frames, stack memory slots, return addresses, and saved registers.
- **GNU mode & two-way transpiler**: Write standard GNU assembly with directives and pseudo-instructions, or convert between GNU and canonical RVS dialect with one click (`[TRANSPILE]`) before submitting assignments.
- **100% autograder compatible**: Headless batch mode (`--batch`) outputs byte-for-byte identical reports to the original `kkbatch` harness, guaranteeing your lab submissions pass grading.
---

## Quick Start

### 1. Requirements
All you need is a standard Rust toolchain (Rust 1.75+):
```bash
# If you don't have Rust installed:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Run Directly
```bash
# Launch the interactive simulator:
cargo run --release

# Or preload an assembly file:
cargo run --release -- tests/differential/t1.asm
```

You can also use the bundled `./run.sh` script, which automatically picks the fastest available target:
```bash
./run.sh [optional_file.s]
```

### 3. Build a Standalone Binary
```bash
cargo build --release
./target/release/rvs [file.s]
```

### 4. Install Globally
```bash
cargo install --path crates/rvs-tui
# Now you can run `rvs` or `rvspp` from anywhere:
rvs solution.s
```

---

## Running on Nix / NixOS

If you use Nix or NixOS, flakes and `nix-shell` are supported out of the box:

```bash
# Run directly via Nix Flakes:
nix run .

# Run with an assembly file:
nix run . -- tests/differential/t1.asm

# Enter a dev shell with all tools available:
nix develop
```

---

## Controls & Keybindings

### Global Navigation
| Key | Action |
| :--- | :--- |
| `Tab` / `Shift+Tab` | Cycle focus between panes forward / backward |
| `Enter` | **Source pane**: Opens buffer in your `$EDITOR` (`nvim`), reloading on exit.<br>**Other panes**: Toggles fullscreen zoom.<br>**INP pane**: Submits input. |
| `z` | Toggle fullscreen zoom on the active pane |
| `:` | Enter Command mode (e.g. `:w`, `:q`, `:c`, `:s`, `:n`, `:r`, `:stats`, `:stack`) |
| `?` | Open the Help / Shortcut reference modal |
| `q` / `Ctrl+C` | Clean exit (prompts to save only if you have unsaved changes) |

### Simulation & Debugging (Normal Mode)
| Key | Action |
| :--- | :--- |
| `c` | **Compile**: Assemble the current source buffer |
| `s` | **START**: Reset PC to `START` address and begin running |
| `n` | **Step**: Execute one instruction forward (or `:n <count>`) |
| `p` / `P` / `N` | **Step Back / Prev**: Rewind simulator by 1 instruction (128-state history) |
| `r` | **Run**: Execute continuously until `ebreak`, error, stop, or breakpoint |
| `x` | **Stop**: Halt continuous execution |
| `B` | **Breakpoint (Source)**: Toggle breakpoint on current cursor line (red `● `) |
| `b` | **Breakpoint (Listing)**: Toggle breakpoint on current PC |
| `+` / `]` | **Speed Up**: Increase animation speed (`1Hz` $\to$ `5Hz` $\to$ `20Hz` $\to$ `Max`) |
| `-` / `[` | **Slow Down**: Decrease animation speed |

### Vim Editing in Source Pane
| Key | Action |
| :--- | :--- |
| `h` / `j` / `k` / `l` | Move cursor left, down, up, right |
| `w` / `b` | Jump word forward / backward |
| `0` / `$` | Jump to start / end of line |
| `gg` / `G` | Jump to top / bottom of buffer |
| `i` / `a` | Enter Insert mode at cursor / after cursor (`Esc` returns to Normal) |
| `I` / `A` | Enter Insert mode at line start / line end |
| `o` / `O` | Insert new line below / above and enter Insert mode |
| `x` | Delete character under cursor |
| `dd` | Delete line and copy to yank buffer |
| `yy` / `Y` | Yank (copy) line |
| `p` / `P` | Paste below / above current line |
| `u` / `Ctrl+R` | Undo / Redo buffer edits |
| `Ctrl+D` / `Ctrl+U` | Scroll half-page down / up |

### Bottom Toolbar Buttons
Each pane has an interactive bottom toolbar you can click:

- **`Source`**:
  - `[RVS]`: Switch dialect to canonical RVS mode.
  - `[GNU]`: Switch dialect to standard GNU mode (directives & pseudo-instructions).
  - `[TRANSPILE]`: In-editor two-way transpiler (translates buffer between GNU and RVS syntax in-place and flips mode).
- **`Listing`**:
  - `[TXT]` (key `1`): Toggle raw source text column.
  - `[CLR]` (key `2`): Toggle correlated bitfield-to-operand colorizer.
  - `[SPD:...]`: Cycle run speed (`1Hz`, `5Hz`, `20Hz`, `Max`).
  - `[BP]`: Active breakpoint indicator.
  - `InitPC` (key `0` / `i`): Reset PC to `START` without running.
- **`Regs`**:
  - `[HEX]` (key `1`), `[INT]` (key `2`), `[FLP]` (key `3`): Toggle representation formats.
- **`Mem`**:
  - `[HEX]`, `[INT]`, `[FLP]`, `[TXT]`: Toggle memory data representations.
  - `[SP]`: Jump memory view straight to current `$sp` (`x2`).
  - `[0x0]`: Jump memory view back to address `0x0`.
  - `[STACK]`: Open Call Stack & Stack Frame Visualizer.
- **`Exec`**:
  - `[Clear]`: Clear execution trace log.
  - `[STATS]`: Open Hardware Counters & Metrics modal.
  - `[CC:ON/OFF]`: Toggle Calling Convention compliance checker.
  - `[STACK]`: Open Call Stack Visualizer.
- **`OUT` / `INP`**:
  - `[DMA:ON/OFF]` (key `d`): Toggle DMA active polling.

---

## Highlights & Features

### 1. Interactive Breakpoints (`● `)
Set breakpoints by line number, PC, symbol, or hex address:
- Press `B` in Normal mode on any line in `Source`.
- Or run `:b` (current line), `:b 15`, `:b pc`, `:b main`, or `:b 0x20`.
- Clear all breakpoints with `:cb` or `:clearbreaks`.
- Active breakpoints appear with a bold red `● ` marker in both the `Source` editor and `Listing` table. Continuous execution (`r`) pauses right before executing the breakpoint.

### 2. "Step Back" Reverse Execution (`Prev`)
Made a wrong turn or stepped past the bug? Press `p`, `P`, or `N` to step backward. RVS++ continuously records a 128-state ring buffer snapshotting registers, memory, and logs, letting you cleanly rewind execution.

### 3. Calling Convention Linter (`:cc`)
Designed specifically to prevent common student pitfalls in computer architecture labs:
- **16-byte alignment**: Verifies `$sp % 16 == 0` on every function call (`jal ra`, `jalr ra`).
- **Callee-saved registers**: Takes a shadow snapshot of registers `s0`–`s11` and `fs0`–`fs11` upon function entry. When the function returns (`jalr zero, ra, 0`), verifies that all saved registers and `$sp` were properly restored.
- Violations are immediately flagged in bright yellow in the `Exec` log (`WARN [Convention]: ...`) and cataloged in the `:stats` modal.

### 4. Call Stack & Stack Frame Visualizer (`:stack`)
Press `[STACK]` or run `:stack` to inspect your active call stack:
- Hierarchical display showing active frames, function names, and caller addresses.
- Inspects double-word memory slots inside each active frame, automatically detecting return addresses (`Saved ra`), preserved callee-saved registers (`Saved s0`..`s11`), and spilled arguments.
- Live stack pointer alignment indicator (`16-byte ALIGNED ✓` or `MISALIGNED ✗`).

### 5. Hardware Counters & Profiling (`:stats`)
Press `[STATS]` or run `:stats` to inspect dynamic execution metrics:
- **Cycle Model & Average CPI**: Tracks 1-cycle ALU ops, branch penalties (+1 for taken branches), load-to-use memory latency (+1), multi-cycle multiply/divide (+2), and `ecall` overhead (+4).
- **Branch Prediction Stats**: Total branches, branches taken, and taken rate (`%`).
- **Memory Traffic**: Total loads and stores retired.
- **Instruction Mix**: Breakdown of ALU, memory, jumps, float, and syscall instructions.

### 6. Standard GNU Mode & Two-Way Transpiler (`:mode`, `:transpile`)
Switch between strict educational syntax and standard GNU RISC-V assembly:
- **GNU Mode (`:mode gnu`)**: Assemble files using standard GNU directives (`.word`, `.dword`, `.half`, `.byte`, `.float`, `.asciz`, `.string`, `.zero`, etc.) and 25+ pseudo-instructions (`li`, `la`, `mv`, `ret`, `call`, `j`, `jr`, `nop`, `not`, `neg`, `beqz`, `bnez`, `bgt`, `ble`, etc.). Linker directives like `.text`, `.data`, `.globl` are gracefully ignored.
- **Two-Way Transpiler (`:transpile [gnu|rvs]`)**: Convert your buffer in-place!
  - `:transpile rvs` expands all pseudo-ops into canonical 3-operand RVS instructions.
  - `:transpile gnu` compresses canonical code back into clean GNU pseudo-instructions.

---

## Colon Command Reference (`:`)

Press `:` in Normal mode to enter Command mode:

| Command | Description |
| :--- | :--- |
| `:w [path]` | Save current file (or Save As if untitled) |
| `:q` | Quit (prompts if there are unsaved changes) |
| `:q!` | Force quit immediately without saving |
| `:wq` / `:x` | Save and quit |
| `:e [path]` | Open file in Yazi-style file explorer (or load path directly) |
| `:c` / `:compile` | Assemble source code |
| `:s` / `:start` | Reset PC to `START` and run |
| `:n [count]` | Step `1` or `count` instructions forward |
| `:p` / `:prev` | Step back / rewind 1 instruction |
| `:r` / `:run` | Run continuously |
| `:stop` / `:halt` | Stop execution |
| `:reset` / `:initpc` | Reset PC to `START` without running |
| `:b [line\|addr\|pc]` | Toggle breakpoint (uses cursor line in Source, or specified target) |
| `:cb` / `:clearbreaks` | Clear all breakpoints |
| `:speed <1\|5\|20\|max>` | Set execution speed preset |
| `:mem <sp\|addr\|label>` | Jump memory view to stack pointer or address |
| `:color [on\|off]` | Toggle bitfield-to-operand colorizer |
| `:cc [on\|off]` | Toggle calling convention compliance checker |
| `:mode [gnu\|rvs]` | Toggle or set assembly dialect mode |
| `:transpile [gnu\|rvs]` | Transpile buffer in-place between GNU and RVS syntax |
| `:stats` / `:metrics` | Open Execution Metrics & Hardware Counters dialog |
| `:stack` / `:callstack` | Open Call Stack & Stack Frame Visualizer dialog |
| `:zoom` / `:z` | Toggle fullscreen zoom on focused pane |
| `:clear` | Clear `Exec` trace log and `OUT` output |
| `:edit` / `:nvim` | Open current buffer in external `$EDITOR` |
| `:help` / `:h` | Open Help modal |

---

## CLI & Headless Batch Execution

In addition to the interactive TUI, `rvs` works as a headless CLI runner for grading scripts, autograders, and CI pipelines:

```bash
rvs --batch <asm_file> <input_file> <output_file> [timeout_ms]
```

### Arguments
- `<asm_file>`: Path to the assembly source file.
- `<input_file>`: File containing standard input lines for interactive `ECALL 5..9` or DMA requests (use `/dev/null` if none).
- `<output_file>`: Where the evaluation report is written.
- `[timeout_ms]`: Execution timeout in milliseconds (default: `3000` ms). Prevents runaway loops.

The output report structure matches the original `RVS046.tcl` batch output verbatim:
1. `Load <asm_file>`: Verbatim source code.
2. `Compile`: Assembled listing with encodings and symbols.
3. Initial `MEMORY`: Starting state of data cells.
4. `Run`: Trace log from `START` to `ebreak` or halt.
5. Final `MEMORY`: Memory contents after execution.
6. Final `REGISTERS`: Integer (`x0`–`x31`) and float (`f0`–`f31`) registers.
7. `OUTPUT`: Accumulated text output from `ecall` and DMA.

---

## Testing

Run the full workspace test suite (59 unit, property and integration tests):
```bash
cargo test --workspace
```

---

## License & Accreditations

This project is dual-licensed and honors the educational foundation that made it possible:

- **RVS++ Rust Workspace (`crates/*`)**: A workspace of small, single-responsibility crates — core: `rvs-tcl`, `rvs-regs`, `rvs-memory`, `rvs-transpile`, `rvs-sim`, `rvs-asm-syntax`, `rvs-assembler`, `rvs-fpu`, `rvs-ecall`, `rvs-executor`, `rvs-format`; TUI: `rvs-editor`, `rvs-explorer`, `rvs-theme`, `rvs-syntax`, `rvs-app`, `rvs-app-keys`, `rvs-app-cmds`, `rvs-ui-panes`, `rvs-ui-modal`, `rvs-ui`, `rvs-tui`. Released under the **MIT License**. See [LICENSE](LICENSE).
- **Original Reference (`src/RVS046.tcl`)**: Copyright (c) 2019 **SIDIC**. Distributed under SIDIC's educational not-for-profit license (reproduced in full in [LICENSE](LICENSE) and within lines 321–325 of `src/RVS046.tcl`).

We gratefully acknowledge SIDIC for the reference simulator and assembly specification that inspired this modernized rewrite.
