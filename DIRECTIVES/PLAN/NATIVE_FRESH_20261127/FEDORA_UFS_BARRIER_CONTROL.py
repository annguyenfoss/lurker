#!/usr/bin/env python3
"""Trace scoped UFS2 backing-file barriers and native EIO propagation."""
import errno, hashlib, json, os, subprocess, tempfile, time
from pathlib import Path


def main():
    if os.uname().nodename != 'fedora-core-01' or os.geteuid() == 0:
        raise RuntimeError('Run as the Fedora login user')
    root = Path.home() / '.cache/lurker/native-fresh-20261127'
    driver = Path('/usr/local/bin/fuse-ufs')
    rows = []
    receipt = root / 'logs/ufs2-barrier-control.json'
    for mode in ('normal', 'file-eio', 'directory-eio'):
        d = Path(tempfile.mkdtemp(prefix='ufs-barrier-' + mode + '-', dir=root))
        image = d / 'native.img'; mount = d / 'mount'; mount.mkdir(); seed = d / 'seed'; seed.mkdir(mode=0o700)
        with image.open('xb') as stream: stream.truncate(128 * 1024**2)
        trace = d / 'fsync.strace'; output = (d / 'driver.log').open('w')
        row = dict(mode=mode, fixture=str(d), driver_sha256=hashlib.sha256(driver.read_bytes()).hexdigest(),
                   client_calls=[], status='running')
        rows.append(row); process = None
        def command(argv):
            r = subprocess.run(list(map(str, argv)), capture_output=True, text=True, timeout=30)
            if r.returncode: raise RuntimeError(str(argv[0]) + ': ' + r.stderr[-1200:])
            return r.stdout
        def trace_text(): return command(['sudo', '-n', 'cat', trace])
        def forwarded_count():
            return sum('fsync(' in line and str(image) in line for line in trace_text().splitlines())
        def barrier(descriptor, kind, expected=0):
            before = forwarded_count(); actual = 0
            try: os.fsync(descriptor)
            except OSError as error: actual = error.errno
            after = forwarded_count()
            row['client_calls'].append(dict(kind=kind, expected_errno=expected, actual_errno=actual,
                                             backing_fsync_delta=after-before))
            if actual != expected or after <= before: raise RuntimeError('Barrier result or forwarded syscall differs')
        try:
            command(['makefs-freebsd', '-t', 'ffs', '-B', 'little', '-o',
                     'version=2,bsize=32768,fsize=4096', '-s', str(image.stat().st_size), image, seed])
            argv = ['sudo', '-n', '/usr/bin/strace', '-f', '-yy', '-e', 'trace=fsync,fdatasync', '-o', str(trace)]
            if mode != 'normal': argv += ['--inject=fsync:error=EIO:when=1']
            argv += [str(driver), '-f', '-o', 'rw,allow_other,fsname=' + str(image), str(image), str(mount)]
            process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT)
            for _ in range(200):
                if os.path.ismount(mount): break
                if process.poll() is not None: raise RuntimeError('Traced native driver exited before mount')
                time.sleep(.025)
            if not os.path.ismount(mount): raise RuntimeError('OS mount missing; refusing underlay writes')
            probe = mount / 'barrier-probe.txt'
            with probe.open('wb') as stream:
                stream.write(b'Owned native UFS2 barrier control.\n'); stream.flush()
                if mode != 'directory-eio':
                    barrier(stream.fileno(), 'file', errno.EIO if mode == 'file-eio' else 0)
                    if mode == 'file-eio': barrier(stream.fileno(), 'file-retry')
            descriptor = os.open(mount, os.O_RDONLY | os.O_DIRECTORY)
            try:
                barrier(descriptor, 'directory', errno.EIO if mode == 'directory-eio' else 0)
                if mode == 'directory-eio': barrier(descriptor, 'directory-retry')
            finally: os.close(descriptor)
            if probe.read_bytes() != b'Owned native UFS2 barrier control.\n': raise RuntimeError('Probe content changed')
            row['status'] = 'passed'
        except Exception as error: row.update(status='failed', error=str(error))
        finally:
            if os.path.ismount(mount):
                try: command(['sudo', '-n', 'umount', mount])
                except Exception as error: row['cleanup_error'] = str(error)
            row['mount_remaining'] = os.path.ismount(mount)
            if process and not row['mount_remaining']:
                try: row['driver_exit'] = process.wait(timeout=10)
                except subprocess.TimeoutExpired: row['cleanup_error'] = 'Owned traced driver still running'
            output.close()
            if trace.exists():
                text = trace_text(); row['trace'] = str(trace)
                row['injected_eio_observed'] = 'EIO' in text and 'INJECTED' in text
                if mode != 'normal' and not row['injected_eio_observed']: row.update(status='failed', error='Missing native injection receipt')
            temporary = receipt.with_suffix('.tmp'); temporary.write_text(json.dumps(rows, indent=2)+'\n'); temporary.replace(receipt)
    print(json.dumps(dict(receipt=str(receipt), cases=[dict(mode=r['mode'], status=r['status']) for r in rows])))

if __name__ == '__main__': main()
