//! Modal overlay renderers for the RVS++ TUI.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use rvs_app::{App, ConfirmAction, FileAction, Modal};
use rvs_ui_panes::expand_tabs;

pub fn draw_modal(f: &mut Frame, app: &mut App, area: Rect) {
    match &app.modal {
        Modal::About => {
            let modal_area = centered_rect(80, 84, area);
            f.render_widget(Clear, modal_area);
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" About / Help & Command Reference (?) ");
            let inner = block.inner(modal_area);
            f.render_widget(block, modal_area);

            let text = vec![
                Line::styled("RVS++ (RISC-V Visual Simulator++) v1.0.0 [Rust Rewrite]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Line::raw("Inspired by and reverse-engineered from RVS v0.46 (Copyright (c) 2019, SIDIC)."),
                Line::raw("Preserves 100% computational and batch report parity with src/RVS046.tcl."),
                Line::raw(""),
                Line::styled("Navigation & Modal Editing:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Line::raw("  Tab / Shift+Tab   - Cycle focus between panes forward / backward"),
                Line::raw("  Enter             - Source pane: open in $EDITOR; Other panes: toggle fullscreen zoom"),
                Line::raw("  z, :zoom          - Toggle fullscreen zoom on focused pane"),
                Line::raw("  :                 - Enter Command mode (:w, :q, :c, :s, :n, :r, :stats, :stack, etc.)"),
                Line::raw("  h / j / k / l     - Vim cursor in Source; scroll line-by-line in other panes"),
                Line::raw("  w / b / 0 / $     - Jump word forward / backward, beginning / end of line"),
                Line::raw("  gg / G            - Jump to start / end of buffer or pane"),
                Line::raw("  i / a / I / A     - Enter INSERT mode (Esc to return to NORMAL)"),
                Line::raw("  o / O             - Insert line below / above and enter INSERT mode"),
                Line::raw("  x / dd / yy / p   - Delete char, delete line, yank line, paste below"),
                Line::raw("  u / Ctrl+R        - Undo / Redo in buffer"),
                Line::raw("  Ctrl+O, :e        - Open File Explorer (Yazi-style modal)"),
                Line::raw("  Ctrl+S, :w        - Save current buffer"),
                Line::raw(""),
                Line::styled("Simulation & Advanced Debugging:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Line::raw("  c, :compile       - Assemble source code into machine listing"),
                Line::raw("  s, :start         - START execution (or :reset / :initpc without running)"),
                Line::raw("  n, :n <count>     - Step 1 or <count> instructions forward"),
                Line::raw("  p, P, N, :prev    - Step back / Reverse step (128-state history ring buffer)"),
                Line::raw("  r, :run           - Run continuously (stops at ebreak, error, or breakpoint)"),
                Line::raw("  x, :stop          - Stop continuous execution"),
                Line::raw("  B in Source       - Toggle breakpoint at current cursor line (red ● marker)"),
                Line::raw("  b in Listing      - Toggle breakpoint at current PC; :cb to clear all"),
                Line::raw("  :b <line|addr|pc> - Toggle breakpoint at line, address, or PC"),
                Line::raw("  +, - ([, ]), :spd - Adjust execution animation speed (1Hz, 5Hz, 20Hz, Max)"),
                Line::raw("  :mem <sp|addr>    - Jump memory scroll to stack ($sp), 0x0, or arbitrary address"),
                Line::raw(""),
                Line::styled("Profiling, Conventions & Dialects:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Line::raw("  :stats, [STATS]   - Hardware counters, estimated cycles, live CPI & branch stats"),
                Line::raw("  :cc [on|off]      - Calling convention checker (16-byte stack, saved registers)"),
                Line::raw("  :stack, [STACK]   - Call Stack & Stack Frame Visualizer with memory slot hierarchy"),
                Line::raw("  :mode [gnu|rvs]   - Toggle between strict RVS dialect and standard GNU mode"),
                Line::raw("  :transpile [mode] - In-editor two-way transpiler between GNU and RVS syntax"),
                Line::raw("  1 / 2 in Listing  - Toggle TXT column / [CLR] correlated bitfield colorizer"),
                Line::raw("  q, :q, :q!, ^C    - Clean exit (prompts to save only if unsaved changes exist)"),
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
                rvs_explorer::ExplorerMode::Normal => {
                    Span::styled(" [NORMAL] ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD))
                }
                rvs_explorer::ExplorerMode::Filter => {
                    Span::styled(" [FILTER] ", Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD))
                }
            };

            let hints_text = match app.explorer.mode {
                rvs_explorer::ExplorerMode::Normal => {
                    "Hints: [j/k] Select  [h] Parent  [l/Enter] Open  [/ or i] Filter/Type  [J/K] Preview  [q/Esc] Close"
                }
                rvs_explorer::ExplorerMode::Filter => {
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

            if app.explorer.mode == rvs_explorer::ExplorerMode::Filter {
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
                    rvs_explorer::format_size(item.size)
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
                .title(Span::styled(" Unsaved Changes: Save Before Leaving? ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
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
        Modal::Stats => {
            let modal_area = centered_rect(76, 75, area);
            f.render_widget(Clear, modal_area);
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Execution Metrics & Hardware Counters (:stats) ");
            let inner = block.inner(modal_area);
            f.render_widget(block, modal_area);

            let m = &app.sim.metrics;
            let cc = &app.sim.convention;
            let mut text = vec![
                Line::styled("Hardware Counters & Dynamic Profiling", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  Total Instructions:   ", Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{:<10}", m.instructions), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled("Estimated Cycles: ", Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{:<10}", m.cycles), Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
                    Span::styled("Average CPI: ", Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{:.2}", m.cpi()), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]),
                Line::raw(""),
                Line::styled("Instruction Mix Breakdown:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Line::from(vec![
                    Span::styled("  ALU / Arithmetic:     ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:<8}", m.alu_ops), Style::default().fg(Color::White)),
                    Span::styled("Memory Loads:     ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:<8}", m.loads), Style::default().fg(Color::LightBlue)),
                    Span::styled("Memory Stores:    ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:<8}", m.stores), Style::default().fg(Color::LightBlue)),
                ]),
                Line::from(vec![
                    Span::styled("  Control Jumps:        ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:<8}", m.jumps), Style::default().fg(Color::White)),
                    Span::styled("Floating Point:   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:<8}", m.floats), Style::default().fg(Color::Yellow)),
                    Span::styled("Syscalls (ECALL): ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:<8}", m.ecalls), Style::default().fg(Color::Magenta)),
                ]),
                Line::raw(""),
                Line::styled("Branch Prediction Statistics:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Line::from(vec![
                    Span::styled("  Total Branches:       ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:<8}", m.branches_total), Style::default().fg(Color::White)),
                    Span::styled("Branches Taken:   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:<8}", m.branches_taken), Style::default().fg(Color::LightGreen)),
                    Span::styled("Taken Rate:       ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:.1}%", m.branch_taken_rate()), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]),
                Line::raw(""),
                Line::styled("Calling Convention Compliance Checker:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Line::from(vec![
                    Span::styled("  Checker Status:       ", Style::default().fg(Color::DarkGray)),
                    Span::styled(if cc.enabled { "ENABLED (Active)" } else { "DISABLED" }, if cc.enabled { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) }),
                    Span::styled("   Call Stack Depth: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:<4}", cc.call_stack.len()), Style::default().fg(Color::White)),
                    Span::styled("Violations: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{}", cc.violations.len()), if cc.violations.is_empty() { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) }),
                ]),
            ];

            if !cc.violations.is_empty() {
                text.push(Line::raw(""));
                text.push(Line::styled("  Recent Convention Violations:", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
                for v in cc.violations.iter().rev().take(5) {
                    text.push(Line::styled(format!("    ! {v}"), Style::default().fg(Color::LightRed)));
                }
            } else if cc.enabled {
                text.push(Line::raw(""));
                text.push(Line::styled("  ✓ No calling convention violations detected (saved registers & stack aligned).", Style::default().fg(Color::Green)));
            }

            text.push(Line::raw(""));
            text.push(Line::styled("Press Esc or Enter to close.", Style::default().fg(Color::DarkGray)));

            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), inner);
        }
        Modal::CallStack => {
            let modal_area = centered_rect(82, 80, area);
            f.render_widget(Clear, modal_area);
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::LightBlue))
                .title(" Call Stack & Stack Frame Visualizer (:stack) ");
            let inner = block.inner(modal_area);
            f.render_widget(block, modal_area);

            let frames = app.sim.get_stack_frames();
            let cur_sp = app.sim.regs.read_x(2);
            let is_aligned = cur_sp % 16 == 0;

            let mut text = vec![
                Line::styled("Active Call Frames & Stack Hierarchy", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  Current $sp: ", Style::default().fg(Color::Cyan)),
                    Span::styled(format!("0x{:016x}", cur_sp), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled("   Alignment: ", Style::default().fg(Color::Cyan)),
                    if is_aligned {
                        Span::styled("16-byte ALIGNED ✓", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                    } else {
                        Span::styled("MISALIGNED ✗", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                    },
                    Span::styled("   Call Stack Depth: ", Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{} frames", frames.len()), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]),
                Line::raw(""),
            ];

            if frames.is_empty() {
                text.push(Line::styled(
                    "  (no active function call frames: call a function via 'jal ra, ...' or 'call ...')",
                    Style::default().fg(Color::DarkGray),
                ));
            } else {
                for frame in frames.iter().rev() {
                    text.push(Line::from(vec![
                        Span::styled(format!("  ┌ Frame {}: ", frame.depth), Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)),
                        Span::styled(&frame.function_name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                        Span::styled(format!(" [0x{:x}]", frame.target_pc), Style::default().fg(Color::Cyan)),
                        Span::styled(" called from ", Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("{} (0x{:x})", frame.caller_name, frame.call_pc), Style::default().fg(Color::Yellow)),
                    ]));
                    text.push(Line::styled(
                        format!("  │   sp: 0x{:016x} → 0x{:016x} (size: {} bytes)", frame.frame_sp, frame.parent_sp, frame.frame_size),
                        Style::default().fg(Color::DarkGray),
                    ));

                    if frame.slots.is_empty() {
                        text.push(Line::styled("  │   (no double-words written to stack in this frame)", Style::default().fg(Color::DarkGray)));
                    } else {
                        for slot in &frame.slots {
                            text.push(Line::from(vec![
                                Span::styled("  │   ", Style::default().fg(Color::LightBlue)),
                                Span::styled(format!("0x{:016x}", slot.address), Style::default().fg(Color::Cyan)),
                                Span::styled(" : ", Style::default().fg(Color::DarkGray)),
                                Span::styled(format!("0x{:016x}", slot.value), Style::default().fg(Color::White)),
                                Span::styled(format!("  ({})", slot.annotation), Style::default().fg(Color::LightGreen)),
                            ]));
                        }
                    }
                    text.push(Line::styled("  └─────────────────────────────────────────────────────────────", Style::default().fg(Color::LightBlue)));
                }
            }

            text.push(Line::raw(""));
            text.push(Line::styled("Press Esc or Enter to close.", Style::default().fg(Color::DarkGray)));

            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), inner);
        }
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
