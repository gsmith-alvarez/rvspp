//! Frame layout, status bars, modal dispatch and mouse hit-testing.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use rvs_app::{App, Modal, Mode, Pane};
use rvs_machine::Machine;
use rvs_app_cmds::AppCommands;
use rvs_ui_modal::{centered_rect, draw_modal};
use rvs_ui_panes::{
    draw_exec, draw_inp, draw_listing, draw_mem, draw_out, draw_regs, draw_source,
};


pub struct LayoutAreas {
    pub top: Rect,
    pub source: Rect,
    pub listing: Rect,
    pub regs: Rect,
    pub mem: Rect,
    pub exec: Rect,
    pub out: Rect,
    pub inp: Rect,
    pub bottom: Rect,
    pub middle: Rect,
}
impl LayoutAreas {
    pub fn pane_rect(&self, pane: Pane, zoomed: Option<Pane>) -> Option<Rect> {
        if let Some(z) = zoomed {
            return (z == pane).then_some(self.middle);
        }
        Some(match pane {
            Pane::Source => self.source,
            Pane::Listing => self.listing,
            Pane::Regs => self.regs,
            Pane::Mem => self.mem,
            Pane::Exec => self.exec,
            Pane::Out => self.out,
            Pane::Inp => self.inp,
        })
    }

    pub fn hit_test_pane(&self, col: u16, row: u16, zoomed: Option<Pane>) -> Option<(Pane, Rect)> {
        for pane in Pane::ALL {
            if let Some(r) = self.pane_rect(pane, zoomed) {
                if col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height {
                    return Some((pane, r));
                }
            }
        }
        None
    }
}


pub fn compute_layout(area: Rect) -> LayoutAreas {
    let vert_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let top = vert_chunks[0];
    let middle = vert_chunks[1];
    let bottom = vert_chunks[2];

    let mid_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(48),
            Constraint::Percentage(52),
        ])
        .split(middle);

    let upper_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(mid_chunks[0]);

    let lower_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(mid_chunks[1]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(lower_chunks[3]);

    LayoutAreas {
        top,
        source: upper_chunks[0],
        listing: upper_chunks[1],
        regs: lower_chunks[0],
        mem: lower_chunks[1],
        exec: lower_chunks[2],
        out: right_chunks[0],
        inp: right_chunks[1],
        bottom,
        middle,
    }
}

pub fn draw<M: Machine>(f: &mut Frame, app: &mut App<M>) {
    let area = f.area();
    app.last_area = area;
    // Paint the whole frame first so no widget leaks the terminal background.
    f.render_widget(Block::default().style(rvs_theme::PANE), area);
    let l = compute_layout(area);
    let view = app.machine.view(&app.view_request());
    draw_top_bar(f, app, l.top);

    if let Some(pane) = app.zoomed_pane {
        match pane {
            Pane::Source => draw_source(f, app, l.middle),
            Pane::Listing => draw_listing(f, app, &view, l.middle),
            Pane::Regs => draw_regs(f, app, &view, l.middle),
            Pane::Mem => draw_mem(f, app, &view, l.middle),
            Pane::Exec => draw_exec(f, app, &view, l.middle),
            Pane::Out => draw_out(f, app, &view, l.middle),
            Pane::Inp => draw_inp(f, app, &view, l.middle),
        }
    } else {
        draw_source(f, app, l.source);
        draw_listing(f, app, &view, l.listing);
        draw_regs(f, app, &view, l.regs);
        draw_mem(f, app, &view, l.mem);
        draw_exec(f, app, &view, l.exec);
        draw_out(f, app, &view, l.out);
        draw_inp(f, app, &view, l.inp);
    }

    draw_bottom_bar(f, app, l.bottom);

    if app.modal != Modal::None {
        draw_modal(f, app, area);
    }
}

