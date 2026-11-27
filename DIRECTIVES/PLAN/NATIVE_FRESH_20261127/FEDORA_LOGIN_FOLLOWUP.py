#!/usr/bin/env python3
"""Retain six Fedora normal-user disk-method lifecycles for the reboot cohort."""
import argparse, importlib.util, json, os, subprocess, time
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'harness', 'root', 'payload'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--commit', required=True)
    a = p.parse_args()
    if os.uname().nodename != 'fedora-core-01' or os.geteuid() != 1300:
        p.error('Run as the normal Fedora login user')
    if a.binary.stat().st_uid != 0 or a.binary.stat().st_mode & 0o022:
        p.error('The installed binary must be administrator-controlled')
    a.root.mkdir(mode=0o700, parents=True, exist_ok=False)
    spec = importlib.util.spec_from_file_location('h', a.harness.with_name('test-filesystem-cli.py'))
    h = importlib.util.module_from_spec(spec); spec.loader.exec_module(h)
    result = dict(commit=a.commit, binary_sha256=h.file_hash(a.binary),
                  harness_sha256=h.file_hash(a.harness), script_sha256=h.file_hash(Path(__file__)),
                  uid=os.geteuid(), pid=os.getpid(), session=os.getsid(0),
                  started=time.time(), status='running', cases=[])
    def save(): h.atomic_json(a.root / 'matrix.json', result)
    save()
    for credential in ('password', 'keyfile'):
        for backend in ('luks', 'luks1', 'veracrypt'):
            name = backend + '-' + credential
            command = ['python3', str(a.harness), '--binary', str(a.binary),
                       '--backend', backend, '--filesystem', 'btrfs', '--credential', credential,
                       '--auto-detect', '--commit', a.commit, '--root', str(a.root),
                       '--size-gb', '0.25', '--payload-dir', str(a.payload), '--timeout', '120']
            row = dict(backend=backend, filesystem='btrfs', route='login-user',
                       credential=credential, status='running')
            result['cases'].append(row); save()
            process = subprocess.run(command, capture_output=True, text=True, timeout=900)
            (a.root / (name + '.stdout')).write_text(process.stdout)
            (a.root / (name + '.stderr')).write_text(process.stderr)
            row['code'] = process.returncode
            try:
                summary = json.loads(process.stdout.splitlines()[-1])
                receipt = json.loads(Path(summary['receipt']).read_text())
                assert receipt['binary_sha256'] == result['binary_sha256']
                assert receipt['commit'] == a.commit and receipt['credential'] == credential
                assert receipt['status'] == 'passed' and not receipt['mount_remaining']
                assert len(receipt['cases']) == 7 and process.returncode == 0
                row.update(receipt=receipt, status='passed', retained_for_reboot=True,
                           terminal_session_survived=os.getsid(0) == result['session'])
                assert row['terminal_session_survived']
            except Exception as error:
                row.update(status='failed', error=str(error))
            save()
            if not row.get('receipt') or row['receipt'].get('mount_remaining'):
                raise RuntimeError('Retain live failed fixture before admitting another case')
    result.update(status='passed' if all(c['status'] == 'passed' for c in result['cases']) else 'failed', finished=time.time())
    save()
    print(json.dumps(dict(status=result['status'], receipt=str(a.root / 'matrix.json'))))
    return int(result['status'] != 'passed')


if __name__ == '__main__': raise SystemExit(main())
