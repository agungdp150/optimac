mod render;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use optimac::{Report, actions::Action, scan_progress};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Gauge, List, ListItem, ListState, Paragraph, Wrap},
};
use render::render;
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
    cleanup_progress: Arc<Mutex<Option<optimac::recovery::CleanupProgress>>>,
    animation_frame: usize,
    scope_from_home: bool,
    plan: Arc<Mutex<Option<optimac::recovery::Plan>>>,
    choices: Arc<Mutex<Vec<String>>>,
    confirm: bool,
    selected_files: std::collections::BTreeSet<PathBuf>,
    file_group: Option<String>,
    cache_selection_initialized: bool,
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
            cleanup_progress: Arc::new(Mutex::new(None)),
            animation_frame: 0,
            scope_from_home: false,
            root,
            plan: Arc::new(Mutex::new(None)),
            choices: Arc::new(Mutex::new(Vec::new())),
            confirm: false,
            selected_files: std::collections::BTreeSet::new(),
            file_group: None,
            cache_selection_initialized: false,
            reviewed: None,
            restore_id: None,
            output: Arc::new(Mutex::new(String::new())),
            scroll: 0,
            job: None,
            cancel: Arc::new(AtomicBool::new(false)),
            shared: Arc::new(Mutex::new((Report::default(), "Ready".into()))),
        }
    }
    fn initialize_cache_selection(&mut self) {
        if self.screen != Screen::Result
            || self.selected != Action::Clean
            || self.cache_selection_initialized
            || self.job.as_ref().is_some_and(|j| !j.is_finished())
        {
            return;
        }
        if let Some(plan) = self.plan.lock().unwrap().as_ref() {
            self.selected_files.extend(
                plan.items
                    .iter()
                    .filter(|item| item.is_cache_candidate())
                    .map(|item| item.path.clone()),
            );
            self.cache_selection_initialized = true;
        }
    }
    fn key(&mut self, key: KeyCode) -> bool {
        self.initialize_cache_selection();
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
                    KeyCode::Char('a') => {
                        self.selected_files.extend(
                            plan.items
                                .iter()
                                .filter(|item| item.is_cache_candidate())
                                .map(|item| item.path.clone()),
                        );
                    }
                    KeyCode::Char('u') => {
                        self.selected_files.clear();
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
                    self.cleanup_progress = Arc::new(Mutex::new(plan.as_ref().map(|p| {
                        optimac::recovery::CleanupProgress {
                            total: p.items.len(),
                            ..Default::default()
                        }
                    })));
                    let progress = self.cleanup_progress.clone();
                    let output = self.output.clone();
                    self.cancel = Arc::new(AtomicBool::new(false));
                    let cancel = self.cancel.clone();
                    *output.lock().unwrap() =
                        "Moving selected items… c cancels between moves".into();
                    self.job = Some(std::thread::spawn(move || {
                        let result = if let Some(id) = restore {
                            optimac::recovery::restore(&home, &id)
                        } else if let Some(plan) = plan {
                            optimac::recovery::execute_with_progress(&home, &plan, &cancel, |p| {
                                *progress.lock().unwrap() = Some(p.clone())
                            })
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
                    let result =
                        scan_progress(&root, &cancel, optimac::limits::SCAN_MAX_ENTRIES, |r| {
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
                self.cleanup_progress = Arc::new(Mutex::new(None));
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
                self.cache_selection_initialized = false;
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

pub fn run(root: PathBuf, cancel: Arc<AtomicBool>) -> io::Result<()> {
    let mut app = App::new(root);
    let result = ratatui::run(|terminal| -> io::Result<()> {
        loop {
            app.initialize_cache_selection();
            app.animation_frame = app.animation_frame.wrapping_add(1);
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
mod tests;
