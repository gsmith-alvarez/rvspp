# Repository Guidelines

## Project Overview

RVS++ (`rvspp`) is a terminal RISC-V (RV64IMFD) simulator, debugger and teaching tool written in Rust. It replaces the 2019 Tcl/Tk simulator `src/RVS046.tcl` with a fast single binary, a ratatui TUI, a GNU-assembly mode with a two-way transpiler, reverse stepping, a calling-convention linter, and a headless batch mode used by autograders.

Hard invariant: the batch report (`rvs --batch ...`) must remain **byte-for-byte identical** to the reference `kkbatch` output. `tests/differential/` is the conformance oracle; never "clean up" formatting that the Tcl reference produces.

## Architecture & Data Flow

The workspace is 21 small crates; each crate owns one concern and is a single `src/lib.rs` (except `rvs-tui`, which also has binaries).

```text
Engine (UI-independent):
  rvs-tcl, rvs-regs, rvs-transpile            leaves
  rvs-memory          -> tcl                  sparse memory, SimError/SimResult
  rvs-asm-syntax      -> tcl, memory          line parsing, tcl_expr, INSTR_TABLE
  rvs-sim             -> memory, regs, tcl, transpile
                                              Simulator state, ViewConfig,
                                              ExecutionMetrics, ConventionChecker
  rvs-assembler       -> asm-syntax, sim, ...
  rvs-fpu, rvs-ecall  -> sim, memory, ...
  rvs-executor        -> sim, fpu, ecall, asm-syntax, ...
  rvs-format          -> sim, executor, assembler, ...   reports + batch runner

TUI (depends on engine, never the reverse):
  rvs-editor, rvs-explorer, rvs-syntax        UI leaves
  rvs-app             -> engine + editor/explorer
  rvs-app-cmds        -> app, explorer, tcl, shell-words, tempfile
  rvs-app-keys        -> app, app-cmds, crossterm
  rvs-ui-panes, rvs-ui-modal, rvs-ui          rendering
  rvs-tui             -> app, app-keys, app-cmds, ui, format; binaries + event loop
```

Edges above are abbreviated; each crate's `Cargo.toml` is authoritative.

Engine flow: `assemble(&mut Simulator, &str)` populates `sim.mem`, `sim.sym` and the listing → `step(&mut Simulator)` / `run(&mut Simulator, timeout_ms)` mutate registers/memory/logs → `format_memory`, `format_registers` and `run_batch_file` emit reports.

TUI flow: `App` owns all UI state (`sim: Simulator`, `editor: Editor`, focus, mode, scroll offsets, modal state). `App::on_key` dispatches; `:` commands run through the `AppCommands` trait; `rvs_ui::draw` renders; the event loop (plus signal/HUP handling) lives in `rvs-tui/src/lib.rs`.

## Key Directories

| Path | Purpose |
|---|---|
| `crates/rvs-*/src/lib.rs` | All first-party logic; one concern per crate |
| `crates/rvs-tui/src/` | `lib.rs` (CLI, route, event loop, signals), `main.rs` (`rvs`), `bin/rvspp.rs` (`rvspp`) |
| `crates/rvs-tui/tests/` | End-to-end TUI tests (`tui_smoke.rs`) + insta snapshots |
| `crates/rvs-assembler/tests/` | Full-ISA integration tests |
| `tests/differential/` | Tcl reference driver, fixtures `t[1-9].asm`, parity harness |
| `src/RVS046.tcl` | Legacy reference simulator; read-only, separate license |
| `target/` | Cargo output. Root `rvs`, `rvs++`, `rvspp` are gitignored build artifacts/symlinks |

## Development Commands

```bash
cargo check --workspace                 # fast type check
cargo test --workspace                  # full suite (59 tests)
cargo clippy --workspace --all-targets  # default lints (no clippy.toml)
cargo fmt --check                       # default rustfmt (no rustfmt.toml)

cargo test -p rvs-tui --test tui_smoke test_initial_frame_snapshot -- --exact
cargo run --release -- path/to/program.s          # TUI
cargo run --bin rvs -- --batch prog.s in.txt out.txt 3000
cargo install --path crates/rvs-tui               # installs rvs + rvspp

nix run . [-- file.s]    nix run .#legacy    nix develop    nix build
bash tests/differential/run.sh   # needs nix-shell -p tk xvfb-run (tclsh)
```

`rvs` routes to batch mode when `--batch` is passed **or** at least three positional args are present (`asm in out [timeout_ms]`).

## Code Conventions & Common Patterns

Project-wide rules (hard requirements):

- **No first-party crate may exceed 1000 lines.** Split by responsibility and add the new crate to the root `Cargo.toml` members.
- **No re-exports.** Zero `pub use` / `pub(crate) use`; callers import from the defining crate (`use rvs_tcl;`, `rvs_sim::Simulator`). Private `use rvs_tcl as tcl;` aliases are fine — they are imports, not re-exports.
- **No `#[path = "..."]` and no `include!`.**

Patterns to follow:

