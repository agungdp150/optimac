use super::*;

pub(super) fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let cyan = Style::default().fg(Color::Rgb(103, 232, 249));
    if area.height < 4 || area.width < 18 {
        frame.render_widget(
            Paragraph::new(format!(
                "OptiMac\n{}\nq quit",
                if app.screen == Screen::Home {
                    HOME[app.home_cursor].1
                } else {
                    action_name(app.selected)
                }
            ))
            .style(cyan),
            area,
        );
        return;
    }
    let status_screen = app.screen == Screen::Result && app.home_action == HomeAction::Status;
    let header_height = if status_screen {
        0
    } else if area.height >= 16 {
        5
    } else {
        1
    };
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);
    let header_text = if header_height == 5 {
        format!(
            " /\\_/\\  zZ    OptiMac\n( o.o )       mac care · Rust {}\n > ^ <        inspect safely, keep your Mac comfortable\nScope: {}",
            env!("CARGO_PKG_VERSION"),
            app.root.display()
        )
    } else {
        "OptiMac  /\\_/\\ zZ".into()
    };
    frame.render_widget(Paragraph::new(header_text).style(cyan), header);
    match app.screen {
        Screen::Scope => {
            frame.render_widget(Paragraph::new(format!("Scope directory (absolute or relative):\n{}\n{}\n\nEnter save · Esc cancel\nClean also reviews supported home cache roots.",app.scope_input,app.scope_error)).wrap(Wrap {trim:false}),body);
        }
        Screen::Home => {
            let entries = HOME.iter().enumerate().map(|(i, (_, name, desc))| {
                ListItem::new(format!("{}. {:12} {}", i + 1, name, desc))
            });
            let mut state = ListState::default().with_selected(Some(app.home_cursor));
            frame.render_stateful_widget(
                List::new(entries)
                    .highlight_symbol("› ")
                    .highlight_style(cyan.add_modifier(Modifier::BOLD)),
                body,
                &mut state,
            );
        }
        Screen::Menu => {
            let mut list_area = body;
            if area.width >= 95 {
                let [left, right] =
                    Layout::horizontal([Constraint::Percentage(52), Constraint::Percentage(48)])
                        .areas(body);
                list_area = left;
                let current = submenu(app.home_action)[app.submenu_cursor];
                let description = if current == Action::Inspect {
                    "Inspect file sizes in the selected directory. Read-only, with scan limits and cancellation.\n\nEnter to start."
                } else {
                    optimac::actions::description(current)
                };
                frame.render_widget(
                    Paragraph::new(description)
                        .wrap(Wrap { trim: true })
                        .block(Block::bordered().title(action_name(current))),
                    right,
                );
            }
            let items: Vec<_> = submenu(app.home_action)
                .iter()
                .map(|&i| ListItem::new(action_name(i)))
                .collect();
            let list = List::new(items)
                .block(Block::default().title("ACTIONS"))
                .highlight_symbol("› ")
                .highlight_style(cyan.bg(Color::Rgb(23, 32, 54)).add_modifier(Modifier::BOLD));
            let mut state = ListState::default().with_selected(Some(app.submenu_cursor));
            frame.render_stateful_widget(list, list_area, &mut state);
        }
        Screen::Scan => {
            let state = app.shared.lock().unwrap();
            let r = &state.0;
            let text = format!(
                "{}\n\nFiles: {}\nLogical size: {}\nSkipped: {}   Errors: {}\n\nRead-only. Logical size is not reclaimable space.",
                state.1,
                r.files,
                optimac::format_size(r.logical_bytes),
                r.skipped,
                r.errors
            );
            frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), body);
        }
        Screen::Result
            if app.home_action == HomeAction::Status && app.selected != Action::Status =>
        {
            let detail = app.status_detail.lock().unwrap();
            match detail.as_ref() {
                Some(detail) => super::status::render_detail(
                    frame,
                    body,
                    action_name(app.selected),
                    detail,
                    app.scroll,
                ),
                None => frame.render_widget(
                    Paragraph::new(
                        " /\\_/\\\n( o.o )  Gathering status…\n > ^ <\n\nPress c to cancel.",
                    )
                    .style(cyan),
                    body,
                ),
            }
        }
        Screen::Result if app.selected == Action::Status => {
            let snapshot = app.status_snapshot.lock().unwrap();
            match snapshot.as_ref() {
                Some(status) => super::status::render(frame, body, status),
                None => frame.render_widget(
                    Paragraph::new(
                        " /\\_/\\\n( o.o )  Gathering system status…\n > ^ <\n\nPress c to cancel.",
                    )
                    .style(cyan),
                    body,
                ),
            }
        }
        Screen::Result
            if app.job.as_ref().is_some_and(|job| !job.is_finished())
                && app.cleanup_progress.lock().unwrap().is_some() =>
        {
            let progress = app.cleanup_progress.lock().unwrap();
            let p = progress.as_ref().unwrap();
            let [title, bar, details] = Layout::vertical([
                Constraint::Length(if body.height >= 8 { 4 } else { 1 }),
                Constraint::Length(if body.height >= 3 { 2 } else { 1 }),
                Constraint::Min(0),
            ])
            .areas(body);
            let cats = [
                " /\\_/\\   zZ\n( o.o )   Carefully moving files\n > ^ <",
                " /\\_/\\  ♪\n( -.- )   Carefully moving files\n > ^ <",
            ];
            let caption = if title.height >= 3 {
                cats[(app.animation_frame / 5) % cats.len()]
            } else {
                "/\\_/\\  Moving files"
            };
            frame.render_widget(Paragraph::new(caption).style(cyan), title);
            let ratio = if p.total == 0 {
                0.0
            } else {
                (p.processed as f64 / p.total as f64).clamp(0.0, 1.0)
            };
            let label = format!("{} / {} checked", p.processed, p.total);
            frame.render_widget(
                Gauge::default().gauge_style(cyan).ratio(ratio).label(label),
                bar,
            );
            let current = p
                .current
                .as_ref()
                .map(|path| format!("{:?}", path))
                .unwrap_or_else(|| "Preparing recovery…".into());
            let state = if app.cancel.load(Ordering::Relaxed) {
                "Cancellation requested; finishing current step"
            } else {
                "c cancel · Esc back"
            };
            frame.render_widget(
                Paragraph::new(format!(
                    "Moved: {}   Skipped: {}\n{}\n{}\nMoving to recovery does not free disk space.",
                    p.moved, p.skipped, current, state
                ))
                .wrap(Wrap { trim: false }),
                details,
            );
        }
        Screen::Result => {
            let text = app.output.lock().unwrap();
            let choices = app.choices.lock().unwrap();
            if app.selected == Action::Clean && !app.confirm && app.plan.lock().unwrap().is_some() {
                let plan = app.plan.lock().unwrap();
                let plan = plan.as_ref().unwrap();
                let groups = optimac::recovery::categories(plan);
                let selected = optimac::recovery::select_files(plan, &app.selected_files);
                let [summary, rows] = Layout::vertical([
                    Constraint::Length(if body.height >= 8 { 4 } else { 1 }),
                    Constraint::Min(0),
                ])
                .areas(body);
                frame.render_widget(Paragraph::new(format!("{} files selected · {}\nEnter: open group · Space: toggle · a: select caches · u: clear · r: review\nEligible old caches preselected; personal files are not.\nScope: {}",selected.items.len(),optimac::format_size(selected.items.iter().fold(0u64,|n,c|n.saturating_add(c.logical_size()))),app.root.display())).wrap(Wrap{trim:false}),summary);
                let labels: Vec<String> = if let Some(group) = &app.file_group {
                    plan.items
                        .iter()
                        .filter(|c| optimac::recovery::category(c) == group)
                        .map(|c| {
                            format!(
                                "[{}] {}  {}",
                                if app.selected_files.contains(&c.path) {
                                    "x"
                                } else {
                                    " "
                                },
                                optimac::format_size(c.logical_size()),
                                c.path
                                    .to_string_lossy()
                                    .chars()
                                    .map(|ch| if ch.is_control() { '�' } else { ch })
                                    .collect::<String>()
                            )
                        })
                        .collect()
                } else {
                    groups
                        .iter()
                        .map(|g| {
                            format!(
                                "{} · {} files · {}",
                                g.name,
                                g.count,
                                optimac::format_size(g.bytes)
                            )
                        })
                        .collect()
                };
                let list = List::new(labels.iter().map(|s| ListItem::new(s.as_str())))
                    .highlight_symbol("› ")
                    .highlight_style(cyan)
                    .block(
                        Block::default().title(app.file_group.as_deref().unwrap_or("File groups")),
                    );
                let mut state = ListState::default().with_selected(if labels.is_empty() {
                    None
                } else {
                    Some((app.scroll as usize).min(labels.len() - 1))
                });
                frame.render_stateful_widget(list, rows, &mut state);
            } else if !app.confirm
                && !choices.is_empty()
                && app.plan.lock().unwrap().is_none()
                && app.restore_id.is_none()
            {
                let list = List::new(choices.iter().map(|s| {
                    ListItem::new(if app.selected == Action::Apps {
                        std::path::Path::new(s)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    } else {
                        s.clone()
                    })
                }))
                .highlight_symbol("› ")
                .highlight_style(cyan)
                .block(Block::default().title("↑/↓ select · Enter review"));
                let mut state = ListState::default()
                    .with_selected(Some((app.scroll as usize).min(choices.len() - 1)));
                frame.render_stateful_widget(list, body, &mut state);
            } else {
                let max_scroll = text
                    .lines()
                    .count()
                    .saturating_sub(1)
                    .min(u16::MAX as usize) as u16;
                let lines: Vec<Line<'_>> = text
                    .lines()
                    .map(|line| {
                        let style = if line.starts_with("  Unavailable:")
                            || line.contains("partial results")
                        {
                            Style::default().fg(Color::Yellow)
                        } else if !line.starts_with(' ') && !line.is_empty() {
                            cyan.add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        Line::from(Span::styled(line, style))
                    })
                    .collect();
                frame.render_widget(
                    Paragraph::new(Text::from(lines))
                        .scroll((app.scroll.min(max_scroll), 0))
                        .wrap(Wrap { trim: false })
                        .block(Block::default().title(action_name(app.selected))),
                    body,
                );
            }
        }
    }
    let status_refreshing = app.screen == Screen::Result
        && app.home_action == HomeAction::Status
        && app.job.as_ref().is_some_and(|j| !j.is_finished());
    let help = if status_refreshing {
        "Refreshing… · Esc back · c cancel · q quit"
    } else if app.confirm {
        "Confirm reviewed operation? y yes · n cancel · ↑/↓ review"
    } else if matches!(app.screen, Screen::Menu | Screen::Home) {
        "↑/↓ move · Enter select · s scope · Esc back · q quit"
    } else if app.screen == Screen::Result
        && matches!(app.selected, Action::Clean | Action::Apps)
        && app.job.as_ref().is_none_or(|j| j.is_finished())
        && app.plan.lock().unwrap().is_none()
    {
        "Esc back · v recovery · ↑/↓ scroll · q quit"
    } else if app.screen == Screen::Result && app.home_action == HomeAction::Status {
        "Esc back · ↑/↓ scroll · PgUp/PgDn · r refresh · q quit"
    } else {
        "Esc back · ↑/↓ move · PgUp/PgDn · Enter open · Space select · r review"
    };
    frame.render_widget(Paragraph::new(help).style(cyan), footer);
}
