use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use optimac::{Report, scan_progress};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const ITEMS: &[&str] = &[
    "Inspect directory",
    "Deep Clean Preview",
    "Clean (preview only)",
    "Large Files",
    "Duplicates",
    "Project Artifacts",
    "Updates",
    "Login Items",
    "Threat Scan",
    "Hidden Space",
    "Leftover data review",
    "Doctor",
    "Apps (inventory only)",
    "Memory diagnostics",
    "System Status",
    "Quit",
];
#[derive(PartialEq, Debug)]
enum Screen {
    Menu,
    Scan,
    Result,
}
struct App {
    selected: usize,
    screen: Screen,
    root: PathBuf,
    output: Arc<Mutex<String>>,
    scroll: u16,
    job: Option<std::thread::JoinHandle<()>>,
    cancel: Arc<AtomicBool>,
    shared: Arc<Mutex<(Report, String)>>,
}
impl App {
    fn new(root: PathBuf) -> Self {
        Self {
            selected: 0,
            screen: Screen::Menu,
            root,
            output: Arc::new(Mutex::new(String::new())),
            scroll: 0,
            job: None,
            cancel: Arc::new(AtomicBool::new(false)),
            shared: Arc::new(Mutex::new((Report::default(), "Ready".into()))),
        }
    }
    fn key(&mut self, key: KeyCode) -> bool {
        if key == KeyCode::Char('q') {
            return true;
        }
        if self.screen != Screen::Menu {
            match key {
                KeyCode::Esc | KeyCode::Backspace => {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.screen = Screen::Menu;
                }
                KeyCode::Char('c') => self.cancel.store(true, Ordering::Relaxed),
                KeyCode::Down | KeyCode::Char('j') => self.scroll = self.scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
                _ => {}
            }
            return false;
        }
        match key {
            KeyCode::Down | KeyCode::Char('j') => self.selected = (self.selected + 1) % ITEMS.len(),
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = (self.selected + ITEMS.len() - 1) % ITEMS.len()
            }
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = ITEMS.len() - 1,
            KeyCode::Enter if self.selected == ITEMS.len() - 1 => return true,
            KeyCode::Enter if self.selected == 0 => {
                if self.job.as_ref().is_some_and(|job| !job.is_finished()) {
                    return false;
                }
                self.screen = Screen::Scan;
                self.cancel = Arc::new(AtomicBool::new(false));
                self.shared = Arc::new(Mutex::new((Report::default(), "Scanning".into())));
                let shared = self.shared.clone();
                let cancel = self.cancel.clone();
                let root = self.root.clone();
                self.job = Some(std::thread::spawn(move || {
                    let result = scan_progress(&root, &cancel, 100_000, |r| {
                        shared.lock().unwrap().0 = r.clone()
                    });
                    let mut state = shared.lock().unwrap();
                    match result {
                        Ok(report) => {
                            state.1 = completion(&report).into();
                            state.0 = report;
                        }
                        Err(error) => state.1 = format!("Scan failed: {error}"),
                    }
                }));
            }
            KeyCode::Enter => {
                if self.job.as_ref().is_some_and(|j| !j.is_finished()) {
                    return false;
                }
                self.screen = Screen::Result;
                self.scroll = 0;
                self.cancel = Arc::new(AtomicBool::new(false));
                self.output = Arc::new(Mutex::new("Working… c cancels".into()));
                let output = self.output.clone();
                let cancel = self.cancel.clone();
                let root = self.root.clone();
                let selected = self.selected;
                self.job = Some(std::thread::spawn(move || {
                    let result = optimac::actions::run(selected, &root, &cancel);
                    *output.lock().unwrap() =
                        result.unwrap_or_else(|e| format!("Action failed: {e}"));
                }));
            }
            _ => {}
        }
        false
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
fn completion(report: &Report) -> &'static str {
    if report.cancelled {
        "Cancelled — partial results"
    } else if report.limited {
        "Stopped at scan limit — partial results"
    } else if report.errors > 0 {
        "Completed with errors — partial results"
    } else if report.skipped > 0 {
        "Completed — some entries skipped"
    } else {
        "Completed"
    }
}
fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let cyan = Style::default().fg(Color::Rgb(103, 232, 249));
    if area.height < 4 || area.width < 18 {
        frame.render_widget(
            Paragraph::new(format!("OptiMac\n{}\nq quit", ITEMS[app.selected])).style(cyan),
            area,
        );
        return;
    }
    let header_height = if area.height >= 16 { 5 } else { 1 };
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
        Screen::Menu => {
            let mut list_area = body;
            if area.width >= 95 {
                let [left, right] =
                    Layout::horizontal([Constraint::Percentage(52), Constraint::Percentage(48)])
                        .areas(body);
                list_area = left;
                let description = if app.selected == 0 {
                    "Inspect file sizes in the selected directory. Read-only, with scan limits and cancellation.\n\nEnter to start."
                } else if app.selected == ITEMS.len() - 1 {
                    "Close OptiMac."
                } else {
                    optimac::actions::description(app.selected)
                };
                frame.render_widget(
                    Paragraph::new(description)
                        .wrap(Wrap { trim: true })
                        .block(Block::bordered().title(ITEMS[app.selected])),
                    right,
                );
            }
            let items: Vec<_> = ITEMS
                .iter()
                .enumerate()
                .map(|(i, title)| ListItem::new(format!("{}  {}", i + 1, title)))
                .collect();
            let list = List::new(items)
                .block(Block::default().title("ACTIONS"))
                .highlight_symbol("› ")
                .highlight_style(cyan.bg(Color::Rgb(23, 32, 54)).add_modifier(Modifier::BOLD));
            let mut state = ListState::default().with_selected(Some(app.selected));
            frame.render_stateful_widget(list, list_area, &mut state);
        }
        Screen::Scan => {
            let state = app.shared.lock().unwrap();
            let r = &state.0;
            let text = format!(
                "{}\n\nFiles: {}\nLogical size: {:.2} GiB\nSkipped: {}   Errors: {}\n\nRead-only. Logical size is not reclaimable space.",
                state.1,
                r.files,
                r.logical_bytes as f64 / 1_073_741_824.0,
                r.skipped,
                r.errors
            );
            frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), body);
        }
        Screen::Result => {
            let text = app.output.lock().unwrap();
            let max_scroll = text
                .lines()
                .count()
                .saturating_sub(1)
                .min(u16::MAX as usize) as u16;
            frame.render_widget(
                Paragraph::new(text.as_str())
                    .scroll((app.scroll.min(max_scroll), 0))
                    .wrap(Wrap { trim: false })
                    .block(Block::default().title(ITEMS[app.selected])),
                body,
            );
        }
    }
    let help = if app.screen == Screen::Menu {
        "↑/↓ j/k move · Enter select · q quit"
    } else {
        "Esc menu · ↑/↓ scroll · c cancel · q quit"
    };
    frame.render_widget(Paragraph::new(help).style(cyan), footer);
}

