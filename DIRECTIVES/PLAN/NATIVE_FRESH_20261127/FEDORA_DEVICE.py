#!/usr/bin/env python3
"""Destructive product cases fenced to the fresh Fedora scratch serial."""
import argparse, hashlib, importlib.util, itertools, json, os, secrets, shutil, subprocess, time
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for arg in ('binary', 'harness', 'payload', 'root', 'device'):
        p.add_argument('--' + arg, type=Path, required=True)
    p.add_argument('--commit', required=True)
    p.add_argument('--filesystems', default='ext4,btrfs,xfs,apfs,ufs1,ufs2,hammer2,ntfs,exfat,zfs')
    p.add_argument('--backends', default='luks,luks1,veracrypt')
    p.add_argument('--ciphers', default='aes')
    p.add_argument('--credential', choices=('password', 'keyfile'), default='password')
    a = p.parse_args()
    if os.geteuid() != 0 or os.uname().nodename != 'fedora-core-01':
        p.error('Run with sudo on fedora-core-01')
    if set(a.backends.split(',')) - {'luks', 'luks1', 'veracrypt'}: p.error('Unknown backend')
    if set(a.ciphers.split(',')) - {'aes', 'serpent', 'twofish'}: p.error('Unknown cipher')
    os.umask(0o077)
    supplied_device = str(a.device)
    a.device = a.device.resolve(strict=True)
    identity = subprocess.check_output(['lsblk', '-dn', '-o', 'SERIAL,SIZE,TYPE', str(a.device)], text=True).strip()
    if identity.split()[0] != 'lurker-fresh-fedora' or identity.split()[-1] != 'disk':
        p.error('Scratch disk serial/type mismatch')
    device_number = a.device.stat().st_rdev
    spec = importlib.util.spec_from_file_location('h', a.harness)
    h = importlib.util.module_from_spec(spec); spec.loader.exec_module(h)
    a.root.mkdir(mode=0o700, parents=True, exist_ok=True)
    expected = {name: (a.payload / name).read_bytes() for name in
                ('bitcoin.pdf', 'ethereum.pdf', 'transformer.pdf', 'page.txt')}
    result = dict(host=os.uname().nodename, kernel=os.uname().release, commit=a.commit,
        binary=str(a.binary), binary_sha256=h.file_hash(a.binary), device=str(a.device),
        supplied_device=supplied_device, identity=identity, cases=[], status='running', pid=os.getpid())
    receipt = a.root / 'device.json'
    def save(): h.atomic_json(receipt, result)
    stop = False
    for fs, backend, cipher in itertools.product(a.filesystems.split(','), a.backends.split(','), a.ciphers.split(',')):
        d = a.root / (fs + '-' + backend + '-' + cipher + '-' + a.credential)
        d.mkdir(mode=0o700); mountpoint = d / 'mount'; mountpoint.mkdir(mode=0o700)
        row = dict(filesystem=fs, backend=backend, cipher=cipher, credential=a.credential,
                   role='inner', source_kind='device', status='running', operations=[])
        result['cases'].append(row); save()
        if stop:
            row.update(status='blocked', error='Prior device failure requires recovery or snapshot capacity'); save(); continue
        password = 'Lurker9!' + secrets.token_urlsafe(24)
        keyfile = None
        header_before = None
        credential = d / '.generated-test-credential'
        if a.credential == 'keyfile':
            keyfile = d / 'generated-test-key.bin'
            with keyfile.open('xb') as stream: stream.write(b'\0\xff\n\r' + secrets.token_bytes(252))
            row['keyfile_sha256'] = h.file_hash(keyfile)
        else:
            with credential.open('x') as stream: stream.write(password)
        def cli(action, operands, kind=None):
            log = d / (str(len(row['operations'])) + '-' + action + '.log')
            argv = [str(a.binary), action, '--type', kind or backend, '--color=never']
            if action == 'create': argv += ['--cipher', cipher]
            if action in {'create', 'mount'} and keyfile: argv += ['--key-file', str(keyfile)]
            argv += list(map(str, operands))
            r = h.terminal(argv, None if keyfile else password, 240, log)
            row['operations'].append(dict(action=action, log=str(log), **r)); save()
            if r['exit_code'] or r['timed_out']: raise RuntimeError('CLI ' + action + ' failed')
        try:
            current = subprocess.check_output(['lsblk', '-dn', '-o', 'SERIAL', str(a.device)], text=True).strip()
            if current != 'lurker-fresh-fedora' or a.device.stat().st_rdev != device_number:
                raise RuntimeError('Scratch identity changed')
            if subprocess.run(['findmnt', '-rn', '-S', str(a.device)], capture_output=True).returncode == 0:
                raise RuntimeError('Scratch device still mounted')
            children = subprocess.check_output(['lsblk', '-nr', '-o', 'NAME', str(a.device)], text=True).splitlines()
            if len(children) != 1: raise RuntimeError('Scratch device has children; refusing overwrite')
            with a.device.open('rb', buffering=0) as stream:
                header_before = hashlib.sha256(stream.read(1024**2)).hexdigest()
            row['header_before_sha256'] = header_before
            cli('create', ['--filesystem', fs, a.device, '--force'])
            wanted = dict(expected)
            for phase in ('write', 'append', 'verify'):
                cli('mount', [a.device, mountpoint], 'auto')
                if not os.path.ismount(mountpoint): raise RuntimeError('OS mount missing; refusing underlay writes')
                view = subprocess.check_output(['findmnt', '-rn', '-M', str(mountpoint), '-o', 'SOURCE,FSTYPE,OPTIONS'], text=True).strip()
                row.setdefault('mounts', []).append(view)
                if 'rw' not in view.split()[-1].split(','): raise RuntimeError('Mount is not read/write')
                target = mountpoint / 'payload'
                if phase == 'write':
                    target.mkdir(mode=0o700)
                    for name, data in wanted.items():
                        with (target / name).open('xb') as stream:
                            stream.write(data); stream.flush(); os.fsync(stream.fileno())
                for name, data in wanted.items():
                    if (target / name).read_bytes() != data: raise RuntimeError('Payload mismatch ' + name)
                if phase == 'append':
                    wanted['page.txt'] += b'\nFedora device reopen append.\n'
                    wanted['second.txt'] = b'New file on reopened device.\n'
                    for name in ('page.txt', 'second.txt'):
                        with (target / name).open('wb') as stream:
                            stream.write(wanted[name]); stream.flush(); os.fsync(stream.fileno())
                row[phase + '_hashes'] = {name: hashlib.sha256((target / name).read_bytes()).hexdigest() for name in wanted}
                cli('unmount', [mountpoint], 'auto')
            row['status'] = 'passed'
        except Exception as error:
            row.update(status='failed', error=str(error))
            if 'Scratch identity' in str(error) or 'Scratch device' in str(error): stop = True
        finally:
            if os.path.ismount(mountpoint):
                try: cli('unmount', [mountpoint], 'auto')
                except Exception as error: row['cleanup_error'] = str(error)
            row['mount_remaining'] = os.path.ismount(mountpoint)
            header_changed = False
            if header_before is not None:
                with a.device.open('rb', buffering=0) as stream:
                    row['header_after_sha256'] = hashlib.sha256(stream.read(1024**2)).hexdigest()
                header_changed = header_before != row['header_after_sha256']
            destructive_started = header_changed or any(x['action'] == 'create' and x['exit_code'] == 0
                                                       for x in row['operations'])
            row['destructive_started'] = destructive_started
            if row['status'] == 'failed' and destructive_started and not row['mount_remaining']:
                try:
                    size = int(subprocess.check_output(['blockdev', '--getsize64', str(a.device)], text=True))
                    if shutil.disk_usage(d).free < size + 512 * 1024**2: raise RuntimeError('Insufficient space to retain failed device')
                    snapshot = d / 'failed-device.img'
                    with a.device.open('rb', buffering=0) as source, snapshot.open('xb', buffering=0) as target:
                        shutil.copyfileobj(source, target, 4 * 1024**2); os.fsync(target.fileno())
                    row['retained_failed_ciphertext'] = str(snapshot)
                except Exception as error: row['snapshot_error'] = str(error); stop = True
            if row['mount_remaining']: stop = True
            save()
    result['status'] = 'terminal'; result['finished'] = time.time(); save()
    print(json.dumps(dict(receipt=str(receipt), counts={s: sum(c['status'] == s for c in result['cases'])
        for s in ('passed', 'failed', 'blocked')})))

if __name__ == '__main__': main()
