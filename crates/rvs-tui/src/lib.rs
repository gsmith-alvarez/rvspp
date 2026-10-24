//! rvs-tui: the `rvs` binary — CLI parsing, batch routing and the TUI.

pub mod app;
pub mod ui;
pub mod editor;
pub mod explorer;
pub mod syntax;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// RVS++ - RISC-V Visual Simulator++ (RV64IMFD), Rust rewrite of RVS046.
#[derive(Parser, Debug)]
#[command(name = "rvs++", version = "1.0.0", about = "RVS++ - RISC-V Visual Simulator++ (RV64IMFD)")]
pub struct Cli {
    /// Positional args: [asm_file] or [asm_file in_file out_file [timeout_ms]]
    pub args: Vec<String>,

    /// Run in headless batch mode: rvs --batch <asm> <in> <out>
    #[arg(short, long)]
    pub batch: bool,

    /// Batch execution timeout in milliseconds (default: 3000)
    #[arg(short, long, default_value_t = 3000)]
    pub timeout: u64,
}

/// Route: batch when `--batch` or 3+ positionals, else the TUI.
pub fn route(cli: Cli) -> ExitCode {
    if cli.batch || cli.args.len() >= 3 {
        let mut it = cli.args.iter();
        let asm = it.next().map(PathBuf::from);
        let input = it.next().map(PathBuf::from);
        let out = it.next().map(PathBuf::from);
        let timeout = cli
            .args
            .get(3)
            .and_then(|t| t.parse::<u64>().ok())
            .unwrap_or(cli.timeout);
        match (asm, input, out) {
            (Some(asm), Some(input), Some(out)) => {
                match rvs_core::format::run_batch_file(&asm, &input, &out, timeout) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::FAILURE
                    }
                }
            }
            _ => {
                eprintln!("batch mode requires: rvs [--batch] <asm> <in> <out> [timeout_ms]");
                ExitCode::FAILURE
            }
        }
    } else {
        app::run_tui(&cli)
    }
}