pub fn run(root: PathBuf, cancel: Arc<AtomicBool>) -> io::Result<()> {
    let mut app = App::new(root);
    ratatui::run(|terminal| -> io::Result<()> {
        loop {
            terminal.draw(|frame| render(frame, &app))?;
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if (key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL))
                            || app.key(key.code)
                        {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_navigation_and_background_actions() {
        let root = std::env::temp_dir().join(format!("optimac-ui-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut app = App::new(root.clone());
        app.selected = 2;
        assert!(!app.key(KeyCode::Down));
        assert_eq!(app.selected, 3);
        app.key(KeyCode::Enter);
        assert_eq!(app.screen, Screen::Result);
        app.job.take().unwrap().join().unwrap();
        std::fs::remove_dir(root).unwrap();
        app.key(KeyCode::Esc);
        assert_eq!(app.screen, Screen::Menu);
        app.key(KeyCode::End);
        assert!(app.key(KeyCode::Enter));
    }
    #[test]
    fn all_screens_resize_and_selected_item_stays_visible() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        let mut app = App::new(".".into());
        for screen in [Screen::Menu, Screen::Scan, Screen::Result] {
            app.screen = screen;
            for (w, h) in [
                (1, 1),
                (20, 5),
                (40, 10),
                (80, 24),
                (120, 40),
                (0, 0),
                (80, 24),
            ] {
                terminal.backend_mut().resize(w, h);
                for selected in 0..ITEMS.len() {
                    app.selected = selected;
                    terminal.draw(|f| render(f, &app)).unwrap();
                }
            }
        }
        app.screen = Screen::Menu;
        app.selected = ITEMS.len() - 1;
        terminal.backend_mut().resize(40, 10);
        terminal.draw(|f| render(f, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        assert!(
            buffer
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
                .contains("Quit")
        );
    }
    #[test]
    fn limited_scan_is_not_called_complete() {
        let report = Report {
            limited: true,
            ..Report::default()
        };
        assert!(completion(&report).starts_with("Stopped at scan limit"));
    }
}
