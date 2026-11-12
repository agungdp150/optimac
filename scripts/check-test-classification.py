#!/usr/bin/env python3
"""Check the macOS fixture-only harness list; never execute test bodies."""
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MANIFEST = ROOT / 'docs/TEST-CLASSIFICATION.json'
CLASSES = {'pure', 'fixture_fs', 'subprocess_audited', 'recovery_blocked'}


def discover():
    build = subprocess.run(['cargo', 'test', '--locked', '--features', 'fixture-only',
                            '--tests', '--no-run', '--message-format=json'],
                           cwd=ROOT, text=True, stdout=subprocess.PIPE, check=True)
    tests = set()
    for line in build.stdout.splitlines():
        item = json.loads(line)
        if item.get('reason') != 'compiler-artifact' or not item.get('executable') or not item.get('profile', {}).get('test'):
            continue
        target = item['target']['name'] + ':' + ','.join(item['target']['kind'])
        listing = subprocess.run([item['executable'], '--list'], cwd=ROOT,
                                 text=True, stdout=subprocess.PIPE, check=True)
        for name in listing.stdout.splitlines():
            if name.endswith(': test'):
                tests.add(target + '/' + name[:-6])
    if not tests:
        raise ValueError('No tests discovered')
    return tests


def check(actual, records):
    names = [r['name'] for r in records]
    if len(set(names)) != len(names):
        raise ValueError('Duplicate classification')
    if any(r['class'] not in CLASSES for r in records):
        raise ValueError('Unknown class')
    missing, stale = actual - set(names), set(names) - actual
    if missing or stale:
        raise ValueError(f'Missing: {sorted(missing)}; stale: {sorted(stale)}')


if __name__ == '__main__':
    actual = discover()
    if sys.argv[1:] == ['--initialize']:
        if MANIFEST.exists():
            raise SystemExit('Refusing to overwrite existing manifest')
        blocked = {'personal_selection_moves_only_reviewed_file_and_restores',
                   'app_bundle_roundtrip_and_changed_candidate_refusal',
                   'recover_without_overwriting', 'purge_removes_only_reviewed_recovery_payload'}
        pure = {'broad_groups', 'readable_units_preserve_boundaries',
                'filesystem_allowlist_is_fail_closed', 'device_boundary_comparison_does_not_authorize_crossing',
                'developer_environment_roots_are_explicit_and_safe'}
        records = [{'name': n, 'class': 'recovery_blocked' if n.split('::')[-1] in blocked else
                    'pure' if n.split('::')[-1] in pure else 'fixture_fs',
                    'review': 'classified' if n.split('::')[-1] in blocked | pure else 'unreviewed'}
                   for n in sorted(actual)]
        MANIFEST.write_text(json.dumps({'schema_version': 1, 'platform': 'macOS', 'tests': records}, indent=2) + '\n')
    data = json.loads(MANIFEST.read_text())
    if data['schema_version'] != 1:
        raise ValueError('Unsupported classification schema')
    check(actual, data['tests'])
    print(f'classification: {len(actual)} tests; ' + ', '.join(
        f'{kind}={sum(r["class"] == kind for r in data["tests"])}' for kind in sorted(CLASSES)))
