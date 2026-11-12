#!/usr/bin/env python3
"""Build outside Seatbelt, execute test harnesses inside it; never fall back."""
import argparse
import json
import os
import re
from pathlib import Path
import subprocess
import tempfile
import sys

ROOT = Path(__file__).resolve().parent.parent
SANDBOX = '/usr/bin/sandbox-exec'


def recovery_metadata(root):
    """Outer observer only: lstat and directory names, never file contents or links."""
    import stat
    result = []
    pending = [root]
    while pending:
        path = pending.pop()
        try:
            info = path.lstat()
        except FileNotFoundError:
            result.append((str(path.relative_to(root)), 'absent'))
            continue
        result.append((str(path.relative_to(root)), info.st_mode, info.st_size, info.st_mtime_ns))
        if stat.S_ISDIR(info.st_mode):
            pending.extend(path.iterdir())
    return sorted(result)


def run():
    parser = argparse.ArgumentParser()
    parser.add_argument('--list-only', action='store_true')
    parser.add_argument('--canary-only', action='store_true')
    parser.add_argument('--recovery-only', action='store_true')
    parser.add_argument('--lock-only', action='store_true')
    args = parser.parse_args()
    if sys.platform != 'darwin' or not Path(SANDBOX).is_file():
        raise RuntimeError('sandbox-exec is required; environment-only isolation is forbidden')
    home = Path.home().resolve()
    selection = ['--lib'] if args.recovery_only else ['--features', 'fixture-only', '--tests']
    build = subprocess.run(['cargo', 'test', '--locked', *selection, '--no-run', '--message-format=json'], cwd=ROOT,
                           text=True, stdout=subprocess.PIPE, check=True)
    executables = []
    for line in build.stdout.splitlines():
        artifact = json.loads(line)
        if artifact.get('reason') == 'compiler-artifact' and artifact.get('executable') and artifact.get('profile', {}).get('test'):
            executables.append(Path(artifact['executable']).resolve())
    if not executables:
        raise RuntimeError('No test harnesses built')
    # Only compiled artifacts beneath the repository may be read within real HOME.
    target = ROOT / 'target/debug'
    if any(not exe.is_relative_to(target) for exe in executables):
        raise RuntimeError('Unexpected artifact location; refusing to broaden sandbox reads')
    with tempfile.TemporaryDirectory(prefix='optimac-sandbox-', dir=tempfile.gettempdir()) as temporary:
        temp = Path(temporary).resolve()
        for child in ['home', 'tmp', 'config', 'cache', 'data', 'state', 'runtime']:
            (temp / child).mkdir(mode=0o700)
        env = {'HOME': str(temp / 'home'), 'TMPDIR': str(temp / 'tmp'),
               'XDG_CONFIG_HOME': str(temp / 'config'), 'XDG_CACHE_HOME': str(temp / 'cache'),
               'XDG_DATA_HOME': str(temp / 'data'), 'XDG_STATE_HOME': str(temp / 'state'),
               'XDG_RUNTIME_DIR': str(temp / 'runtime'), 'PATH': '', 'LANG': 'C'}
        q = json.dumps
        profile = temp / 'profile.sb'
        profile.write_text(f'''(version 1)
(allow default)
(deny network*)
(deny file-write* (require-all
    (require-not (subpath {q(str(temp))}))
    (require-not (literal "/dev/null"))
    (require-not (literal "/dev/tty"))))
(deny file-read-data (require-all (subpath {q(str(home))})
    (require-not (subpath {q(str(target))}))))
''')
        def isolated(command):
            result = subprocess.run([SANDBOX, '-f', str(profile), *command], cwd=temp,
                                    env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            if result.returncode:
                raise RuntimeError(f'Sandbox command failed ({result.returncode}): {command}\n{result.stdout}{result.stderr}\nNo unsandboxed fallback permitted.')
            return result.stdout
        isolated(['/usr/bin/true'])
        # Compile the canary outside Seatbelt. It uses create_new, never truncation.
        source = temp / 'canary.rs'
        source.write_text(r'''use std::fs::OpenOptions;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    for path in &args[1..3] {
        let error = OpenOptions::new().write(true).create_new(true).open(path)
            .expect_err("sandbox allowed forbidden write");
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied, "{path}: {error}");
    }
    std::fs::write(&args[3], b"fixture canary").unwrap();
    println!("canary: real home denied; real recovery denied; temporary write allowed");
}''')
        canary = temp / 'canary'
        subprocess.run(['rustc', str(source), '-o', str(canary)], cwd=ROOT, check=True)
        recovery = home / '.optimac-rust-recovery'
        nonce = temp.name + '-canary'
        print(isolated([str(canary), str(home / nonce), str(recovery / nonce), str(temp / 'canary-output')]), flush=True)
        if args.canary_only:
            return
        count = 0
        for exe in executables:
            listing = isolated([str(exe), '--list'])
            count += sum(line.endswith(': test') for line in listing.splitlines())
        print(f'Sandboxed harness discovery: {count} tests', flush=True)
        if args.list_only:
            return
        before = recovery_metadata(recovery)
        failures = []
        passed = ignored = 0
        try:
            for exe in executables:
                try:
                    output = isolated([str(exe), *(['--ignored'] if args.recovery_only else ['recovery_lock', '--nocapture'] if args.lock_only else [])])
                    print(output, flush=True)
                    summary = re.search(r'test result: ok\. (\d+) passed; \d+ failed; (\d+) ignored;', output)
                    if not summary:
                        raise RuntimeError('Missing test result summary')
                    passed += int(summary[1])
                    ignored += int(summary[2])
                except RuntimeError as error:
                    failures.append(str(error))
                    print(error, file=sys.stderr, flush=True)
        finally:
            after = recovery_metadata(recovery)
            if before != after:
                raise RuntimeError('Real recovery metadata CHANGED')
            print(f'Real recovery metadata unchanged ({len(before)} entries)', flush=True)
        if failures:
            raise RuntimeError(f'{len(failures)} test harnesses failed; full suite is not green')
        print(f'Sandboxed suite complete: {count} listed, {passed} passed, {ignored} ignored')


if __name__ == '__main__':
    try:
        run()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f'test-sandboxed: {error}', file=sys.stderr)
        sys.exit(1)
