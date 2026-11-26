#!/usr/bin/env python3
"""Fresh host-filesystem no-clobber and ordinary file/directory rename controls."""
import argparse, ctypes, errno, hashlib, json, os, subprocess, tempfile, time
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--filesystems', nargs='+', choices=('exfat', 'ufs2'), default=['ufs2'])
    p.add_argument('--receipt-name', default='rename-control-enhanced.json')
    a = p.parse_args()
    if os.uname().nodename != 'fedora-core-01' or os.geteuid() == 0:
        p.error('Run as the Fedora login user; only native mounts use sudo')
    if Path(a.receipt_name).name != a.receipt_name: p.error('Receipt must be a basename')
    root = Path.home() / '.cache/lurker/native-fresh-20261127'
    rows = []
    libc = ctypes.CDLL(None, use_errno=True)
    rename = libc.renameat2
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    for fs in a.filesystems:
        d = Path(tempfile.mkdtemp(prefix='rename-' + fs + '-', dir=root))
        image = d / 'native.img'; mount = d / 'mount'; mount.mkdir(); seed = d / 'seed'; seed.mkdir()
        with image.open('xb') as stream: stream.truncate(128 * 1024**2)
        row = dict(filesystem=fs, fixture=str(d), commands=[], renames=[], status='running')
        rows.append(row)
        def run(argv):
            r = subprocess.run(list(map(str, argv)), capture_output=True, text=True, timeout=180)
            row['commands'].append(dict(argv=list(map(str, argv)), code=r.returncode,
                                       stdout=r.stdout[-3000:], stderr=r.stderr[-3000:]))
            if r.returncode: raise RuntimeError(str(argv[0]) + ' failed')
        def move(source, target, flags=1, expected=0):
            ctypes.set_errno(0)
            code = rename(-100, os.fsencode(source), -100, os.fsencode(target), flags)
            error = ctypes.get_errno() if code < 0 else 0
            row['renames'].append(dict(source=str(source), target=str(target), flags=flags,
                                       code=code, errno=error, expected_errno=expected))
            if (code == 0) != (expected == 0) or error != expected:
                raise RuntimeError('renameat2 result differs from expected atomic operation')
        try:
            if fs == 'exfat':
                run(['mkfs.exfat', image])
                run(['sudo', '-n', 'mount', '-o', f'loop,uid={os.getuid()},gid={os.getgid()},umask=0077', image, mount])
            else:
                run(['makefs-freebsd', '-t', 'ffs', '-B', 'little', '-o',
                     'version=2,bsize=32768,fsize=4096', '-s', str(image.stat().st_size), image, seed])
                driver = Path('/usr/local/bin/fuse-ufs')
                row['driver_sha256'] = hashlib.sha256(driver.read_bytes()).hexdigest()
                run(['sudo', '-n', driver, '-o', 'rw,allow_other,fsname=' + str(image), image, mount])
                run(['sudo', '-n', 'chown', f'{os.getuid()}:{os.getgid()}', mount])
            for _ in range(100):
                if os.path.ismount(mount): break
                time.sleep(.02)
            if not os.path.ismount(mount): raise RuntimeError('OS mount missing')
            run(['findmnt', '-rn', '-M', mount, '-o', 'SOURCE,FSTYPE,OPTIONS'])
            source = mount / 'source'; target = mount / 'target'
            source.write_bytes(b'original-source'); target.write_bytes(b'original-target')
            move(source, target, expected=errno.EEXIST)
            if source.read_bytes() != b'original-source' or target.read_bytes() != b'original-target':
                raise RuntimeError('No-clobber collision changed either file')
            published = mount / 'published'
            move(source, published)
            if source.exists() or published.read_bytes() != b'original-source': raise RuntimeError('Publication failed')
            move(published, target, flags=0)
            if published.exists() or target.read_bytes() != b'original-source': raise RuntimeError('Ordinary replacement failed')
            left = mount / 'left'; right = mount / 'right'; left.mkdir(); right.mkdir()
            (left / 'file').write_bytes(b'cross-parent-file')
            move(left / 'file', right / 'file')
            if (right / 'file').read_bytes() != b'cross-parent-file': raise RuntimeError('Cross-parent file rename failed')
            directory = mount / 'directory'; directory.mkdir(); (directory / 'leaf').write_bytes(b'directory-leaf')
            moved = mount / 'directory-moved'; move(directory, moved)
            occupied = mount / 'occupied'; occupied.mkdir(); (occupied / 'other').write_bytes(b'untouched')
            move(moved, occupied, expected=errno.EEXIST)
            if (moved / 'leaf').read_bytes() != b'directory-leaf' or (occupied / 'other').read_bytes() != b'untouched':
                raise RuntimeError('Directory collision changed original directories')
            ordinary = mount / 'directory-ordinary'; move(moved, ordinary, flags=0)
            if (ordinary / 'leaf').read_bytes() != b'directory-leaf': raise RuntimeError('Ordinary directory rename failed')
            run(['sync', '-f', mount]); row['status'] = 'passed'
        except Exception as e: row.update(status='failed', error=str(e))
        finally:
            if os.path.ismount(mount):
                try: run(['sudo', '-n', 'umount', mount])
                except Exception as e: row['cleanup_error'] = str(e)
            row['mount_remaining'] = os.path.ismount(mount)
    receipt = root / 'logs' / a.receipt_name
    receipt.write_text(json.dumps(rows, indent=2) + '\n')
    print(json.dumps(dict(receipt=str(receipt), cases=[dict(filesystem=r['filesystem'], status=r['status']) for r in rows])))

if __name__ == '__main__': main()
