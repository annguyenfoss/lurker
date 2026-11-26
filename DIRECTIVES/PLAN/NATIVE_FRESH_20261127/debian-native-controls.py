#!/usr/bin/env python3
"""Fresh Debian native namespace/payload controls, independent of Lurker create.

These are native-tool controls, never product passes. Four new regular-file
fixtures distinguish driver capability from encryption-engine integration.
"""
import argparse
import ctypes
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', required=True, type=Path)
    p.add_argument('--harness', required=True, type=Path)
    p.add_argument('--qualifier', required=True, type=Path)
    p.add_argument('--driver', choices=('ntfs-3g', 'ntfs3', 'exfat', 'fuse-ufs'), action='append')
    p.add_argument('--attempt', default='')
    args = p.parse_args()
    root = args.root.resolve(strict=True)
    harness = load('payload', args.harness)
    qualifier = load('qualifier', args.qualifier)
    os.environ['PATH'] = '/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin'
    controls = root / 'native-controls'
    controls.mkdir(mode=0o700, exist_ok=True)
    libc = ctypes.CDLL(None, use_errno=True)
    rename = libc.renameat2
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    for fs, driver in [('ntfs', 'ntfs-3g'), ('ntfs', 'ntfs3'), ('exfat', 'exfat'), ('ufs2', 'fuse-ufs')]:
        if args.driver and driver not in args.driver:
            continue
        fixture = controls / (fs + '-' + driver + ('-' + args.attempt if args.attempt else ''))
        fixture.mkdir(mode=0o700)
        image, mount = fixture / 'backing.img', fixture / 'mount'
        mount.mkdir(mode=0o700)
        with image.open('xb') as stream:
            stream.truncate(512 * 1024**2)
        result = {'host': 'debian', 'kernel': os.uname().release, 'route': 'native-tool-control',
                  'filesystem': fs, 'driver': driver, 'source_kind': 'file', 'source': str(image),
                  'fixture': str(fixture), 'commands': [], 'status': 'running'}
        result['tool_versions'] = json.loads((root / 'tools.json').read_text())
        if driver == 'fuse-ufs':
            result['driver_sha256'] = harness.file_hash(Path('/usr/local/bin/fuse-ufs'))
        receipt = fixture / 'result.json'

        def command(argv):
            observed = subprocess.run(list(map(str, argv)), capture_output=True, text=True, timeout=120)
            log = fixture / f'{len(result["commands"]):02d}-command.log'
            log.write_text(observed.stdout + observed.stderr)
            result['commands'].append({'argv': list(map(str, argv)), 'code': observed.returncode, 'log': str(log)})
            qualifier.atomic(receipt, result)
            if observed.returncode:
                raise RuntimeError('Native command failed; see ' + str(log))

        def open_mount():
            if driver == 'fuse-ufs':
                command(['/usr/bin/sudo', '-n', '/usr/local/bin/fuse-ufs', '-o', f'rw,allow_other,fsname={image}', image, mount])
            else:
                command(['/usr/bin/sudo', '-n', '/usr/bin/mount', '-t', driver, '-o', f'loop,rw,uid={os.getuid()},gid={os.getgid()},umask=0077', image, mount])
            deadline = time.monotonic() + 5
            while not os.path.ismount(mount) and time.monotonic() < deadline:
                time.sleep(0.025)
            result.setdefault('mounts', []).append(qualifier.require_native_filesystem(mount, fs))

        try:
            if fs == 'ufs2':
                seed = fixture / 'seed'
                seed.mkdir(mode=0o700)
                command(['/usr/local/bin/makefs-freebsd', '-t', 'ffs', '-B', 'little', '-o', 'version=2,bsize=32768,fsize=4096', '-s', str(512 * 1024**2), image, seed])
            else:
                command((['/usr/sbin/mkntfs', '-Q', '-F'] if fs == 'ntfs' else ['/usr/sbin/mkfs.exfat']) + [image])
            expected = harness.load_payload(root / 'payload')
            for phase in ('write', 'append', 'verify'):
                open_mount()
                if phase == 'write':
                    directory = mount / 'Lurker payload'
                    directory.mkdir(mode=0o700)
                    for name, contents in expected.items():
                        harness.payload_write(directory / name, contents)
                    observations = {}
                    a, b = mount / 'link-source', mount / 'link-destination'
                    a.write_bytes(b'namespace probe\n')
                    try:
                        os.link(a, b)
                        observations['hardlink'] = {'code': 0}
                    except OSError as error:
                        observations['hardlink'] = {'errno': error.errno, 'error': str(error)}
                    for kind in ('file', 'directory'):
                        source, destination = mount / (kind + '-source'), mount / (kind + '-destination')
                        source.mkdir() if kind == 'directory' else source.write_bytes(b'atomic no-clobber probe\n')
                        ctypes.set_errno(0)
                        code = rename(-100, os.fsencode(source), -100, os.fsencode(destination), 1)
                        observations['renameat2_noreplace_' + kind] = {'code': code, 'errno': ctypes.get_errno()}
                    fd = os.open(mount, os.O_RDONLY | os.O_DIRECTORY)
                    try:
                        try:
                            os.fsync(fd)
                            observations['directory_fsync'] = {'code': 0}
                        except OSError as error:
                            observations['directory_fsync'] = {'errno': error.errno, 'error': str(error)}
                    finally:
                        os.close(fd)
                    result['namespace'] = observations
                    result['features'] = qualifier.observe_features(mount, native=True)
                else:
                    harness.verify_payload(mount, expected)
                    if phase == 'append':
                        addition = b'\nFresh Debian second-mount append.\n'
                        harness.payload_write(mount / 'Lurker payload/page.txt', addition, append=True)
                        expected['page.txt'] += addition
                        expected['second.txt'] = b'Fresh Debian second-mount new file.\n'
                        harness.payload_write(mount / 'Lurker payload/second.txt', expected['second.txt'])
                result[phase + '_sha256'] = harness.verify_payload(mount, expected)
                command(['/usr/bin/sudo', '-n', '/usr/bin/umount', mount])
            result['status'] = 'passed'
        except Exception as error:
            result.update(status='failed', error=str(error))
        finally:
            if os.path.ismount(mount):
                try:
                    command(['/usr/bin/sudo', '-n', '/usr/bin/umount', mount])
                except Exception as error:
                    result['cleanup_error'] = str(error)
            result['mount_remaining'] = os.path.ismount(mount)
            qualifier.atomic(receipt, result)
        print(json.dumps({'filesystem': fs, 'driver': driver, 'status': result['status'],
                          'namespace': result.get('namespace'), 'receipt': str(receipt)}), flush=True)


if __name__ == '__main__':
    main()
