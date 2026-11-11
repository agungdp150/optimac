use super::*;
use optimac::actions::status::Snapshot;
use ratatui::layout::Rect;

const ACCENT: Color = Color::Rgb(200, 160, 220);
const MUTED: Color = Color::Rgb(150, 163, 180);

fn title(frame: &mut Frame, area: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(format!("● {text}"))
            .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        area,
    );
}
fn section(frame: &mut Frame, area: Rect, name: &str, lines: impl Into<Text<'static>>) {
    let [head, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    title(frame, head, name);
    frame.render_widget(Paragraph::new(lines), body);
}
fn metric(frame: &mut Frame, area: Rect, name: &str, total: Option<u64>, available: Option<u64>) {
    let [head, bar, details] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    title(frame, head, name);
    if let Some((total, available)) = total.zip(available).filter(|(t, a)| *t > 0 && a <= t) {
        let other = total - available;
        let ratio = other as f64 / total as f64;
        // These are capacity estimates, not health or memory-pressure warnings.
        let bar_width = bar.width.min(32);
        let bar_area = Rect {
            width: bar_width,
            ..bar
        };
        frame.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(Color::Rgb(125, 200, 185)))
                .ratio(ratio)
                .label(format!(
                    "{:.0}% {}",
                    ratio * 100.0,
                    if name == "Memory" {
                        "other"
                    } else {
                        "unavailable"
                    }
                )),
            bar_area,
        );
        let kind = if name == "Memory" {
            "estimated available"
        } else {
            "available"
        };
        frame.render_widget(
            Paragraph::new(format!(
                "{} {}  /  {} total\n{}",
                optimac::format_size(available),
                kind,
                optimac::format_size(total),
                if name == "Memory" {
                    "Not memory pressure"
                } else {
                    "APFS available to this user"
                }
            ))
            .style(Style::default().fg(MUTED)),
            details,
        );
    } else {
        frame.render_widget(
            Paragraph::new("Unavailable").style(Style::default().fg(Color::Yellow)),
            bar,
        );
    }
}
fn battery_summary(raw: Option<&str>) -> String {
    let Some(raw) = raw else {
        return "Unavailable / no battery".into();
    };
    let parts: Vec<_> = raw.split(';').map(str::trim).collect();
    let percent = parts
        .first()
        .and_then(|p| p.split_whitespace().last())
        .filter(|p| p.ends_with('%'))
        .unwrap_or("?");
    let state = parts
        .get(1)
        .copied()
        .filter(|s| !s.is_empty())
        .unwrap_or("state unavailable");
    let charging = parts
        .get(2)
        .and_then(|p| p.split(" present:").next())
        .map(str::trim)
        .filter(|p| !p.is_empty());
    match charging {
        Some(c) => format!("{percent} · {state} · {c}"),
        None => format!("{percent} · {state}"),
    }
}
fn process_row(row: &str) -> String {
    let mut parts = row.split_whitespace();
    let Some(cpu) = parts.next() else {
        return "Unavailable".into();
    };
    let Some(rss) = parts.next() else {
        return "Unavailable".into();
    };
    let path = parts.collect::<Vec<_>>().join(" ");
    let name = path
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown");
    let mem = rss
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(1024))
        .map(optimac::format_size)
        .unwrap_or_else(|| "?".into());
    format!(
        "{:>5}%  {:>9}  {}",
        cpu,
        mem,
        name.chars()
            .map(|c| if c.is_control() { '�' } else { c })
            .collect::<String>()
    )
}
pub(super) fn render(frame: &mut Frame, area: Rect, s: &Snapshot) {
    if area.width < 30 || area.height < 7 {
        frame.render_widget(
            Paragraph::new("Status\nResize to see details\nr refresh")
                .style(Style::default().fg(Color::Cyan)),
            area,
        );
        return;
    }
    let collected = s
        .collected
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| super::format_timestamp(d.as_secs()))
        .unwrap_or_else(|| "unknown".into());
    let [header, content] = Layout::vertical([
        Constraint::Length(if area.height >= 14 { 3 } else { 1 }),
        Constraint::Min(0),
    ])
    .areas(area);
    let header_text = if header.height >= 3 {
        format!(
            " /\\_/\\   OptiMac  /  Status\n( o.o )   {} · {}\n > ^ <    Updated {} · auto refresh 3s",
            s.os.as_deref().unwrap_or("macOS unavailable"),
            s.cpu.as_deref().unwrap_or("CPU unavailable"),
            collected
        )
    } else {
        format!("Status · {}", s.os.as_deref().unwrap_or("Unavailable"))
    };
    frame.render_widget(
        Paragraph::new(header_text).style(Style::default().fg(Color::Cyan)),
        header,
    );
    if content.width < 76 || content.height < 12 {
        let summary = format!(
            "CPU     {}\nLoad    {}\nMemory  {} available / {}\nDisk    {} available / {}\nPower   {}\n\nTop processes (CPU · memory · name)\n{}",
            s.cpu.as_deref().unwrap_or("Unavailable"),
            s.load.as_deref().unwrap_or("Unavailable"),
            s.memory_available
                .map(optimac::format_size)
                .unwrap_or_else(|| "Unavailable".into()),
            s.memory_total
                .map(optimac::format_size)
                .unwrap_or_else(|| "?".into()),
            s.disk_available
                .map(optimac::format_size)
                .unwrap_or_else(|| "Unavailable".into()),
            s.disk_total
                .map(optimac::format_size)
                .unwrap_or_else(|| "?".into()),
            battery_summary(s.battery.as_deref()),
            s.processes
                .iter()
                .map(|p| process_row(p))
                .collect::<Vec<_>>()
                .join("\n")
        );
        frame.render_widget(Paragraph::new(summary), content);
        return;
    }
    // Fixed-height rows keep related metrics close together on tall terminals.
    let [top, mid, bottom, _] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Min(3),
        Constraint::Min(0),
    ])
    .areas(content);
    let [cpu, memory] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(top);
    let [disk, power] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(mid);
    section(
        frame,
        cpu,
        "CPU",
        format!(
            "{}\nLoad (1 / 5 / 15 min)  {}\nUtilization unavailable",
            s.cpu.as_deref().unwrap_or("Unavailable"),
            s.load.as_deref().unwrap_or("Unavailable")
        ),
    );
    metric(frame, memory, "Memory", s.memory_total, s.memory_available);
    metric(frame, disk, "Disk", s.disk_total, s.disk_available);
    section(frame, power, "Power", battery_summary(s.battery.as_deref()));
    let processes = if s.processes.is_empty() {
        "Unavailable".into()
    } else {
        s.processes
            .iter()
            .map(|p| process_row(p))
            .collect::<Vec<_>>()
            .join("\n")
    };
    section(
        frame,
        bottom,
        "Top processes",
        format!(
            " CPU     Memory  Process\n{}{}",
            processes,
            if s.warnings.is_empty() {
                ""
            } else {
                "\nSome checks unavailable."
            }
        ),
    );
}
pub(super) fn render_detail(
    frame: &mut Frame,
    area: Rect,
    name: &str,
    detail: &optimac::actions::status::Detail,
    scroll: u16,
) {
    let [header, body] = Layout::vertical([
        Constraint::Length(if area.height >= 10 { 3 } else { 1 }),
        Constraint::Min(0),
    ])
    .areas(area);
    let collected = detail
        .collected
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| super::format_timestamp(d.as_secs()))
        .unwrap_or_else(|| "unknown".into());
    let text = if header.height >= 3 {
        format!(
            " /\\_/\\   OptiMac  /  {name}\n( o.o )   Updated {collected}\n > ^ <    r refresh · ↑/↓ scroll"
        )
    } else {
        format!("Status / {name}")
    };
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(Color::Cyan)),
        header,
    );
    let mut lines = Vec::new();
    for (heading, rows) in &detail.sections {
        lines.push(Line::styled(
            format!("● {heading}"),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
        for (label, value) in rows {
            let style = if value == "Unavailable" {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {label:<15}"), Style::default().fg(MUTED)),
                Span::styled(value.as_str(), style),
            ]));
        }
        lines.push(Line::from(""));
    }
    let max = lines
        .len()
        .saturating_sub(body.height as usize)
        .min(u16::MAX as usize) as u16;
    frame.render_widget(
        Paragraph::new(Text::from(lines)).scroll((scroll.min(max), 0)),
        body,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detail_pages_are_readable_and_resize() {
        let detail = optimac::actions::status::Detail {
            collected: None,
            sections: vec![(
                "Security checks".into(),
                vec![
                    ("FileVault".into(), "Unavailable".into()),
                    ("Firewall".into(), "Enabled".into()),
                ],
            )],
        };
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|f| render_detail(f, f.area(), "Security checks", &detail, 0))
            .unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(screen.contains("FileVault"));
        assert!(screen.contains("Unavailable"));
        for (w, h) in [(120, 40), (80, 24), (40, 10), (20, 5), (1, 1), (0, 0)] {
            terminal.backend_mut().resize(w, h);
            terminal
                .draw(|f| render_detail(f, f.area(), "Security checks", &detail, 0))
                .unwrap();
        }
    }
    #[test]
    fn rows_are_compact_and_do_not_expose_full_paths() {
        assert_eq!(
            process_row("25.7 607616 /Applications/OrbStack.app/Contents/MacOS/OrbStack Helper"),
            " 25.7%  622.20 MB  OrbStack Helper"
        );
        assert_eq!(
            battery_summary(Some(
                "-InternalBattery-0 (id=6946915) 90%; AC attached; not charging present: true"
            )),
            "90% · AC attached · not charging"
        );
    }
    #[test]
    fn renders_at_all_sizes() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
        let s = Snapshot {
            memory_total: Some(100),
            memory_available: Some(25),
            disk_total: Some(1000),
            disk_available: Some(200),
            processes: vec!["5.0 1024 /Applications/Example.app/Contents/MacOS/Example".into()],
            ..Default::default()
        };
        terminal.draw(|f| render(f, f.area(), &s)).unwrap();
        let buffer = terminal.backend().buffer();
        let lines: Vec<String> = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect()
            })
            .collect();
        let cpu = lines
            .iter()
            .position(|line| line.contains("● CPU"))
            .unwrap();
        let disk = lines
            .iter()
            .position(|line| line.contains("● Disk"))
            .unwrap();
        assert!(disk - cpu <= 6, "panels should stay close");
        assert!(lines.iter().any(|line| line.contains("Example")));
        assert!(!lines.iter().any(|line| line.contains("/Applications/")));
        for (w, h) in [(120, 40), (80, 24), (40, 10), (20, 5), (1, 1), (0, 0)] {
            terminal.backend_mut().resize(w, h);
            terminal.draw(|f| render(f, f.area(), &s)).unwrap();
        }
    }
}
