use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;
use rvs_tui::app::{App, ConfirmAction, FileAction, Modal, Mode, Pane};
use rvs_tui::ui;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn char_key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        app.on_key(char_key(c));
    }
}

#[test]
fn test_interactive_tui_lifecycle_and_rendering() {
    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None);
    app.editor.set_text("addi x1, x0, 5\naddi x2, x0, 10\nadd x3, x1, x2\nebreak x0, x0, 0\n");

    // 1. Initial draw
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let frame = terminal.backend().to_string();
    for token in ["[NORMAL]", "Source", "Listing", "Regs", "Mem", "Exec", "OUT", "INP"] {
        assert!(frame.contains(token));
    }

    // 2. Compile ('c')
    app.on_key(char_key('c'));
    assert!(app.sim.listing.contains("ASSEMBLY LISTING"));
    assert!(app.sim.listing.contains("addi"));

    // 3. START ('s')
    app.on_key(char_key('s'));
    assert_eq!(app.sim.regs.pc, 0);
    assert!(app.sim.exec_log.contains("START\n"));

    // 4. Step ('n')
    app.on_key(char_key('n'));
    assert_eq!(app.sim.regs.pc, 4);
    assert_eq!(app.sim.regs.read_x(1), 5);
    assert!(app.sim.exec_log.contains("addi    x1 x0 0x005"));

    // 5. Run ('r') to completion
    app.on_key(char_key('r'));
    assert_eq!(app.mode, Mode::Running);
    while app.running {
        app.run_chunk(std::time::Duration::from_millis(50));
    }
    assert_eq!(app.sim.regs.read_x(3), 15);
    assert!(app.sim.exec_log.contains("ebreak"));
    assert_eq!(app.mode, Mode::Normal);

    // 6. About modal ('?') and Esc close
    app.on_key(char_key('?'));
    assert_eq!(app.modal, Modal::About);
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert!(terminal.backend().to_string().contains("About / Help"));

    app.on_key(key(KeyCode::Esc));
    assert_eq!(app.modal, Modal::None);

    // 7. Quit ('q')
    app.on_key(char_key('q'));
    assert!(app.quit_requested);
}

#[test]
fn test_continuous_run_resumes_across_input() {
    let mut app = App::new(None);
    app.editor.set_text("addi x1, x0, 5\necall x2, x0, 5\nadd x3, x1, x2\nebreak x0, x0, 0\n");
    app.compile();

    // 1. Run ('r')
    app.on_key(char_key('r'));
    assert_eq!(app.mode, Mode::Running);
    assert!(app.running);

    // Run chunk until it pauses for input
    app.run_chunk(std::time::Duration::from_millis(50));
    assert_eq!(app.mode, Mode::Input);
    assert!(!app.running);
    assert!(app.was_running);
    assert_eq!(app.sim.regs.read_x(1), 5);

    // 2. User enters "40" and presses Enter
    type_str(&mut app, "40");
    app.on_key(key(KeyCode::Enter));

    // Execution MUST automatically resume running
    assert_eq!(app.mode, Mode::Running);
    assert!(app.running);
    assert_eq!(app.sim.regs.read_x(2), 40);

    // 3. Process remaining instructions to ebreak
    while app.running {
        app.run_chunk(std::time::Duration::from_millis(50));
    }
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.sim.regs.read_x(3), 45);
    assert!(app.sim.exec_log.contains("ebreak"));
}

#[test]
fn test_run_vs_start_pc_semantics() {
    let mut app = App::new(None);
    app.editor.set_text("addi x1, x0, 5\naddi x2, x0, 10\nebreak x0, x0, 0\n");
    app.compile();

    // Step first instruction: PC becomes 4, x1 becomes 5
    app.action_next();
    assert_eq!(app.sim.regs.pc, 4);
    assert_eq!(app.sim.regs.read_x(1), 5);
    assert_eq!(app.sim.regs.read_x(2), 0);

    // Press 'r' (Run) — MUST NOT reset PC! It continues from PC=4
    app.action_run();
    assert_eq!(app.sim.regs.pc, 4);
    while app.running {
        app.run_chunk(std::time::Duration::from_millis(50));
    }
    assert_eq!(app.sim.regs.read_x(2), 10);

    // Press 's' (START) — MUST reset PC to 0 and run from beginning
    app.action_start();
    assert_eq!(app.sim.regs.pc, 0);
    while app.running {
        app.run_chunk(std::time::Duration::from_millis(50));
    }
    assert_eq!(app.sim.regs.read_x(1), 5);
    assert_eq!(app.sim.regs.read_x(2), 10);
}

