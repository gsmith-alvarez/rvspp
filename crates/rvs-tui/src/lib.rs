//! rvs-tui: the `rvs` binary: CLI parsing, batch routing and the TUI.


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
                match rvs_format::run_batch_file(&asm, &input, &out, timeout) {
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

use std::sync::atomic::{AtomicBool, Ordering};

static TERMINATED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn handle_signal(_: libc::c_int) {
    TERMINATED.store(true, Ordering::Relaxed);
}

#[cfg(unix)]
pub fn setup_signal_handlers() {
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = handle_signal as *const () as usize;
        libc::sigaction(libc::SIGINT, &sa, std::ptr::null_mut());
        libc::sigaction(libc::SIGTERM, &sa, std::ptr::null_mut());
        libc::sigaction(libc::SIGHUP, &sa, std::ptr::null_mut());
    }
}

#[cfg(not(unix))]
pub fn setup_signal_handlers() {}

#[cfg(unix)]
fn poll_terminal_event(timeout: Duration) -> Result<bool, Box<dyn std::error::Error>> {
    use std::os::unix::io::AsRawFd;
    if TERMINATED.load(Ordering::Relaxed) {
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
    if TERMINATED.load(Ordering::Relaxed) {
        return Ok(false);
    }
    Ok(crossterm::event::poll(timeout)?)
}


/// Terminal setup/teardown and the event loop.
pub fn run_tui(cli: &Cli) -> ExitCode {
    setup_signal_handlers();
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
        Err(_) => return ExitCode::FAILURE,
    };
    let result = event_loop(&mut terminal, cli);
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = crossterm::terminal::disable_raw_mode();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new(cli.args.first());
    loop {
        if TERMINATED.load(Ordering::Relaxed) {
            return Ok(());
        }
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
        if app.quit_requested {
            return Ok(());
        }
    }
}
