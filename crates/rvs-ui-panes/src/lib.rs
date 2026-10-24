//! Pane renderers for the RVS++ TUI.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use rvs_app::{App, Modal, Mode, Pane};

pub fn pane_block<'a>(title: &'a str, active: bool, zoomed: bool) -> Block<'a> {
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

pub fn pill<'a>(label: &'a str, on: bool) -> Span<'a> {
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

pub fn btn<'a>(label: &'a str) -> Span<'a> {
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

pub fn expand_tabs(s: &str, tab_width: usize) -> String {
    let mut res = String::new();
    let mut col = 0usize;
    for grapheme in s.graphemes(true) {
        if grapheme == "\r" {
            continue;
        }
        if grapheme == "\t" {
            let spaces = tab_width - (col % tab_width);
            res.extend(std::iter::repeat(' ').take(spaces));
            col += spaces;
        } else {
            res.push_str(grapheme);
            col += UnicodeWidthStr::width(grapheme);
        }
    }
    res
}
pub(crate) fn visual_col_at(line: &str, char_col: usize, tab_width: usize) -> usize {
    let mut col = 0usize;
    let mut seen = 0usize;
    for grapheme in line.graphemes(true) {
        let chars = grapheme.chars().count();
        // Stop at the requested char index; a cursor inside a cluster renders
        // at the cluster's first column (the editor never parks it there).
        if seen >= char_col || seen + chars > char_col {
            return col;
        }
        if grapheme == "\r" {
            // Zero-width control character.
        } else if grapheme == "\t" {
            col += tab_width - (col % tab_width);
        } else {
            col += UnicodeWidthStr::width(grapheme);
        }
        seen += chars;
    }
    if char_col > seen {
        col += char_col - seen;
    }
    col
}



pub fn draw_source(f: &mut Frame, app: &mut App, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Source, area) else {
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
        let mut spans = rvs_syntax::highlight_source_line(&expanded);
        spans.insert(0, Span::styled(num_str, num_style));
        spans.insert(0, bp_mark);
        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), text_area);

    let is_rvs = app.sim.dialect == rvs_transpile::AssemblyDialect::Rvs;
    let is_gnu = app.sim.dialect == rvs_transpile::AssemblyDialect::Gnu;
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
        let screen_row = text_area.y + (app.editor.row.saturating_sub(app.editor.offset_row)) as u16;
        let screen_col = text_area.x + 2 + (num_digits + 1) as u16 + (visual_col.saturating_sub(app.editor.offset_col)) as u16;
        if screen_row < text_area.y + text_area.height && screen_col < text_area.x + text_area.width {
            f.set_cursor_position((screen_col, screen_row));
        }
    }

}

