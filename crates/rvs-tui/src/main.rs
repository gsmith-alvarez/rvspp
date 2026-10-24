use clap::Parser;
use rvs_tui::{route, Cli};

fn main() -> std::process::ExitCode {
    route(Cli::parse())
}
