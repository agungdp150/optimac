use super::*;
#[test]
fn automatically_selects_only_cache_candidates_before_review() {
    let plan:optimac::recovery::Plan=serde_json::from_str(r#"{"items":[{"path":"/Users/test/.cache/pip/old","dev":1,"ino":1,"size":10,"modified":0,"personal":false},{"path":"/Users/test/Documents/report.pdf","dev":1,"ino":2,"size":20,"modified":0,"personal":true}],"notes":[]}"#).unwrap();
    let mut app = App::new(".".into());
    app.screen = Screen::Result;
    app.selected = Action::Clean;
    *app.plan.lock().unwrap() = Some(plan);
    app.initialize_cache_selection();
    assert_eq!(app.selected_files.len(), 1);
    assert!(
        app.selected_files
            .contains(&PathBuf::from("/Users/test/.cache/pip/old"))
    );
    app.key(KeyCode::Char('u'));
    assert!(app.selected_files.is_empty());
    app.initialize_cache_selection();
    assert!(app.selected_files.is_empty());
    app.key(KeyCode::Char('a'));
    assert_eq!(app.selected_files.len(), 1);
    assert!(app.job.is_none());
    app.key(KeyCode::Char('r'));
    assert!(app.confirm);
    let reviewed = app.reviewed.as_ref().unwrap();
    assert_eq!(reviewed.items.len(), 1);
    assert!(reviewed.items[0].path.ends_with("pip/old"));
    app.key(KeyCode::Char('n'));
    assert!(!app.confirm);
    assert!(app.job.is_none());
}
#[test]
fn cleanup_progress_renders_at_various_terminal_sizes() {
    let mut app = App::new(".".into());
    app.screen = Screen::Result;
    app.selected = Action::Clean;
    *app.cleanup_progress.lock().unwrap() = Some(optimac::recovery::CleanupProgress {
        total: 5,
        processed: 2,
        moved: 1,
        skipped: 1,
        current: Some("/tmp/example".into()),
    });
    let (send, receive) = std::sync::mpsc::channel::<()>();
    app.job = Some(std::thread::spawn(move || {
        let _ = receive.recv();
    }));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
    for (w, h) in [(80, 24), (40, 10), (20, 5), (1, 1), (0, 0), (80, 24)] {
        terminal.backend_mut().resize(w, h);
        terminal.draw(|f| render(f, &app)).unwrap();
    }
    let buffer = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(buffer.contains("2 / 5 checked"));
    assert!(buffer.contains("Moved: 1"));
    send.send(()).unwrap();
    app.job.take().unwrap().join().unwrap();
}
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
        &[Action::CleanPreview, Action::Clean, Action::Restore]
    );
    assert_eq!(
        submenu(HomeAction::Uninstall),
        &[Action::Apps, Action::Restore]
    );
    assert_eq!(submenu(HomeAction::Duplicates), &[Action::Duplicates]);
    assert!(!submenu(HomeAction::Status).contains(&Action::AiTools));
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
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(20, 5)).unwrap();
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
    let (root, _authority) = optimac::fixture_paths::FixtureRoot::create_test_fixture().unwrap();
    let paths = optimac::paths::AppPaths::from_home(Some(root.as_os_str().to_owned())).unwrap();
    let mut app = App::with_home(root.clone(), paths.home().to_path_buf());
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

#[test]
fn status_refresh_keeps_previous_values_and_does_not_overlap() {
    use std::sync::mpsc;
    let mut app = App::new(".".into());
    app.screen = Screen::Result;
    app.home_action = HomeAction::Status;
    app.selected = Action::Memory;
    app.scroll = 3;
    *app.status_detail.lock().unwrap() = Some(optimac::actions::status::Detail {
        collected: None,
        sections: vec![("Memory pages".into(), vec![("Free".into(), "42 MB".into())])],
    });
    let (tx, rx) = mpsc::channel::<()>();
    app.job = Some(std::thread::spawn(move || {
        let _ = rx.recv();
    }));
    let previous = app.status_detail.clone();
    app.key(KeyCode::Char('r'));
    assert_eq!(app.screen, Screen::Result);
    assert_eq!(app.scroll, 3);
    assert!(Arc::ptr_eq(&previous, &app.status_detail));
    assert_eq!(
        app.status_detail.lock().unwrap().as_ref().unwrap().sections[0].1[0].1,
        "42 MB"
    );
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
    terminal.draw(|f| render(f, &app)).unwrap();
    let screen = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(screen.contains("42 MB"));
    assert!(screen.contains("Refreshing"));
    tx.send(()).unwrap();
    app.job.take().unwrap().join().unwrap();
}

#[test]
fn configured_inventory_runs_in_menu_without_default_roots() {
    let (fixture_home, _authority) =
        optimac::fixture_paths::FixtureRoot::create_test_fixture().unwrap();
    let mut app = App::with_home(fixture_home.clone(), fixture_home.clone());
    app.inventory_config =
        Some(optimac::cleanup::config::Config::parse("schema_version=2").unwrap());
    app.screen = Screen::Menu;
    app.selected = Action::CleanPreview;
    app.key(KeyCode::Enter);
    if let Some(job) = app.job.take() {
        job.join().unwrap();
    }
    let output = app.output.lock().unwrap();
    assert!(
        output.contains("Configured read-only inventory"),
        "{output}"
    );
    assert!(!output.contains("Default-location hints"));
    drop(output);
    std::fs::remove_dir(fixture_home).unwrap();
}

#[test]
fn missing_fixture_home_fails_closed() {
    let app = App::new(std::path::PathBuf::from("."));
    assert!(app.home_path.as_os_str().is_empty());
    assert!(
        optimac::paths::AppPaths::from_home(Some(app.home_path.as_os_str().to_owned())).is_err()
    );
}
