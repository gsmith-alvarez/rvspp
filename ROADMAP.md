# RVS++ Roadmap & Development Changelog

A plain-English breakdown of what we've built, why each feature exists, and where RVS++ is headed.

---

## The Backstory

This project started while taking an undergraduate Computer Organization course.

Our labs centered around **RVS v0.46** (`RVS046.tcl`), a 2019 educational simulator written in Tcl/Tk by SIDIC. While it did an admirable job visualizing registers and memory, using it every week was an exercise in friction:
- **Dialect lock-in**: When practicing problems or reading code online, everything used standard RISC-V pseudo-instructions (`li`, `mv`, `ret`, `call`) and GNU directives (`.word`, `.dword`). We were taught these in lecture, but RVS outright rejected them with syntax errors.
- **Clunky UI**: The Listing and Memory views weren't real aligned tables—just manually spaced text. You were constantly wrestling with awkward horizontal scrollbars just to see what an instruction was doing.
- **Environment headaches**: Getting RVS to run on a personal machine was like pulling teeth. It was only designed for university lab machines, and its legacy Tcl/Tk dependencies conflicted with modern systems.
- **Missing basic editor features**: The built-in editor didn't even support standard shortcuts like `Ctrl+Backspace`.
- **No time travel**: Stepping past a bug meant manually restarting the entire program from scratch.
- **Silent convention penalties**: Forgetting 16-byte stack pointer alignment or accidentally clobbering callee-saved registers (`s0`–`s11`) would silently cost marks on the course autograder with zero feedback from the simulator.

After wrestling with Nix to finally get RVS working locally, a question arose:
> *Look at all the modern CLI and TUI tools developers build in Rust today. Why can't we have a simulator like that for computer architecture—one that runs anywhere without fighting your OS, has real ergonomics, and actually helps you learn?*

What began as an experiment turned into **RVS++**: a complete, zero-dependency modernization that preserves 100% autograder parity while adding every tool a systems student could wish for.
---

## What We've Built (v1.0.0)

### 1. Pure-Rust Core & 100% Autograder Compatibility
- **No dependencies**: Replaced all Tcl/Tk requirements with a clean, standalone Rust application (RV64IMFD) that compiles into a single binary.
- **Autograder parity**: Headless batch mode (`rvs --batch <asm> <in> <out>`) produces reports that match the original `kkbatch` autograder byte-for-byte. Your assignments evaluate identically.
- **Works everywhere**: Runs natively on Linux, macOS, and Windows WSL, with both Cargo and Nix Flakes (`nix run .`) supported.

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
- **Dialect toggle**: Switch between strict educational RVS syntax and standard GNU assembly (`:mode gnu`). GNU mode lets you compile standard directives (`.word`, `.dword`, `.asciz`, `.zero`) and 25+ pseudo-instructions (`li`, `mv`, `ret`, `call`, `j`, `nop`, `beqz`, `bgt`, etc.) directly.
- **Two-way transpiler**: Click `[TRANSPILE]` or run `:transpile rvs` to expand pseudo-ops into canonical 3-operand code for homework submissions, or `:transpile gnu` to tidy canonical code back into clean GNU syntax.

---

## Future Ideas (Under Consideration)

If there's interest down the road, here are some cool things we could explore:

1. **Cache Simulation & Visualizer**:
   - Modeling configurable L1 Instruction and Data caches (direct-mapped vs. 2-way/4-way associative).
   - Displaying live hit/miss rates and block replacement animations.
2. **5-Stage Pipeline Hazard Diagram**:
   - Visualizing IF, ID, EX, MEM, and WB stages to teach data hazards, load-use interlocks, and branch delays.
3. **Multi-Hart / Concurrency Modeling**:
   - Simulating multi-threaded RISC-V execution with atomic memory operations (the RV64A extension).