#[test]
fn test_unsaved_changes_modal_and_newline_exemption() {
    let mut app = App::new(None);

    // a) Newline-only modifications do not trigger ConfirmSave on Ctrl+O or q
    app.on_key(char_key('i'));
    app.on_key(key(KeyCode::Enter));
    app.on_key(key(KeyCode::Enter));
    app.on_key(key(KeyCode::Esc));
    assert!(app.editor.modified);
    assert!(!app.editor.needs_save());

    app.on_key(ctrl('o'));
    assert!(matches!(app.modal, Modal::FilePrompt(FileAction::Load)));
    app.on_key(key(KeyCode::Esc));

    app.on_key(char_key('q'));
    assert!(app.quit_requested);

    // b) Real modifications open ConfirmSave on q, Esc cancels, typing edits path, Ctrl+D discards
    let mut app2 = App::new(None);
    app2.fname = "file.s".to_string();
    app2.editor.set_text("modified code\n");
    app2.editor.modified = true;
    assert!(app2.editor.needs_save());

    app2.on_key(char_key('q'));
    assert!(matches!(app2.modal, Modal::ConfirmSave(ConfirmAction::Quit)));
    assert_eq!(app2.modal_input, "file.s");

    // Esc cancels and retains edits
    app2.on_key(key(KeyCode::Esc));
    assert_eq!(app2.modal, Modal::None);
    assert!(!app2.quit_requested);
    assert!(app2.editor.modified);

    // Re-open ConfirmSave, edit path, verify UI rendering, Ctrl+D discards
    app2.on_key(char_key('q'));
    app2.on_key(key(KeyCode::Backspace));
    app2.on_key(key(KeyCode::Backspace));
    type_str(&mut app2, ".asm");
    assert_eq!(app2.modal_input, "file.asm");

    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app2)).unwrap();
    let frame = terminal.backend().to_string();
    assert!(frame.contains("Unsaved Changes"));
    assert!(frame.contains("File path: [ file.asm ]"));
    assert!(frame.contains("[Ctrl+D]"));

    app2.on_key(ctrl('d'));
    assert_eq!(app2.modal, Modal::None);
    assert!(app2.quit_requested);
}

#[test]
fn test_file_explorer_navigation_filter_and_load() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let _ = std::env::set_current_dir(&root);

    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new(None);

    // Open Load modal (Ctrl+O)
    app.on_key(ctrl('o'));
    assert!(matches!(app.modal, Modal::FilePrompt(_)));
    assert_eq!(app.explorer.mode, rvs_tui::explorer::ExplorerMode::Normal);

    // a) Yazi Normal-mode navigation: j/k, g/G, J/K preview scroll, parent .. / h / l
    let initial_selected = app.explorer.selected;
    app.on_key(char_key('j'));
    if app.explorer.entries.len() > 1 {
        assert_eq!(app.explorer.selected, initial_selected + 1);
    }
    app.on_key(char_key('k'));
    assert_eq!(app.explorer.selected, initial_selected);

    app.on_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
    assert_eq!(app.explorer.selected, app.explorer.entries.len().saturating_sub(1));
    app.on_key(char_key('g'));
    assert_eq!(app.explorer.selected, 0);

    // Preview scroll J/K
    app.explorer.preview_lines = (0..20).map(|i| format!("preview line {i}")).collect();
    app.on_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
    assert!(app.explorer.scroll_preview > 0);
    app.on_key(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT));
    assert_eq!(app.explorer.scroll_preview, 0);

    // Select ".." and open with 'l'
    if let Some(pos) = app.explorer.entries.iter().position(|e| e.name == "..") {
        app.explorer.selected = pos;
        app.on_key(char_key('l'));
        assert!(app.modal_input.ends_with("../") || app.modal_input == "../");
    }
    // Also test 'h' for parent navigation
    app.on_key(char_key('h'));

    // b) Switch to Filter mode ('/'), tilde ~/ resolution
    app.on_key(char_key('/'));
    assert_eq!(app.explorer.mode, rvs_tui::explorer::ExplorerMode::Filter);
    while !app.modal_input.is_empty() {
        app.on_key(key(KeyCode::Backspace));
    }
    type_str(&mut app, "~/");
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
    assert_eq!(app.explorer.base_dir, std::path::PathBuf::from(home));

    // Prefix filtering + Tab completion
    while !app.modal_input.is_empty() {
        app.on_key(key(KeyCode::Backspace));
    }
    type_str(&mut app, "tests/diff");
    app.on_key(key(KeyCode::Tab));
    assert_eq!(app.modal_input, "tests/differential/");

    type_str(&mut app, "t1.a");
    app.on_key(key(KeyCode::Tab));
    assert_eq!(app.modal_input, "tests/differential/t1.asm");

    // Tab-sanitized preview rendering
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let frame = terminal.backend().to_string();
    assert!(frame.contains("t1.asm"));
    assert!(frame.contains("addi x1, x0, 5"));
    assert!(!frame.contains('\t'));

    // Enter loads file and triggers needs_full_clear
    app.on_key(key(KeyCode::Enter));
    assert_eq!(app.modal, Modal::None);
    assert_eq!(app.fname, "tests/differential/t1.asm");
    assert!(app.editor.text().contains("addi x1, x0, 5"));
    assert!(app.needs_full_clear);
}