fn draw_top_bar<M: Machine>(f: &mut Frame, app: &App<M>, area: Rect) {
    f.render_widget(Block::default().style(rvs_theme::BAR), area);
    let modified = if app.editor.needs_save() { "[*]" } else { "" };
    let fname = if app.fname.is_empty() {
        "<untitled>"
    } else {
        &app.fname
    };
    let mode_str = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::Edit => "INSERT",
        Mode::Input => "INPUT",
        Mode::Running => "RUNNING",
        Mode::Command => "COMMAND",
    };
    let mode_style = match app.mode {
        Mode::Normal => rvs_theme::INFO,
        Mode::Edit => rvs_theme::OK,
        Mode::Input => rvs_theme::FOCUS,
        Mode::Running => rvs_theme::ERR,
        Mode::Command => Style::default().fg(rvs_theme::MAUVE).add_modifier(Modifier::BOLD),
    };

    let title_span = Span::styled(
        " RVS++ v1.0.0 ",
        rvs_theme::BADGE_INFO,
    );
    let fname_span = Span::styled(
        format!(" Fname: {fname}{modified} "),
        if app.editor.needs_save() {
            Style::default().bg(rvs_theme::SURFACE0).fg(rvs_theme::YELLOW).add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(rvs_theme::SURFACE0).fg(rvs_theme::TEXT).add_modifier(Modifier::BOLD)
        },
    );
    let mode_span = Span::styled(format!(" [{mode_str}] "), mode_style);
    let zoom_span = if let Some(pane) = app.zoomed_pane {
        Span::styled(
            format!(" [ZOOMED: {pane:?}] "),
            rvs_theme::BADGE_ACCENT,
        )
    } else {
        Span::raw("")
    };
    let hints_span = Span::styled(
        " ?:Help ^O:Load ^S:Save c:Compile s:START n:Next r:Run x:Stop Tab/S-Tab:Pane Enter/z:Zoom/Edit :Cmd q:Quit ",
        rvs_theme::MUTED,
    );

    let line = Line::from(vec![title_span, fname_span, mode_span, zoom_span, hints_span]);
    f.render_widget(Paragraph::new(line), area);
}

fn draw_bottom_bar<M: Machine>(f: &mut Frame, app: &App<M>, area: Rect) {
    f.render_widget(Block::default().style(rvs_theme::BAR), area);
    if app.mode == Mode::Command {
        let line = Line::from(vec![
            Span::styled(":", rvs_theme::FOCUS),
            Span::styled(&app.command_input, Style::default().fg(rvs_theme::TEXT).add_modifier(Modifier::BOLD)),
        ]);
        f.render_widget(Paragraph::new(line), area);
        let cursor_col = area.x + 1 + app.command_input.chars().count() as u16;
        if cursor_col < area.x + area.width {
            f.set_cursor_position((cursor_col, area.y));
        }
    } else if !app.status.is_empty() {
        let is_err = app.status.starts_with("ERROR");
        let style = if is_err {
            rvs_theme::ERR
        } else {
            rvs_theme::OK_PLAIN
        };
        f.render_widget(Paragraph::new(Span::styled(format!(" {}", app.status), style)), area);
    } else if let Some(pane) = app.zoomed_pane {
        let hint = Line::from(vec![
            Span::styled(format!(" [ZOOMED: {pane:?}] "), rvs_theme::BADGE_ACCENT),
            Span::styled(" Press Enter, z, or Esc to restore 6-pane view", rvs_theme::MUTED),
        ]);
        f.render_widget(Paragraph::new(hint), area);
    }
}


