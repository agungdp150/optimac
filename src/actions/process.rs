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
pub(crate) fn command(program: &str, args: &[&str], cancel: &AtomicBool) -> io::Result<String> {
    use std::os::unix::process::CommandExt;
    if cancel.load(Ordering::Relaxed) {
        return Err(io::Error::other("Command cancelled"));
    }
    let mut child = Command::new(program)
        .args(args)
        .process_group(0)
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    // Drain continuously to avoid pipe deadlocks, retaining at most 256 KiB.
    fn drain(mut pipe: impl Read) -> Vec<u8> {
        let mut data = Vec::new();
        let mut buf = [0; 4096];
        while let Ok(n) = pipe.read(&mut buf) {
            if n == 0 {
                break;
            }
            let keep = n.min(crate::limits::COMMAND_OUTPUT_LIMIT.saturating_sub(data.len()));
            data.extend_from_slice(&buf[..keep]);
        }
        data
    }
    let (out_tx, out_rx) = std::sync::mpsc::channel();
    let (err_tx, err_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = out_tx.send(drain(stdout));
    });
    std::thread::spawn(move || {
        let _ = err_tx.send(drain(stderr));
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if cancel.load(Ordering::Relaxed) || start.elapsed() > crate::limits::COMMAND_TIMEOUT {
            // The child owns a separate process group, including normal descendants.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other(
                "Command cancelled or timed out; partial output discarded",
            ));
        }
        std::thread::sleep(crate::limits::COMMAND_POLL_INTERVAL);
    };
    // A descendant may retain a pipe even after the direct child exits.
    let remaining = || crate::limits::COMMAND_TIMEOUT.saturating_sub(start.elapsed());
    let output = out_rx.recv_timeout(remaining());
    let error = err_rx.recv_timeout(remaining());
    let (output, error) = match (output, error) {
        (Ok(out), Ok(err)) => (out, err),
        _ => {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            return Err(io::Error::other("Output collection timed out"));
        }
    };
    let text = safe_text(&format!(
        "{}{}",
        String::from_utf8_lossy(&output),
        String::from_utf8_lossy(&error)
    ));
    if program == "/usr/sbin/lsof"
        && status.code() == Some(1)
        && output.is_empty()
        && error.is_empty()
    {
        return Ok(String::new());
    }
    if !status.success() {
        return Err(io::Error::other(format!(
            "{program} exited {status}: {text}"
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
