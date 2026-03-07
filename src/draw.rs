use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, CheckStatus, ListEntry, Mode};

// ---------------------------------------------------------------------------
// Main draw dispatcher
// ---------------------------------------------------------------------------

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5), Constraint::Length(3)])
        .split(area);

    // Setup screen is rendered entirely separately.
    if let Mode::Setup = &app.mode {
        draw_setup(f, app, area);
        return;
    }

    // ── Title ─────────────────────────────────────────────────────────────────
    let state_label = match &app.mode {
        Mode::Setup => unreachable!(),
        Mode::Selecting => "Select & Run",
        Mode::Running { .. } => "Running…",
        Mode::Done => "Done",
    };
    let cpu_color = if app.cpu_pct > 90.0 { Color::Red } else if app.cpu_pct > 70.0 { Color::Yellow } else { Color::Cyan };
    let mem_color = if app.mem_pct > 90.0 { Color::Red } else if app.mem_pct > 70.0 { Color::Yellow } else { Color::Cyan };
    let left  = format!(" {}  [{}]  branch: {} ", app.repo_root.display(), state_label, app.current_branch);
    let right_cpu = format!("CPU:{:3.0}% ", app.cpu_pct);
    let right_mem = format!("MEM:{:3.0}% ", app.mem_pct);
    // -2 for the left/right border chars; saturate so we never underflow
    let inner_width = root[0].width.saturating_sub(2) as usize;
    let pad = inner_width.saturating_sub(left.len() + right_cpu.len() + right_mem.len());
    let title_line = Line::from(vec![
        Span::raw(left),
        Span::raw(" ".repeat(pad)),
        Span::styled(right_cpu, Style::default().fg(cpu_color).add_modifier(Modifier::BOLD)),
        Span::styled(right_mem, Style::default().fg(mem_color).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(
        Paragraph::new(title_line).block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ),
        root[0],
    );

    // ── Main split ────────────────────────────────────────────────────────────
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(root[1]);

    app.list_area = Some(main[0]);

    // ── Check list ────────────────────────────────────────────────────────────
    let running_check_idx = match &app.mode {
        Mode::Running { idx } => Some(*idx),
        _ => None,
    };

    let items: Vec<ListItem> = app.entries.iter().map(|entry| {
        match entry {
            ListEntry::GroupHeader(group) => {
                let (sel, tot) = app.group_state(*group);
                let cb = if sel == 0 { "[ ]" } else if sel == tot { "[x]" } else { "[-]" };
                let color = group.color();
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{cb} ─ {} ({sel}/{tot})", group.label()),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                ]))
            }
            ListEntry::Check(ci) => {
                let ci = *ci;
                let check = &app.checks[ci];
                let cb = if app.selected[ci] { "[x]" } else { "[ ]" };
                let cb_style = if app.selected[ci] {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let (icon, icon_style, name_style, elapsed_str, elapsed_style) = match &app.statuses[ci] {
                    CheckStatus::Pending => {
                        let s = if running_check_idx == Some(ci) {
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        ("   ", s, s, String::new(), Style::default().fg(Color::DarkGray))
                    }
                    CheckStatus::Running => (
                        ">> ",
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        String::new(),
                        Style::default().fg(Color::DarkGray),
                    ),
                    CheckStatus::Passed(t) => (
                        "✓  ",
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::Green),
                        format!(" {t:.1}s"),
                        Style::default().fg(Color::Green),
                    ),
                    CheckStatus::Failed(t) => (
                        "✗  ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        format!(" {t:.1}s"),
                        Style::default().fg(Color::Red),
                    ),
                    CheckStatus::Skipped => (
                        "---",
                        Style::default().fg(Color::DarkGray),
                        Style::default().fg(Color::DarkGray),
                        String::new(),
                        Style::default().fg(Color::DarkGray),
                    ),
                    CheckStatus::Advisory(t) => (
                        "⚠  ",
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::Yellow),
                        format!(" {t:.1}s"),
                        Style::default().fg(Color::Yellow),
                    ),
                };

                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{cb} "), cb_style),
                    Span::styled(icon, icon_style),
                    Span::raw(" "),
                    Span::styled(check.name.clone(), name_style),
                    Span::styled(elapsed_str, elapsed_style),
                ]))
            }
        }
    }).collect();

    let total_sel: usize = app.selected.iter().filter(|&&s| s).count();
    let list_title = format!(" Checks ({}/{} selected) ", total_sel, app.checks.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, main[0], &mut app.list_state);

    // ── Output / description panel ────────────────────────────────────────────
    let output_content: Vec<Line> = if app.output_lines.is_empty() {
        match app.list_state.selected().map(|ei| &app.entries[ei]) {
            Some(ListEntry::GroupHeader(group)) => {
                let (sel, tot) = app.group_state(*group);
                vec![
                    Line::from(vec![
                        Span::styled("Group: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(
                            group.label(),
                            Style::default().fg(group.color()).add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("Checks selected: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(format!("{sel}/{tot}")),
                    ]),
                    Line::raw(""),
                    Line::raw("Space / click: toggle all checks in this group"),
                ]
            }
            Some(ListEntry::Check(ci)) => {
                let check = &app.checks[*ci];
                let staged_note = match &check.only_when_staged {
                    Some(p) => format!("Only runs when '{p}' files are staged."),
                    None => "Runs unconditionally.".to_string(),
                };
                vec![
                    Line::from(vec![
                        Span::styled("Name:  ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(check.name.clone()),
                    ]),
                    Line::from(vec![
                        Span::styled("About: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(check.description.clone()),
                    ]),
                    Line::from(vec![
                        Span::styled("When:  ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(staged_note),
                    ]),
                    Line::raw(""),
                    Line::from(vec![
                        Span::styled("Cmd:   ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(check.cmd.join(" "), Style::default().fg(Color::Yellow)),
                    ]),
                ]
            }
            None => vec![],
        }
    } else {
        app.output_lines
            .iter()
            .skip(app.output_scroll)
            .map(|l| {
                let style = if l.starts_with("└─ [+]") || l.starts_with("[+]") {
                    Style::default().fg(Color::Green)
                } else if l.starts_with("└─ [x]") || l.starts_with("[x]") {
                    Style::default().fg(Color::Red)
                } else if l.starts_with("└─ [!]") || l.starts_with("[!]") {
                    Style::default().fg(Color::Yellow)
                } else if l.starts_with("[-]") {
                    Style::default().fg(Color::DarkGray)
                } else if l.starts_with("┌─") {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(l.as_str(), style))
            })
            .collect()
    };

    let mouse_indicator = if app.mouse_capture { "" } else { " [mouse off — select freely] " };
    let output_title = if app.output_lines.is_empty() {
        format!(" Description{mouse_indicator}")
    } else {
        format!(" Output  {}/{}{}",
            app.output_scroll + 1,
            app.output_lines.len(),
            if mouse_indicator.is_empty() { " ".to_string() } else { format!(" |{mouse_indicator}") }
        )
    };

    f.render_widget(
        Paragraph::new(output_content)
            .block(Block::default().borders(Borders::ALL).title(output_title))
            .wrap(Wrap { trim: false }),
        main[1],
    );

    // ── Status bar ────────────────────────────────────────────────────────────
    if let Mode::Done = &app.mode {
        let (passed, failed, skipped, advisory) = app.summary_counts();
        let summary = format!(
            " Done — OK: {passed}  FAIL: {failed}  WARN: {advisory}  SKIP: {skipped}  | r: back to select | Enter: run again | q: quit "
        );
        let color = if failed > 0 {
            Color::Red
        } else if advisory > 0 {
            Color::Yellow
        } else {
            Color::Green
        };
        f.render_widget(
            Paragraph::new(summary).block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
            ),
            root[2],
        );
        return;
    }

    let (staged_info, help_text) = match &app.mode {
        Mode::Setup => unreachable!(),
        Mode::Selecting => {
            let n = app.staged_files.len();
            (
                format!(" {n} staged file(s) "),
                " ↑↓/jk: move | Space/Click: toggle | a: all | n: none | Enter: run | m: mouse | q: quit "
                    .to_string(),
            )
        }
        Mode::Running { idx } => (
            format!(" Running {}/{} ", idx + 1, app.checks.len()),
            " Running… PgUp/PgDn: scroll | m: toggle mouse (to select text) ".to_string(),
        ),
        Mode::Done => unreachable!(),
    };

    let status_bar = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(10)])
        .split(root[2]);

    f.render_widget(
        Paragraph::new(staged_info).block(
            Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Cyan)),
        ),
        status_bar[0],
    );
    f.render_widget(
        Paragraph::new(help_text).block(
            Block::default().borders(Borders::ALL).style(Style::default().fg(Color::DarkGray)),
        ),
        status_bar[1],
    );
}

