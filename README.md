# RVS — RISC-V Visual Simulator (RV64IMFD)

> **Vibe-Ware Notice**: This project is **vibe-ware** — rapidly designed and iterated with AI-assisted engineering, refactored into a clean production Rust architecture, and verified for full behavioral and assembly parity against the original RVS reference simulator.

RVS is a terminal-based visual simulator and headless batch runner for the RISC-V **RV64IMFD** instruction set architecture. It features a responsive 6-pane terminal interface (powered by Ratatui and Crossterm) with Vim-style navigation, syntax highlighting, bitfield-to-operand colorization, and memory-mapped DMA I/O.

---

## Prerequisites & Dependencies

### Machines without Nix (Linux, macOS, BSD, Windows WSL)
- **Rust Toolchain**: `cargo` and `rustc` (1.75 or later). Install via [rustup.rs](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Zero C / GUI / X11 dependencies required**: RVS is 100% pure Rust and runs natively in any standard terminal emulator (`xterm-256color`, Alacritty, Kitty, WezTerm, GNOME Terminal, tmux, etc.).
- *(Optional)*:
  - Your favorite `$EDITOR` (`nvim`, `vim`, `nano`) if using the external editor buffer hand-off feature.
  - `tk` / `wish` only if you wish to run the legacy 2019 Tcl/Tk GUI script (`./run-source.sh`).

### Nix / NixOS
Nix Flakes and classic `nix-shell` are supported out-of-the-box. No system-wide `nix-ld` or manual library configuration is required.

---

## Running in Local Repo (Without Installing)

You can run RVS immediately from this repository without installing anything globally:

### Option A: Using the Runner Script (Easiest)
```bash
# Launch interactive TUI
./run.sh

# Launch TUI with an assembly source file preloaded
./run.sh tests/differential/t1.asm

# Run headless batch execution
./run.sh --batch <asm_file> <input_file> <output_file> [timeout_ms]
```
The `run.sh` script automatically detects whether a built release binary, `cargo`, or `nix` is present and runs the appropriate target.

### Option B: Using Cargo Directly
```bash
# Interactive TUI
cargo run --release

# Load a specific assembly file
cargo run --release -- tests/differential/t1.asm

# Headless batch mode
cargo run --release -- --batch program.s input.txt output.txt 3000
```

### Option C: Standalone Release Binary
```bash
cargo build --release
./target/release/rvs [optional_file.asm]
```

---

## Installing to System

To install the `rvs` binary globally onto your PATH:

```bash
cargo install --path crates/rvs-tui
```
This places `rvs` into `~/.cargo/bin/rvs`. Make sure `~/.cargo/bin` is in your `PATH`.

To uninstall:
```bash
cargo uninstall rvs-tui
```
---

## CLI & Headless Batch Execution

In addition to the interactive TUI, `rvs` functions as a headless CLI execution tool. This is ideal for CI/CD pipelines, automated testing, scripting, and course autograding.

Batch mode automatically activates whenever `--batch` (or `-b`) is passed, or when 3 or more positional arguments are supplied.

### Usage

```bash
rvs [--batch] <asm_file> <input_file> <output_file> [timeout_ms]
```

Or via the local runner script:
```bash
./run.sh --batch <asm_file> <input_file> <output_file> [timeout_ms]
```

### Arguments & Options

| Parameter | Description |
| :--- | :--- |
| `<asm_file>` | Path to the RISC-V assembly source file to assemble and run (e.g. `solution.s`). |
| `<input_file>` | Text file containing stdin input lines to be consumed by interactive `ECALL 5..9` (integer, float, string, char) or DMA input requests. Pass an empty file (e.g. `/dev/null`) if no input is expected. |
| `<output_file>` | Destination file where the comprehensive simulator execution report is written. |
| `[timeout_ms]` | *(Optional)* Maximum execution time in milliseconds before halting (default: `3000` ms). Protects against infinite loops. |
| `-b, --batch` | Explicitly flag batch mode. |
| `-t, --timeout <ms>` | Execution timeout in milliseconds (default: `3000`). |
| `-h, --help` | Print CLI help information and exit. |
| `-V, --version` | Print version information and exit. |

### Example: Autograding & Automated Testing

```bash
# Run headless simulation with a 5000ms timeout
rvs --batch tests/differential/t1.asm tests/differential/t1.in.txt report.txt 5000

# Inspect exit status in shell scripts
if [ $? -eq 0 ]; then
    echo "Simulation completed successfully. Report generated at report.txt"
fi
```

### Report Output Structure
The generated `<output_file>` contains the standard complete evaluation record:
1. **`Load <asm_file>`**: Verbatim source code loaded.
2. **`Compile`**: Assembly listing with machine encodings, addresses, and symbol table.
3. **Initial `MEMORY`**: Initial state of all `DD` data cells before execution.
4. **`Run`**: Instruction-by-instruction execution log from `START` to `ebreak` or halt.
5. **Final `MEMORY`**: State of memory after execution.
6. **Final `REGISTERS`**: Full integer (`x0`–`x31`) and float (`f0`–`f31`) register file contents.
7. **`OUTPUT`**: Accumulated output text emitted by `ecall` (syscalls 0–4) and DMA output.

---

## Running on Nix / NixOS

### 1. Direct Runner with Nix Flakes
```bash
# Run the Rust TUI directly
nix run .

# Run with an assembly file
nix run . -- tests/differential/t1.asm

# Build binary package into ./result/bin/rvs
nix build .
./result/bin/rvs

# (Optional) Run the legacy 2019 Tcl/Tk version via Nix
nix run .#legacy
```

### 2. Development Shell
```bash
# Using Flakes
nix develop

# Or classic nix-shell
nix-shell
```
Once inside the development shell, all tools (`cargo`, `rustc`, `rustfmt`, `clippy`, `tcl`, `wish`) are in your environment:
```bash
cargo run --release
```

---

## Controls & Keybindings

### Global Controls
| Key | Action |
| :--- | :--- |
| `Tab` | Cycle focused pane forward (`Source` $\to$ `Listing` $\to$ `Regs` $\to$ `Mem` $\to$ `Exec` $\to$ `OUT` $\to$ `INP`) |
| `Shift+Tab` / `BackTab` | Cycle focused pane backward |
| `Enter` | **Source pane**: Opens source in your `$EDITOR` (`nvim`), auto-reloading buffer on exit.<br>**Other panes**: Toggles fullscreen zoom on the focused pane.<br>**INP pane**: Submits input. |
| `z` / `Z` | Toggles fullscreen zoom on the currently focused pane |
| `:` | Enter Command Mode (e.g. `:w`, `:q`, `:c`, `:s`, `:n`, `:r`, `:zoom`, `:color`, `:help`) |
| `?` | Open Help / About modal |
| `q` / `Ctrl+Q` / `Ctrl+C` | Quit simulator (prompts to save only if real changes exist) |

### Simulation Commands (Normal Mode)
| Key | Action |
| :--- | :--- |
| `c` | **Compile** assembly source |
| `s` | **START**: Reset PC to `START` address and begin execution |
| `n` | **Next / Step**: Execute one instruction (or `:n <count>` in Command mode) |
| `p` / `P` / `N` | **Step Back / Prev**: Rewind simulator by 1 instruction from the 128-state history ring buffer |
| `r` | **Run**: Execute continuously until `ebreak`, error, stop, or breakpoint |
| `x` | **Stop**: Halt continuous execution (when not in Source pane) |
| `B` | **Breakpoint**: Toggle breakpoint on the current line in Source (or current PC in Listing) |
| `b` | Toggle breakpoint on current PC (when in Listing pane) |
| `+` / `]` | **Speed Up**: Increase execution animation speed (`1Hz` $\to$ `5Hz` $\to$ `20Hz` $\to$ `Max`) |
| `-` / `[` | **Slow Down**: Decrease execution animation speed |
### Vim Editing in Source Pane
| Key | Action |
| :--- | :--- |
| `h` / `j` / `k` / `l` | Cursor left, down, up, right |
| `w` / `b` | Jump word forward / backward |
| `0` / `$` | Jump to beginning / end of line |
| `gg` / `G` | Jump to beginning / end of file |
| `i` / `a` | Enter `INSERT` mode at cursor / after cursor |
| `I` / `A` | Enter `INSERT` mode at line start / line end |
| `o` / `O` | Insert line below / above and enter `INSERT` mode |
| `x` | Delete character under cursor |
| `dd` | Delete current line and copy to yank buffer |
| `yy` / `Y` | Yank (copy) current line |
| `p` / `P` | Paste below / above current line |
| `u` / `Ctrl+R` | Undo / Redo |
| `Ctrl+D` / `Ctrl+U` | Half-page scroll down / up |
| `Esc` | Return to `NORMAL` mode |

### Pane Toolbar & Shortcut Toggles
- **`Listing` Pane**:
  - `1`: Toggle TXT source column
  - `2`: Toggle Correlated `[CLR]` colorizer (maps bitfields to operands)
  - `0` / `i`: InitPC (reset PC to START without running)
  - Click `[SPD:...]`: Cycle execution speed (`1Hz`, `5Hz`, `20Hz`, `Max`)
  - `[BP]`: Active breakpoint indicator (click or press `b` to toggle at PC)
- **`Regs` Pane**:
  - `1`: Toggle HEX values ` │ ` `2`: Toggle INT registers ` │ ` `3`: Toggle FLP registers
- **`Mem` Pane**:
  - `1`: Toggle HEX ` │ ` `2`: Toggle INT ` │ ` `3`: Toggle FLP ` │ ` `4`: Toggle TXT
  - `[SP]`: Jump memory scroll directly to current stack pointer (`$sp` / `x2`)
  - `[0x0]`: Jump memory scroll back to base address `0x0`
- **`Exec` Pane**:
  - `[Clear]`: Clear execution trace log
  - `[STATS]`: Open Execution Metrics & Hardware Counters modal
  - `[CC:ON/OFF]`: Toggle Calling Convention compliance checker
- **`OUT` / `INP` Panes**:
  - `d` (or `1`): Toggle DMA active state
### File Explorer Modal (`Ctrl+O` or `:e`)
- **Normal Mode (Yazi-style)**:
  - `j` / `k` / `Down` / `Up`: Navigate file list
  - `h` / `Left`: Navigate to parent directory (`..`)
  - `l` / `Right` / `Enter`: Open file or descend into directory
  - `g` / `G`: Jump to first / last item
  - `J` / `K`: Scroll file preview pane
  - `/` or `i`: Switch to Filter/Path typing mode
  - `q` / `Esc`: Close explorer
- **Filter Mode**:
  - Type path or prefix filter directly; `Tab` to autocomplete; `Enter` to open; `Esc` returns to Normal mode.

### Memory-Mapped I/O (DMA)
RVS supports memory-mapped I/O at fixed low-memory addresses:
- **Output (`OUT`)**: Define labels `OUTC` (`0x0`, control word) and `OUTD` (`0x8`, data word). When present, DMA OUT auto-enables on compile.
- **Input (`INP`)**: Define labels `INPC` (`0x10`, control word) and `INPD` (`0x18`, data word). When present, DMA INP auto-enables on compile.


### Advanced Debugging, Metrics & Profiling Capabilities

#### 1. Interactive Breakpoints (Source & Listing)
- Set breakpoints by:
  - Pressing `B` in Normal mode on the current line in `Source`.
  - Running `:b` (uses current line in `Source`, or current PC in `Listing`).
  - Running `:b <line>` (e.g. `:b 15`), `:b pc`, `:b <label>`, or `:b <addr>`.
  - Running `:cb` or `:clearbreaks` to clear all breakpoints.
- Visuals: Lines with active breakpoints display a prominent red `● ` indicator and bold red line numbers in both the `Source` editor and `Listing` table.
- Running (`r`) pauses immediately before executing any breakpoint address.

#### 2. "Step Back" / Reverse Step (`Prev`)
- `App` continuously maintains a 128-state ring buffer of full simulator snapshots (`regs`, `mem`, `exec_log`, `output`, `pc`).
- Press `p`, `P`, `N` (Shift+N), or run `:prev` / `:back` to step backward by 1 instruction, restoring exact register and memory states.

#### 3. Visual Execution Animation & Speed Control
- Control simulation speed using `+` / `-` (or `[` / `]`), clicking `[SPD:...]` in the `Listing` toolbar, or running `:speed <1|5|20|max>`.
- In `1Hz`, `5Hz`, or `20Hz` modes, pressing `r` (Run) steps instruction-by-instruction with live visual updates to all 6 panes, animating algorithm execution in real time.

#### 4. Hardware Counters & Dynamic Profiling (`:stats`)
- Run `:stats`, `:metrics`, or click `[STATS]` in the `Exec` toolbar to inspect:
  - **Total Instructions & Estimated Cycles**: Cycle accurate model accounting for 1-cycle ALU ops, branch penalties (+1 for taken branches), load-to-use latency (+1), multi-cycle multiply/divide (+2), and `ecall` syscall overhead (+4).
  - **Average CPI**: Live dynamic CPI (`cycles / instructions`).
  - **Branch Prediction Stats**: Total branches, branches taken, and taken percentage rate (`%`).
  - **Memory Accesses**: Total loads and stores.
  - **Instruction Mix Breakdown**: Categorized counts for ALU, Memory, Jumps, Floating-Point, and Syscalls.

#### 5. Calling Convention Compliance Checker (`:cc`)
- An automated linter designed for university computer architecture courses:
  - **16-Byte Stack Alignment**: On every function call (`jal ra`, `jalr ra`), verifies that `$sp` satisfies `sp % 16 == 0`.
  - **Saved Register Preservation**: Shadow-tracks all 12 integer saved registers (`s0`–`s11` / `x8`, `x9`, `x18`–`x27`) and float saved registers upon function entry. On function return (`jalr zero, ra, 0`), verifies that all saved registers and `$sp` were properly restored.
  - Logs bold `WARN [Convention]` messages in `Exec` and lists violations in `:stats`.
  - Toggle via `:cc on`, `:cc off`, `:cc`, or clicking `[CC:ON]` / `[CC:OFF]`.

#### 6. Memory Navigation & Stack Pointer Jump
- Click `[SP]` in the `Mem` toolbar or run `:mem sp` / `:mem stack` to jump memory view directly to `$sp` (`x2`) to inspect active stack frames.
- Click `[0x0]` or run `:mem 0` to jump back to address `0x0`.
- Jump to any arbitrary address or symbol with `:mem <addr>` or `:mem <symbol>` (e.g. `:mem 0x2000`, `:mem OUTC`).

---

### Colon Command Reference (`:`)

Press `:` in Normal mode to enter Command mode:

| Command | Description |
| :--- | :--- |
| `:w [path]` | Save current file (or Save As if untitled) |
| `:q` | Quit simulator (prompts to save if real modifications exist) |
| `:q!` | Force quit immediately without saving |
| `:wq` / `:x` | Save and quit |
| `:e [path]` | Open file in File Explorer (or load path directly) |
| `:c` / `:compile` | Compile assembly source |
| `:s` / `:start` | Reset PC to `START` address and run |
| `:n [count]` | Step `1` or `count` instructions |
| `:p` / `:prev` | Step back / rewind 1 instruction |
| `:r` / `:run` | Run continuously |
| `:stop` / `:halt` | Halt continuous execution |
| `:reset` / `:initpc` | Reset PC to `START` without running |
| `:b [line\|addr\|label\|pc]` | Toggle breakpoint (uses cursor line in Source, or specified target) |
| `:cb` / `:clearbreaks` | Clear all active breakpoints |
| `:speed <1\|5\|20\|max>` | Set execution speed preset |
| `:mem <sp\|addr\|label>` | Jump memory view to stack pointer or address |
| `:color [on\|off]` | Toggle or set Listing correlated bitfield colorizing |
| `:cc [on\|off]` | Toggle Calling Convention compliance checker |
| `:stats` / `:metrics` | Open Execution Metrics & Hardware Counters dialog |
| `:zoom` / `:z` | Toggle fullscreen zoom on focused pane |
| `:clear` | Clear `Exec` trace log and `OUT` output |
| `:edit` / `:nvim` | Open current buffer in external `$EDITOR` |
| `:help` / `:h` | Open Help / About modal |
---

## Testing

Run the full workspace test suite (50 unit and integration tests):
```bash
cargo test --workspace
```

---

## Project Structure

```
rvs/
├── Cargo.toml               # Workspace configuration
├── Cargo.lock
├── crates/
│   ├── rvs-core/            # Pure, headless RV64IMFD simulator engine
│   │   ├── src/
│   │   │   ├── assembler.rs # Two-pass assembler, directives (DD, DW, DC, etc.)
│   │   │   ├── executor.rs  # RV64IMFD instruction interpreter & DMA engine
│   │   │   ├── format.rs    # Memory/register formatters & kkbatch runner
│   │   │   ├── memory.rs    # Sparse segmented memory
│   │   │   ├── registers.rs # Register files & ABI name maps
│   │   │   └── tcl.rs       # Exact arithmetic and formatting primitives
│   │   └── tests/
│   └── rvs-tui/             # Interactive Ratatui TUI application
│       ├── src/
│       │   ├── app.rs       # Application state, event loop & key routing
│       │   ├── editor.rs    # Multi-line Vim text buffer & undo stack
│       │   ├── explorer.rs  # Yazi-style path suggestions & preview engine
│       │   ├── syntax.rs    # Exact-width RISC-V syntax & bitfield highlighter
│       │   ├── ui.rs        # 6-pane layout, tabular formatters & hit-testing
│       │   ├── main.rs      # Binary entry point
│       │   └── lib.rs
│       └── tests/           # High-signal behavioral integration tests
├── default.nix              # Standard Nix package derivation
├── flake.nix                # Flake outputs (packages, apps, devShells)
├── shell.nix                # Classic nix-shell environment
├── legacy.nix               # Launcher for legacy Tcl/Tk source
├── run.sh                   # Portable multi-platform runner script
├── run-source.sh            # Script to run legacy Tcl/Tk source with wish
└── src/
    └── RVS046.tcl           # Reference 2019 Tcl/Tk implementation
```

---

## License

- **Rust Workspace (`crates/rvs-core`, `crates/rvs-tui`)**: Released under the **MIT License**. See [Cargo.toml](Cargo.toml).
- **Reference Implementation (`src/RVS046.tcl`)**: Copyright (c) 2019 SIDIC. Distributed under SIDIC's educational not-for-profit license (see lines 321–325 of `src/RVS046.tcl`).
