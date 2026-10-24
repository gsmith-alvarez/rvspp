//! 6-pane TUI layout, styling, modal overlays and mouse hit-testing.
//!
//! Mirrors `RVS046.tcl`'s visual grid in `ratatui`:
//! - Top status/menu bar (1 row)
//! - Upper split (48%): Source (editor) + Listing (assembly & controls)
//! - Lower split (52%): Regs | Mem | Exec | OUT/INP

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Paragraph, Wrap,
};
use ratatui::Frame;

use crate::app::{App, ConfirmAction, FileAction, Modal, Mode, Pane};

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

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    app.last_area = area;
    let l = compute_layout(area);
    draw_top_bar(f, app, l.top);

    if let Some(pane) = app.zoomed_pane {
        match pane {
            Pane::Source => draw_source(f, app, l.middle),
            Pane::Listing => draw_listing(f, app, l.middle),
            Pane::Regs => draw_regs(f, app, l.middle),
            Pane::Mem => draw_mem(f, app, l.middle),
            Pane::Exec => draw_exec(f, app, l.middle),
            Pane::Out => draw_out(f, app, l.middle),
            Pane::Inp => draw_inp(f, app, l.middle),
        }
    } else {
        draw_source(f, app, l.source);
        draw_listing(f, app, l.listing);
        draw_regs(f, app, l.regs);
        draw_mem(f, app, l.mem);
        draw_exec(f, app, l.exec);
        draw_out(f, app, l.out);
        draw_inp(f, app, l.inp);
    }

    draw_bottom_bar(f, app, l.bottom);

    if app.modal != Modal::None {
        draw_modal(f, app, area);
    }
}

fn draw_top_bar(f: &mut Frame, app: &App, area: Rect) {
    f.render_widget(Clear, area);
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
        Mode::Normal => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        Mode::Edit => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        Mode::Input => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        Mode::Running => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        Mode::Command => Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
    };

    let title_span = Span::styled(
        " RVS v0.5.0 ",
        Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD),
    );
    let fname_span = Span::styled(
        format!(" Fname: {fname}{modified} "),
        if app.editor.needs_save() {
            Style::default().bg(Color::DarkGray).fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD)
        },
    );
    let mode_span = Span::styled(format!(" [{mode_str}] "), mode_style);
    let zoom_span = if let Some(pane) = app.zoomed_pane {
        Span::styled(
            format!(" [ZOOMED: {pane:?}] "),
            Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };
    let hints_span = Span::styled(
        " ?:Help ^O:Load ^S:Save c:Compile s:START n:Next r:Run x:Stop Tab/S-Tab:Pane Enter/z:Zoom/Edit :Cmd q:Quit ",
        Style::default().fg(Color::DarkGray),
    );

    let line = Line::from(vec![title_span, fname_span, mode_span, zoom_span, hints_span]);
    f.render_widget(Paragraph::new(line), area);
}

fn draw_bottom_bar(f: &mut Frame, app: &App, area: Rect) {
    f.render_widget(Clear, area);
    if app.mode == Mode::Command {
        let line = Line::from(vec![
            Span::styled(":", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(&app.command_input, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]);
        f.render_widget(Paragraph::new(line), area);
        let cursor_col = area.x + 1 + app.command_input.chars().count() as u16;
        if cursor_col < area.x + area.width {
            f.set_cursor_position((cursor_col, area.y));
        }
    } else if !app.status.is_empty() {
        let is_err = app.status.starts_with("ERROR");
        let style = if is_err {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };
        f.render_widget(Paragraph::new(Span::styled(format!(" {}", app.status), style)), area);
    } else if let Some(pane) = app.zoomed_pane {
        let hint = Line::from(vec![
            Span::styled(format!(" [ZOOMED: {pane:?}] "), Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)),
            Span::styled(" Press Enter, z, or Esc to restore 6-pane view", Style::default().fg(Color::DarkGray)),
        ]);
        f.render_widget(Paragraph::new(hint), area);
    }
}

fn pane_block<'a>(title: &'a str, active: bool, zoomed: bool) -> Block<'a> {
    let border_style = if active {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title_str = if zoomed {
        format!(" [ZOOMED] {title} ")
    } else {
        format!(" {title} ")
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(if active { BorderType::Double } else { BorderType::Plain })
        .border_style(border_style)
        .title(Span::styled(title_str, if active {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }))
}

fn pill<'a>(label: &'a str, on: bool) -> Span<'a> {
    if on {
        Span::styled(
            format!(" [{label}] "),
            Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            format!(" [{label}] "),
            Style::default().fg(Color::DarkGray),
        )
    }
}