// ---------------------------------------------------------------------------
// Setup screen
// ---------------------------------------------------------------------------

pub fn draw_setup(f: &mut Frame, app: &App, area: Rect) {
    // Log panel height: show if there's log output (checkout errors), min 0 max 8
    let log_height = if app.setup_log.is_empty() { 0u16 } else { 8u16 };

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Length(14 + log_height),
            Constraint::Min(0),
        ])
        .split(area);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Min(50),
            Constraint::Percentage(10),
        ])
        .split(vert[1]);
    let form_area = horiz[1];

    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(" mcp-context-forge Pre-commit — Project Setup ")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        form_area,
    );

    // Layout inside the form:
    // [0] repo label  [1] repo input  [2] blank
    // [3] branch label  [4] branch input  [5] current-branch info
    // [6] error line  [7] hint
    // [8] log panel (only when log_height > 0)
    let mut constraints = vec![
        Constraint::Length(1), // repo label
        Constraint::Length(3), // repo input
        Constraint::Length(1), // blank
        Constraint::Length(1), // branch label
        Constraint::Length(3), // branch input
        Constraint::Length(1), // current branch info
        Constraint::Length(1), // error
        Constraint::Length(1), // hint
    ];
    if log_height > 0 {
        constraints.push(Constraint::Length(log_height));
    }

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .margin(1)
        .split(form_area);

    // ── Repo field ────────────────────────────────────────────────────────────
    f.render_widget(
        Paragraph::new("Project path (required):")
            .style(Style::default().add_modifier(Modifier::BOLD)),
        inner[0],
    );
    let repo_style = if app.setup_focus == 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let repo_text = if app.setup_focus == 0 {
        format!("{}\u{2588}", app.setup_repo)
    } else {
        app.setup_repo.clone()
    };
    f.render_widget(
        Paragraph::new(repo_text)
            .block(Block::default().borders(Borders::ALL).style(repo_style)),
        inner[1],
    );

    // ── Branch / PR field ─────────────────────────────────────────────────────
    f.render_widget(
        Paragraph::new("Branch or PR number (optional — leave blank to stay on current):")
            .style(Style::default().add_modifier(Modifier::BOLD)),
        inner[3],
    );
    let branch_style = if app.setup_focus == 1 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let branch_text = if app.setup_focus == 1 {
        format!("{}\u{2588}", app.setup_branch)
    } else {
        app.setup_branch.clone()
    };
    f.render_widget(
        Paragraph::new(branch_text)
            .block(Block::default().borders(Borders::ALL).style(branch_style)),
        inner[4],
    );

    // ── Current branch info ───────────────────────────────────────────────────
    f.render_widget(
        Paragraph::new(format!("  Current branch: {}", app.current_branch))
            .style(Style::default().fg(Color::Cyan)),
        inner[5],
    );

    // ── Error line ────────────────────────────────────────────────────────────
    if let Some(err) = &app.setup_error {
        f.render_widget(
            Paragraph::new(format!("  ✗ {err}"))
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            inner[6],
        );
    }

    // ── Hint ──────────────────────────────────────────────────────────────────
    f.render_widget(
        Paragraph::new("  Tab/↑↓: switch field   Enter: confirm   Esc: quit")
            .style(Style::default().fg(Color::DarkGray)),
        inner[7],
    );

    // ── Checkout log (shown on error) ─────────────────────────────────────────
    if log_height > 0 {
        let log_text = app.setup_log.join("\n");
        let log_style = if app.setup_error.is_some() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        f.render_widget(
            Paragraph::new(log_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Checkout Output ")
                        .style(log_style),
                )
                .wrap(Wrap { trim: false }),
            inner[8],
        );
    }
}
