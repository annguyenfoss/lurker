#!/usr/bin/env python3
"""Prepare owned passed fixtures for an ordinary reboot, then verify and close.

The operator performs the reboot separately after checking all live jobs.
Only fixture-owned test credentials are read; none are printed or put in argv.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('phase', choices=('prepare', 'verify'))
    p.add_argument('--root', type=Path, required=True)
    p.add_argument('--prefix', type=Path, required=True)
    p.add_argument('--harness', type=Path, required=True)
    p.add_argument('--qualifier', type=Path, required=True)
    p.add_argument('--name', required=True)
    p.add_argument('--cases', help='Comma-separated receipt filenames, without .json; prepare only')
    args = p.parse_args()
    if not args.name.replace('-', '').isalnum():
        p.error('Use an alphanumeric run name')
    root = args.root.resolve(strict=True)
    harness = module('payload_harness', args.harness)
    qualifier = module('debian_qualifier', args.qualifier)
    validation = root / 'validation'
    validation.mkdir(exist_ok=True)
    state_path = validation / (args.name + '.json')
    binary = args.prefix / 'bin/lurker'
    boot_id = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    if args.phase == 'prepare':
        if not args.cases or state_path.exists():
            p.error('Prepare requires selected cases and a new run name')
        records = []
        for name in args.cases.split(','):
            if '/' in name or not name.startswith(('containers-', 'directories-')):
                p.error('Only owned file-container or directory receipt names are accepted')
            receipt = root / 'cases' / (name + '.json')
            record = json.loads(receipt.read_text())
            if record['status'] != 'passed' or record['source_kind'] == 'device':
                p.error('Prepare accepts only passed file-backed fixtures')
            records.append({'receipt': str(receipt), 'record': record})
        state = {'status': 'preparing', 'host': 'debian', 'boot_before': boot_id,
                 'binary_sha256': harness.file_hash(binary), 'cases': records, 'operations': []}
    else:
        state = json.loads(state_path.read_text())
        if state['status'] != 'reboot_ready' or state['boot_before'] == boot_id:
            p.error('Verification requires an actual different boot after preparation')
        if state['binary_sha256'] != harness.file_hash(binary):
            p.error('Runtime binary changed across the reboot')
        state.update(status='verifying', boot_after=boot_id)
    expected = harness.load_payload(root / 'payload')
    expected['page.txt'] += b'\nFresh Debian second-mount append.\n'
    expected['second.txt'] = b'Fresh Debian second-mount new file.\n'
    opened = []

    def cli(action, source, mount, backend, record, suffix):
        fixture = Path(record['fixture'])
        credential = Path(record['private_fixture_credential'])
        if credential.stat().st_mode & 0o077:
            raise RuntimeError('Test credential file has nonprivate permissions')
        argv = ([] if backend in qualifier.DIRECTORIES else ['/usr/bin/sudo', '-n']) + [str(binary), action, '--type', backend, '--color=never']
        password = None
        if action == 'mount':
            argv += [str(source), str(mount)]
            if record['authentication'] == 'keyfile':
                argv += ['--key-file', str(credential)]
            else:
                password = credential.read_text()
        else:
            argv += [str(mount)]
        log = validation / f'{args.name}-{args.phase}-{len(state["operations"]):03d}-{suffix}.log'
        status = harness.terminal(argv, password, 240, log)
        state['operations'].append({'action': action, 'mountpoint': str(mount), 'backend': backend, 'log': str(log), **status})
        qualifier.atomic(state_path, state)
        if status['exit_code'] or status['timed_out']:
            raise RuntimeError('Normal product operation failed; see ' + str(log))

    try:
        for row in state['cases']:
            record = row['record']
            fixture = Path(record['fixture'])
            mount = fixture / 'mount'
            backend = record['backend']
            fs = record['filesystem']
            if record['binary_sha256'] != state['binary_sha256']:
                raise RuntimeError('Selected fixture used a different binary')
            if backend in qualifier.DIRECTORIES:
                backing = fixture / 'backing'
                kind = 'luks1' if fs == 'ufs1' else 'luks'
                if os.path.ismount(backing):
                    raise RuntimeError('Backing unexpectedly mounted before reopen')
                cli('mount', fixture / 'backing.img', backing, kind, record, 'backing-open')
                opened.append((None, backing, kind, record))
                qualifier.require_native_filesystem(backing, fs)
                source = backing / 'encrypted'
            else:
                source = fixture / 'vault.img'
            if os.path.ismount(mount):
                raise RuntimeError('Plaintext unexpectedly mounted before reopen')
            cli('mount', source, mount, backend, record, 'open')
            opened.append((source, mount, backend, record))
            if backend not in qualifier.DIRECTORIES:
                qualifier.require_native_filesystem(mount, fs)
            row[args.phase + '_sha256'] = harness.verify_payload(mount, expected)
        if args.phase == 'prepare':
            subprocess.run(['/usr/bin/sync'], check=True)
            state['status'] = 'reboot_ready'
        else:
            for source, mount, backend, record in reversed(opened):
                cli('unmount', source, mount, backend, record, 'close')
            opened.clear()
            state['status'] = 'passed'
    except Exception as error:
        state.update(status='failed', error=str(error))
        for source, mount, backend, record in reversed(opened):
            if os.path.ismount(mount):
                try:
                    cli('unmount', source, mount, backend, record, 'cleanup')
                except Exception as cleanup:
                    state.setdefault('cleanup_errors', []).append(str(cleanup))
    qualifier.atomic(state_path, state)
    print(json.dumps({'status': state['status'], 'receipt': str(state_path), 'cases': len(state['cases'])}))
    return 0 if state['status'] in ('passed', 'reboot_ready') else 1


if __name__ == '__main__':
    raise SystemExit(main())
