//! rvs-tui: the `rvs` binary: CLI parsing, batch routing and the TUI.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// RVS++ - RISC-V Visual Simulator++ (RV64IMFD), Rust rewrite of RVS046.
#[derive(Parser, Debug)]
#[command(name = "rvs++", version = env!("CARGO_PKG_VERSION"), about = "RVS++ - RISC-V Visual Simulator++ (RV64IMFD)")]
pub struct Cli {
    /// Positional args: [asm_file] or [asm_file in_file out_file [timeout_ms]]
    pub args: Vec<String>,

    /// Run in headless batch mode: rvs --batch <asm> <in> <out>
    #[arg(short, long)]
    pub batch: bool,

    /// Batch execution timeout in milliseconds (default: 3000)
    #[arg(short, long, default_value_t = 3000)]
    pub timeout: u64,

    /// Assembly dialect for batch mode: auto (default), rvs, or gnu
    #[arg(long, default_value = "auto", value_parser = ["auto", "rvs", "rvs046", "gnu", "gas"])]
    pub dialect: String,

    /// Omit the per-instruction trace from the batch report (faster, smaller)
    #[arg(long)]
    pub no_trace: bool,

    /// Also write a machine-readable JSON summary to this path
    #[arg(long, value_name = "PATH")]
    pub json: Option<PathBuf>,

    /// Exit 2 when assembly or execution failed (default: 0, like kkbatch)
    #[arg(long)]
    pub fail_on_error: bool,

    /// Syscall convention: rvs (default, course dialect) or a7 (RARS/GNU)
    #[arg(long, default_value = "rvs", value_parser = ["rvs", "a7"])]
    pub abi: String,
}

impl Cli {
    /// `None` means "detect from the source".
    fn dialect(&self) -> Option<rvs_machine::Dialect> {
        match self.dialect.to_ascii_lowercase().as_str() {
            "rvs" | "rvs046" => Some(rvs_machine::Dialect::Rvs),
            "gnu" | "gas" => Some(rvs_machine::Dialect::Gnu),
            _ => None,
        }
    }
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
                let request = rvs_machine_riscv::BatchRequest {
                    dialect: cli.dialect(),
                    trace: !cli.no_trace,
                    json: cli.json.as_deref(),
                    a7: cli.abi == "a7",
                };
                match rvs_machine_riscv::run_batch_file(&asm, &input, &out, timeout, request) {
                    Ok(report) if cli.fail_on_error && report.failed() => ExitCode::from(2),
                    Ok(_) => ExitCode::SUCCESS,
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
        run_tui(&cli)
    }
}

use std::time::Duration;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use rvs_app::{App, RunSpeed};
use rvs_app_cmds::AppCommands;
use rvs_app_keys::AppKeys;
use rvs_machine_riscv::RiscV;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};

static TERMINATED: LazyLock<Arc<AtomicBool>> = LazyLock::new(|| Arc::new(AtomicBool::new(false)));

fn is_terminated() -> bool {
    TERMINATED.load(Ordering::Relaxed)
}

#[cfg(unix)]
pub fn setup_signal_handlers() {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};

    for sig in [SIGINT, SIGTERM, SIGHUP] {
        let _ = signal_hook::flag::register(sig, Arc::clone(&TERMINATED));
    }
}

#[cfg(not(unix))]
pub fn setup_signal_handlers() {}

#[cfg(test)]
mod crash_tests {
    use super::save_recovery_buffer;
    use rvs_app::App;
    use rvs_machine_riscv::RiscV;

