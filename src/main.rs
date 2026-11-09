mod ui;
use std::{
    path::Path,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args == ["version"] || args == ["--version"] {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args == ["--help"] || args == ["help"] || args == ["-h"] {
        println!(
            "OptiMac: read-only Rust edition\nUsage: optimac inspect <directory> | ui <directory>\nMetadata only; at most 100000 entries and 32 directory levels.\nLogical bytes are not reclaimable space. 30-second scan budget. Use local test directories.\nRead-only commands: analyze, duplicates, artifacts [directory]; clean (preview), apps (inventory), updates, login, threats, space, orphans (review), doctor, optimize (diagnostics), status."
        );
        return ExitCode::SUCCESS;
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let signal_cancel = cancel.clone();
    if let Err(error) = ctrlc::set_handler(move || signal_cancel.store(true, Ordering::Relaxed)) {
        eprintln!("Cannot install cancellation handler: {error}");
        return ExitCode::FAILURE;
    }
    if args.is_empty() || (args.len() <= 2 && (args[0] == "ui" || args[0] == "menu")) {
        use std::io::IsTerminal;
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            eprintln!(
                "The menu requires an interactive terminal. Use optimac inspect <directory> or --help."
            );
            return ExitCode::from(2);
        }
        let root = args
            .get(1)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| ".".into());
        return match ui::run(root, cancel) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("UI failed: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if args == ["log"] || args == ["history"] {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        return match home
            .ok_or_else(|| std::io::Error::other("HOME unavailable"))
            .and_then(|h| optimac::recovery::history(&h))
        {
            Ok(text) => {
                println!("{text}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    let command = args.first().and_then(|a| a.to_str()).unwrap_or("");
    if let Some(index) = optimac::actions::COMMANDS
        .iter()
        .position(|c| *c == command && *c != "inspect")
    {
        if args
            .iter()
            .skip(1)
            .any(|a| a.to_string_lossy().starts_with('-'))
            || args.len() > 2
        {
            eprintln!(
                "Unsupported flags or extra arguments. These commands are read-only; execution is not yet ported."
            );
            return ExitCode::from(2);
        }
        if args.len() == 2 && !matches!(command, "analyze" | "duplicates" | "artifacts") {
            eprintln!("This diagnostic accepts no arguments.");
            return ExitCode::from(2);
        }
        let root = args
            .get(1)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| ".".into());
        return match optimac::actions::run(index, &root, &cancel) {
            Ok(text) => {
                println!("{text}");
                if cancel.load(Ordering::Relaxed) {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().and_then(|a| a.to_str()).is_some_and(|a| {
        matches!(
            a,
            "scan"
                | "smart-scan"
                | "clean"
                | "analyze"
                | "duplicates"
                | "optimize"
                | "status"
                | "artifacts"
                | "updates"
                | "apps"
                | "uninstall"
                | "browser"
                | "login"
                | "startup"
                | "threats"
                | "scan-threats"
                | "space"
                | "hidden"
                | "orphans"
                | "doctor"
                | "health"
                | "restore"
                | "log"
                | "history"
                | "trash"
                | "config"
        )
    }) {
        eprintln!(
            "This legacy command is not yet ported to Rust. No data was changed. Use inspect <directory> or ui <directory>. Existing recovery data is untouched; use a previous release if restoration is needed."
        );
        return ExitCode::from(2);
    }
    if args.len() != 2 || args[0] != "inspect" {
        eprintln!(
            "Usage: optimac inspect <directory> | ui <directory>; cleanup is not implemented"
        );
        return ExitCode::from(2);
    }
    match optimac::scan(Path::new(&args[1]), &cancel, 100_000) {
        Ok(report) => {
            println!("{report:#?}");
            if report.errors > 0 || report.limited || report.cancelled {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("Scan failed: {error}");
            ExitCode::from(1)
        }
    }
}
