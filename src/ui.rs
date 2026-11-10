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
    "Run cleanup",
    "Large Files",
    "Duplicates",
    "Project Artifacts",
    "Updates",
    "Login Items",
    "Threat Scan",
    "Hidden Space",
    "Leftover data review",
    "Doctor",
    "Uninstall apps",
    "Memory diagnostics",
    "System Status",
    "Restore recovered items",
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
    plan: Arc<Mutex<Option<optimac::recovery::Plan>>>,
    choices: Arc<Mutex<Vec<String>>>,
    confirm: bool,
    selected_files: std::collections::BTreeSet<PathBuf>,
    file_group: Option<String>,
    reviewed: Option<optimac::recovery::Plan>,
    restore_id: Option<String>,
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
            plan: Arc::new(Mutex::new(None)),
            choices: Arc::new(Mutex::new(Vec::new())),
            confirm: false,
            selected_files: std::collections::BTreeSet::new(),
            file_group: None,
            reviewed: None,
            restore_id: None,
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
        if self.screen == Screen::Result && self.job.as_ref().is_none_or(|j| j.is_finished()) {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            if !self.confirm && self.selected == 2 && self.plan.lock().unwrap().is_some() {
                let plan = self.plan.lock().unwrap();
                let plan = plan.as_ref().unwrap();
                let groups = optimac::recovery::categories(plan);
                match key {
                    KeyCode::Esc | KeyCode::Backspace => {
                        if self.file_group.take().is_none() {
                            self.screen = Screen::Menu;
                        }
                        self.scroll = 0;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.scroll = self.scroll.saturating_add(1)
                    }
                    KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
                    KeyCode::Enter if self.file_group.is_none() => {
                        if let Some(g) =
                            groups.get((self.scroll as usize).min(groups.len().saturating_sub(1)))
                        {
                            self.file_group = Some(g.name.into());
                            self.scroll = 0;
                        }
                    }
                    KeyCode::Char(' ') => {
                        if let Some(group) = &self.file_group {
                            let files: Vec<_> = plan
                                .items
                                .iter()
                                .filter(|c| optimac::recovery::category(c) == group)
                                .collect();
                            if let Some(c) =
                                files.get((self.scroll as usize).min(files.len().saturating_sub(1)))
                                && !self.selected_files.remove(&c.path)
                            {
                                self.selected_files.insert(c.path.clone());
                            }
                        }
                    }
                    KeyCode::Char('r') => {
                        let reviewed = optimac::recovery::select_files(plan, &self.selected_files);
                        if !reviewed.items.is_empty() {
                            *self.output.lock().unwrap() = optimac::recovery::describe(&reviewed);
                            self.reviewed = Some(reviewed);
                            self.confirm = true;
                            self.scroll = 0;
                        }
                    }
                    _ => {}
                }
                return false;
            }
            if self.confirm {
                if key == KeyCode::Char('y') {
                    self.confirm = false;
                    self.choices.lock().unwrap().clear();
                    self.scroll = 0;
                    let plan = self
                        .reviewed
                        .take()
                        .or_else(|| self.plan.lock().unwrap().take());
                    self.plan.lock().unwrap().take();
                    let restore = self.restore_id.take();
                    let output = self.output.clone();
                    self.cancel = Arc::new(AtomicBool::new(false));
                    let cancel = self.cancel.clone();
                    *output.lock().unwrap() =
                        "Moving selected items… c cancels between moves".into();
                    self.job = Some(std::thread::spawn(move || {
                        let result = if let Some(id) = restore {
                            optimac::recovery::restore(&home, &id)
                        } else if let Some(plan) = plan {
                            optimac::recovery::execute(&home, &plan, &cancel)
                        } else {
                            Err(io::Error::other("No reviewed plan"))
                        };
                        *output.lock().unwrap() = result.unwrap_or_else(|e| {
                            format!(
                                "Operation stopped: {e}. Check recovery records before retrying."
                            )
                        });
                    }));
                    return false;
                }
                if matches!(key, KeyCode::Char('n') | KeyCode::Esc) {
                    self.confirm = false;
                    self.reviewed = None;
                    self.scroll = 0;
                    self.restore_id = None;
                    if self.selected == 12 {
                        self.plan.lock().unwrap().take();
                    }
                    return false;
                }
            } else if key == KeyCode::Enter {
                if matches!(self.selected, 12 | 15) {
                    self.scroll = self.scroll.min(
                        self.choices
                            .lock()
                            .unwrap()
                            .len()
                            .saturating_sub(1)
                            .min(u16::MAX as usize) as u16,
                    );
                }
                if self.selected == 12 {
                    let choice = self
                        .choices
                        .lock()
                        .unwrap()
                        .get(self.scroll as usize)
                        .cloned();
                    if let Some(path) = choice {
                        match optimac::recovery::app_plan(&home, std::path::Path::new(&path)) {
                            Ok(plan) => {
                                *self.output.lock().unwrap() = optimac::recovery::describe(&plan);
                                *self.plan.lock().unwrap() = Some(plan);
                                self.confirm = true;
                            }
                            Err(e) => {
                                *self.output.lock().unwrap() = format!("Cannot uninstall: {e}")
                            }
                        }
                    }
                } else if self.selected == 15 {
                    let choice = self
                        .choices
                        .lock()
                        .unwrap()
                        .get(self.scroll as usize)
                        .cloned();
                    if let Some(id) = choice {
                        *self.output.lock().unwrap() = format!(
                            "Restore recovery record {id}? Existing destinations will never be overwritten."
                        );
                        self.restore_id = Some(id);
                        self.confirm = true;
                    }
                }
                return false;
            }
        }
        if self.screen != Screen::Menu {
            match key {
                KeyCode::Esc | KeyCode::Backspace => {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.screen = Screen::Menu;
                    self.confirm = false;
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
                self.plan = Arc::new(Mutex::new(None));
                self.choices = Arc::new(Mutex::new(Vec::new()));
                self.confirm = false;
                self.selected_files.clear();
                self.file_group = None;
                self.reviewed = None;
                let plan_state = self.plan.clone();
                let choices = self.choices.clone();
                self.job = Some(std::thread::spawn(move || {
                    let home = std::env::var_os("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_default();
                    let result = if selected == 2 {
                        optimac::recovery::review_files(&home, &root, &cancel).map(|plan| {
                            let text = optimac::recovery::describe(&plan);
                            *plan_state.lock().unwrap() = Some(plan);
                            text + "\nSelect individual files, then press r to review. Close apps using selected files first."
                        })
                    } else if selected == 12 || selected == 15 {
                        let items = if selected == 12 {
                            optimac::recovery::app_paths(&home)
                                .iter()
                                .filter_map(|p| p.to_str().map(String::from))
                                .collect()
                        } else {
                            optimac::recovery::recovery_ids(&home)
                        };
                        *choices.lock().unwrap() = items;
                        Ok("Use arrows to select, Enter to review. Only eligible owned apps are listed; personal data stays in place.".into())
                    } else {
                        optimac::actions::run(selected, &root, &cancel)
                    };
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
                "{}\n\nFiles: {}\nLogical size: {}\nSkipped: {}   Errors: {}\n\nRead-only. Logical size is not reclaimable space.",
                state.1,
                r.files,
                optimac::format_size(r.logical_bytes),
                r.skipped,
                r.errors
            );
            frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), body);
        }
        Screen::Result => {
            let text = app.output.lock().unwrap();
            let choices = app.choices.lock().unwrap();
            if app.selected == 2 && !app.confirm && app.plan.lock().unwrap().is_some() {
                let plan = app.plan.lock().unwrap();
                let plan = plan.as_ref().unwrap();
                let groups = optimac::recovery::categories(plan);
                let selected = optimac::recovery::select_files(plan, &app.selected_files);
                let [summary, rows] = Layout::vertical([
                    Constraint::Length(if body.height >= 8 { 4 } else { 1 }),
                    Constraint::Min(0),
                ])
                .areas(body);
                frame.render_widget(Paragraph::new(format!("{} files selected · {}\nEnter: open group · Space: select file · r: review\nPersonal files are not junk. Nothing is preselected.\nScope: {}",selected.items.len(),optimac::format_size(selected.items.iter().fold(0u64,|n,c|n.saturating_add(c.logical_size()))),app.root.display())).wrap(Wrap{trim:false}),summary);
                let labels: Vec<String> = if let Some(group) = &app.file_group {
                    plan.items
                        .iter()
                        .filter(|c| optimac::recovery::category(c) == group)
                        .map(|c| {
                            format!(
                                "[{}] {}  {:?}",
                                if app.selected_files.contains(&c.path) {
                                    "x"
                                } else {
                                    " "
                                },
                                optimac::format_size(c.logical_size()),
                                c.path
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
                    ListItem::new(if app.selected == 12 {
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
                frame.render_widget(
                    Paragraph::new(text.as_str())
                        .scroll((app.scroll.min(max_scroll), 0))
                        .wrap(Wrap { trim: false })
                        .block(Block::default().title(ITEMS[app.selected])),
                    body,
                );
            }
        }
    }
    let help = if app.confirm {
        "Confirm reviewed operation? y yes · n cancel · ↑/↓ review"
    } else if app.screen == Screen::Menu {
        "↑/↓ j/k move · Enter select · q quit"
    } else {
        "Esc back · ↑/↓ move · Enter group · Space select · r review · q quit"
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
    fn selected_files_are_frozen_for_confirmation() {
        let plan:optimac::recovery::Plan=serde_json::from_str(r#"{"items":[{"path":"/Users/test/Documents/a.pdf","dev":1,"ino":1,"size":1000,"modified":0,"personal":true},{"path":"/Users/test/Documents/main.rs","dev":1,"ino":2,"size":2000,"modified":0,"personal":true}],"notes":[]}"#).unwrap();
        let mut app = App::new(".".into());
        app.screen = Screen::Result;
        app.selected = 2;
        *app.plan.lock().unwrap() = Some(plan);
        app.key(KeyCode::Char('r'));
        assert!(!app.confirm);
        app.key(KeyCode::Enter);
        app.key(KeyCode::Char(' '));
        app.key(KeyCode::Char('r'));
        assert!(app.confirm);
        assert_eq!(app.reviewed.as_ref().unwrap().items.len(), 1);
        assert!(
            app.reviewed.as_ref().unwrap().items[0]
                .path
                .ends_with("a.pdf")
        );
        app.key(KeyCode::Char('n'));
        assert!(!app.confirm);
        app.key(KeyCode::Char(' '));
        app.key(KeyCode::Char('r'));
        assert!(!app.confirm);
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(20, 5)).unwrap();
        terminal.draw(|f| render(f, &app)).unwrap();
    }
    #[test]
    fn cleanup_requires_explicit_confirmation() {
        let mut app = App::new(".".into());
        app.screen = Screen::Result;
        app.selected = 2;
        app.key(KeyCode::Char('y'));
        assert!(app.job.is_none());
        app.confirm = true;
        app.key(KeyCode::Char('n'));
        assert!(!app.confirm);
        assert!(app.job.is_none());
    }
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