pub fn handle_click<M: Machine>(app: &mut App<M>, col: u16, row: u16) {
    if let Modal::FilePrompt(_) = app.modal {
        let modal_area = centered_rect(84, 82, app.last_area);
        let inner = Block::default().borders(Borders::ALL).inner(modal_area);
        if inner.height >= 6 && inner.width >= 24 {
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(inner);
            let split_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
                .split(main_chunks[1]);
            let list_inner = Block::default().borders(Borders::ALL).inner(split_chunks[0]);
            if col >= list_inner.x && col < list_inner.x + list_inner.width && row >= list_inner.y && row < list_inner.y + list_inner.height {
                let clicked_row = (row - list_inner.y) as usize;
                let clicked_idx = app.explorer.scroll_list + clicked_row;
                if clicked_idx < app.explorer.entries.len() {
                    if clicked_idx == app.explorer.selected {
                        app.explorer_open_selected();
                    } else {
                        app.explorer.selected = clicked_idx;
                        app.explorer.update_preview();
                    }
                }
            }
        }
        return;
    }
    let l = compute_layout(app.last_area);
    if let Some((pane, rect)) = l.hit_test_pane(col, row, app.zoomed_pane) {
        app.focus = pane;
        if pane == Pane::Inp {
            app.mode = Mode::Input;
        } else if app.mode != Mode::Input && app.mode != Mode::Running {
            app.mode = Mode::Normal;
        }
        let is_toolbar_row = row == rect.y + rect.height.saturating_sub(2);
        if is_toolbar_row {
            let rel_x = col.saturating_sub(rect.x + 1);
            match pane {
                Pane::Listing => {
                    if rel_x < 10 {
                        app.reset_pc();
                    } else if rel_x >= 31 && rel_x < 38 {
                        app.toggle_pane_option(Pane::Listing, 1);
                    } else if rel_x >= 38 && rel_x < 45 {
                        app.toggle_pane_option(Pane::Listing, 2);
                    } else if rel_x >= 45 && rel_x < 56 {
                        app.toggle_pane_option(Pane::Listing, 3);
                    } else if rel_x >= 56 && rel_x < 63 {
                        app.toggle_pane_option(Pane::Listing, 4);
                    }
                }
                Pane::Regs => {
                    if rel_x < 7 {
                        app.toggle_pane_option(Pane::Regs, 1);
                    } else if rel_x >= 7 && rel_x < 14 {
                        app.toggle_pane_option(Pane::Regs, 2);
                    } else if rel_x >= 14 && rel_x < 21 {
                        app.toggle_pane_option(Pane::Regs, 3);
                    }
                }
                Pane::Mem => {
                    if rel_x < 7 {
                        app.toggle_pane_option(Pane::Mem, 1);
                    } else if rel_x >= 7 && rel_x < 14 {
                        app.toggle_pane_option(Pane::Mem, 2);
                    } else if rel_x >= 14 && rel_x < 21 {
                        app.toggle_pane_option(Pane::Mem, 3);
                    } else if rel_x >= 21 && rel_x < 28 {
                        app.toggle_pane_option(Pane::Mem, 4);
                    } else if rel_x >= 28 && rel_x < 34 {
                        app.toggle_pane_option(Pane::Mem, 5);
                    } else if rel_x >= 34 && rel_x < 42 {
                        app.toggle_pane_option(Pane::Mem, 6);
                    } else if rel_x >= 42 && rel_x < 52 {
                        app.toggle_pane_option(Pane::Mem, 7);
                    }
                }
                Pane::Exec => {
                    if rel_x < 9 {
                        app.clear_pane(Pane::Exec);
                    } else if rel_x >= 9 && rel_x < 18 {
                        app.toggle_pane_option(Pane::Exec, 1);
                    } else if rel_x >= 18 && rel_x < 28 {
                        app.toggle_pane_option(Pane::Exec, 2);
                    } else if rel_x >= 28 && rel_x < 38 {
                        app.toggle_pane_option(Pane::Exec, 3);
                    }
                }
                Pane::Out => {
                    if rel_x < 9 {
                        app.clear_pane(Pane::Out);
                    } else {
                        app.toggle_pane_option(Pane::Out, 1);
                    }
                }
                Pane::Inp => {
                    if rel_x < 19 {
                        app.clear_pane(Pane::Inp);
                    } else {
                        app.toggle_pane_option(Pane::Inp, 1);
                    }
                }
                Pane::Source => {
                    if rel_x < 7 {
                        app.toggle_pane_option(Pane::Source, 1);
                    } else if rel_x >= 7 && rel_x < 14 {
                        app.toggle_pane_option(Pane::Source, 2);
                    } else if rel_x >= 14 && rel_x < 28 {
                        app.toggle_pane_option(Pane::Source, 3);
                    }
                }
            }
        }
    }
}

