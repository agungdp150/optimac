use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use optimac::{Report, actions::Action, scan_progress};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
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

fn action_name(action: Action) -> &'static str {
    match action {
        Action::Inspect => "Inspect directory",
        Action::CleanPreview => "Deep Clean Preview",
        Action::Clean => "Run cleanup",
        Action::LargeFiles => "Large Files",
        Action::Duplicates => "Duplicates",
        Action::Artifacts => "Project Artifacts",
        Action::Updates => "Updates",
        Action::Login => "Login Items",
        Action::Threats => "Threat Scan",
        Action::Storage => "Storage",
        Action::Orphans => "Leftover data review",
        Action::Doctor => "Security checks",
        Action::Apps => "Uninstall apps",
        Action::Memory => "Memory and swap",
        Action::Status => "System overview",
        Action::Restore => "Restore recovered items",
        Action::AiTools => "AI tools status",
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HomeAction {
    Clean,
    Uninstall,
    Analyze,
    Duplicates,
    Status,
    More,
}
const HOME: &[(HomeAction, &str, &str)] = &[
    (
        HomeAction::Clean,
        "Clean",
        "Review files and move to recovery",
    ),
    (
        HomeAction::Uninstall,
        "Uninstall",
        "Review and remove an app bundle",
    ),
    (
        HomeAction::Analyze,
        "Analyze",
        "Explore files and disk usage",
    ),
    (
        HomeAction::Duplicates,
        "Duplicates",
        "Find matching file contents",
    ),
    (HomeAction::Status, "Status", "Check system health"),
    (HomeAction::More, "More", "Recovery and maintenance tools"),
];
fn submenu(action: HomeAction) -> &'static [Action] {
    match action {
        HomeAction::Clean => &[Action::Clean, Action::Restore],
        HomeAction::Uninstall => &[Action::Apps, Action::Restore],
        HomeAction::Analyze => &[Action::Inspect, Action::LargeFiles, Action::Artifacts],
        HomeAction::Duplicates => &[Action::Duplicates],
        HomeAction::Status => &[
            Action::Status,
            Action::Memory,
            Action::Storage,
            Action::Doctor,
            Action::AiTools,
        ],
        HomeAction::More => &[Action::Restore, Action::Updates, Action::Login],
    }
}
#[derive(PartialEq, Debug)]
enum Screen {
    Home,
    Menu,
    Scan,
    Result,
    Scope,
}
struct App {
    selected: Action,
    home_cursor: usize,
    submenu_cursor: usize,
    home_action: HomeAction,
    screen: Screen,
    root: PathBuf,
    scope_input: String,
    scope_error: String,
    scope_from_home: bool,
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
            selected: Action::Inspect,
            home_cursor: 0,
            submenu_cursor: 0,
            home_action: HomeAction::Clean,
            screen: Screen::Home,
            scope_input: root.to_string_lossy().into_owned(),
            scope_error: String::new(),
            scope_from_home: false,
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
        if key == KeyCode::Char('q') && self.screen != Screen::Scope {
            return true;
        }
        if self.screen == Screen::Scope {
            match key {
                KeyCode::Esc => {
                    self.screen = if self.scope_from_home {
                        Screen::Home
                    } else {
                        Screen::Menu
                    };
                    self.scope_error.clear();
                }
                KeyCode::Enter => {
                    let path = PathBuf::from(&self.scope_input);
                    if path.is_dir() {
                        self.root = path;
                        self.scope_error.clear();
                        self.screen = if self.scope_from_home {
                            Screen::Home
                        } else {
                            Screen::Menu
                        };
                    } else {
                        self.scope_error = "Enter an existing directory".into();
                    }
                }
                KeyCode::Backspace => {
                    self.scope_input.pop();
                }
                KeyCode::Char(c) => self.scope_input.push(c),
                _ => {}
            }
            return false;
        }
        if self.screen == Screen::Home {
            match key {
                KeyCode::Char('s') => {
                    self.scope_from_home = true;
                    self.scope_input = self.root.to_string_lossy().into_owned();
                    self.screen = Screen::Scope;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.home_cursor = (self.home_cursor + 1) % HOME.len()
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.home_cursor = (self.home_cursor + HOME.len() - 1) % HOME.len()
                }
                KeyCode::Home => self.home_cursor = 0,
                KeyCode::End => self.home_cursor = HOME.len() - 1,
                KeyCode::Enter => {
                    self.home_action = HOME[self.home_cursor].0;
                    self.submenu_cursor = 0;
                    self.screen = Screen::Menu;
                    if self.home_action == HomeAction::Duplicates {
                        return self.key(KeyCode::Enter);
                    }
                }
                _ => {}
            }
            return false;
        }
        if self.screen == Screen::Result
            && !self.confirm
            && matches!(self.selected, Action::Clean | Action::Apps)
            && key == KeyCode::Char('v')
            && self.job.as_ref().is_none_or(|j| j.is_finished())
            && let Some(position) = submenu(self.home_action)
                .iter()
                .position(|a| *a == Action::Restore)
        {
            self.submenu_cursor = position;
            self.screen = Screen::Menu;
            return self.key(KeyCode::Enter);
        }
        if self.screen == Screen::Result && self.job.as_ref().is_none_or(|j| j.is_finished()) {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            if !self.confirm
                && self.selected == Action::Clean
                && self.plan.lock().unwrap().is_some()
            {
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
                    if self.selected == Action::Apps {
                        self.plan.lock().unwrap().take();
                    }
                    return false;
                }
            } else if key == KeyCode::Enter {
                if matches!(self.selected, Action::Apps | Action::Restore) {
                    self.scroll = self.scroll.min(
                        self.choices
                            .lock()
                            .unwrap()
                            .len()
                            .saturating_sub(1)
                            .min(u16::MAX as usize) as u16,
                    );
                }
                if self.selected == Action::Apps {
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
                } else if self.selected == Action::Restore {
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
        if self.screen == Screen::Result
            && self.home_action == HomeAction::Status
            && key == KeyCode::Char('r')
        {
            if self.job.as_ref().is_none_or(|j| j.is_finished()) {
                self.screen = Screen::Menu;
                return self.key(KeyCode::Enter);
            }
            return false;
        }
        if self.screen != Screen::Menu {
            match key {
                KeyCode::Esc | KeyCode::Backspace => {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.screen = if self.home_action == HomeAction::Duplicates {
                        Screen::Home
                    } else {
                        Screen::Menu
                    };
                    self.confirm = false;
                }
                KeyCode::Char('c') => self.cancel.store(true, Ordering::Relaxed),
                KeyCode::Down | KeyCode::Char('j') => self.scroll = self.scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
                KeyCode::PageDown => self.scroll = self.scroll.saturating_add(10),
                KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
                KeyCode::Home => self.scroll = 0,
                KeyCode::End => self.scroll = u16::MAX,
                _ => {}
            }
            return false;
        }
        match key {
            KeyCode::Char('s') => {
                self.scope_from_home = false;
                self.scope_input = self.root.to_string_lossy().into_owned();
                self.screen = Screen::Scope;
            }
            KeyCode::Esc | KeyCode::Backspace => {
                self.screen = Screen::Home;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.submenu_cursor = (self.submenu_cursor + 1) % submenu(self.home_action).len()
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.submenu_cursor = (self.submenu_cursor + submenu(self.home_action).len() - 1)
                    % submenu(self.home_action).len()
            }
            KeyCode::Home => self.submenu_cursor = 0,
            KeyCode::End => self.submenu_cursor = submenu(self.home_action).len() - 1,
            KeyCode::Enter
                if {
                    self.selected = submenu(self.home_action)[self.submenu_cursor];
                    self.selected == Action::Inspect
                } =>
            {
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
                    let result = if selected == Action::Clean {
                        optimac::recovery::review_files(&home, &root, &cancel).map(|plan| {
                            let text = optimac::recovery::describe(&plan);
                            *plan_state.lock().unwrap() = Some(plan);
                            text + "\nSelect individual files, then press r to review. Close apps using selected files first."
                        })
                    } else if selected == Action::Apps || selected == Action::Restore {
                        let items = if selected == Action::Apps {
                            optimac::recovery::app_paths(&home)
                                .iter()
                                .filter_map(|p| p.to_str().map(String::from))
                                .collect()
                        } else {
                            optimac::recovery::recovery_ids(&home)
                        };
                        *choices.lock().unwrap() = items;
                        Ok(if selected == Action::Apps {"Use arrows to select, Enter to review. Only eligible owned apps are listed; personal data stays in place."} else {"Select a recovery record to review."}.into())
                    } else if selected == Action::AiTools {
                        Ok(optimac::actions::ai_tools_status(&home))
                    } else {
                        optimac::actions::run(selected, &root, &cancel)
                    };
                    let text = result.unwrap_or_else(|e| format!("Action failed: {e}"));
                    let text = if matches!(
                        selected,
                        Action::Status
                            | Action::Memory
                            | Action::Storage
                            | Action::Doctor
                            | Action::AiTools
                    ) {
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        format!(
                            "Updated {} · r refresh\n\n{text}",
                            format_timestamp(timestamp)
                        )
                    } else {
                        text
                    };
                    *output.lock().unwrap() = text;
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
fn format_timestamp(timestamp: u64) -> String {
    let t = timestamp as libc::time_t;
    let mut value = std::mem::MaybeUninit::<libc::tm>::uninit();
    let mut buffer = [0u8; 64];
    // localtime_r fills the caller-owned tm; strftime writes at most buffer.len() bytes.
    unsafe {
        if libc::localtime_r(&t, value.as_mut_ptr()).is_null() {
            return format!("Unix {timestamp} UTC");
        }
        let size = libc::strftime(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            c"%Y-%m-%d %H:%M %Z".as_ptr(),
            value.as_ptr(),
        );
        if size == 0 {
            return format!("Unix {timestamp} UTC");
        }
        String::from_utf8_lossy(&buffer[..size]).into_owned()
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
                frame.render_widget(Paragraph::new(format!("{} files selected · {}\nEnter: open group · Space: select file · r: review\nPersonal files are not junk. Nothing is preselected.\nScope: {}",selected.items.len(),optimac::format_size(selected.items.iter().fold(0u64,|n,c|n.saturating_add(c.logical_size()))),app.root.display())).wrap(Wrap{trim:false}),summary);
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
    let help = if app.confirm {
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

pub fn run(root: PathBuf, cancel: Arc<AtomicBool>) -> io::Result<()> {
    let mut app = App::new(root);
    let result = ratatui::run(|terminal| -> io::Result<()> {
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
    });
    app.cancel.store(true, Ordering::Relaxed);
    if let Some(worker) = app.job.take() {
        let _ = worker.join();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn result_page_navigation_and_human_time() {
        let mut app = App::new(".".into());
        app.screen = Screen::Result;
        app.selected = Action::Status;
        app.key(KeyCode::PageDown);
        assert_eq!(app.scroll, 10);
        app.key(KeyCode::PageUp);
        assert_eq!(app.scroll, 0);
        assert!(!format_timestamp(1_700_000_000).is_empty());
    }
    #[test]
    fn scope_navigation_and_direct_duplicates_return() {
        let mut app = App::new(".".into());
        app.key(KeyCode::Char('s'));
        assert_eq!(app.screen, Screen::Scope);
        app.scope_input = "/not-a-real-optimac-scope".into();
        app.key(KeyCode::Enter);
        assert_eq!(app.screen, Screen::Scope);
        assert!(!app.scope_error.is_empty());
        app.key(KeyCode::Esc);
        assert_eq!(app.screen, Screen::Home);
        app.home_action = HomeAction::Duplicates;
        app.home_cursor = 3;
        app.screen = Screen::Result;
        app.key(KeyCode::Esc);
        assert_eq!(app.screen, Screen::Home);
        assert_eq!(app.home_cursor, 3);
    }
    #[test]
    fn home_and_submenus_map_without_launching_mutations() {
        let mut app = App::new(".".into());
        assert_eq!(HOME.len(), 6);
        assert_eq!(
            submenu(HomeAction::Clean),
            &[Action::Clean, Action::Restore]
        );
        assert_eq!(
            submenu(HomeAction::Uninstall),
            &[Action::Apps, Action::Restore]
        );
        assert_eq!(submenu(HomeAction::Duplicates), &[Action::Duplicates]);
        assert!(submenu(HomeAction::Status).contains(&Action::AiTools));
        app.key(KeyCode::Enter);
        assert_eq!(app.screen, Screen::Menu);
        assert!(app.job.is_none());
        app.key(KeyCode::Esc);
        assert_eq!(app.screen, Screen::Home);
    }
    #[test]
    fn selected_files_are_frozen_for_confirmation() {
        let plan:optimac::recovery::Plan=serde_json::from_str(r#"{"items":[{"path":"/Users/test/Documents/a.pdf","dev":1,"ino":1,"size":1000,"modified":0,"personal":true},{"path":"/Users/test/Documents/main.rs","dev":1,"ino":2,"size":2000,"modified":0,"personal":true}],"notes":[]}"#).unwrap();
        let mut app = App::new(".".into());
        app.screen = Screen::Result;
        app.selected = Action::Clean;
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
        app.selected = Action::Clean;
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
        app.screen = Screen::Menu;
        app.home_action = HomeAction::Analyze;
        app.submenu_cursor = 0;
        assert!(!app.key(KeyCode::Down));
        assert_eq!(app.submenu_cursor, 1);
        app.key(KeyCode::Enter);
        assert_eq!(app.screen, Screen::Result);
        app.job.take().unwrap().join().unwrap();
        std::fs::remove_dir(root).unwrap();
        app.key(KeyCode::Esc);
        assert_eq!(app.screen, Screen::Menu);
        app.key(KeyCode::Esc);
        assert_eq!(app.screen, Screen::Home);
    }
    #[test]
    fn all_screens_resize_and_selected_item_stays_visible() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        let mut app = App::new(".".into());
        for screen in [Screen::Home, Screen::Menu, Screen::Scan, Screen::Result] {
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
                for _selected in 0..17 {
                    app.selected = Action::Inspect;
                    terminal.draw(|f| render(f, &app)).unwrap();
                }
            }
        }
        app.screen = Screen::Menu;
        app.home_action = HomeAction::More;
        app.submenu_cursor = 2;
        terminal.backend_mut().resize(40, 10);
        terminal.draw(|f| render(f, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        assert!(
            buffer
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
                .contains("Login Items")
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
