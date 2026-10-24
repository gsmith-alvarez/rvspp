//! Pane renderers for the RVS++ TUI.
//!
//! These renderers are target-agnostic: they consume [`ViewState`] produced by
//! a [`Machine`] and never import a language/ISA crate.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use rvs_app::{App, Modal, Mode, Pane};
use rvs_machine::{expand_tabs, visual_col_at, Machine, RichLine, ViewState};

pub fn pane_block<'a>(title: &'a str, active: bool, zoomed: bool) -> Block<'a> {
    let border_style = if active {
        rvs_theme::FOCUS
    } else {
        rvs_theme::FAINT
    };
    let title_str = if zoomed {
        format!(" [ZOOMED] {title} ")
    } else {
        format!(" {title} ")
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(if active {
            BorderType::Double
        } else {
            BorderType::Plain
        })
        .border_style(border_style)
        .style(rvs_theme::PANE)
        .title(Span::styled(
            title_str,
            if active {
                rvs_theme::FOCUS
            } else {
                rvs_theme::TEXT_STYLE
            },
        ))
}

pub fn pill<'a>(label: &'a str, on: bool) -> Span<'a> {
    if on {
        Span::styled(format!(" [{label}] "), rvs_theme::BADGE_ON)
    } else {
        Span::styled(format!(" [{label}] "), rvs_theme::MUTED)
    }
}

pub fn btn<'a>(label: &'a str) -> Span<'a> {
    Span::styled(format!(" [{label}] "), rvs_theme::INFO)
}

fn render_line(line: &RichLine) -> Line<'_> {
    Line::from(
        line.segments
            .iter()
            .map(|s| Span::styled(s.text.clone(), rvs_theme::tone_style(s.tone)))
            .collect::<Vec<_>>(),
    )
}

fn render_lines<'a>(lines: &'a [RichLine], skip: usize, take: usize) -> Vec<Line<'a>> {
    lines
        .iter()
        .skip(skip)
        .take(take)
        .map(render_line)
        .collect()
}

pub fn draw_source<M: Machine>(f: &mut Frame, app: &mut App<M>, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Source, area)
    else {
        return;
    };
    let active = app.focus == Pane::Source;
    let height = text_area.height as usize;
    if app.editor.row < app.editor.offset_row {
        app.editor.offset_row = app.editor.row;
    } else if app.editor.row >= app.editor.offset_row + height {
        app.editor.offset_row = app.editor.row - height + 1;
    }

    let mut lines = Vec::new();
    let num_digits = format!("{}", app.editor.lines.len().max(1)).len().max(3);

    for (idx, line_str) in app
        .editor
        .lines
        .iter()
        .enumerate()
        .skip(app.editor.offset_row)
        .take(height)
    {
        let is_cur = idx == app.editor.row && active;
        let has_bp = app.has_breakpoint_on_line(idx);
        let bp_mark = if has_bp {
            Span::styled("● ", rvs_theme::ERR)
        } else {
            Span::raw("  ")
        };
        let num_str = format!("{line:>width$} ", line = idx + 1, width = num_digits);
        let num_style = if has_bp {
            rvs_theme::ERR
        } else if is_cur {
            rvs_theme::FOCUS
        } else {
            rvs_theme::MUTED
        };

        let expanded = expand_tabs(line_str, 8);
        let segments = app.machine.highlight_source_line(&expanded);
        let mut spans: Vec<Span> = segments
            .iter()
            .map(|s| Span::styled(s.text.clone(), rvs_theme::tone_style(s.tone)))
            .collect();
        spans.insert(0, Span::styled(num_str, num_style));
        spans.insert(0, bp_mark);
        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), text_area);

    let dialect = app.machine.dialect();
    let is_rvs = dialect == rvs_machine::Dialect::Rvs;
    let is_gnu = dialect == rvs_machine::Dialect::Gnu;
    let toolbar = Line::from(vec![
        pill("RVS", is_rvs),
        pill("GNU", is_gnu),
        btn("TRANSPILE"),
    ]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);

    if active && (app.mode == Mode::Edit || app.mode == Mode::Normal) && app.modal == Modal::None {
        let visual_col = if let Some(line) = app.editor.lines.get(app.editor.row) {
            visual_col_at(line, app.editor.col, 8)
        } else {
            app.editor.col
        };
        let screen_row =
            text_area.y + (app.editor.row.saturating_sub(app.editor.offset_row)) as u16;
        let screen_col = text_area.x
            + 2
            + (num_digits + 1) as u16
            + (visual_col.saturating_sub(app.editor.offset_col)) as u16;
        if screen_row < text_area.y + text_area.height && screen_col < text_area.x + text_area.width
        {
            f.set_cursor_position((screen_col, screen_row));
        }
    }
}