#[test]
fn test_vim_editor_motions_operators_and_cursor_alignment() {
    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None);
    // Line 0 starts with a tab character to verify tab visual expansion (8 cols)
    app.editor.set_text("\taddi x1, x0, 5\naddi x2, x0, 10\nadd x3, x1, x2");
    assert_eq!(app.focus, Pane::Source);
    assert_eq!(app.mode, Mode::Normal);

    // 1. Terminal cursor position across tab in Normal and Edit modes
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let pos_start = terminal.get_cursor_position().unwrap();

    // 'l' moves across the tab character -> visual column jumps by 8
    app.on_key(char_key('l'));
    assert_eq!(app.editor.col, 1);
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let pos_after_tab = terminal.get_cursor_position().unwrap();
    assert_eq!(pos_after_tab.x, pos_start.x + 8);
    assert_eq!(pos_after_tab.y, pos_start.y);

    // 'j' moves cursor down one line
    app.on_key(char_key('j'));
    assert_eq!(app.editor.row, 1);
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let pos_row1 = terminal.get_cursor_position().unwrap();
    assert_eq!(pos_row1.y, pos_start.y + 1);

    // Switch to Edit mode ('i') and move cursor with Right arrow
    app.on_key(char_key('i'));
    assert_eq!(app.mode, Mode::Edit);
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert_eq!(terminal.get_cursor_position().unwrap(), pos_row1);

    app.on_key(key(KeyCode::Right));
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let pos_edit_moved = terminal.get_cursor_position().unwrap();
    assert_eq!(pos_edit_moved.x, pos_row1.x + 1);

    // Esc returns to Normal mode and preserves cursor
    app.on_key(key(KeyCode::Esc));
    assert_eq!(app.mode, Mode::Normal);
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert_eq!(terminal.get_cursor_position().unwrap(), pos_edit_moved);

    // 2. Motions: gg (buffer start), G (buffer end), w (word), b (back), $ (eol), 0 (bol)
    app.on_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
    assert_eq!(app.editor.row, 2);
    app.on_key(char_key('g'));
    app.on_key(char_key('g'));
    assert_eq!(app.editor.row, 0);

    app.on_key(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::SHIFT));
    assert_eq!(app.editor.col, app.editor.lines[0].chars().count());
    app.on_key(char_key('0'));
    assert_eq!(app.editor.col, 0);

    app.on_key(char_key('w'));
    assert!(app.editor.col > 0);
    app.on_key(char_key('b'));
    assert_eq!(app.editor.col, 0);

    // 3. Operators: dd (delete/yank line), p (paste), u (undo), Ctrl+r (redo), x (delete char)
    app.on_key(char_key('d'));
    app.on_key(char_key('d'));
    assert!(app.editor.yank_buffer.contains("addi x1, x0, 5"));
    assert_eq!(app.editor.lines[0], "addi x2, x0, 10");

    app.on_key(char_key('p'));
    assert!(app.editor.lines[1].contains("addi x1, x0, 5"));

    app.on_key(char_key('u'));
    assert_eq!(app.editor.lines.len(), 2);

    app.on_key(ctrl('r'));
    assert_eq!(app.editor.lines.len(), 3);

    // 'x' deletes char under cursor
    let line_before = app.editor.lines[app.editor.row].clone();
    app.on_key(char_key('x'));
    assert_ne!(app.editor.lines[app.editor.row], line_before);

    // 'o' opens line below and enters Edit mode
    app.on_key(char_key('o'));
    assert_eq!(app.mode, Mode::Edit);
    type_str(&mut app, "sub x4, x3, x1");
    app.on_key(key(KeyCode::Esc));
    assert_eq!(app.mode, Mode::Normal);
    assert!(app.editor.text().contains("sub x4, x3, x1"));
}