pub fn draw_listing(f: &mut Frame, app: &mut App, area: Rect) {
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
                let spans = rvs_syntax::highlight_source_line(&clean_line);
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
                rvs_memory::MemoryCell::Data { value, case, expr, source_line } => {
                    let val_str = expr.as_deref().map(str::to_string)
                        .unwrap_or_else(|| rvs_memory::render_value(*value, *case));
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
                            row_spans.extend(rvs_syntax::highlight_source_line(&expanded));
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
                rvs_memory::MemoryCell::Instruction(inst) => {
                    let enc = rvs_assembler::encode(&inst.op, &inst.args, addr, &app.sim.sym);
                    let fmt_tag = enc.bin.split_whitespace().next();
                    if app.sim.view.list_bin {
                        row_spans.push(sep());
                        let bin_padded = format!("{:<40}", enc.bin);
                        if clr {
                            row_spans.extend(rvs_syntax::highlight_bin_correlated(&bin_padded));
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
                        let hex_ops = format!("{:<23}", rvs_asm_syntax::fmti(new_op, &new_args));
                        if clr {
                            row_spans.extend(rvs_syntax::highlight_operands_correlated(&hex_ops, fmt_tag));
                        } else {
                            row_spans.push(Span::styled(hex_ops, Style::default().fg(Color::Gray)));
                        }
                    }
                    if app.sim.view.list_int {
                        row_spans.push(sep());
                        let words: Vec<&str> = enc.asm.split_whitespace().collect();
                        let int_op = words.first().copied().unwrap_or("");
                        let int_rest: Vec<String> = words.iter().skip(1).map(|s| s.to_string()).collect();
                        let int_ops = format!("{:<23}", rvs_asm_syntax::fmti(int_op, &int_rest));
                        if clr {
                            row_spans.extend(rvs_syntax::highlight_operands_correlated(&int_ops, fmt_tag));
                        } else {
                            row_spans.push(Span::styled(int_ops, Style::default().fg(Color::Gray)));
                        }
                    }
                    if app.sim.view.list_txt {
                        row_spans.push(sep());
                        let expanded = expand_tabs(&inst.source_line, 8);
                        if clr {
                            row_spans.extend(rvs_syntax::highlight_source_line_correlated(&expanded, fmt_tag));
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
            let v = rvs_tcl::parse_int(value).unwrap_or(0) as u64;
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
        pill(&spd_label, app.run_speed != rvs_app::RunSpeed::Max),
        pill("BP", !app.breakpoints.is_empty()),
    ]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

pub fn draw_regs(f: &mut Frame, app: &mut App, area: Rect) {
    let Some((text_area, toolbar_area)) = render_pane_with_toolbar(f, app, Pane::Regs, area) else {
        return;
    };
    let height = text_area.height as usize;

    let full_text = rvs_format::format_registers(&app.sim);
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

                let int_str = rvs_tcl::fmt_ld_pad(val as i64, 20);
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

pub fn draw_mem(f: &mut Frame, app: &mut App, area: Rect) {
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
            rvs_memory::MemoryCell::Data {
                value,
                case,
                expr,
                source_line,
            } => (
                match expr {
                    Some(e) => std::borrow::Cow::Borrowed(e.as_str()),
                    None => std::borrow::Cow::Owned(rvs_memory::render_value(*value, *case)),
                },
                source_line.as_str(),
            ),
            _ => continue,
        };
        let hex = &hex_borrowed;
        data_rows_count += 1;

        let is_zero = rvs_syntax::is_zero_hex(&hex);
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
            let int_str = match rvs_tcl::fmt_ld_str(&hex) {
                Some(v) => rvs_tcl::fmt_s_pad(v.trim(), 20),
                None => rvs_tcl::fmt_s_pad("NOT_AN_INT", 20),
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
            let flp_str = match rvs_tcl::x2f(&hex).and_then(rvs_tcl::fmt_e24) {
                Some(v) => format!("{:<24}", v.trim()),
                None => rvs_tcl::fmt_s_pad("NOT_A_FLP", 24),
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
            let formatted_src = rvs_asm_syntax::render_line_f1(&rvs_asm_syntax::split_line(&source_line));
            let mut src_spans = rvs_syntax::highlight_source_line(&formatted_src);
            row_spans.append(&mut src_spans);
        }

        all_rows.push(Line::from(row_spans));
    }

    if data_rows_count == 0 {
        all_rows.push(Line::styled(
            "  (no data cells: use DD/DW/DH/DB/DC/DF/DM)",
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
        btn("STACK"),
    ]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

pub fn draw_exec(f: &mut Frame, app: &mut App, area: Rect) {
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
        } else if clean_line.starts_with("WARN") {
            lines.push(Line::styled(
                clean_line,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
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
            let mut rest_spans = rvs_syntax::highlight_inst_operands(&rest);
            spans.append(&mut rest_spans);
            lines.push(Line::from(spans));
        } else {
            lines.push(Line::styled(clean_line, Style::default().fg(Color::Gray)));
        }
    }
    f.render_widget(Paragraph::new(lines), text_area);

    let cc_label = if app.sim.convention.enabled { "CC:ON" } else { "CC:OFF" };
    let toolbar = Line::from(vec![
        btn("Clear"),
        btn("STATS"),
        pill(cc_label, app.sim.convention.enabled),
        btn("STACK"),
    ]);
    f.render_widget(Paragraph::new(toolbar), toolbar_area);
}

pub fn draw_out(f: &mut Frame, app: &mut App, area: Rect) {
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

pub fn draw_inp(f: &mut Frame, app: &mut App, area: Rect) {
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

    #[test]
    fn test_visual_col_wide_and_combining() {
        // CJK ideographs occupy two cells each.
        assert_eq!(visual_col_at("日本", 1, 8), 2);
        assert_eq!(visual_col_at("日本", 2, 8), 4);
        // "e" + combining acute is one grapheme spanning two chars.
        assert_eq!(visual_col_at("e\u{301}", 1, 8), 0);
        assert_eq!(visual_col_at("e\u{301}", 2, 8), 1);
        // Tabs advance past the wide glyph.
        assert_eq!(expand_tabs("日\tx", 8), "日      x");
    }
}
