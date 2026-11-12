#!/usr/bin/env python3
"""Two independent sandboxed harness processes must not share a recovery lock."""
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import re
import subprocess
import sys

root = Path(__file__).resolve().parent.parent

def worker(_):
    result = subprocess.run([sys.executable, str(root / 'scripts/test-sandboxed.py'), '--lock-only'],
                            cwd=root, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    print(result.stdout)
    if result.returncode:
        raise RuntimeError('Sandboxed worker failed')
    proof = re.findall(r'LOCK_PROOF (\S+) (\d+) (\d+)', result.stdout)
    if len(proof) != 1 or '1 passed, 0 ignored' not in result.stdout:
        raise RuntimeError('Expected one lock proof and one passing test')
    return proof[0]

with ThreadPoolExecutor(max_workers=2) as workers:
    first, second = list(workers.map(worker, range(2)))
assert first[0] != second[0], 'Shared recovery directory'
assert first[1:] != second[1:], 'Shared lock inode'
print('concurrency: 2 sandboxed processes passed; distinct recovery roots and lock identities')