#[test]
fn test_pane_cycling_zoom_and_command_bar() {
    let mut app = App::new(None);
    app.editor.set_text("addi x1, x0, 42\nebreak x0, x0, 0\n");

    // 1. Pane focus cycling forward (Tab) and backward (BackTab)
    let order = [
        Pane::Listing,
        Pane::Regs,
        Pane::Mem,
        Pane::Exec,
        Pane::Out,
        Pane::Inp,
        Pane::Source,
    ];
    for &expected in &order {
        app.on_key(key(KeyCode::Tab));
        assert_eq!(app.focus, expected);
        assert_eq!(app.mode, Mode::Normal);
    }
    for &expected in order.iter().rev().skip(1) {
        app.on_key(key(KeyCode::BackTab));
        assert_eq!(app.focus, expected);
    }
    app.on_key(key(KeyCode::BackTab));
    assert_eq!(app.focus, Pane::Source);

    // 2. External editor trigger on Source and Fullscreen Zoom on other panes
    app.on_key(key(KeyCode::Enter));
    assert!(app.pending_external_editor);
    app.pending_external_editor = false;

    // Zoom into Listing via Enter
    app.on_key(key(KeyCode::Tab));
    app.on_key(key(KeyCode::Enter));
    assert_eq!(app.zoomed_pane, Some(Pane::Listing));

    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert!(terminal.backend().to_string().contains("[ZOOMED: Listing]"));

    // Tab while zoomed updates zoom target
    app.on_key(key(KeyCode::Tab));
    assert_eq!(app.focus, Pane::Regs);
    assert_eq!(app.zoomed_pane, Some(Pane::Regs));

    // Esc unzooms
    app.on_key(key(KeyCode::Esc));
    assert_eq!(app.zoomed_pane, None);

    // 'z' toggles zoom
    app.on_key(char_key('z'));
    assert_eq!(app.zoomed_pane, Some(Pane::Regs));
    app.on_key(char_key('z'));
    assert_eq!(app.zoomed_pane, None);

    // 3. Command mode execution (:compile, :next, :zoom, :q!)
    app.on_key(char_key(':'));
    assert_eq!(app.mode, Mode::Command);
    type_str(&mut app, "compile");
    app.on_key(key(KeyCode::Enter));
    assert_eq!(app.mode, Mode::Normal);
    assert!(app.sim.listing.contains("ASSEMBLY LISTING"));

    // Step with :next
    app.command_input = "next".to_string();
    app.execute_command();
    assert_eq!(app.sim.regs.pc, 4);
    assert_eq!(app.sim.regs.read_x(1), 42);

    // Zoom with :zoom and unzoom with :z
    app.command_input = "zoom".to_string();
    app.execute_command();
    assert_eq!(app.zoomed_pane, Some(Pane::Regs));
    app.command_input = "z".to_string();
    app.execute_command();
    assert_eq!(app.zoomed_pane, None);

    // Quit with :q!
    app.command_input = "q!".to_string();
    app.execute_command();
    assert!(app.quit_requested);
}