pub fn draw_listing<M: Machine>(f: &mut Frame, app: &mut App<M>, view: &ViewState, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Listing, area)
    else {
        return;
    };
    let height = text_area.height as usize;
    let lines = render_lines(&view.listing, app.scroll_listing, height);
    f.render_widget(Paragraph::new(lines), text_area);

    let spd_label = format!("SPD:{}", app.run_speed.label());
    let toolbar = Line::from(vec![
        btn("InitPC"),
        Span::styled(format!(" PC:0x{:016x} ", view.pc), rvs_theme::TEXT_STYLE),
        pill("TXT", app.view.exec_txt),
        pill("CLR", app.colorize_listing),
        pill(&spd_label, app.run_speed != rvs_app::RunSpeed::Max),
        pill("BP", !app.breakpoints.is_empty()),
    ]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

pub fn draw_regs<M: Machine>(f: &mut Frame, app: &mut App<M>, view: &ViewState, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Regs, area) else {
        return;
    };
    let height = text_area.height as usize;
    let lines = render_lines(&view.registers, app.scroll_regs, height);
    f.render_widget(Paragraph::new(lines), text_area);

    let toolbar = Line::from(vec![
        pill("HEX", app.view.reg_hex),
        pill("INT", app.view.reg_int),
        pill("FLP", app.view.reg_flp),
    ]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

pub fn draw_mem<M: Machine>(f: &mut Frame, app: &mut App<M>, view: &ViewState, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Mem, area) else {
        return;
    };
    let height = text_area.height as usize;
    let lines = render_lines(&view.memory, app.scroll_mem, height);
    f.render_widget(Paragraph::new(lines), text_area);

    let toolbar = Line::from(vec![
        pill("HEX", app.view.mem_hex),
        pill("INT", app.view.mem_int),
        pill("FLP", app.view.mem_flp),
        pill("TXT", app.view.mem_txt),
        btn("SP"),
        btn("0x0"),
        btn("STACK"),
    ]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

pub fn draw_exec<M: Machine>(f: &mut Frame, app: &mut App<M>, view: &ViewState, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Exec, area) else {
        return;
    };
    let height = text_area.height as usize;
    let total_lines = view.exec.len();
    let auto_scroll = total_lines.saturating_sub(height);
    let scroll = if app.scroll_exec == 0 {
        auto_scroll
    } else {
        app.scroll_exec
    };
    let lines = render_lines(&view.exec, scroll, height);
    f.render_widget(Paragraph::new(lines), text_area);

    let enabled = app.machine.convention_enabled();
    let cc_label = if enabled { "CC:ON" } else { "CC:OFF" };
    let toolbar = Line::from(vec![
        btn("Clear"),
        btn("STATS"),
        pill(cc_label, enabled),
        btn("STACK"),
    ]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

pub fn draw_out<M: Machine>(f: &mut Frame, app: &mut App<M>, view: &ViewState, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Out, area) else {
        return;
    };
    let height = text_area.height as usize;
    let total = view.output.len();
    let scroll = if app.scroll_out == 0 {
        total.saturating_sub(height)
    } else {
        app.scroll_out
    };
    let lines = render_lines(&view.output, scroll, height);
    f.render_widget(Paragraph::new(lines), text_area);

    let dma_status = if view.dma_out_available {
        pill(
            if app.view.dma_out {
                "DMA:ON"
            } else {
                "DMA:OFF"
            },
            app.view.dma_out,
        )
    } else {
        Span::styled(" [DMA:N/A] ", rvs_theme::MUTED)
    };
    let toolbar = Line::from(vec![btn("Clear"), dma_status]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

pub fn draw_inp<M: Machine>(f: &mut Frame, app: &mut App<M>, view: &ViewState, area: Rect) {
    let Some((entry_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Inp, area) else {
        return;
    };

    let prompt = view.prompt.as_ref();
    let awaiting = prompt.is_some();
    let prompt_style = if awaiting && app.blink_on() {
        rvs_theme::BADGE_ERR
    } else if awaiting {
        rvs_theme::BADGE_NEUTRAL
    } else {
        rvs_theme::MUTED
    };
    let label = prompt.map(|p| p.label.as_str()).unwrap_or("  ");

    let entry_line = Line::from(vec![
        Span::styled(format!(" {label} "), prompt_style),
        Span::raw(" "),
        Span::styled(&app.input_line, rvs_theme::FOCUS),
    ]);
    f.render_widget(Paragraph::new(entry_line), entry_area);

    if (app.focus == Pane::Inp || app.mode == Mode::Input) && app.mode == Mode::Input {
        let screen_col = entry_area.x + 5 + app.input_line.chars().count() as u16;
        let screen_row = entry_area.y;
        if screen_col < entry_area.x + entry_area.width {
            f.set_cursor_position((screen_col, screen_row));
        }
    }

    let dma_status = if view.dma_inp_available {
        pill(
            if app.view.dma_inp {
                "DMA:ON"
            } else {
                "DMA:OFF"
            },
            app.view.dma_inp,
        )
    } else {
        Span::styled(" [DMA:N/A] ", rvs_theme::MUTED)
    };
    let toolbar = Line::from(vec![btn("Enter"), btn("Cancel"), dma_status]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

fn render_pane_with_toolbar<M: Machine>(
    f: &mut Frame,
    app: &App<M>,
    pane: Pane,
    area: Rect,
) -> Option<(Rect, Rect)> {
    let active = app.focus == pane || (pane == Pane::Inp && app.mode == Mode::Input);
    let is_zoomed = app.zoomed_pane == Some(pane);
    let block = pane_block(pane.title(), active, is_zoomed);
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    if inner.height < 2 {
        return None;
    }
    f.render_widget(Block::default().style(rvs_theme::PANE), inner);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    Some((chunks[0], chunks[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visual_col_at() {
        assert_eq!(visual_col_at("", 0, 8), 0);
        assert_eq!(visual_col_at("", 5, 8), 5);
        assert_eq!(visual_col_at("hello", 3, 8), 3);
        assert_eq!(visual_col_at("hello", 10, 8), 10);

        assert_eq!(visual_col_at("\taddi", 0, 8), 0);
        assert_eq!(visual_col_at("\taddi", 1, 8), 8);
        assert_eq!(visual_col_at("\taddi", 2, 8), 9);

        assert_eq!(visual_col_at("a\tb", 0, 8), 0);
        assert_eq!(visual_col_at("a\tb", 1, 8), 1);
        assert_eq!(visual_col_at("a\tb", 2, 8), 8);
        assert_eq!(visual_col_at("a\tb", 3, 8), 9);

        assert_eq!(visual_col_at("\t\tx", 0, 8), 0);
        assert_eq!(visual_col_at("\t\tx", 1, 8), 8);
        assert_eq!(visual_col_at("\t\tx", 2, 8), 16);
        assert_eq!(visual_col_at("\t\tx", 3, 8), 17);
    }

    #[test]
    fn test_visual_col_wide_and_combining() {
        assert_eq!(visual_col_at("日本", 1, 8), 2);
        assert_eq!(visual_col_at("日本", 2, 8), 4);
        assert_eq!(visual_col_at("e\u{301}", 1, 8), 0);
        assert_eq!(visual_col_at("e\u{301}", 2, 8), 1);
        assert_eq!(expand_tabs("日\tx", 8), "日      x");
    }
}
