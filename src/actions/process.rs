use std::{
    io::{self, Read},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

fn safe_text(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                '�'
            } else {
                c
            }
        })
        .collect()
}
/// Structured observations, not proof of a native cleanup postcondition.
#[derive(Debug)]
pub struct CommandResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
    pub duration: std::time::Duration,
    pub timed_out: bool,
    pub cancelled: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

/// Caller must independently validate executable identity and adapter capability.
/// Only explicit environment entries reach the child; no shell is involved.
pub fn run(
    program: &std::path::Path,
    args: &[&std::ffi::OsStr],
    cwd: &std::path::Path,
    environment: &[(&std::ffi::OsStr, &std::ffi::OsStr)],
    timeout: std::time::Duration,
    cancel: &AtomicBool,
) -> io::Result<CommandResult> {
    use std::os::unix::process::CommandExt;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    if !program.is_absolute() || !cwd.is_absolute() || timeout.is_zero() {
        return Err(io::Error::other(
            "Absolute executable/cwd and positive timeout required",
        ));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(io::Error::other("Command cancelled before spawn"));
    }
    let start = Instant::now();
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(environment.iter().copied())
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    #[derive(Default)]
    struct Capture {
        bytes: Vec<u8>,
        truncated: bool,
        done: bool,
    }
    fn drain(mut pipe: impl Read + Send + 'static) -> Arc<Mutex<Capture>> {
        let capture = Arc::new(Mutex::new(Capture::default()));
        let output = capture.clone();
        std::thread::spawn(move || {
            let mut buffer = [0; 4096];
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let mut data = output.lock().unwrap();
                        let keep = n.min(
                            crate::limits::COMMAND_OUTPUT_LIMIT.saturating_sub(data.bytes.len()),
                        );
                        data.bytes.extend_from_slice(&buffer[..keep]);
                        data.truncated |= keep < n;
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => {
                        output.lock().unwrap().truncated = true;
                        break;
                    }
                }
            }
            output.lock().unwrap().done = true;
        });
        capture
    }
    let out = drain(child.stdout.take().unwrap());
    let err = drain(child.stderr.take().unwrap());
    let mut timed_out = false;
    let mut cancelled = false;
    let mut termination = None;
    let mut status = None;
    loop {
        if status.is_none() {
            status = child.try_wait()?;
        }
        if status.is_some() && out.lock().unwrap().done && err.lock().unwrap().done {
            break;
        }
        if termination.is_none() {
            cancelled = cancel.load(Ordering::Relaxed);
            timed_out = start.elapsed() >= timeout;
            if cancelled || timed_out {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGTERM);
                }
                termination = Some(Instant::now());
            }
        }
        if termination.is_some_and(|t| t.elapsed() >= Duration::from_secs(2)) {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            if status.is_none() {
                status = Some(child.wait()?);
            }
            // Pipe readers publish incrementally; no unbounded join on escaped descendants.
            break;
        }
        std::thread::sleep(crate::limits::COMMAND_POLL_INTERVAL);
    }
    let out = out.lock().unwrap();
    let err = err.lock().unwrap();
    Ok(CommandResult {
        stdout: out.bytes.clone(),
        stderr: err.bytes.clone(),
        exit_code: status.and_then(|s| s.code()),
        duration: start.elapsed(),
        timed_out,
        cancelled,
        stdout_truncated: out.truncated || !out.done,
        stderr_truncated: err.truncated || !err.done,
    })
}

