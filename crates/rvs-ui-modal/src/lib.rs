//! Modal overlay renderers for the RVS++ TUI.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
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
                .style(rvs_theme::PANE)
                .border_type(BorderType::Double)
                .border_style(rvs_theme::INFO_PLAIN)
                .title(Span::styled(" About / Help & Command Reference (?) ", rvs_theme::FOCUS));
            let inner = block.inner(modal_area);
            f.render_widget(block, modal_area);

            let text = vec![
                Line::styled("RVS++ (RISC-V Visual Simulator++) v1.0.0 [Rust Rewrite]", rvs_theme::HEADING),
                Line::raw("Inspired by and reverse-engineered from RVS v0.46 (Copyright (c) 2019, SIDIC)."),
                Line::raw("Preserves 100% computational and batch report parity with src/RVS046.tcl."),
                Line::raw(""),
                Line::styled("Navigation & Modal Editing:", rvs_theme::INFO),
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
                Line::styled("Simulation & Advanced Debugging:", rvs_theme::INFO),
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
                Line::styled("Profiling, Conventions & Dialects:", rvs_theme::INFO),
                Line::raw("  :stats, [STATS]   - Hardware counters, estimated cycles, live CPI & branch stats"),
                Line::raw("  :cc [on|off]      - Calling convention checker (16-byte stack, saved registers)"),
                Line::raw("  :stack, [STACK]   - Call Stack & Stack Frame Visualizer with memory slot hierarchy"),
                Line::raw("  :mode [gnu|rvs]   - Toggle between strict RVS dialect and standard GNU mode"),
                Line::raw("  :transpile [mode] - In-editor two-way transpiler between GNU and RVS syntax"),
                Line::raw("  1 / 2 in Listing  - Toggle TXT column / [CLR] correlated bitfield colorizer"),
                Line::raw("  q, :q, :q!, ^C    - Clean exit (prompts to save only if unsaved changes exist)"),
                Line::raw(""),
                Line::styled("Press Esc or Enter to close.", rvs_theme::MUTED),
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
                .style(rvs_theme::PANE)
                .border_type(BorderType::Double)
                .border_style(rvs_theme::INFO_PLAIN)
                .title(Span::styled(title, rvs_theme::HEADING));
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
                    Span::styled(" [NORMAL] ", rvs_theme::BADGE_INFO)
                }
                rvs_explorer::ExplorerMode::Filter => {
                    Span::styled(" [FILTER] ", rvs_theme::BADGE_ACCENT)
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
                    Span::styled("Path: ", rvs_theme::HEADING),
                    Span::styled(&app.modal_input, rvs_theme::INFO),
                ]),
                Line::styled(
                    hints_text,
                    rvs_theme::MUTED,
                ),
                Line::styled("─".repeat(inner.width as usize), rvs_theme::MUTED),
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
                .style(rvs_theme::PANE)
                .border_style(rvs_theme::INFO_PLAIN)
                .title(Span::styled(dir_title, rvs_theme::INFO));
            let list_inner = list_block.inner(list_area);
            f.render_widget(list_block, list_area);
            f.render_widget(Block::default().style(rvs_theme::PANE), list_inner);
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
                    rvs_theme::SELECTED
                } else if item.is_dir {
                    rvs_theme::INFO_PLAIN.add_modifier(Modifier::BOLD)
                } else {
                    rvs_theme::TEXT_STYLE
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
                list_rows.push(Line::styled("  [No matching items]", rvs_theme::MUTED));
            }
            f.render_widget(Paragraph::new(list_rows), list_inner);

            let preview_title = format!(" Preview: {} ", app.explorer.preview_title);
            let prev_block = Block::default()
                .borders(Borders::ALL)
                .style(rvs_theme::PANE)
                .border_style(rvs_theme::FAINT)
                .title(Span::styled(preview_title, rvs_theme::HEADING));
            let prev_inner = prev_block.inner(preview_area);
            f.render_widget(prev_block, preview_area);
            f.render_widget(Block::default().style(rvs_theme::PANE), prev_inner);

            let prev_width = prev_inner.width as usize;
            let prev_height = prev_inner.height as usize;
            let mut prev_lines = Vec::new();
            for raw_line in app.explorer.preview_lines.iter().skip(app.explorer.scroll_preview).take(prev_height) {
                let expanded = expand_tabs(raw_line, 8);
                let l: String = expanded.chars().take(prev_width).collect();
                if l.trim_start().starts_with("[DIR]") {
                    prev_lines.push(Line::styled(l, rvs_theme::INFO_PLAIN.add_modifier(Modifier::BOLD)));
                } else if l.contains('│') {
                    let parts: Vec<&str> = l.splitn(2, '│').collect();
                    if parts.len() == 2 {
                        prev_lines.push(Line::from(vec![
                            Span::styled(format!("{}│", parts[0]), rvs_theme::FAINT),
                            Span::styled(parts[1].to_string(), rvs_theme::SECONDARY),
                        ]));
                        continue;
                    }
                    prev_lines.push(Line::styled(l, rvs_theme::SECONDARY));
                } else if l.starts_with('[') {
                    prev_lines.push(Line::styled(l, rvs_theme::FOCUS));
                } else {
                    prev_lines.push(Line::styled(l, rvs_theme::TEXT_STYLE));
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
                .style(rvs_theme::PANE)
                .border_type(BorderType::Double)
                .border_style(rvs_theme::ERR)
                .title(Span::styled(" Unsaved Changes: Save Before Leaving? ", rvs_theme::HEADING));
            let inner = block.inner(modal_area);
            f.render_widget(block, modal_area);

            let text = vec![
                Line::styled("There are unsaved changes in your source buffer.", rvs_theme::TEXT_STYLE),
                Line::styled(format!("Would you like to save changes before {action_desc}?"), rvs_theme::WARN_PLAIN),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("File path: [ ", rvs_theme::INFO),
                    Span::styled(&app.modal_input, Style::default().fg(rvs_theme::TEXT).add_modifier(Modifier::BOLD)),
                    Span::styled(" ]", rvs_theme::INFO),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(" [Enter]  ", rvs_theme::BADGE_ON),
                    Span::styled(" Save to file path above and proceed", rvs_theme::TEXT_STYLE),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(" [Ctrl+D] ", rvs_theme::BADGE_ERR),
                    Span::styled(" Discard changes and proceed without saving", rvs_theme::TEXT_STYLE),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(" [Esc]    ", rvs_theme::BADGE_NEUTRAL),
                    Span::styled(" Cancel and return to editor", rvs_theme::SECONDARY),
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
                .style(rvs_theme::PANE)
                .border_type(BorderType::Double)
                .border_style(rvs_theme::INFO_PLAIN)
                .title(Span::styled(" Execution Metrics & Hardware Counters (:stats) ", rvs_theme::HEADING));
            let inner = block.inner(modal_area);
            f.render_widget(block, modal_area);

            let m = &app.sim.metrics;
            let cc = &app.sim.convention;
            let mut text = vec![
                Line::styled("Hardware Counters & Dynamic Profiling", rvs_theme::HEADING),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  Total Instructions:   ", rvs_theme::SECONDARY),
                    Span::styled(format!("{:<10}", m.instructions), Style::default().fg(rvs_theme::TEXT).add_modifier(Modifier::BOLD)),
                    Span::styled("Estimated Cycles: ", rvs_theme::SECONDARY),
                    Span::styled(format!("{:<10}", m.cycles), rvs_theme::OK),
                    Span::styled("Average CPI: ", rvs_theme::SECONDARY),
                    Span::styled(format!("{:.2}", m.cpi()), rvs_theme::HEADING),
                ]),
                Line::raw(""),
                Line::styled("Instruction Mix Breakdown:", rvs_theme::INFO),
                Line::from(vec![
                    Span::styled("  ALU / Arithmetic:     ", rvs_theme::MUTED),
                    Span::styled(format!("{:<8}", m.alu_ops), rvs_theme::TEXT_STYLE),
                    Span::styled("Memory Loads:     ", rvs_theme::MUTED),
                    Span::styled(format!("{:<8}", m.loads), rvs_theme::INFO_PLAIN),
                    Span::styled("Memory Stores:    ", rvs_theme::MUTED),
                    Span::styled(format!("{:<8}", m.stores), rvs_theme::INFO_PLAIN),
                ]),
                Line::from(vec![
                    Span::styled("  Control Jumps:        ", rvs_theme::MUTED),
                    Span::styled(format!("{:<8}", m.jumps), rvs_theme::TEXT_STYLE),
                    Span::styled("Floating Point:   ", rvs_theme::MUTED),
                    Span::styled(format!("{:<8}", m.floats), rvs_theme::WARN_PLAIN),
                    Span::styled("Syscalls (ECALL): ", rvs_theme::MUTED),
                    Span::styled(format!("{:<8}", m.ecalls), Style::default().fg(rvs_theme::MAUVE)),
                ]),
                Line::raw(""),
                Line::styled("Branch Prediction Statistics:", rvs_theme::INFO),
                Line::from(vec![
                    Span::styled("  Total Branches:       ", rvs_theme::MUTED),
                    Span::styled(format!("{:<8}", m.branches_total), rvs_theme::TEXT_STYLE),
                    Span::styled("Branches Taken:   ", rvs_theme::MUTED),
                    Span::styled(format!("{:<8}", m.branches_taken), rvs_theme::OK_PLAIN),
                    Span::styled("Taken Rate:       ", rvs_theme::MUTED),
                    Span::styled(format!("{:.1}%", m.branch_taken_rate()), rvs_theme::HEADING),
                ]),
                Line::raw(""),
                Line::styled("Calling Convention Compliance Checker:", rvs_theme::INFO),
                Line::from(vec![
                    Span::styled("  Checker Status:       ", rvs_theme::MUTED),
                    Span::styled(if cc.enabled { "ENABLED (Active)" } else { "DISABLED" }, if cc.enabled { rvs_theme::OK } else { rvs_theme::MUTED }),
                    Span::styled("   Call Stack Depth: ", rvs_theme::MUTED),
                    Span::styled(format!("{:<4}", cc.call_stack.len()), rvs_theme::TEXT_STYLE),
                    Span::styled("Violations: ", rvs_theme::MUTED),
                    Span::styled(format!("{}", cc.violations.len()), if cc.violations.is_empty() { rvs_theme::OK_PLAIN } else { rvs_theme::ERR }),
                ]),
            ];

            if !cc.violations.is_empty() {
                text.push(Line::raw(""));
                text.push(Line::styled("  Recent Convention Violations:", rvs_theme::ERR));
                for v in cc.violations.iter().rev().take(5) {
                    text.push(Line::styled(format!("    ! {v}"), rvs_theme::ERR_PLAIN));
                }
            } else if cc.enabled {
                text.push(Line::raw(""));
                text.push(Line::styled("  ✓ No calling convention violations detected (saved registers & stack aligned).", rvs_theme::OK_PLAIN));
            }

            text.push(Line::raw(""));
            text.push(Line::styled("Press Esc or Enter to close.", rvs_theme::MUTED));

            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), inner);
        }
        Modal::CallStack => {
            let modal_area = centered_rect(82, 80, area);
            f.render_widget(Clear, modal_area);
            let block = Block::default()
                .borders(Borders::ALL)
                .style(rvs_theme::PANE)
                .border_type(BorderType::Double)
                .border_style(rvs_theme::INFO_PLAIN)
                .title(Span::styled(" Call Stack & Stack Frame Visualizer (:stack) ", rvs_theme::HEADING));
            let inner = block.inner(modal_area);
            f.render_widget(block, modal_area);

            let frames = app.sim.get_stack_frames();
            let cur_sp = app.sim.regs.read_x(2);
            let is_aligned = cur_sp % 16 == 0;

            let mut text = vec![
                Line::styled("Active Call Frames & Stack Hierarchy", rvs_theme::HEADING),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("  Current $sp: ", rvs_theme::SECONDARY),
                    Span::styled(format!("0x{:016x}", cur_sp), Style::default().fg(rvs_theme::TEXT).add_modifier(Modifier::BOLD)),
                    Span::styled("   Alignment: ", rvs_theme::SECONDARY),
                    if is_aligned {
                        Span::styled("16-byte ALIGNED ✓", rvs_theme::OK)
                    } else {
                        Span::styled("MISALIGNED ✗", rvs_theme::ERR)
                    },
                    Span::styled("   Call Stack Depth: ", rvs_theme::SECONDARY),
                    Span::styled(format!("{} frames", frames.len()), rvs_theme::HEADING),
                ]),
                Line::raw(""),
            ];

            if frames.is_empty() {
                text.push(Line::styled(
                    "  (no active function call frames: call a function via 'jal ra, ...' or 'call ...')",
                    rvs_theme::MUTED,
                ));
            } else {
                for frame in frames.iter().rev() {
                    text.push(Line::from(vec![
                        Span::styled(format!("  ┌ Frame {}: ", frame.depth), rvs_theme::INFO_PLAIN.add_modifier(Modifier::BOLD)),
                        Span::styled(&frame.function_name, Style::default().fg(rvs_theme::TEXT).add_modifier(Modifier::BOLD)),
                        Span::styled(format!(" [0x{:x}]", frame.target_pc), rvs_theme::INFO_PLAIN),
                        Span::styled(" called from ", rvs_theme::MUTED),
                        Span::styled(format!("{} (0x{:x})", frame.caller_name, frame.call_pc), Style::default().fg(rvs_theme::LAVENDER)),
                    ]));
                    text.push(Line::styled(
                        format!("  │   sp: 0x{:016x} → 0x{:016x} (size: {} bytes)", frame.frame_sp, frame.parent_sp, frame.frame_size),
                        rvs_theme::MUTED,
                    ));

                    if frame.slots.is_empty() {
                        text.push(Line::styled("  │   (no double-words written to stack in this frame)", rvs_theme::MUTED));
                    } else {
                        for slot in &frame.slots {
                            text.push(Line::from(vec![
                                Span::styled("  │   ", rvs_theme::INFO_PLAIN),
                                Span::styled(format!("0x{:016x}", slot.address), rvs_theme::INFO_PLAIN),
                                Span::styled(" : ", rvs_theme::FAINT),
                                Span::styled(format!("0x{:016x}", slot.value), rvs_theme::TEXT_STYLE),
                                Span::styled(format!("  ({})", slot.annotation), rvs_theme::OK_PLAIN),
                            ]));
                        }
                    }
                    text.push(Line::styled("  └─────────────────────────────────────────────────────────────", rvs_theme::INFO_PLAIN));
                }
            }

            text.push(Line::raw(""));
            text.push(Line::styled("Press Esc or Enter to close.", rvs_theme::MUTED));

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
