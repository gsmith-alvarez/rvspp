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
| `r` | **Run**: Execute continuously until `ebreak`, error, or stop |
| `x` | **Stop**: Halt continuous execution (when not in Source pane) |

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

### Format Pill Toggles (Number Keys in Normal Mode)
- **`Listing` Pane**: `1`: Toggle TXT column ` │ ` `2`: Toggle Correlated `[CLR]` colorizer ` │ ` `0` / `i`: InitPC
- **`Regs` Pane**: `1`: Toggle HEX values ` │ ` `2`: Toggle INT registers ` │ ` `3`: Toggle FLP registers
- **`Mem` Pane**: `1`: Toggle HEX ` │ ` `2`: Toggle INT ` │ ` `3`: Toggle FLP ` │ ` `4`: Toggle TXT
- **`OUT` / `INP` Panes**: `d` (or `1`): Toggle DMA active state

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
