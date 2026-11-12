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
    #[cfg(feature = "fixture-only")]
    if args == ["--fixture-only-test-probe"] {
        println!("optimac-fixture-only-v1");
        return ExitCode::SUCCESS;
    }
    if args == ["version"] || args == ["--version"] {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args == ["--help"] || args == ["help"] || args == ["-h"] {
        println!(
            "OptiMac: Rust edition (reviewed cleanup available in the menu)\nUsage: optimac inspect <directory> | ui <directory>\nMetadata only; at most 100000 entries and 32 directory levels.\nLogical bytes are not reclaimable space. 30-second scan budget. Use local test directories.\nRead-only commands: analyze, duplicates, artifacts [directory]; clean [scan [--json] [--deep]] (inventory), apps (inventory), updates, login, space, doctor, optimize (diagnostics), status."
        );
        return ExitCode::SUCCESS;
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let signal_cancel = cancel.clone();
    if let Err(error) = ctrlc::set_handler(move || signal_cancel.store(true, Ordering::Relaxed)) {
        eprintln!("Cannot install cancellation handler: {error}");
        return ExitCode::FAILURE;
    }
    if args.is_empty() || args.first().is_some_and(|a| a == "ui" || a == "menu") {
        let mut ui_root = None;
        let mut config = None;
        let mut cursor = 1;
        while cursor < args.len() {
            if args[cursor] == "--config" && cursor + 1 < args.len() && config.is_none() {
                match optimac::cleanup::config::Config::load(Path::new(&args[cursor + 1])) {
                    Ok(c) => config = Some(c),
                    Err(e) => {
                        eprintln!("Invalid configuration: {e}");
                        return ExitCode::from(2);
                    }
                }
                cursor += 2;
            } else if !args[cursor].to_string_lossy().starts_with('-') && ui_root.is_none() {
                ui_root = Some(std::path::PathBuf::from(&args[cursor]));
                cursor += 1;
            } else {
                eprintln!("Usage: optimac ui [directory] [--config PATH]");
                return ExitCode::from(2);
            }
        }
        use std::io::IsTerminal;
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            eprintln!(
                "The menu requires an interactive terminal. Use optimac inspect <directory> or --help."
            );
            return ExitCode::from(2);
        }
        let root = ui_root.unwrap_or_else(|| ".".into());
        return match ui::run(root, cancel, config) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("UI failed: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if args == ["log"] || args == ["history"] {
        return match optimac::paths::AppPaths::from_env()
            .and_then(|paths| optimac::recovery::history(paths.home()))
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
    if command == "config" && args.get(1).is_some_and(|a| a == "check") {
        if args.len() != 3 {
            eprintln!("Usage: optimac config check <path>");
            return ExitCode::from(2);
        }
        return match optimac::cleanup::config::Config::load(Path::new(&args[2])) {
            Ok(_) => {
                println!("Valid schema-v2 configuration. Validation only; no rules executed.");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Invalid configuration: {e}");
                ExitCode::from(2)
            }
        };
    }
    if command == "recovery" {
        use std::io::{IsTerminal, Write};
        if args.len() == 3 && args[1] == "inspect" {
            let result = optimac::paths::AppPaths::from_env().and_then(|paths| {
                optimac::recovery::purge::inspect(
                    optimac::recovery::RecoveryPolicy::Production,
                    paths.home(),
                    args[2].to_str().unwrap_or(""),
                )
            });
            return match result {
                Ok(s) => {
                    println!("{s}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            };
        }
        if args.len() != 3 || args[1] != "purge" {
            eprintln!("Usage: optimac recovery purge <id>; regular-file payloads only");
            return ExitCode::from(2);
        }
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            eprintln!("Permanent purge requires an interactive terminal");
            return ExitCode::from(2);
        }
        let result = (|| -> std::io::Result<String> {
            let paths = optimac::paths::AppPaths::from_env()?;
            let home = paths.home();
            let id = args[2]
                .to_str()
                .ok_or_else(|| std::io::Error::other("Invalid ID"))?;
            let plan = optimac::recovery::purge::plan(
                optimac::recovery::RecoveryPolicy::Production,
                home,
                id,
            )?;
            println!("{}", plan.describe());
            print!("Type DELETE {id} to confirm, or Enter to cancel: ");
            std::io::stdout().flush()?;
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer)?;
            if answer.trim() != format!("DELETE {id}") {
                return Ok("Cancelled; no data changed".into());
            }
            optimac::recovery::purge::execute(
                optimac::recovery::RecoveryPolicy::Production,
                home,
                &plan,
                &cancel,
            )
        })();
        return match result {
            Ok(s) => {
                println!("{s}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    if command == "clean" {
        let tail = &args[1..];
        if tail.first().is_some_and(|arg| arg == "providers") {
            if tail.len() > 2 || (tail.len() == 2 && tail[1] != "--json") {
                eprintln!("Usage: optimac clean providers [--json]");
                return ExitCode::from(2);
            }
            let catalog = optimac::cleanup::catalog();
            if tail.len() == 2 {
                let pip = optimac::paths::AppPaths::from_env()
                    .ok()
                    .map(|paths| optimac::cleanup::pip::resolve(paths.home(), &cancel));
                println!(
                    "{}",
                    serde_json::json!({"schema_version": 1, "providers": catalog, "pip": pip})
                );
            } else {
                for provider in catalog {
                    println!(
                        "{}: {} ({}, {}); mutation disabled",
                        provider.provider, provider.certification, provider.class, provider.risk
                    );
                }
            }
            return ExitCode::SUCCESS;
        }
        let mut config_path = None;
        let mut class_filter = "all";
        let mut valid =
            tail.is_empty() || tail[0] == "scan" || tail[0] == "retry" || tail[0] == "plan";
        let mut i = 1;
        while i < tail.len() {
            if tail[i] == "--config" && config_path.is_none() && i + 1 < tail.len() {
                config_path = Some(&tail[i + 1]);
                i += 2;
            } else if tail[i] == "--class" && i + 1 < tail.len() {
                match tail[i + 1].to_str() {
                    Some("cache") => class_filter = "cache",
                    Some("review") => class_filter = "review",
                    Some("all") => class_filter = "all",
                    _ => {
                        valid = false;
                        break;
                    }
                }
                i += 2;
            } else if tail[i] == "--json"
                || tail[i] == "--deep"
                || tail[i] == "--all"
                || tail[i] == "--verbose"
                || tail[i] == "--by-category"
            {
                i += 1;
            } else {
                valid = false;
                break;
            }
        }
        if tail.first().is_some_and(|a| a == "plan")
            && tail.iter().any(|a| {
                matches!(
                    a.to_str(),
                    Some("--all" | "--verbose" | "--by-category" | "--class")
                )
            })
        {
            valid = false;
        }
        if !valid {
            eprintln!(
                "Execution is not yet ported; usage: optimac clean scan [--config PATH] [--json] [--deep]"
            );
            return ExitCode::from(2);
        }
        let paths = match optimac::paths::AppPaths::from_env() {
            Ok(paths) => paths,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        };
        let home = paths.home();
        let budget = if tail.iter().any(|a| a == "--deep") {
            optimac::cleanup::Budget::deep()
        } else {
            optimac::cleanup::Budget::quick()
        };
        let mut progress = |r: &optimac::cleanup::ProviderReport| {
            if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
                eprint!(
                    "\rScanning {}: {} files                    ",
                    r.provider, r.files
                );
            }
        };
        let result = if let Some(path) = config_path {
            match optimac::cleanup::config::Config::load(Path::new(path)) {
                Ok(config) => optimac::cleanup::configured_inventory_with_progress(
                    home,
                    &cancel,
                    budget,
                    &config,
                    &mut progress,
                ),
                Err(e) => {
                    eprintln!("Invalid configuration: {e}");
                    return ExitCode::from(2);
                }
            }
        } else {
            optimac::cleanup::inventory_with_progress(home, &cancel, budget, &mut progress)
        };
        if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            eprint!("\r\x1b[2K");
        }
        return match result {
            Ok(report) => {
                if tail.first().is_some_and(|a| a == "plan") {
                    let plan = optimac::cleanup::plan::from_inventory(&report);
                    if tail.iter().any(|a| a == "--json") {
                        println!("{}", serde_json::to_string_pretty(&plan).unwrap());
                    } else {
                        print!("{}", plan.render());
                    }
                } else if tail.iter().any(|a| a == "--json") {
                    let mut json = serde_json::to_value(&report).expect("serializable report");
                    json["summary"] = report.summary();
                    json["grouped_totals"] = serde_json::to_value(report.grouped_totals()).unwrap();
                    if tail.iter().any(|a| a == "--by-category") {
                        json["categories"] = serde_json::to_value(
                            optimac::cleanup::category::totals(&report.providers),
                        )
                        .unwrap();
                    }
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                } else if tail.iter().any(|a| a == "--by-category") {
                    print!("{}", optimac::cleanup::category::render(&report.providers));
                } else {
                    println!(
                        "{}",
                        report.render_filtered(
                            tail.iter().any(|a| a == "--all"),
                            tail.iter().any(|a| a == "--verbose"),
                            class_filter
                        )
                    );
                }
                if report.complete {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                }
            }
            Err(e) => {
                eprintln!("Inventory failed: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if matches!(command, "threats" | "scan-threats" | "orphans") {
        eprintln!(
            "This feature was retired: filename matches and folder listings cannot reliably establish threats or orphaned data."
        );
        return ExitCode::from(2);
    }
    if let Some(index) = optimac::actions::Action::command(command) {
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
    match optimac::scan(
        Path::new(&args[1]),
        &cancel,
        optimac::limits::SCAN_MAX_ENTRIES,
    ) {
        Ok(report) => {
            println!(
                "Files: {}\nLogical size: {} ({} bytes)\nSkipped: {}\nErrors: {}\nCancelled: {}\nBudget reached: {}",
                report.files,
                optimac::format_size(report.logical_bytes),
                report.logical_bytes,
                report.skipped,
                report.errors,
                report.cancelled,
                report.limited
            );
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