pub(crate) fn command(program: &str, args: &[&str], cancel: &AtomicBool) -> io::Result<String> {
    use std::{ffi::OsStr, path::Path};
    let arguments: Vec<_> = args.iter().map(OsStr::new).collect();
    let result = run(
        Path::new(program),
        &arguments,
        Path::new("/"),
        &[
            (OsStr::new("LC_ALL"), OsStr::new("C")),
            (OsStr::new("HOMEBREW_NO_AUTO_UPDATE"), OsStr::new("1")),
        ],
        crate::limits::COMMAND_TIMEOUT,
        cancel,
    )?;
    let text = safe_text(&format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    ));
    if result.cancelled || result.timed_out {
        return Err(io::Error::other(format!(
            "Command cancelled or timed out; partial output: {text}"
        )));
    }
    if program == "/usr/sbin/lsof"
        && result.exit_code == Some(1)
        && result.stdout.is_empty()
        && result.stderr.is_empty()
        && !result.stdout_truncated
        && !result.stderr_truncated
    {
        return Ok(String::new());
    }
    if result.exit_code != Some(0) || result.stdout_truncated || result.stderr_truncated {
        return Err(io::Error::other(format!(
            "{program} exited {:?} (output incomplete: {}): {text}",
            result.exit_code,
            result.stdout_truncated || result.stderr_truncated
        )));
    }
    Ok(text)
}
pub(super) fn commands(items: &[(&str, &[&str])], cancel: &AtomicBool) -> String {
    let mut result = String::new();
    for (name, args) in items {
        if cancel.load(Ordering::Relaxed) {
            result.push_str("Cancelled — partial results\n");
            break;
        }
        let title = match (*name, args.first().copied()) {
            ("/usr/bin/sw_vers", _) => "macOS",
            ("/usr/bin/vm_stat", _) => "Memory",
            ("/usr/sbin/sysctl", Some("vm.swapusage")) => "Swap",
            ("/usr/sbin/sysctl", _) => "Hardware",
            ("/bin/df", _) => "Disk volumes",
            ("/usr/bin/tmutil", _) => "Time Machine snapshots",
            ("/usr/bin/fdesetup", _) => "FileVault",
            ("/usr/bin/csrutil", _) => "System Integrity Protection",
            ("/usr/sbin/spctl", _) => "Gatekeeper",
            ("/usr/libexec/ApplicationFirewall/socketfilterfw", _) => "Firewall",
            _ => "Details",
        };
        result.push_str(&format!("\n{title}\n"));
        match command(name, args, cancel) {
            Ok(text) if text.trim().is_empty() => result.push_str("  No information reported\n"),
            Ok(text) => {
                for line in text.lines() {
                    result.push_str(&format!("  {}\n", line.trim()));
                }
            }
            Err(e) => result.push_str(&format!("  Unavailable: {e}\n")),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsStr, path::Path, time::Duration};

    #[test]
    fn structured_output_preserves_streams_exit_and_timeout_output() {
        let cancel = AtomicBool::new(false);
        let result = run(
            Path::new("/bin/sh"),
            &[
                OsStr::new("-c"),
                OsStr::new("printf out; printf err >&2; exit 7"),
            ],
            Path::new("/"),
            &[],
            Duration::from_secs(3),
            &cancel,
        )
        .unwrap();
        assert_eq!(result.stdout, b"out");
        assert_eq!(result.stderr, b"err");
        assert_eq!(result.exit_code, Some(7));
        assert!(!result.stdout_truncated && !result.stderr_truncated);
        let result = run(
            Path::new("/bin/sh"),
            &[
                OsStr::new("-c"),
                OsStr::new("printf partial; exec /bin/sleep 10"),
            ],
            Path::new("/"),
            &[],
            Duration::from_millis(200),
            &cancel,
        )
        .unwrap();
        assert!(result.timed_out);
        assert_eq!(result.stdout, b"partial");
        assert!(result.duration < Duration::from_secs(4));
    }

    #[test]
    fn output_is_bounded_and_untrusted_path_is_not_searched() {
        let cancel = AtomicBool::new(false);
        assert!(
            run(
                Path::new("sh"),
                &[],
                Path::new("/"),
                &[],
                Duration::from_secs(1),
                &cancel
            )
            .is_err()
        );
        let result = run(
            Path::new("/bin/sh"),
            &[
                OsStr::new("-c"),
                OsStr::new("/usr/bin/yes x | /usr/bin/head -c 300000"),
            ],
            Path::new("/"),
            &[],
            Duration::from_secs(3),
            &cancel,
        )
        .unwrap();
        assert!(result.stdout_truncated);
        assert_eq!(result.stdout.len(), crate::limits::COMMAND_OUTPUT_LIMIT);
    }
}