#[test]
fn test_load_and_recompile_clear_all_panes_and_scrolls() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let _ = std::env::set_current_dir(&root);

    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None);
    app.editor.set_text("addi x1, x0, 5\nebreak x0, x0, 0\n");
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();

    // 1. Compile, run a step, populate output and memory, set non-zero scrolls
    app.compile();
    app.action_start();
    app.action_next();
    assert_eq!(app.sim.regs.read_x(1), 5);
    app.sim.output.push_str("Program Output Line\n");
    app.sim.mem.insert_dd(0x1000, "42", "42");
    app.scroll_listing = 5;
    app.scroll_exec = 5;
    app.scroll_regs = 5;
    app.scroll_mem = 5;
    app.scroll_out = 5;

    // 2. Load file via command :e
    app.command_input = "e tests/differential/t1.asm".to_string();
    app.execute_command();

    // Verify all simulator state and scroll offsets are completely reset
    assert!(app.sim.listing.is_empty(), "Listing must be cleared on file load");
    assert!(app.sim.exec_log.is_empty(), "Exec log must be cleared on file load");
    assert!(app.sim.output.is_empty(), "Output must be cleared on file load");
    assert!(app.sim.mem.cells.is_empty(), "Memory must be cleared on file load");
    assert_eq!(app.sim.regs.read_x(1), 0, "Registers must be reset on file load");
    assert_eq!([app.scroll_listing, app.scroll_exec, app.scroll_regs, app.scroll_mem, app.scroll_out], [0; 5]);
    assert!(app.needs_full_clear, "needs_full_clear must be set on file load");

    terminal.clear().unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let frame = terminal.backend().to_string();
    assert!(!frame.contains("Program Output Line"));

    // 3. Compile new file, step, generate output and non-zero scrolls
    app.compile();
    app.action_start();
    app.action_next();
    app.sim.output.push_str("New output\n");
    app.scroll_listing = 3;
    app.scroll_exec = 3;
    app.scroll_regs = 3;
    app.scroll_mem = 3;
    app.scroll_out = 3;

    // Recompile: verify exec_log, output, and all 5 scrolls reset to 0, needs_full_clear set
    app.compile();
    assert!(app.sim.exec_log.is_empty(), "Exec log must be cleared on recompile");
    assert!(app.sim.output.is_empty(), "Output must be cleared on recompile");
    assert_eq!([app.scroll_listing, app.scroll_exec, app.scroll_regs, app.scroll_mem, app.scroll_out], [0; 5]);
    assert!(app.needs_full_clear, "needs_full_clear must be set on recompile");
}

