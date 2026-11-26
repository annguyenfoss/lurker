#!/usr/bin/env python3
"""Debian-owned fresh-round driver; all storage stays under the supplied root.

Run on Debian only. The prefix must be an administrator-controlled private
installation. Existing installed Lurker and unrelated storage are never selected.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import time

FILESYSTEMS = ('ext4', 'btrfs', 'xfs', 'apfs', 'ufs1', 'ufs2', 'hammer2', 'ntfs', 'exfat', 'zfs')
BACKENDS = ('luks1', 'luks', 'veracrypt')
DIRECTORIES = ('gocryptfs', 'cryfs', 'cryptomator')


def atomic(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    os.replace(temporary, path)


def observe_features(mount, native=False):
    """Small probes, retaining observations rather than inferring from exit codes."""
    feature = mount / 'feature-check'
    feature.write_text('permissions and ACL probe\n')
    features = {'ownership': {'uid': feature.stat().st_uid, 'gid': feature.stat().st_gid}}
    for name, argv in [('permissions', ['chmod', '600', str(feature)]),
                       ('acl', ['setfacl', '-m', 'u:65534:r', str(feature)]),
                       ('reflink', ['cp', '--reflink=always', str(feature), str(mount / 'reflink-check')])]:
        observed = subprocess.run(argv, capture_output=True, text=True)
        features[name] = {'command': argv, 'yes': observed.returncode == 0,
                          'observation': observed.stderr.strip()}
        if name == 'permissions':
            features[name]['observed_mode'] = oct(feature.stat().st_mode & 0o7777)
            features[name]['yes'] &= (feature.stat().st_mode & 0o7777) == 0o600
    acl = subprocess.run(['getfacl', '-cp', str(feature)], capture_output=True, text=True)
    features['acl']['getfacl'] = acl.stdout.strip()
    features['journaling'] = {'status': 'unknown' if native else 'not-applicable',
                              'observation': 'Directory-engine layer' if not native else 'No journal-specific probe for this filesystem'}
    row = subprocess.check_output(['findmnt', '-rn', '-M', str(mount), '-o', 'SOURCE,FSTYPE,OPTIONS'], text=True).strip()
    features['mount'] = row
    parts = row.split()
    if native and len(parts) >= 2 and parts[1] in ('ext4', 'xfs'):
        argv = ['/usr/bin/sudo', '-n'] + (['/usr/sbin/tune2fs', '-l', parts[0]] if parts[1] == 'ext4' else ['/usr/sbin/xfs_info', str(mount)])
        observed = subprocess.run(argv, capture_output=True, text=True)
        lines = [line for line in observed.stdout.splitlines() if 'Filesystem features:' in line or 'log' in line]
        features['journaling'] = {'command': argv, 'status': 'yes' if observed.returncode == 0 and any('has_journal' in line or 'log      =' in line for line in lines) else 'unknown',
                                  'observation': '\n'.join(lines) or observed.stderr.strip()}
    return features


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--prefix', type=Path, required=True)
    parser.add_argument('--harness', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--mode', choices=('containers', 'directories', 'device'), required=True)
    parser.add_argument('--device', type=Path)
    parser.add_argument('--serial')
    parser.add_argument('--filesystems', default=','.join(FILESYSTEMS))
    parser.add_argument('--backends', help='Comma-separated subset for an affected-case retest')
    parser.add_argument('--attempt', default='')
    parser.add_argument('--archive-to', type=Path, help='Owned mounted storage for closed retained fixtures')
    parser.add_argument('--cipher', choices=('aes', 'serpent', 'twofish'), default='aes')
    parser.add_argument('--keyfile', action='store_true', help='Fresh fixture-owned binary keyfile credentials')
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    if os.uname().sysname != 'Linux' or 'debian' not in Path('/etc/os-release').read_text():
        parser.error('Debian native host required')
    if args.mode == 'device':
        if not args.device or not args.serial or not args.serial.startswith('lurker-fresh-debian'):
            parser.error('Device mode requires an identified round-specific scratch serial')
        observed = subprocess.check_output(['lsblk', '-dnro', 'SERIAL', str(args.device)], text=True).strip()
        if observed != args.serial or not args.device.is_block_device():
            parser.error('Scratch device identity mismatch')
    if args.mode == 'directories' and (args.keyfile or args.cipher != 'aes'):
        parser.error('Directory engines retain their fixed credential/profile selection')
    os.environ['PATH'] = '/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:' + os.environ.get('PATH', '')
    spec = importlib.util.spec_from_file_location('filesystem_harness', args.harness)
    harness = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(harness)
    binary = args.prefix / 'bin/lurker'
    payload = harness.load_payload(root / 'payload')
    receipts = root / ('device' if args.mode == 'device' else 'cases')
    receipts.mkdir(exist_ok=True)
    fixtures = root / 'fixtures'
    fixtures.mkdir(exist_ok=True)
    selected_fs = args.filesystems.split(',')
    selected_backends = args.backends.split(',') if args.backends else (DIRECTORIES if args.mode == 'directories' else BACKENDS)
    if not set(selected_fs) <= set(FILESYSTEMS) or not set(selected_backends) <= set(DIRECTORIES if args.mode == 'directories' else BACKENDS):
        parser.error('Selection must be a supported round filesystem/backend subset')
    if args.archive_to:
        args.archive_to = args.archive_to.resolve(strict=True)
        if not os.path.ismount(args.archive_to):
            parser.error('Archive destination must be an identified mounted storage root')
    for fs in selected_fs:
        for backend in selected_backends:
            label = f'{args.mode}-{fs}-{backend}'
            label += '-' + args.cipher if args.cipher != 'aes' else ''
            label += '-keyfile' if args.keyfile else ''
            label += '-' + args.attempt if args.attempt else ''
            result_path = receipts / (label + '.json')
            if result_path.exists():
                continue  # Never overwrite a failed receipt or reuse its fixture.
            fixture = fixtures / label
            fixture.mkdir(mode=0o700)
            mount = fixture / 'mount'
            mount.mkdir(mode=0o700)
            source = args.device if args.mode == 'device' else fixture / 'vault.img'
            password = 'Lurker9!' + secrets.token_urlsafe(24)
            credential = fixture / ('credential.key' if args.keyfile else 'credential.txt')
            descriptor = os.open(credential, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(descriptor, 'wb') as stream:
                stream.write(os.urandom(256) if args.keyfile else password.encode())
            result = {'host': 'debian', 'os': os.uname().sysname, 'kernel': os.uname().release,
                      'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip(),
                      'backend': backend, 'filesystem': fs, 'role': 'backing' if args.mode == 'directories' else 'inner',
                      'route': 'private-prefix-product', 'commit': args.commit,
                      'binary': str(binary), 'binary_sha256': harness.file_hash(binary),
                      'fixture': str(fixture), 'source_kind': 'device' if args.mode == 'device' else 'file',
                      'cipher': args.cipher, 'authentication': 'keyfile' if args.keyfile else 'password',
                      'private_fixture_credential': str(credential),
                      'tool_inventory': str(root / 'tools.json'),
                      'common_harness_sha256': harness.file_hash(args.harness),
                      'operations': [], 'status': 'running'}
            backing_mount = None
            backing_source = None
            backing_kind = 'luks1' if fs == 'ufs1' else 'luks'

            def command(action, operands, kind=backend):
                privileged = backend not in DIRECTORIES or kind in BACKENDS
                argv = (['/usr/bin/sudo', '-n'] if privileged else []) + [str(binary), action, '--type', kind, '--color=never'] + list(map(str, operands))
                if args.keyfile and action in ('create', 'mount'):
                    argv += ['--key-file', str(credential)]
                log = fixture / f'{len(result["operations"]):02d}-{action}.log'
                status = harness.terminal(argv, None if args.keyfile else password, 240, log)
                result['operations'].append({'action': action, 'log': str(log), **status})
                atomic(result_path, result)
                if status['exit_code'] or status['timed_out']:
                    raise RuntimeError(f'{action} failed: {log}')

            try:
                if args.mode == 'directories':
                    # A fresh LUKS2 container supplies a native, identified backing
                    # filesystem. Its receipt is distinct from the folder backend.
                    backing_source = fixture / 'backing.img'
                    backing_mount = fixture / 'backing'
                    backing_mount.mkdir(mode=0o700)
                    command('create', ['--filesystem', fs, backing_source, '0.5'], kind=backing_kind)
                    command('mount', [backing_source, backing_mount], kind=backing_kind)
                    if not os.path.ismount(backing_mount):
                        raise RuntimeError('Backing mount absent')
                    result['backing_features'] = observe_features(backing_mount, native=True)
                    source = backing_mount / 'encrypted'
                    command('create', [source])
                else:
                    operands = ['--filesystem', fs, '--cipher', args.cipher]
                    if args.mode == 'device':
                        operands += ['--force', source]
                    else:
                        operands += [source, '0.5']
                    command('create', operands)
                expected = dict(payload)
                for phase in ('write', 'append', 'verify'):
                    # Explicit create/type followed by Auto discovery on reopen.
                    command('mount', [source, mount], kind=backend if phase == 'write' else 'auto')
                    if not os.path.ismount(mount):
                        raise RuntimeError('Mount absent; refusing underlay access')
                    result.setdefault('mounts', []).append(subprocess.check_output(['findmnt', '-rn', '-M', str(mount), '-o', 'SOURCE,FSTYPE,OPTIONS'], text=True).strip())
                    if phase == 'write':
                        result['root_metadata'] = {'uid': mount.stat().st_uid, 'gid': mount.stat().st_gid, 'mode': oct(mount.stat().st_mode & 0o7777)}
                        directory = mount / 'Lurker payload'
                        directory.mkdir(mode=0o700)
                        for name, contents in expected.items():
                            harness.payload_write(directory / name, contents)
                        result['features'] = observe_features(mount, native=backend not in DIRECTORIES)
                    else:
                        harness.verify_payload(mount, expected)
                        if phase == 'append':
                            addition = b'\nFresh Debian second-mount append.\n'
                            harness.payload_write(mount / 'Lurker payload/page.txt', addition, append=True)
                            expected['page.txt'] += addition
                            expected['second.txt'] = b'Fresh Debian second-mount new file.\n'
                            harness.payload_write(mount / 'Lurker payload/second.txt', expected['second.txt'])
                    result[phase + '_sha256'] = harness.verify_payload(mount, expected)
                    command('unmount', [mount])
                    if os.path.ismount(mount):
                        raise RuntimeError('Normal unmount left mount active')
                result['status'] = 'passed'
            except Exception as error:
                result.update(status='failed', error=str(error))
            finally:
                for target, kind in [(mount, backend), (backing_mount, backing_kind)]:
                    if target is not None and os.path.ismount(target):
                        try:
                            command('unmount', [target], kind=kind)
                        except Exception as error:
                            result.setdefault('cleanup_errors', []).append(str(error))
                result['mount_remaining'] = os.path.ismount(mount) or bool(backing_mount and os.path.ismount(backing_mount))
                result['finished'] = time.time()
                atomic(result_path, result)
                loops = subprocess.run(['/usr/bin/sudo', '-n', '/sbin/losetup', '--list', '--json', '--output', 'BACK-FILE'], capture_output=True, text=True)
                result['owned_loop_remaining'] = loops.returncode != 0 or str(fixture) in loops.stdout
                atomic(result_path, result)
                if args.archive_to and not result['mount_remaining'] and not result['owned_loop_remaining']:
                    # Keep the original evidence paths valid. Mounted or failed
                    # teardown fixtures stay in place for investigation.
                    retained = args.archive_to / 'retained'
                    retained.mkdir(exist_ok=True)
                    target = retained / fixture.name
                    shutil.move(str(fixture), str(target))
                    fixture.symlink_to(target, target_is_directory=True)
                print(json.dumps({'case': label, 'status': result['status'], 'receipt': str(result_path), 'mount_remaining': result['mount_remaining']}), flush=True)
            if args.mode == 'device' and (result['mount_remaining'] or result['owned_loop_remaining']):
                return 1


if __name__ == '__main__':
    raise SystemExit(main())