    #[test]
    fn recovery_buffer_holds_the_editor_text() {
        // `dirs` honours XDG_DATA_HOME, so the test never touches $HOME.
        let tmp = std::env::temp_dir().join(format!("rvs-recovery-test-{}", std::process::id()));
        std::env::set_var("XDG_DATA_HOME", &tmp);
        let mut app = App::<RiscV>::new(None);
        app.editor.set_text("START: addi x1, x0, 1\n");
        let path = save_recovery_buffer(&app).expect("recovery file");
        let saved = std::fs::read_to_string(&path).expect("read back");
        assert!(saved.contains("addi x1, x0, 1"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

#[cfg(all(test, unix))]
mod signal_tests {
    use super::{is_terminated, setup_signal_handlers};

    #[test]
    fn sigterm_sets_termination_flag() {
        setup_signal_handlers();
        assert!(!is_terminated());
        // `raise` delivers to the calling thread; signal-hook's handler sets
        // the shared flag instead of terminating the process.
        unsafe {
            libc::raise(libc::SIGTERM);
        }
        assert!(is_terminated());
    }
}

#[cfg(unix)]
fn poll_terminal_event(timeout: Duration) -> Result<bool, Box<dyn std::error::Error>> {
    use std::os::unix::io::AsRawFd;
    if is_terminated() {
        return Ok(false);
    }
    let fd = std::io::stdin().as_raw_fd();
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
    let ret = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    if ret < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::Interrupted {
            return Ok(false);
        }
        return Err(Box::new(err));
    }
    if ret == 0 {
        return Ok(false);
    }
    if (pfd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL)) != 0 {
        TERMINATED.store(true, Ordering::Relaxed);
        return Ok(false);
    }
    Ok(crossterm::event::poll(Duration::from_millis(0))?)
}

#[cfg(not(unix))]
fn poll_terminal_event(timeout: Duration) -> Result<bool, Box<dyn std::error::Error>> {
    if is_terminated() {
        return Ok(false);
    }
    Ok(crossterm::event::poll(timeout)?)
}

/// Leave the alternate screen and raw mode. Safe to call more than once and
/// from a panic hook, where every operation has to stay best-effort.
fn restore_terminal() {
    let mut stdout = std::io::stdout();
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        stdout,
        crossterm::terminal::LeaveAlternateScreen,
        DisableMouseCapture,
        crossterm::cursor::Show
    );
}

/// Install a hook so a panic anywhere (including inside the simulator) still
/// leaves a usable terminal behind.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));
}

/// Write the editor buffer somewhere durable so a crash cannot cost the user
/// their file. Returns the path on success.
fn save_recovery_buffer(app: &App<RiscV>) -> Option<std::path::PathBuf> {
    let dir = dirs::data_dir()?.join("rvs");
    std::fs::create_dir_all(&dir).ok()?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("recovery-{stamp}.s"));
    std::fs::write(&path, app.editor.text()).ok()?;
    Some(path)
}

/// Terminal setup/teardown and the event loop.
pub fn run_tui(cli: &Cli) -> ExitCode {
    setup_signal_handlers();
    install_panic_hook();
    let mut stdout = std::io::stdout();
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        EnableMouseCapture
    );
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(_) => {
            restore_terminal();
            return ExitCode::FAILURE;
        }
    };
    let result = event_loop(&mut terminal, cli);
    restore_terminal();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

type LoopError = Box<dyn std::error::Error>;

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    cli: &Cli,
) -> Result<(), LoopError> {
    let mut app = App::<RiscV>::new(cli.args.first());
    loop {
        if is_terminated() {
            return Ok(());
        }
        // One iteration is one unwind boundary: a bug in the assembler or the
        // executor must not take the user's unsaved buffer with it.
        let tick = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<bool, LoopError> {
                if app.needs_full_clear {
                    app.needs_full_clear = false;
                    terminal.clear()?;
                }
                terminal.draw(|f| rvs_ui::draw(f, &mut app))?;
                let timeout = if app.running {
                    app.run_speed.delay().unwrap_or(Duration::from_millis(16))
                } else {
                    Duration::from_millis(80)
                };
                if poll_terminal_event(timeout)? {
                    match crossterm::event::read()? {
                        Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => {
                            app.on_key(key)
                        }
                        Event::Mouse(m) => rvs_ui::handle_mouse(&mut app, m),
                        Event::Resize(_, _) => {}
                        _ => {}
                    }
                }
                if app.pending_external_editor {
                    app.pending_external_editor = false;
                    let _ = app.run_external_editor(terminal);
                }
                if app.running {
                    if app.run_speed == RunSpeed::Max {
                        app.run_chunk(Duration::from_millis(8));
                    } else {
                        app.do_step();
                    }
                }
                app.tick += 1;
                Ok(app.quit_requested)
            },
        ));
        match tick {
            Ok(Ok(true)) => return Ok(()),
            Ok(Ok(false)) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                let saved = save_recovery_buffer(&app);
                return Err(match saved {
                    Some(p) => format!("internal error; buffer saved to {}", p.display()).into(),
                    None => "internal error; buffer could not be saved".into(),
                });
            }
        }
    }
}