#[test]
fn test_pill_toggles_and_dma_ux() {
    let backend = TestBackend::new(160, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new(None);

    // 1. Regs pane toggles 1 (HEX), 2 (INT), 3 (FLP)
    app.focus = Pane::Regs;
    app.on_key(char_key('1'));
    assert!(!app.sim.view.reg_hex);
    app.on_key(char_key('2'));
    assert!(!app.sim.view.reg_int);
    app.on_key(char_key('3'));
    assert!(!app.sim.view.reg_flp);

    // 2. Mem pane toggles 1 (HEX), 2 (INT), 3 (FLP), 4 (TXT)
    app.focus = Pane::Mem;
    app.on_key(char_key('1'));
    assert!(!app.sim.view.mem_hex);
    app.on_key(char_key('2'));
    assert!(!app.sim.view.mem_int);
    app.on_key(char_key('3'));
    assert!(!app.sim.view.mem_flp);
    app.on_key(char_key('4'));
    assert!(!app.sim.view.mem_txt);

    // 3. DMA unavailable indicators on Out and Inp
    app.focus = Pane::Out;
    app.on_key(char_key('d'));
    assert!(app.status.contains("DMA OUT requires OUTC (0x0) and OUTD (0x8)"));

    app.focus = Pane::Inp;
    app.on_key(char_key('d'));
    assert!(app.status.contains("DMA INP requires INPC (0x10) and INPD (0x18)"));

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert!(terminal.backend().to_string().contains("[DMA:N/A]"));

    // 4. Regs Hex toggle affects rendered register values
    app.focus = Pane::Regs;
    app.zoomed_pane = Some(Pane::Regs);
    app.sim.view.reg_int = true;
    app.sim.view.reg_hex = true;
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert!(terminal.backend().to_string().contains("0x0000000000000000"));

    app.on_key(char_key('1'));
    assert!(!app.sim.view.reg_hex);
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert!(!terminal.backend().to_string().contains("0x0000000000000000"));
    app.zoomed_pane = None;

    // 5. DMA auto-enables on compile with OUTC and OUTD
    app.editor.set_text("OUTC: DD 0\nOUTD: DD 0\nSTART: addi x1, x0, 5\n");
    app.compile();
    assert!(app.dma_out_available);
    assert!(app.sim.view.dma_out);

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert!(terminal.backend().to_string().contains("[DMA:ON]"));
}

#[test]
fn test_listing_correlated_colorizing_and_toggle() {
    let backend = TestBackend::new(140, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None);
    assert!(app.colorize_listing);

    app.editor.set_text("addi x1, x0, 5\nebreak x0, x0, 0\n");
    app.compile();
    app.sim.view.list_bin = true;
    app.focus = Pane::Listing;
    app.zoomed_pane = Some(Pane::Listing);

    // 1. Render and verify correlated bitfield/operand colors when colorize_listing is ON
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer().clone();

    let y = (0..buffer.area.height)
        .find(|&y| {
            let row: String = (0..buffer.area.width).map(|x| buffer[(x, y)].symbol()).collect();
            row.contains("0x0000000000000000") && row.contains("0010011")
        })
        .expect("Should find addi row in Listing buffer");

    let fgs: Vec<Color> = (0..buffer.area.width).map(|x| buffer[(x, y)].fg).collect();
    assert!(fgs.contains(&Color::Cyan), "Opcode/funct should be Cyan");
    assert!(fgs.contains(&Color::LightBlue), "rd should be LightBlue");
    assert!(fgs.contains(&Color::LightMagenta), "rs1 should be LightMagenta");
    assert!(fgs.contains(&Color::LightGreen), "imm should be LightGreen");

    // 2. Toggle colorize OFF via key '2' on Listing pane
    app.on_key(char_key('2'));
    assert!(!app.colorize_listing);

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buf_off = terminal.backend().buffer().clone();
    for x in 22..(buf_off.area.width - 2) {
        let fg = buf_off[(x, y)].fg;
        assert_ne!(fg, Color::LightBlue);
        assert_ne!(fg, Color::LightMagenta);
        assert_ne!(fg, Color::LightGreen);
    }

    // 3. Command toggles :color on / off / toggle
    for (cmd, expected) in [(":color on", true), (":color off", false), (":color", true)] {
        app.command_input = cmd.to_string();
        app.execute_command();
        assert_eq!(app.colorize_listing, expected);
    }
}

#[test]
fn test_tier1_debugging_features() {
    let backend = TestBackend::new(140, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None);
    let src = "START: addi x1, x0, 10\naddi x2, x0, 20\nadd x3, x1, x2\nebreak x0, x0, 0\n";
    app.editor.set_text(src);
    app.compile();

    // 1. Breakpoints on Source lines & PC:
    // a) In Source pane on line 2 (row 1: addi x2, x0, 20), press 'B' in Normal mode
    app.focus = Pane::Source;
    app.editor.row = 1;
    app.on_key(char_key('B'));
    assert!(app.breakpoints.contains(&4), "Line 2 should map to address 0x4");
    assert!(app.has_breakpoint_on_line(1));

    // b) Run ':b' with no args while in Source on line 3 (row 2: add x3, x1, x2)
    app.editor.row = 2;
    app.command_input = ":b".to_string();
    app.execute_command();
    assert!(app.breakpoints.contains(&8), "Line 3 should map to address 0x8");
    assert!(app.has_breakpoint_on_line(2));

    // c) Run ':b pc' to toggle breakpoint specifically at current PC (0x0)
    app.command_input = ":b pc".to_string();
    app.execute_command();
    assert!(app.breakpoints.contains(&0));
    // Toggle 0x0 back off so execution can proceed
    app.command_input = ":b pc".to_string();
    app.execute_command();
    assert!(!app.breakpoints.contains(&0));

    // Render check: both Source and Listing show breakpoint indicators
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let frame = terminal.backend().to_string();
    assert!(frame.contains("●"), "Listing and Source must show breakpoint dot");

    // Run: must stop at first breakpoint (0x4, line 2) before executing it
    app.action_start();
    while app.running {
        app.run_chunk(std::time::Duration::from_millis(50));
    }
    assert_eq!(app.sim.regs.pc, 4);
    assert_eq!(app.sim.regs.read_x(1), 10);
    assert_eq!(app.sim.regs.read_x(2), 0); // line 2 not executed yet
    assert!(app.status.contains("Breakpoint hit"));
    // Continue run from 0x4 to next breakpoint at 0x8
    app.action_run();
    while app.running {
        app.run_chunk(std::time::Duration::from_millis(50));
    }
    assert_eq!(app.sim.regs.pc, 8);
    assert_eq!(app.sim.regs.read_x(1), 10);
    assert_eq!(app.sim.regs.read_x(2), 20);
    assert_eq!(app.sim.regs.read_x(3), 0);
    assert!(app.status.contains("Breakpoint hit"));

    // 2. Step back: execute 1 step forward, then step back
    app.action_next();
    assert_eq!(app.sim.regs.pc, 12);
    assert_eq!(app.sim.regs.read_x(3), 30);

    // Press 'p' to rewind / step back
    app.focus = Pane::Listing;
    app.on_key(char_key('p'));
    assert_eq!(app.sim.regs.pc, 8);
    assert_eq!(app.sim.regs.read_x(3), 0);
    assert!(app.status.contains("Stepped back"));

    // 3. Execution speed adjustments:
    assert_eq!(app.run_speed, rvs_tui::app::RunSpeed::Max);
    app.on_key(char_key('+'));
    assert_eq!(app.run_speed, rvs_tui::app::RunSpeed::Hz1);
    app.command_input = ":speed 20".to_string();
    app.execute_command();
    assert_eq!(app.run_speed, rvs_tui::app::RunSpeed::Hz20);

    // 4. Memory navigation & stack pointer jump
    app.sim.regs.write_x(2, 0x2000, false);
    app.sim.mem.insert_dd(0x2000, "12345", "12345");
    app.focus = Pane::Mem;
    app.command_input = ":mem sp".to_string();
    app.execute_command();
    assert!(app.status.contains("Jumped memory view to 0x0000000000002000"));

    // Button [SP] in Mem toolbar (slot 5)
    app.toggle_pane_option(Pane::Mem, 5);
    assert!(app.status.contains("Jumped memory view to 0x0000000000002000"));
}

#[test]
fn test_tier2_execution_metrics_and_calling_convention() {
    let backend = TestBackend::new(140, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None);
    let src = "START: addi sp, x0, 8\njal ra, func\nebreak x0, x0, 0\nfunc: addi s0, x0, 99\njalr x0, ra, 0\n";
    app.editor.set_text(src);
    app.compile();
    assert!(app.sim.convention.enabled);

    // Run to completion
    app.action_start();
    while app.running {
        app.run_chunk(std::time::Duration::from_millis(50));
    }

    // 1. Verify metrics counter accumulated accurately
    let m = &app.sim.metrics;
    assert!(m.instructions >= 4, "Should execute at least 4 instructions");
    assert!(m.cycles >= m.instructions, "Cycles should equal or exceed instruction count");
    assert_eq!(m.jumps, 2, "Should record 2 jumps (jal + jalr)");

    // 2. Verify calling convention checker caught both violations
    let cc = &app.sim.convention;
    assert!(cc.violations.len() >= 2, "Must detect stack misalignment and saved register clobber");
    assert!(cc.violations.iter().any(|v| v.contains("Stack pointer sp is not 16-byte aligned")));
    assert!(cc.violations.iter().any(|v| v.contains("Saved register s0 was modified")));

    // Execution log should contain WARN messages
    assert!(app.sim.exec_log.contains("WARN [Convention]:"));

    // 3. Open :stats modal and verify rendered output
    app.command_input = ":stats".to_string();
    app.execute_command();
    assert_eq!(app.modal, Modal::Stats);

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let frame = terminal.backend().to_string();
    assert!(frame.contains("Execution Metrics & Hardware Counters"));
    assert!(frame.contains("Total Instructions:"));
    assert!(frame.contains("Average CPI:"));
    assert!(frame.contains("Calling Convention Compliance Checker"));
    assert!(frame.contains("Recent Convention Violations:"));

    // Close stats modal with Esc
    app.on_key(key(KeyCode::Esc));
    assert_eq!(app.modal, Modal::None);

    // 4. Test toggling convention checker with command :cc off / :cc on
    app.command_input = ":cc off".to_string();
    app.execute_command();
    assert!(!app.sim.convention.enabled);
    app.command_input = ":cc on".to_string();
    app.execute_command();
    assert!(app.sim.convention.enabled);
}

#[test]
fn test_gnu_assembly_dialect_and_transpiler() {
    let mut app = App::new(None);

    // 1. Verify GNU mode assembling standard GNU syntax & pseudo-instructions
    app.command_input = ":mode gnu".to_string();
    app.execute_command();
    assert_eq!(app.sim.dialect, rvs_core::transpiler::AssemblyDialect::Gnu);

    let gnu_source = r#"
.globl main
.text
main:
    li   a0, 42
    mv   a1, a0
    nop
    call helper
    ebreak x0, x0, 0

helper:
    addi a0, a0, 10
    ret
"#;
    app.editor.set_text(gnu_source);
    app.compile();
    assert!(!app.sim.listing.contains("ERROR:"), "Listing had error: {}", app.sim.listing);

    app.action_start();
    while app.running {
        app.run_chunk(std::time::Duration::from_millis(50));
    }

    // a0 should be 42 + 10 = 52, a1 should be 42
    assert_eq!(app.sim.regs.read_x(10), 52); // a0 = x10
    assert_eq!(app.sim.regs.read_x(11), 42); // a1 = x11

    // 2. Test in-place buffer transpiler commands
    app.command_input = ":transpile rvs".to_string();
    app.execute_command();
    let rvs_buf = app.editor.text();
    assert!(rvs_buf.contains("addi a0, x0, 42"));
    assert!(rvs_buf.contains("addi a1, a0, 0"));
    assert!(rvs_buf.contains("jalr x0, ra, 0"));

    app.command_input = ":transpile gnu".to_string();
    app.execute_command();
    let gnu_buf = app.editor.text();
    assert!(gnu_buf.contains("li a0, 42"));
    assert!(gnu_buf.contains("mv a1, a0"));
    assert!(gnu_buf.contains("ret"));

    // 3. Verify repeatedly running transpiler does NOT add spurious newlines
    let line_count_before = app.editor.lines.len();
    app.command_input = ":tp rvs".to_string();
    app.execute_command();
    assert_eq!(app.editor.lines.len(), line_count_before, "Transpiler must not add extra newlines");
    app.command_input = ":tp gnu".to_string();
    app.execute_command();
    assert_eq!(app.editor.lines.len(), line_count_before, "Transpiler must not add extra newlines");

    // 4. Test Source toolbar buttons: [RVS] (slot 1), [GNU] (slot 2), [TRANSPILE] (slot 3)
    let backend = TestBackend::new(140, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let frame = terminal.backend().to_string();
    assert!(frame.contains("[RVS]"));
    assert!(frame.contains("[GNU]"));
    assert!(frame.contains("[TRANSPILE]"));
    assert!(!frame.contains("Source [RVS]"));
    assert!(!frame.contains("Source [GNU]"));

    app.toggle_pane_option(Pane::Source, 1);
    assert_eq!(app.sim.dialect, rvs_core::transpiler::AssemblyDialect::Rvs);
    app.toggle_pane_option(Pane::Source, 2);
    assert_eq!(app.sim.dialect, rvs_core::transpiler::AssemblyDialect::Gnu);
    app.toggle_pane_option(Pane::Source, 3);
    assert_eq!(app.sim.dialect, rvs_core::transpiler::AssemblyDialect::Rvs);
}

#[test]
fn test_call_stack_and_frame_visualizer() {
    let backend = TestBackend::new(140, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None);
    // A program with a call frame that allocates stack and saves return address
    let src = r#"
START:  addi sp, x0, 0x100      # 16-byte aligned sp = 256
        jal  ra, outer
        ebreak x0, x0, 0

outer:  addi sp, sp, -16        # allocate frame
        sd   ra, 8(sp)          # save return address
        jal  ra, inner
        ld   ra, 8(sp)          # restore return address
        addi sp, sp, 16         # deallocate frame
        jalr x0, ra, 0          # return

inner:  ebreak x0, x0, 0        # pause execution inside inner function
        jalr x0, ra, 0
"#;
    app.editor.set_text(src);
    app.compile();

    // Run until ebreak inside `inner`
    app.action_start();
    while app.running {
        app.run_chunk(std::time::Duration::from_millis(50));
    }

    // 1. Verify CallFrame stack inspection
    let frames = app.sim.get_stack_frames();
    assert_eq!(frames.len(), 2, "Should have 2 active call frames (outer and inner)");

    let outer_frame = &frames[0];
    assert_eq!(outer_frame.function_name, "outer");
    assert_eq!(outer_frame.slots.len(), 2, "16-byte frame has 2 double-word slots");
    assert!(outer_frame.slots.iter().any(|s| s.annotation.contains("Saved ra")), "Must identify saved ra slot");

    let inner_frame = &frames[1];
    assert_eq!(inner_frame.function_name, "inner");

    // 2. Open :stack modal and test UI rendering
    app.command_input = ":stack".to_string();
    app.execute_command();
    assert_eq!(app.modal, Modal::CallStack);

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let frame = terminal.backend().to_string();
    assert!(frame.contains("Call Stack & Stack Frame Visualizer"));
    assert!(frame.contains("Current $sp:"));
    assert!(frame.contains("16-byte ALIGNED ✓"));
    assert!(frame.contains("Frame 0: outer"));
    assert!(frame.contains("Frame 1: inner"));
    assert!(frame.contains("Saved ra"));

    // Close modal with Esc
    app.on_key(key(KeyCode::Esc));
    assert_eq!(app.modal, Modal::None);
}
