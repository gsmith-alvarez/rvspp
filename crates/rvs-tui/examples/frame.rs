//! Dev-only helper: render one App frame to stdout as text.
//!
//! Usage: cargo run -q --example frame -- <file.asm> [cols] [rows] [keys]
//! `keys` is a `,`-separated list; `<tab>`, `<esc>` and `<enter>` are special.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rvs_app::App;
use rvs_app::Mode;
use rvs_app_keys::AppKeys as _;
use rvs_machine_riscv::RiscV;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).cloned();
    let cols: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(150);
    let rows: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(44);
    let keys = args.get(4).cloned().unwrap_or_default();

    let mut app = App::<RiscV>::new(path.as_ref());
    for token in keys.split(',').filter(|s| !s.is_empty()) {
        match token {
            "<tab>" => app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            "<esc>" => app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            "<enter>" => app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            s => {
                for c in s.chars() {
                    app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
                }
            }
        }
        if app.mode == Mode::Running {
            while app.running {
                app.run_chunk(std::time::Duration::from_millis(5));
            }
        }
    }

    let backend = TestBackend::new(cols, rows);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| rvs_ui::draw(f, &mut app)).unwrap();
    println!("{}", terminal.backend());
}
