#!/usr/bin/env python3
"""Optional NTFS/XFS interoperability tests. Uses only new disposable files.

Requires mkntfs/mkfs.xfs on PATH, or --formatters <locally extracted package root>.
No package is installed and no administrator command is used.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binaries', type=Path, default=Path('target/release'))
parser.add_argument('--formatters', type=Path)
args = parser.parse_args()
if os.geteuid() == 0:
    parser.error('Run as your normal user, without sudo.')
binaries = args.binaries.resolve()
tools = binaries / 'userspace'
helper = binaries / 'lurker-helper'
crypto = tools / 'lurker-veracrypt'
filesystem_driver = tools / 'lurker-fs'
env = os.environ.copy()
env.update(LC_ALL='C.UTF-8', LURKER_USERSPACE_DIR=str(tools))
if args.formatters:
    formatters = args.formatters.resolve()
    env['PATH'] = str(formatters / 'usr/bin') + os.pathsep + env.get('PATH', '')
    env['LD_LIBRARY_PATH'] = str(formatters / 'usr/lib') + os.pathsep + env.get('LD_LIBRARY_PATH', '')
for name in ['mkntfs', 'mkfs.xfs']:
    if not shutil.which(name, path=env['PATH']):
        parser.error(f'{name} is required for this optional suite.')
secret = '  Disposable native-filesystem password 密碼  '


def run(command, data=None):
    result = subprocess.run(list(map(str, command)), input=data, env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
    assert result.returncode == 0, (command, result.stdout.decode(errors='replace'), result.stderr.decode(errors='replace'))
    return result


def request(action, **fields):
    payload = dict(action=action, privilege_mode='unprivileged', volume_type='veracrypt', **fields)
    result = run([helper, 'run'], json.dumps(payload).encode())
    assert secret not in result.stdout.decode()
    response = json.loads(result.stdout)
    assert response['ok'], response


for kind in ['ntfs3', 'xfs']:
    directory = tempfile.mkdtemp(prefix='lurker-userspace-extra-')
    root = Path(directory)
    runtime = root / 'runtime'; runtime.mkdir(mode=0o700)
    native = root / 'native'; native.mkdir(mode=0o700)
    mountpoint = root / 'mounted'; mountpoint.mkdir(mode=0o700)
    source = root / f'{kind} file 密碼.hc'
    raw_dir = native / '.veracrypt_aux_mnt1'
    raw = raw_dir / 'volume'
    ready = root / 'ready'
    env.update(XDG_RUNTIME_DIR=str(runtime), TMPDIR=str(native))
    child = None
    try:
        run([crypto, '-t', '--create', '--force', '--volume-type=normal', '--hash=SHA-512',
             '--filesystem=none', '--encryption=AES', '--random-source=/dev/urandom', '--pim=0',
             '-k', '', '--stdin', '--non-interactive', '--size=512M', source], (secret + '\n').encode())
        run([crypto, '-t', '--mount', '--filesystem=none', '--mount-options=nokernelcrypto',
             '--pim=0', '-k', '', '--protect-hidden=no', '--stdin', '--non-interactive', source], (secret + '\n').encode())
        if kind == 'ntfs3':
            run(['mkntfs', '-Q', '-F', raw])
        else:
            run(['mkfs.xfs', '-f', '-m', 'metadir=0', raw])
            # Initialize the fresh filesystem's root ownership through the same
            # non-root driver used during Lurker create, without editing old data.
            with (root / 'driver.log').open('wb') as log:
                child = subprocess.Popen([str(filesystem_driver), '-f', '-s', '-o', 'type=xfs,mb=192', str(raw), str(mountpoint)],
                                         env=env | {'LURKER_INIT_OWNER': '1', 'LURKER_READY_FILE': str(ready)},
                                         stdout=log, stderr=subprocess.STDOUT)
                deadline = time.monotonic() + 45
                while not ready.exists():
                    assert child.poll() is None and time.monotonic() < deadline, (root / 'driver.log').read_text()
                    time.sleep(.05)
                run(['fusermount3', '-u', mountpoint])
                assert child.wait(timeout=30) == 0
                child = None
        raw.open('rb').close()
        run(['fusermount3', '-u', raw_dir])
        raw_dir.rmdir()
        request('mount', source=str(source), mountpoint=str(mountpoint), tag=None, readonly=False, passphrase=secret)
        data = mountpoint / 'persistent.txt'
        data.write_text(f'userspace {kind} survived remount')
        assert data.stat().st_uid == os.getuid()
        with data.open('rb') as file:
            os.fsync(file.fileno())
        request('unmount', target=str(source), tag=None)
        request('mount', source=str(source), mountpoint=str(mountpoint), tag=None, readonly=True, passphrase=secret)
        assert data.read_text() == f'userspace {kind} survived remount'
        try:
            (mountpoint / 'blocked').write_text('no')
        except OSError:
            pass
        else:
            raise AssertionError('read-only filesystem accepted a write')
        request('unmount', target=str(source), tag=None)
        print(f'PASS {kind}: standard VeraCrypt, user-owned writes, fsync, persistence, readonly, helper JSON, no elevation', flush=True)
    finally:
        if os.path.ismount(mountpoint):
            # A core-managed mount must drain through the helper before crypto.
            try:
                request('unmount', target=str(source), tag=None)
            except Exception:
                run(['fusermount3', '-u', mountpoint])
        if child:
            child.wait(timeout=30)
        if os.path.ismount(raw_dir):
            run(['fusermount3', '-u', raw_dir])
        # Never recursively delete a fixture containing any surviving mount,
        # including a core-managed crypto image below its private runtime.
        live = [line.split()[4] for line in Path('/proc/self/mountinfo').read_text().splitlines()
                if line.split()[4] == str(root) or line.split()[4].startswith(str(root) + '/')]
        assert not live, f'Retaining live fixture {root}: {live}'
        shutil.rmtree(root)