fn btn<'a>(label: &'a str) -> Span<'a> {
    Span::styled(
        format!(" [{label}] "),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )
}
fn build_table_header(cols: &[(&str, usize)]) -> (Line<'static>, Line<'static>) {
    let mut hdr_spans: Vec<Span<'static>> = Vec::new();
    let mut div_str = String::new();
    for (i, &(title, width)) in cols.iter().enumerate() {
        if i > 0 {
            hdr_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            div_str.push_str("─┼─");
        }
        let text = if width > 0 {
            format!("{:<width$}", title)
        } else {
            title.to_string()
        };
        hdr_spans.push(Span::styled(
            text,
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
        let div_width = if width > 0 { width } else { title.len().max(16) };
        div_str.push_str(&"─".repeat(div_width));
    }
    (Line::from(hdr_spans), Line::styled(div_str, Style::default().fg(Color::DarkGray)))
}

fn render_pane_with_toolbar(f: &mut Frame, app: &App, pane: Pane, area: Rect) -> Option<(Rect, Rect)> {
    let active = app.focus == pane || (pane == Pane::Inp && app.mode == Mode::Input);
    let is_zoomed = app.zoomed_pane == Some(pane);
    let block = pane_block(pane.title(), active, is_zoomed);
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    if inner.height < 2 {
        return None;
    }
    f.render_widget(Clear, inner);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    Some((chunks[0], chunks[1]))
}

fn expand_tabs(s: &str, tab_width: usize) -> String {
    let mut res = String::new();
    let mut col = 0;
    for c in s.chars() {
        if c == '\r' {
            continue;
        }
        if c == '\t' {
            let spaces = tab_width - (col % tab_width);
            for _ in 0..spaces {
                res.push(' ');
            }
            col += spaces;
        } else {
            res.push(c);
            col += 1;
        }
    }
    res
}
pub(crate) fn visual_col_at(line: &str, char_col: usize, tab_width: usize) -> usize {
    let mut col = 0;
    let mut count = 0;
    for (i, c) in line.chars().enumerate() {
        if i >= char_col {
            return col;
        }
        if c == '\r' {
            continue;
        }
        if c == '\t' {
            col += tab_width - (col % tab_width);
        } else {
            col += 1;
        }
        count = i + 1;
    }
    if char_col > count {
        col += char_col - count;
    }
    col
}



fn draw_source(f: &mut Frame, app: &mut App, area: Rect) {
    let active = app.focus == Pane::Source;
    let is_zoomed = app.zoomed_pane == Some(Pane::Source);
    let block = pane_block("Source", active, is_zoomed);
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    if inner.height < 1 {
        return;
    }
    f.render_widget(Clear, inner);

    let text_area = inner;
    let height = text_area.height as usize;
    if app.editor.row < app.editor.offset_row {
        app.editor.offset_row = app.editor.row;
    } else if app.editor.row >= app.editor.offset_row + height {
        app.editor.offset_row = app.editor.row - height + 1;
    }

    let mut lines = Vec::new();
    let num_digits = format!("{}", app.editor.lines.len().max(1)).len().max(3);

    for (idx, line_str) in app.editor.lines.iter().enumerate().skip(app.editor.offset_row).take(height) {
        let is_cur = idx == app.editor.row && active;
        let has_bp = app.has_breakpoint_on_line(idx);
        let bp_mark = if has_bp {
            Span::styled("● ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        } else {
            Span::raw("  ")
        };
        let num_str = format!("{line:>width$} ", line = idx + 1, width = num_digits);
        let num_style = if has_bp {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else if is_cur {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let expanded = expand_tabs(line_str, 8);
        let mut spans = crate::syntax::highlight_source_line(&expanded);
        spans.insert(0, Span::styled(num_str, num_style));
        spans.insert(0, bp_mark);
        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), text_area);

    if active && (app.mode == Mode::Edit || app.mode == Mode::Normal) && app.modal == Modal::None {
        let visual_col = if let Some(line) = app.editor.lines.get(app.editor.row) {
            visual_col_at(line, app.editor.col, 8)
        } else {
            app.editor.col
        };
        let screen_row = text_area.y + (app.editor.row.saturating_sub(app.editor.offset_row)) as u16;
        let screen_col = text_area.x + 2 + (num_digits + 1) as u16 + (visual_col.saturating_sub(app.editor.offset_col)) as u16;
        if screen_row < text_area.y + text_area.height && screen_col < text_area.x + text_area.width {
            f.set_cursor_position((screen_col, screen_row));
        }
    }

}

fn draw_listing(f: &mut Frame, app: &mut App, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Listing, area) else {
        return;
    };
    let height = text_area.height as usize;
    let mut all_rows: Vec<Line> = Vec::new();

    if app.sim.listing.is_empty() || app.sim.listing.contains("ERROR:") {
        for line_str in app.sim.listing.lines() {
            let clean_line = expand_tabs(line_str, 8);
            if clean_line.contains("ERROR:") {
                all_rows.push(Line::styled(
                    clean_line,
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));
            } else {
                let spans = crate::syntax::highlight_source_line(&clean_line);
                all_rows.push(Line::from(spans));
            }
        }
    } else {
        all_rows.push(Line::styled(
            "ASSEMBLY LISTING",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));

        let mut cols = vec![("  ADDRESS", 20)];
        if app.sim.view.list_bin {
            cols.push(("BIN / HEX CODE", 40));
        }
        if app.sim.view.list_hex {
            cols.push(("HEX OPERANDS", 23));
        }
        if app.sim.view.list_int {
            cols.push(("INT OPERANDS", 23));
        }
        if app.sim.view.list_txt {
            cols.push(("TEXT SOURCE", 34));
        }
        let (hdr_line, div_line) = build_table_header(&cols);
        all_rows.push(hdr_line);
        all_rows.push(div_line);

        let clr = app.colorize_listing;
        let sep = || Span::styled(" │ ", Style::default().fg(Color::DarkGray));

        for (&addr, cell) in app.sim.mem.cells.iter() {
            let is_pc = addr == app.sim.regs.pc;
            let has_bp = app.breakpoints.contains(&addr);
            let bp_span = if has_bp {
                Span::styled("● ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            } else {
                Span::raw("  ")
            };
            let addr_style = if is_pc {
                Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
            } else if has_bp {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };
            let mut row_spans = vec![bp_span, Span::styled(format!("0x{addr:016x}"), addr_style)];

            match cell {
                rvs_core::memory::MemoryCell::Data { value, case, expr, source_line } => {
                    let val_str = expr.as_deref().map(str::to_string)
                        .unwrap_or_else(|| rvs_core::memory::render_value(*value, *case));
                    if app.sim.view.list_bin {
                        row_spans.push(sep());
                        let is_err = val_str.contains("ERROR");
                        if clr {
                            row_spans.push(Span::styled("DD ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
                            let val_style = if is_err {
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::LightGreen)
                            };
                            row_spans.push(Span::styled(format!("{:<37}", val_str), val_style));
                        } else {
                            let val_style = if is_err {
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::Gray)
                            };
                            row_spans.push(Span::styled("DD ", Style::default().fg(Color::Gray)));
                            row_spans.push(Span::styled(format!("{:<37}", val_str), val_style));
                        }
                    }
                    if app.sim.view.list_hex {
                        row_spans.push(sep());
                        row_spans.push(Span::raw(" ".repeat(23)));
                    }
                    if app.sim.view.list_int {
                        row_spans.push(sep());
                        row_spans.push(Span::raw(" ".repeat(23)));
                    }
                    if app.sim.view.list_txt {
                        row_spans.push(sep());
                        let expanded = expand_tabs(source_line, 8);
                        if clr {
                            row_spans.extend(crate::syntax::highlight_source_line(&expanded));
                        } else {
                            let is_err = expanded.contains("ERROR");
                            let txt_style = if is_err {
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::Gray)
                            };
                            row_spans.push(Span::styled(expanded, txt_style));
                        }
                    }
                }
                rvs_core::memory::MemoryCell::Instruction(inst) => {
                    let enc = rvs_core::assembler::encode(&inst.op, &inst.args, addr, &app.sim.sym);
                    let fmt_tag = enc.bin.split_whitespace().next();
                    if app.sim.view.list_bin {
                        row_spans.push(sep());
                        let bin_padded = format!("{:<40}", enc.bin);
                        if clr {
                            row_spans.extend(crate::syntax::highlight_bin_correlated(&bin_padded));
                        } else {
                            let is_err = bin_padded.contains("ERROR");
                            let bin_style = if is_err {
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::Gray)
                            };
                            row_spans.push(Span::styled(bin_padded, bin_style));
                        }
                    }
                    if app.sim.view.list_hex {
                        row_spans.push(sep());
                        let new_op = enc.asmx.first().map(String::as_str).unwrap_or("");
                        let new_args: Vec<String> = enc.asmx.iter().skip(1).cloned().collect();
                        let hex_ops = format!("{:<23}", rvs_core::assembler::fmti(new_op, &new_args));
                        if clr {
                            row_spans.extend(crate::syntax::highlight_operands_correlated(&hex_ops, fmt_tag));
                        } else {
                            row_spans.push(Span::styled(hex_ops, Style::default().fg(Color::Gray)));
                        }
                    }
                    if app.sim.view.list_int {
                        row_spans.push(sep());
                        let words: Vec<&str> = enc.asm.split_whitespace().collect();
                        let int_op = words.first().copied().unwrap_or("");
                        let int_rest: Vec<String> = words.iter().skip(1).map(|s| s.to_string()).collect();
                        let int_ops = format!("{:<23}", rvs_core::assembler::fmti(int_op, &int_rest));
                        if clr {
                            row_spans.extend(crate::syntax::highlight_operands_correlated(&int_ops, fmt_tag));
                        } else {
                            row_spans.push(Span::styled(int_ops, Style::default().fg(Color::Gray)));
                        }
                    }
                    if app.sim.view.list_txt {
                        row_spans.push(sep());
                        let expanded = expand_tabs(&inst.source_line, 8);
                        if clr {
                            row_spans.extend(crate::syntax::highlight_source_line_correlated(&expanded, fmt_tag));
                        } else {
                            let is_err = expanded.contains("ERROR");
                            let txt_style = if is_err {
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::Gray)
                            };
                            row_spans.push(Span::styled(expanded, txt_style));
                        }
                    }
                }
            }
            all_rows.push(Line::from(row_spans));
        }

        // SYMBOL TABLE section
        all_rows.push(Line::raw(""));
        all_rows.push(Line::styled(
            "SYMBOL TABLE",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
        let (sym_hdr, sym_div) = build_table_header(&[("ADDRESS", 18), ("SYMBOL", 16)]);
        all_rows.push(sym_hdr);
        all_rows.push(sym_div);

        for (name, value) in app.sim.sym.iter() {
            let v = rvs_core::tcl::parse_int(value).unwrap_or(0) as u64;
            let row = Line::from(vec![
                Span::styled(format!("0x{v:016x}"), Style::default().fg(Color::Cyan)),
                Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
                Span::styled(name.clone(), Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD)),
            ]);
            all_rows.push(row);
        }
    }

    let lines: Vec<Line> = all_rows.into_iter().skip(app.scroll_listing).take(height).collect();
    f.render_widget(Paragraph::new(lines), text_area);

    let spd_label = format!("SPD:{}", app.run_speed.label());
    let toolbar = Line::from(vec![
        btn("InitPC"),
        Span::styled(format!(" PC:0x{:016x} ", app.sim.regs.pc), Style::default().fg(Color::White)),
        pill("TXT", app.sim.view.exec_txt),
        pill("CLR", app.colorize_listing),
        pill(&spd_label, app.run_speed != crate::app::RunSpeed::Max),
        pill("BP", !app.breakpoints.is_empty()),
    ]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

fn draw_regs(f: &mut Frame, app: &mut App, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Regs, area) else {
        return;
    };
    let height = text_area.height as usize;

    let full_text = rvs_core::format::format_registers(&app.sim);
    let mut lines = Vec::new();

    for line_str in full_text.lines().skip(app.scroll_regs).take(height) {
        let clean_line = expand_tabs(line_str, 8);
        if clean_line.starts_with("INT Regs") || clean_line.starts_with("FLP Regs") {
            lines.push(Line::styled(
                clean_line,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        } else if clean_line.starts_with("---OFF---") {
            lines.push(Line::styled(
                clean_line,
                Style::default().fg(Color::DarkGray),
            ));
        } else if let Some(stripped) = clean_line.strip_prefix('x') {
            let parts: Vec<&str> = stripped.split_whitespace().collect();
            let i_opt = parts.first().and_then(|n| n.parse::<usize>().ok());
            if let Some(i) = i_opt {
                if i < 32 && app.changed_x(i) {
                    lines.push(Line::styled(
                        clean_line,
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ));
                    continue;
                }
                let val = if i < 32 { app.sim.regs.read_x(i as u8) } else { 0 };
                let reg_name = format!("x{:<2} ", i);
                let abi = if parts.len() > 1 {
                    format!("{:<4}", parts[1])
                } else {
                    "    ".to_string()
                };

                let mut spans = vec![
                    Span::styled(reg_name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(abi, Style::default().fg(Color::LightMagenta)),
                ];

                if app.sim.view.reg_hex {
                    let hex_val = format!(" 0x{:016x}", val);
                    let hex_style = if val == 0 {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    spans.push(Span::styled(hex_val, hex_style));
                }

                let int_str = rvs_core::tcl::fmt_ld_pad(val as i64, 20);
                let int_style = if val == 0 {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::LightGreen)
                };
                spans.push(Span::raw(" "));
                spans.push(Span::styled(int_str, int_style));

                lines.push(Line::from(spans));
                continue;
            }
            lines.push(Line::styled(clean_line, Style::default().fg(Color::Gray)));
        } else if let Some(stripped) = clean_line.strip_prefix('f') {
            let parts: Vec<&str> = stripped.split_whitespace().collect();
            let i_opt = parts.first().and_then(|n| n.parse::<usize>().ok());
            if let Some(i) = i_opt {
                if i < 32 && app.changed_f(i) {
                    lines.push(Line::styled(
                        clean_line,
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ));
                    continue;
                }
                let bits = if i < 32 { app.sim.regs.read_f_bits(i as u8) } else { 0 };
                let reg_name = format!("f{:<2}", i);

                let mut spans = vec![
                    Span::styled(reg_name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ];

                if app.sim.view.reg_hex {
                    let hex_val = format!(" 0x{:016x}", bits);
                    let hex_style = if bits == 0 {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    spans.push(Span::styled(hex_val, hex_style));
                }

                let flp_str = if parts.len() > 2 && app.sim.view.reg_hex {
                    parts[2]
                } else if parts.len() > 1 && !app.sim.view.reg_hex {
                    parts[1]
                } else {
                    "0.0000000000000000E+00"
                };
                let flp_style = if bits == 0 {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                spans.push(Span::raw(" "));
                spans.push(Span::styled(flp_str.to_string(), flp_style));

                lines.push(Line::from(spans));
                continue;
            }
            lines.push(Line::styled(clean_line, Style::default().fg(Color::Gray)));
        } else {
            lines.push(Line::styled(clean_line, Style::default().fg(Color::DarkGray)));
        }
    }

    f.render_widget(Paragraph::new(lines), text_area);

    let toolbar = Line::from(vec![
        pill("HEX", app.sim.view.reg_hex),
        pill("INT", app.sim.view.reg_int),
        pill("FLP", app.sim.view.reg_flp),
    ]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

fn draw_mem(f: &mut Frame, app: &mut App, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Mem, area) else {
        return;
    };
    let height = text_area.height as usize;
    let mut all_rows: Vec<Line> = Vec::new();

    // Line 1: Title
    all_rows.push(Line::styled(
        "MEMORY",
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    ));

    // Table headers
    let mut cols = vec![("ADDRESS", 18)];
    if app.sim.view.mem_hex {
        cols.push(("HEXADECIMAL", 18));
    }
    if app.sim.view.mem_int {
        cols.push(("INTEGER", 20));
    }
    if app.sim.view.mem_flp {
        cols.push(("FLOAT", 24));
    }
    if app.sim.view.mem_txt {
        cols.push(("SOURCE TEXT", 34));
    }
    let (hdr_line, div_line) = build_table_header(&cols);
    all_rows.push(hdr_line);
    all_rows.push(div_line);
    // Data rows
    let mut data_rows_count = 0;
    for (&addr, cell) in app.sim.mem.cells.iter() {
        let (hex_borrowed, source_line): (std::borrow::Cow<'_, str>, &str) = match cell {
            rvs_core::memory::MemoryCell::Data {
                value,
                case,
                expr,
                source_line,
            } => (
                match expr {
                    Some(e) => std::borrow::Cow::Borrowed(e.as_str()),
                    None => std::borrow::Cow::Owned(rvs_core::memory::render_value(*value, *case)),
                },
                source_line.as_str(),
            ),
            _ => continue,
        };
        let hex = &hex_borrowed;
        data_rows_count += 1;

        let is_zero = crate::syntax::is_zero_hex(&hex);
        let mut row_spans = vec![
            Span::styled(format!("0x{addr:016x}"), Style::default().fg(Color::Cyan)),
        ];

        if app.sim.view.mem_hex {
            row_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            let hex_style = if is_zero {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };
            row_spans.push(Span::styled(format!("{:<18}", hex), hex_style));
        }

        if app.sim.view.mem_int {
            row_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            let int_str = match rvs_core::tcl::fmt_ld_str(&hex) {
                Some(v) => rvs_core::tcl::fmt_s_pad(v.trim(), 20),
                None => rvs_core::tcl::fmt_s_pad("NOT_AN_INT", 20),
            };
            let int_style = if is_zero {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::LightGreen)
            };
            row_spans.push(Span::styled(int_str, int_style));
        }

        if app.sim.view.mem_flp {
            row_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            let flp_str = match rvs_core::tcl::x2f(&hex).and_then(rvs_core::tcl::fmt_e24) {
                Some(v) => format!("{:<24}", v.trim()),
                None => rvs_core::tcl::fmt_s_pad("NOT_A_FLP", 24),
            };
            let flp_style = if is_zero {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Yellow)
            };
            row_spans.push(Span::styled(flp_str, flp_style));
        }

        if app.sim.view.mem_txt {
            row_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            let formatted_src = rvs_core::assembler::render_line_f1(&rvs_core::assembler::split_line(&source_line));
            let mut src_spans = crate::syntax::highlight_source_line(&formatted_src);
            row_spans.append(&mut src_spans);
        }

        all_rows.push(Line::from(row_spans));
    }

    if data_rows_count == 0 {
        all_rows.push(Line::styled(
            "  (no data cells — use DD/DW/DH/DB/DC/DF/DM)",
            Style::default().fg(Color::DarkGray),
        ));
    }
    let lines: Vec<Line> = all_rows.into_iter().skip(app.scroll_mem).take(height).collect();
    f.render_widget(Paragraph::new(lines), text_area);

    let toolbar = Line::from(vec![
        pill("HEX", app.sim.view.mem_hex),
        pill("INT", app.sim.view.mem_int),
        pill("FLP", app.sim.view.mem_flp),
        pill("TXT", app.sim.view.mem_txt),
        btn("SP"),
        btn("0x0"),
    ]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

fn draw_exec(f: &mut Frame, app: &mut App, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Exec, area) else {
        return;
    };
    let height = text_area.height as usize;

    let total_lines = app.sim.exec_log.lines().count();
    let auto_scroll = total_lines.saturating_sub(height);
    let scroll = if app.scroll_exec == 0 {
        auto_scroll
    } else {
        app.scroll_exec
    };

    let mut lines = Vec::new();
    for line_str in app.sim.exec_log.lines().skip(scroll).take(height) {
        let clean_line = expand_tabs(line_str, 8);
        if clean_line.starts_with("ERROR:") {
            lines.push(Line::styled(
                clean_line,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        } else if clean_line.starts_with("ebreak") {
            lines.push(Line::styled(
                clean_line,
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ));
        } else if clean_line == "START" || clean_line == "NEXT" || clean_line == "RUN" {
            lines.push(Line::styled(
                clean_line,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        } else if clean_line.starts_with("0x") && clean_line.len() >= 18 {
            let addr_part = clean_line[..18].to_string();
            let rest = clean_line[18..].to_string();
            let mut spans = vec![Span::styled(
                addr_part,
                Style::default().fg(Color::Cyan),
            )];
            let mut rest_spans = crate::syntax::highlight_inst_operands(&rest);
            spans.append(&mut rest_spans);
            lines.push(Line::from(spans));
        } else {
            lines.push(Line::styled(clean_line, Style::default().fg(Color::Gray)));
        }
    }
    f.render_widget(Paragraph::new(lines), text_area);

    let toolbar = Line::from(vec![btn("Clear")]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

fn draw_out(f: &mut Frame, app: &mut App, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Out, area) else {
        return;
    };
    let height = text_area.height as usize;

    let total = app.sim.output.lines().count();
    let scroll = if app.scroll_out == 0 {
        total.saturating_sub(height)
    } else {
        app.scroll_out
    };

    let mut lines = Vec::new();
    for line_str in app.sim.output.lines().skip(scroll).take(height) {
        let clean_line = expand_tabs(line_str, 8);
        lines.push(Line::styled(clean_line, Style::default().fg(Color::White)));
    }

    f.render_widget(Paragraph::new(lines), text_area);

    let dma_status = if app.dma_out_available {
        pill(if app.sim.view.dma_out { "DMA:ON" } else { "DMA:OFF" }, app.sim.view.dma_out)
    } else {
        Span::styled(" [DMA:N/A] ", Style::default().fg(Color::DarkGray))
    };
    let toolbar = Line::from(vec![btn("Clear"), dma_status]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

fn draw_inp(f: &mut Frame, app: &mut App, area: Rect) {
    let Some((entry_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Inp, area) else {
        return;
    };

    let awaiting = app.sim.pending.is_some() || !app.sim.entry_type.is_empty();
    let prompt_label = if let Some(p) = &app.sim.pending {
        match p.imm {
            5 => "I?",
            6 => "F?",
            7 => "X?",
            8 => "C?",
            9 => "S?",
            _ => "?",
        }
    } else if !app.sim.entry_type.is_empty() {
        match app.sim.entry_type.as_str() {
            "I" => "I?",
            "F" => "F?",
            "C" => "C?",
            "S" => "S?",
            _ => "?",
        }
    } else {
        "  "
    };

    let prompt_style = if awaiting && app.blink_on() {
        Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD)
    } else if awaiting {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let entry_line = Line::from(vec![
        Span::styled(format!(" {prompt_label} "), prompt_style),
        Span::raw(" "),
        Span::styled(&app.input_line, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(entry_line), entry_area);

    if (app.focus == Pane::Inp || app.mode == Mode::Input) && app.mode == Mode::Input {
        let screen_col = entry_area.x + 5 + app.input_line.chars().count() as u16;
        let screen_row = entry_area.y;
        if screen_col < entry_area.x + entry_area.width {
            f.set_cursor_position((screen_col, screen_row));
        }
    }

    let dma_status = if app.dma_inp_available {
        pill(if app.sim.view.dma_inp { "DMA:ON" } else { "DMA:OFF" }, app.sim.view.dma_inp)
    } else {
        Span::styled(" [DMA:N/A] ", Style::default().fg(Color::DarkGray))
    };
    let toolbar = Line::from(vec![btn("Enter"), btn("Cancel"), dma_status]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

fn draw_modal(f: &mut Frame, app: &mut App, area: Rect) {
    match &app.modal {
        Modal::About => {
            let modal_area = centered_rect(66, 60, area);
            f.render_widget(Clear, modal_area);
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" About / Help (?) ");
            let inner = block.inner(modal_area);
            f.render_widget(block, modal_area);

            let text = vec![
                Line::styled("RVS (RISC-V Visual Simulator) v0.5.0 [Rust Rewrite]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Line::raw(""),
                Line::raw("Reverse-engineered from RVS v0.46 (SIDIC, 2019) with modern Vim & Yazi workflows."),
                Line::raw("Preserves full assembly and batch report parity with RVS046.tcl."),
                Line::raw(""),
                Line::styled("Keyboard Shortcuts:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Line::raw("  Tab / Shift+Tab  - Cycle focus between panes forward / backward"),
                Line::raw("  Enter            - Open Source in $EDITOR; Zoom/unzoom on other panes"),
                Line::raw("  z / :zoom        - Toggle fullscreen zoom on focused pane"),
                Line::raw("  :                - Enter Command mode (:w, :q, :c, :s, :n, :r, etc.)"),
                Line::raw("  h / j / k / l    - Vim cursor motion in Source; scroll in other panes"),
                Line::raw("  w / b / 0 / $    - Word forward / backward, start / end of line"),
                Line::raw("  gg / G           - Jump to start / end of buffer or pane"),
                Line::raw("  i / a / I / A    - Enter INSERT mode in Source (Esc to return to NORMAL)"),
                Line::raw("  o / O            - Insert line below / above and enter INSERT mode"),
                Line::raw("  x / dd / yy / p  - Delete char, delete line, yank line, paste below"),
                Line::raw("  u / Ctrl+R       - Undo / Redo in editor"),
                Line::raw(""),
                Line::styled("Simulation Controls (Normal Mode):", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Line::raw("  ?                - Help / About"),
                Line::raw("  Ctrl+O, :e       - Load file"),
                Line::raw("  Ctrl+S, :w       - Save file"),
                Line::raw("  c, :compile      - Compile assembly"),
                Line::raw("  s, :start        - START (reset PC to START)"),
                Line::raw("  n, :next         - Next / Step one instruction (or :n <count>)"),
                Line::raw("  r, :run          - Run continuously"),
                Line::raw("  p, P, N          - Step back / Prev (rewind 1 instruction)"),
                Line::raw("  b, :b <addr>     - Toggle breakpoint (run stops at breakpoint)"),
                Line::raw("  +, - ([, ])      - Adjust animation speed (1Hz, 5Hz, 20Hz, Max)"),
                Line::raw("  s, 0 in Mem      - Jump memory view to $sp (stack) or 0x0"),
                Line::raw("  1 / 2            - Listing pane: toggle TXT / CLR (correlated colorizing)"),
                Line::raw("  :color [on|off]  - Toggle or set Listing correlated colorizing"),
                Line::raw("  q, :q, :q!       - Quit simulator"),
                Line::raw(""),
                Line::styled("DMA (Direct Memory Access / Memory-Mapped I/O):", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Line::raw("  Enabled automatically when OUTC/OUTD (0x0/0x8) or INPC/INPD (0x10/0x18) labels exist."),
                Line::raw("  Toggle active state with 'd' (or '1') when focused on OUT/INP, or by clicking the pill."),
                Line::raw(""),
                Line::styled("Press Esc or Enter to close.", Style::default().fg(Color::DarkGray)),
            ];
            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), inner);
        }
        Modal::FilePrompt(action) => {
            let modal_area = centered_rect(84, 82, area);
            f.render_widget(Clear, modal_area);

            let title = match action {
                FileAction::Load => " Load File (Explorer) ",
                FileAction::Append => " Append File (Explorer) ",
                FileAction::SaveAs => " Save File As (Explorer) ",
                FileAction::Batch => " Run Batch File (.b) ",
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Yellow))
                .title(Span::styled(title, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            let inner = block.inner(modal_area);
            f.render_widget(block, modal_area);

            if inner.height < 6 || inner.width < 24 {
                return;
            }

            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(inner);

            let mode_pill = match app.explorer.mode {
                crate::explorer::ExplorerMode::Normal => {
                    Span::styled(" [NORMAL] ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD))
                }
                crate::explorer::ExplorerMode::Filter => {
                    Span::styled(" [FILTER] ", Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD))
                }
            };

            let hints_text = match app.explorer.mode {
                crate::explorer::ExplorerMode::Normal => {
                    "Hints: [j/k] Select  [h] Parent  [l/Enter] Open  [/ or i] Filter/Type  [J/K] Preview  [q/Esc] Close"
                }
                crate::explorer::ExplorerMode::Filter => {
                    "Hints: [Typing] Filter/Path  [Tab] Complete  [Enter] Open  [Esc] Normal Mode"
                }
            };

            let top_lines = vec![
                Line::from(vec![
                    mode_pill,
                    Span::raw(" "),
                    Span::styled("Path: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(&app.modal_input, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::styled(
                    hints_text,
                    Style::default().fg(Color::DarkGray),
                ),
                Line::styled("─".repeat(inner.width as usize), Style::default().fg(Color::DarkGray)),
            ];
            f.render_widget(Paragraph::new(top_lines), main_chunks[0]);

            if app.explorer.mode == crate::explorer::ExplorerMode::Filter {
                let cursor_x = inner.x + 17 + app.modal_input.chars().count() as u16;
                let cursor_y = inner.y;
                if cursor_x < inner.x + inner.width {
                    f.set_cursor_position((cursor_x, cursor_y));
                }
            }

            let split_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
                .split(main_chunks[1]);

            let list_area = split_chunks[0];
            let preview_area = split_chunks[1];

            let dir_disp = if !app.explorer.dir_display_prefix.is_empty() {
                app.explorer.dir_display_prefix.as_str()
            } else {
                "./"
            };
            let dir_title = format!(" Dir: {} ({} items) ", dir_disp, app.explorer.entries.len());
            let list_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(Span::styled(dir_title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
            let list_inner = list_block.inner(list_area);
            f.render_widget(list_block, list_area);
            f.render_widget(Clear, list_inner);
            let visible_height = list_inner.height as usize;
            if app.explorer.selected < app.explorer.scroll_list {
                app.explorer.scroll_list = app.explorer.selected;
            } else if app.explorer.selected >= app.explorer.scroll_list + visible_height {
                app.explorer.scroll_list = app.explorer.selected - visible_height + 1;
            }

            let mut list_rows = Vec::new();
            for (idx, item) in app.explorer.entries.iter().enumerate().skip(app.explorer.scroll_list).take(visible_height) {
                let is_sel = idx == app.explorer.selected;
                let pfx = if is_sel { "> " } else { "  " };
                let disp_name = if item.name == ".." {
                    "../".to_string()
                } else if item.is_dir {
                    format!("{}/", item.name)
                } else {
                    format!("  {}", item.name)
                };
                let size_str = if item.is_dir {
                    String::new()
                } else {
                    crate::explorer::format_size(item.size)
                };

                let item_style = if is_sel {
                    Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
                } else if item.is_dir {
                    Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let total_w = list_inner.width as usize;
                let line_str = if size_str.is_empty() {
                    let raw = format!("{pfx}{disp_name}");
                    if raw.chars().count() > total_w && total_w > 3 {
                        format!("{}...", raw.chars().take(total_w - 3).collect::<String>())
                    } else {
                        raw
                    }
                } else {
                    let pfx_len = pfx.chars().count();
                    let size_len = size_str.chars().count();
                    let fixed_len = pfx_len + size_len + 1;
                    let name_str = if total_w > fixed_len {
                        let max_name = total_w - fixed_len;
                        if disp_name.chars().count() > max_name && max_name > 3 {
                            format!("{}...", disp_name.chars().take(max_name - 3).collect::<String>())
                        } else {
                            disp_name
                        }
                    } else {
                        disp_name
                    };
                    let used_len = pfx_len + name_str.chars().count() + size_len;
                    let pad = total_w.saturating_sub(used_len);
                    format!("{pfx}{name_str}{}{size_str}", " ".repeat(pad))
                };
                let truncated: String = line_str.chars().take(total_w).collect();
                list_rows.push(Line::styled(truncated, item_style));
            }
            if list_rows.is_empty() {
                list_rows.push(Line::styled("  [No matching items]", Style::default().fg(Color::DarkGray)));
            }
            f.render_widget(Paragraph::new(list_rows), list_inner);

            let preview_title = format!(" Preview: {} ", app.explorer.preview_title);
            let prev_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White))
                .title(Span::styled(preview_title, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            let prev_inner = prev_block.inner(preview_area);
            f.render_widget(prev_block, preview_area);
            f.render_widget(Clear, prev_inner);

            let prev_width = prev_inner.width as usize;
            let prev_height = prev_inner.height as usize;
            let mut prev_lines = Vec::new();
            for raw_line in app.explorer.preview_lines.iter().skip(app.explorer.scroll_preview).take(prev_height) {
                let expanded = expand_tabs(raw_line, 8);
                let l: String = expanded.chars().take(prev_width).collect();
                if l.trim_start().starts_with("[DIR]") {
                    prev_lines.push(Line::styled(l, Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)));
                } else if l.contains('│') {
                    let parts: Vec<&str> = l.splitn(2, '│').collect();
                    if parts.len() == 2 {
                        prev_lines.push(Line::from(vec![
                            Span::styled(format!("{}│", parts[0]), Style::default().fg(Color::DarkGray)),
                            Span::styled(parts[1].to_string(), Style::default().fg(Color::Gray)),
                        ]));
                        continue;
                    }
                    prev_lines.push(Line::styled(l, Style::default().fg(Color::Gray)));
                } else if l.starts_with('[') {
                    prev_lines.push(Line::styled(l, Style::default().fg(Color::Yellow)));
                } else {
                    prev_lines.push(Line::styled(l, Style::default().fg(Color::White)));
                }
            }
            f.render_widget(Paragraph::new(prev_lines), prev_inner);
        }
        Modal::ConfirmSave(action) => {
            let modal_area = centered_rect(68, 52, area);
            f.render_widget(Clear, modal_area);
            let action_desc = match action {
                ConfirmAction::Quit => "quitting",
                ConfirmAction::Load => "loading another file",
                ConfirmAction::Append => "appending another file",
                ConfirmAction::Batch => "running batch file",
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Red))
                .title(Span::styled(" Unsaved Changes — Save Before Leaving? ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            let inner = block.inner(modal_area);
            f.render_widget(block, modal_area);

            let text = vec![
                Line::styled("There are unsaved changes in your source buffer.", Style::default().fg(Color::White)),
                Line::styled(format!("Would you like to save changes before {action_desc}?"), Style::default().fg(Color::Yellow)),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("File path: [ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(&app.modal_input, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled(" ]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(" [Enter]  ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::styled(" Save to file path above and proceed", Style::default().fg(Color::White)),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(" [Ctrl+D] ", Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::styled(" Discard changes and proceed without saving", Style::default().fg(Color::White)),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(" [Esc]    ", Style::default().fg(Color::Black).bg(Color::DarkGray).add_modifier(Modifier::BOLD)),
                    Span::styled(" Cancel and return to editor", Style::default().fg(Color::Gray)),
                ]),
            ];
            f.render_widget(Paragraph::new(text), inner);

            let cursor_x = inner.x + 13 + app.modal_input.chars().count() as u16;
            let cursor_y = inner.y + 3;
            if cursor_x < inner.x + inner.width {
                f.set_cursor_position((cursor_x, cursor_y));
            }
        }
        Modal::None => {}
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// ----- mouse hit-testing -----------------------------------------------

pub fn handle_click(app: &mut App, col: u16, row: u16) {
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
                    }
                }
                Pane::Exec => {
                    app.clear_pane(Pane::Exec);
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
                Pane::Source => {}
            }
        }
    }
}

pub fn handle_scroll(app: &mut App, col: u16, row: u16, delta: i32) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visual_col_at() {
        assert_eq!(visual_col_at("", 0, 8), 0);
        assert_eq!(visual_col_at("", 5, 8), 5);
        assert_eq!(visual_col_at("hello", 3, 8), 3);
        assert_eq!(visual_col_at("hello", 10, 8), 10);

        // Tab at col 0:
        assert_eq!(visual_col_at("\taddi", 0, 8), 0);
        assert_eq!(visual_col_at("\taddi", 1, 8), 8);
        assert_eq!(visual_col_at("\taddi", 2, 8), 9);

        // Tab in middle: "a\tb"
        assert_eq!(visual_col_at("a\tb", 0, 8), 0);
        assert_eq!(visual_col_at("a\tb", 1, 8), 1);
        assert_eq!(visual_col_at("a\tb", 2, 8), 8);
        assert_eq!(visual_col_at("a\tb", 3, 8), 9);

        // Two tabs: "\t\tx"
        assert_eq!(visual_col_at("\t\tx", 0, 8), 0);
        assert_eq!(visual_col_at("\t\tx", 1, 8), 8);
        assert_eq!(visual_col_at("\t\tx", 2, 8), 16);
        assert_eq!(visual_col_at("\t\tx", 3, 8), 17);
    }
}
