#!/usr/bin/env python3
"""Force multi-page UFS2 directory enumeration through a root-owned host driver."""
import argparse, hashlib, importlib.util, json, os, subprocess, tempfile, time
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--driver', type=Path, required=True)
    p.add_argument('--harness', type=Path, required=True)
    p.add_argument('--root', type=Path, required=True)
    p.add_argument('--expect-truncation', action='store_true')
    a = p.parse_args()
    if os.geteuid() != 0 or os.uname().nodename != 'fedora-core-01': p.error('Run as root on Fedora only')
    driver = a.driver.resolve(strict=True)
    for path in [driver, *driver.parents]:
        s = path.stat()
        if s.st_uid != 0 or s.st_mode & 0o022: p.error('Host driver is not administrator-controlled')
    spec = importlib.util.spec_from_file_location('h', a.harness)
    h = importlib.util.module_from_spec(spec); spec.loader.exec_module(h)
    a.root.mkdir(mode=0o700, exist_ok=False)
    seed = a.root / 'seed'; seed.mkdir()
    names = ['entry-' + str(i).zfill(4) + '-' + 'x' * 120 for i in range(512)]
    for name in names: (seed / name).write_bytes(b'Native directory pagination control.\n')
    image = a.root / 'native.img'
    with image.open('xb') as stream: stream.truncate(128 * 1024**2)
    subprocess.run(['makefs-freebsd', '-t', 'ffs', '-B', 'little', '-o', 'version=2,bsize=32768,fsize=4096', '-s', str(image.stat().st_size), image, seed], check=True, capture_output=True)
    mount = a.root / 'mount'; mount.mkdir()
    result = dict(status='running', driver_sha256=h.file_hash(driver), expected_names=len(names), expected_truncation=a.expect_truncation)
    log = (a.root / 'driver.log').open('w')
    process = subprocess.Popen([str(driver), '-f', '-o', 'rw,allow_other,fsname=' + str(image), str(image), str(mount)], stdout=log, stderr=subprocess.STDOUT)
    def probe():
        if not os.path.ismount(mount): raise RuntimeError('Refuse underlay access')
        before = {entry.name for entry in os.scandir(mount)}
        added = 'zz-newly-created-after-first-read-' + 'y' * 120
        with (mount / added).open('xb') as stream:
            stream.write(b'New entry must be enumerated after existing pages.\n'); stream.flush(); os.fsync(stream.fileno())
        fd = os.open(mount, os.O_RDONLY | os.O_DIRECTORY)
        try: os.fsync(fd)
        finally: os.close(fd)
        after = {entry.name for entry in os.scandir(mount)}
        again = {entry.name for entry in os.scandir(mount)}
        return dict(before_count=len(before), after_count=len(after), missing_before_count=len(set(names)-before), missing_before_sample=sorted(set(names)-before)[:4],
                    missing_after_count=len(set(names)-after), missing_after_sample=sorted(set(names)-after)[:4], added_stat_inode=(mount / added).stat().st_ino,
                    added_visible=added in after, repeated_enumeration_equal=after == again)
    try:
        for _ in range(200):
            if os.path.ismount(mount): break
            if process.poll() is not None: raise RuntimeError('Driver exited before mount')
            time.sleep(.025)
        result.update(h.bounded_payload_call(probe, 60))
        correct = result['before_count'] == 512 and result['after_count'] == 513 and not result['missing_after_count'] and result['added_visible'] and result['repeated_enumeration_equal']
        result['status'] = 'expected-truncation-observed' if a.expect_truncation and not correct else 'passed' if correct and not a.expect_truncation else 'failed'
    except Exception as error:
        result.update(status='failed', error=str(error))
        if hasattr(error, 'worker'): result['payload_worker'] = error.worker
    finally:
        if not result.get('payload_worker', {}).get('remaining') and os.path.ismount(mount):
            c = subprocess.run(['umount', mount], capture_output=True, text=True, timeout=30); result['unmount_exit'] = c.returncode
        result['mount_remaining'] = os.path.ismount(mount)
        if not result['mount_remaining']:
            try: result['driver_exit'] = process.wait(timeout=10)
            except subprocess.TimeoutExpired: result['cleanup_error'] = 'Owned driver remains'
        log.close(); h.atomic_json(a.root / 'result.json', result)
    print(json.dumps(dict(status=result['status'], receipt=str(a.root / 'result.json'), before_count=result.get('before_count'), after_count=result.get('after_count'), added_visible=result.get('added_visible'))))
    return int(result['status'] not in {'passed', 'expected-truncation-observed'} or result['mount_remaining'])


if __name__ == '__main__': raise SystemExit(main())