- **Engine code is free functions taking `&mut Simulator`**, not methods on `Simulator` (`assemble`, `step`, `run`, `format_memory`). One exception: state helpers that belong to the state type (`Simulator::get_stack_frames`).
- **TUI behavior is split into extension traits** because inherent impls must live with the type: `AppKeys` (`rvs-app-keys`) and `AppCommands` (`rvs-app-cmds`) are `impl ... for App`. Add new `App` behavior to the matching crate and declare it on the trait.
- **State management**: plain structs, explicit `&mut App` / `&mut Simulator` parameters, no globals, no async, no DI framework. The TUI is a single-threaded event loop.
- **Error handling**: `SimError`/`SimResult<T>` (`rvs-memory`) for runtime execution and memory; `AsmError { progress, message }` (`rvs-sim`) for assembly; `String` at CLI/batch boundaries. Do not panic on user input; surface through the status line or report.
- **Tcl parity is intentional.** Helpers such as `tcl_div`, `fmt_e24`, `a64`, `x12i`, and the upper/lowercase register-writer tracking exist to match Tcl 8.6 output. Never rename or "simplify" them without re-running the differential harness.
- **`INSTR_TABLE` in `rvs-asm-syntax`** is the single source of truth for mnemonics/encodings. Add instructions there, not in the executor's match tables alone.
- **Dependencies** are declared once in `[workspace.dependencies]` and referenced as `{ workspace = true }`. Existing choices: `clap` (CLI), `ratatui` + `crossterm` (TUI), `unicode-width`/`unicode-segmentation` (editor/cursor), `dirs` (home dir), `shell-words` (editor argv + batch entries), `tempfile` (scratch buffers), `signal-hook` + `libc` (Unix signals/HUP), `insta`/`proptest` (dev).
- **External editor**: split `$VISUAL`/`$EDITOR` with `shell_words`; scratch buffers go through `tempfile` (0600, `.s` suffix).
- **Formatting**: `cargo fmt` defaults; module-level `//!` docs; explain non-obvious Tcl-compat decisions in comments.

## Important Files

| File | Why it matters |
|---|---|
| `Cargo.toml` | Workspace members, shared versions, all dependency pins |
| `crates/rvs-tui/Cargo.toml` | Binaries `rvs` (`src/main.rs`) and `rvspp` (`src/bin/rvspp.rs`), `default-run` |
| `crates/rvs-tui/src/lib.rs` | `Cli`, batch/TUI routing, event loop, signal + terminal-hangup handling |
| `crates/rvs-sim/src/lib.rs` | `Simulator` (all simulation state), convention checker, metrics |
| `crates/rvs-assembler/src/lib.rs` | `assemble`, encoding, listing dump |
| `crates/rvs-executor/src/lib.rs` | `step`, `run`, integer dispatch |
| `crates/rvs-fpu/`, `crates/rvs-ecall/` | Float ops, `ECALL` syscalls, DMA I/O |
| `crates/rvs-format/src/lib.rs` | Report formatting and `run_batch_file` |
| `crates/rvs-asm-syntax/src/lib.rs` | `INSTR_TABLE`, expression evaluation |
| `crates/rvs-app/src/lib.rs` | `App` state and simulation/file actions |
| `crates/rvs-ui*/src/lib.rs` | Layout, panes, modals, mouse hit-testing |
| `tests/differential/run.sh` | Tcl-vs-Rust parity harness |
| `README.md`, `ROADMAP.md`, `LICENSE` | User behavior, history, dual licensing |

## Runtime/Tooling Preferences

- **Rust + Cargo only** (no Node/Bun, no npm). Edition 2021; no `rust-version`, `rust-toolchain`, `rustfmt.toml` or `clippy.toml` — tool defaults apply.
- Nix is optional: `flake.nix` pins `x86_64-linux` and exposes `default` (`bin/rvs`) and `legacy` (`bin/rvs-legacy`) apps plus a dev shell with cargo/rustfmt/clippy/tcl/tk.
- Unix-specific code is `#[cfg(unix)]` (`signal-hook`, `libc::poll`); keep non-Unix fallbacks compiling.
- `src/RVS046.tcl` is SIDIC-licensed for educational, not-for-profit use — do not relicense it or copy its code into MIT crates.

## Testing & QA

- **59 tests**: inline unit tests per crate, `proptest` properties (`rvs-editor` grapheme cursor, `rvs-tcl` integer/hex round-trips), 5 full-ISA integration tests, and 16 headless TUI tests (one of which is an insta frame snapshot).
- **TUI tests** use `ratatui::backend::TestBackend`, `App::new(None)`, helpers `key`/`char_key`/`ctrl`/`type_str`, and assert on `app` state plus `terminal.backend().to_string()`. Render with `terminal.draw(|f| ui::draw(f, &mut app))`.
- **Snapshot updates**: `INSTA_UPDATE=always cargo test -p rvs-tui --test tui_smoke test_initial_frame_snapshot`, then review `crates/rvs-tui/tests/snapshots/`.
- **Parity gate**: after touching `rvs-tcl`, `rvs-asm-syntax`, `rvs-assembler`, `rvs-executor`, `rvs-fpu`, `rvs-ecall` or `rvs-format`, run `bash tests/differential/run.sh` (t1–t5, t7–t9 must diff clean; t6 is smoke-only).
- No coverage tooling or CI config in-repo. Treat `cargo test --workspace` + the differential harness as the acceptance bar.
- Known hazards: two TUI tests call `std::env::set_current_dir` process-wide, and the differential harness writes fixed `/tmp/*.out` paths — avoid running suites concurrently or from a different CWD.