pub fn handle_scroll<M: Machine>(app: &mut App<M>, col: u16, row: u16, delta: i32) {
    if let Modal::FilePrompt(_) = app.modal {
        let modal_area = centered_rect(84, 82, app.last_area);
        let inner = Block::default().borders(Borders::ALL).inner(modal_area);
        if inner.height >= 6 && inner.width >= 24 {
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(inner);
            let split_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
                .split(main_chunks[1]);
            let list_area = split_chunks[0];
            let prev_area = split_chunks[1];
            if col >= list_area.x && col < list_area.x + list_area.width {
                if delta < 0 {
                    app.explorer.select_prev();
                } else {
                    app.explorer.select_next();
                }
            } else if col >= prev_area.x && col < prev_area.x + prev_area.width {
                if delta < 0 {
                    app.explorer.scroll_preview = app.explorer.scroll_preview.saturating_sub(3);
                } else {
                    app.explorer.scroll_preview = app.explorer.scroll_preview.saturating_add(3);
                }
            }
        }
        return;
    }
    let l = compute_layout(app.last_area);
    if let Some((pane, _)) = l.hit_test_pane(col, row, app.zoomed_pane) {
        app.scroll_pane(pane, delta);
    }
}


    pub fn handle_mouse<M: Machine>(app: &mut App<M>, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                handle_click(app, mouse.column, mouse.row);
            }
            MouseEventKind::ScrollUp => handle_scroll(app, mouse.column, mouse.row, -3),
            MouseEventKind::ScrollDown => handle_scroll(app, mouse.column, mouse.row, 3),
            _ => {}
        }
    }


#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use rvs_machine::{
        Dialect, LoadOutcome, Machine, RegisterChanges, RichLine, Segment, StepOutcome,
        ViewRequest, ViewState,
    };

    #[derive(Clone)]
    struct MockSnapshot;

    /// A machine with no RISC-V dependency at all: proves the UI stays generic.
    struct MockMachine;

    impl Machine for MockMachine {
        type Snapshot = MockSnapshot;

        fn new() -> Self {
            MockMachine
        }
        fn load(&mut self, _source: &str) -> LoadOutcome {
            LoadOutcome::Ok
        }
        fn reset(&mut self) {}
        fn start(&mut self) {}
        fn step(&mut self) -> StepOutcome {
            StepOutcome::Halted
        }
        fn request_stop(&mut self) {}
        fn stop_requested(&self) -> bool {
            false
        }
        fn clear_stop_request(&mut self) {}
        fn snapshot(&self) -> MockSnapshot {
            MockSnapshot
        }
        fn restore(&mut self, _snapshot: &MockSnapshot) {}
        fn changes_since(&self, _snapshot: &MockSnapshot) -> RegisterChanges {
            RegisterChanges::default()
        }
        fn view(&self, request: &ViewRequest) -> ViewState {
            ViewState {
                listing: vec![RichLine::plain("mocked-listing")],
                registers: vec![RichLine::plain("mocked-reg")],
                memory: vec![RichLine::plain("mocked-mem")],
                exec: vec![RichLine::plain("mocked-log")],
                output: vec![RichLine::plain("mocked-out")],
                prompt: None,
                dialect: Dialect::Rvs,
                dma_out_available: false,
                dma_inp_available: false,
                pc: request.pc,
            }
        }
        fn highlight_source_line(&self, line: &str) -> Vec<Segment> {
            vec![Segment::plain(line)]
        }
        fn dialect(&self) -> Dialect {
            Dialect::Rvs
        }
        fn pc(&self) -> u64 {
            0
        }
        fn reset_pc(&mut self) {}
        fn submit_input(&mut self, _value: &str) -> Result<(), String> {
            Ok(())
        }
        fn cancel_input(&mut self) {}
    }

    #[test]
    fn renders_a_non_riscv_machine_through_the_neutral_view_model() {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::<MockMachine>::new(None);
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let frame = terminal.backend().to_string();
        assert!(frame.contains("mocked-listing"));
        assert!(frame.contains("mocked-reg"));
        assert!(frame.contains("mocked-mem"));
        assert!(frame.contains("mocked-log"));
        assert!(frame.contains("mocked-out"));
    }
}
