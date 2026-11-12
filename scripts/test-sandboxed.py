#!/usr/bin/env python3
"""Build outside Seatbelt, execute test harnesses inside it; never fall back."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import sys

ROOT = Path(__file__).resolve().parent.parent
SANDBOX = '/usr/bin/sandbox-exec'


def run():
    parser = argparse.ArgumentParser()
    parser.add_argument('--list-only', action='store_true')
    args = parser.parse_args()
    if sys.platform != 'darwin' or not Path(SANDBOX).is_file():
        raise RuntimeError('sandbox-exec is required; environment-only isolation is forbidden')
    home = Path.home().resolve()
    build = subprocess.run(['cargo', 'test', '--locked', '--features', 'fixture-only',
                            '--tests', '--no-run', '--message-format=json'], cwd=ROOT,
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
    with tempfile.TemporaryDirectory(prefix='optimac-sandbox-', dir='/private/tmp') as temporary:
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
        count = 0
        for exe in executables:
            listing = isolated([str(exe), '--list'])
            count += sum(line.endswith(': test') for line in listing.splitlines())
        print(f'Sandboxed harness discovery: {count} tests', flush=True)
        if args.list_only:
            return
        for exe in executables:
            print(isolated([str(exe)]), flush=True)
        print(f'Sandboxed full suite complete: {count} listed tests (see harness ignored counts)')


if __name__ == '__main__':
    try:
        run()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f'test-sandboxed: {error}', file=sys.stderr)
        sys.exit(1)
