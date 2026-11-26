#!/usr/bin/env python3
"""Bounded Linux filesystem round: PDF/text CLI cases, no SQLite/fault tests."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def command(args, **kw):
    completed = subprocess.run(list(map(str, args)), capture_output=True, text=True, **kw)
    if completed.returncode:
        raise RuntimeError(f'{args[0]} failed ({completed.returncode}): ' + (completed.stderr or completed.stdout)[-4000:])
    return completed


def ntfs_mount_driver():
    if any(line.split()[-1:] == ['ntfs3'] for line in Path('/proc/filesystems').read_text().splitlines()):
        return 'ntfs3'
    modinfo = shutil.which('modinfo')
    if modinfo and subprocess.run([modinfo, 'ntfs3'], stdout=subprocess.DEVNULL,
                                 stderr=subprocess.DEVNULL).returncode == 0:
        return 'ntfs3'
    if modinfo:
        description = subprocess.run([modinfo, '-F', 'description', 'ntfs'],
                                     capture_output=True, text=True)
        if description.returncode == 0 and any(
                line.strip() == 'NTFS read-write filesystem driver'
                for line in description.stdout.splitlines()):
            return 'ntfs'
    return 'ntfs-3g'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--harness', type=Path, required=True, help='Administrator-owned common harness')
    p.add_argument('--commit', required=True)
    p.add_argument('--root', type=Path, required=True)
    p.add_argument('--filesystems', default='btrfs,ext4,xfs,ntfs,exfat,fat32')
    p.add_argument('--directory-filesystems', default='btrfs,ext4,xfs,ntfs,exfat,fat32')
    p.add_argument('--backends', type=Path, required=True)
    p.add_argument('--userspace-backends', type=Path)
    p.add_argument('--payload-dir', type=Path,
                   help='Downloaded common payload directory for the native qualification round')
    p.add_argument('--skip-block', action='store_true')
    p.add_argument('--only-userspace', action='store_true')
    p.add_argument('--skip-userspace', action='store_true',
                   help='Run host-native filesystem routes without the LKL compatibility route')
    a = p.parse_args()
    if a.only_userspace and a.skip_userspace:
        p.error('--only-userspace and --skip-userspace are mutually exclusive')
    if os.geteuid() == 0:
        p.error('Run the coordinator as the login user; only block cases use sudo')
    a.root.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.environ['PATH'] = '/usr/local/sbin:/usr/sbin:/sbin:' + os.environ.get('PATH', '')
    os.environ['LURKER_BACKEND_DIR'] = str(a.backends)
    os.environ['LURKER_USERSPACE_DIR'] = str(a.userspace_backends or a.backends)
    ntfs_driver = ntfs_mount_driver()
    result = {'commit': a.commit, 'started': time.time(), 'pid': os.getpid(), 'cases': [], 'status': 'running',
              'ntfs_driver': ntfs_driver}
    receipt = a.root / 'matrix.json'

    def save():
        tmp = receipt.with_suffix('.tmp')
        tmp.write_text(json.dumps(result, indent=2) + '\n')
        tmp.replace(receipt)

    def run_case(backend, fs, privileged=False, root=None, directory=False):
        args = ([shutil.which('sudo'), '-n'] if privileged else []) + [('/usr/bin/python3' if privileged else shutil.which('python3')), str(a.harness),
            '--binary', str(a.binary), '--backend', backend, '--filesystem', fs, '--commit', a.commit,
            '--root', str(root or a.root), '--size-gb', '0.5' if fs in {'xfs', 'hammer2'} else '0.25', '--timeout', '90']
        if directory:
            args.extend(['--mount-root', str(a.root)])
        if a.payload_dir:
            args.extend(['--payload-dir', str(a.payload_dir)])
        if backend == 'veracrypt' and not privileged:
            args.append('--unprivileged')
        started = time.time()
        proc = subprocess.run(args, capture_output=True, text=True, timeout=900)
        row = {'backend': backend, 'filesystem': fs, 'route': 'directory-on-filesystem' if directory else ('privileged' if privileged else 'login-user'),
               'code': proc.returncode, 'seconds': round(time.time() - started, 2)}
        try:
            summary = json.loads(proc.stdout.splitlines()[-1]); path = Path(summary['receipt'])
            native = json.loads(command(([shutil.which('sudo'), '-n'] if privileged else []) + ['cat', path]).stdout)
            row.update(summary=summary, receipt=native)
            if not native.get('mount_remaining') and native.get('status') == 'passed' and not directory:
                source = Path(native['source'])
                if source.parent == path.parent and source.name == 'vault.img':
                    command(([shutil.which('sudo'), '-n'] if privileged else []) + ['rm', '--', source])
                    row['passed_ciphertext_removed'] = True
            elif not native.get('mount_remaining') and native.get('status') != 'passed' and not directory:
                # A failed FUSE mount can leave its mapper/loop open even when
                # no mount was published. Moving the backing file would make
                # its active identity stale and obstruct normal cleanup.
                row['retained_fixture'] = str(path.parent)
                row['failed_fixture_preserved_in_place'] = True
        except Exception as error:
            row.update(error=str(error), stdout=proc.stdout[-2000:], stderr=proc.stderr[-2000:])
        result['cases'].append(row)
        save()

    save()
    if not a.skip_block:
        for fs in a.filesystems.split(','):
            if not a.only_userspace:
                for backend in ('luks', 'luks1', 'veracrypt'):
                    run_case(backend, fs, True)
            if not a.skip_userspace and fs in {'btrfs', 'ext4', 'xfs', 'ntfs', 'exfat', 'fat32'}:
                run_case('veracrypt', fs)
    # Directory formats have their own layout. The label below names their
    # disposable backing filesystem, never an alternate inner format.
    for fs in filter(None, a.directory_filesystems.split(',')):
        backing = Path(tempfile.mkdtemp(prefix='backing-' + fs + '-', dir=a.root))
        image = backing / 'backing.img'; mount = backing / 'mount'; mount.mkdir(mode=0o700)
        with image.open('wb') as f:
            f.truncate(512 * 1024**2)
        pool = 'lrkf_backing_' + backing.name.replace('-', '_')
        mounted = False
        preserve = False
        try:
            if fs == 'zfs':
                command(['sudo', '-n', 'zpool', 'create', '-o', 'cachefile=none', '-m', mount, pool, image])
            elif fs in {'ufs1', 'ufs2'}:
                seed = backing / 'seed'; seed.mkdir(mode=0o700)
                version = 1 if fs == 'ufs1' else 2
                command(['makefs-freebsd','-t','ffs','-B','little','-o',f'version={version},bsize=32768,fsize=4096','-s',str(512 * 1024**2),image,seed])
                if fs == 'ufs1':
                    command(['sudo','-n','mount','-i','-t','ufs','-o','loop,rw,ufstype=44bsd',image,mount])
                else:
                    command(['sudo','-n','/usr/local/bin/fuse-ufs','-o',f'rw,allow_other,fsname={image}',image,mount])
            else:
                formatter = {'btrfs':['mkfs.btrfs','-q'], 'ext4':['mkfs.ext4','-q'], 'xfs':['mkfs.xfs','-q'],
                    'ntfs':['mkntfs','-Q','-F'], 'exfat':['mkfs.exfat'], 'fat32':['mkfs.fat','-F','32'],
                    'apfs':['mkapfs'], 'hammer2':['newfs_hammer2','-L','DATA']}[fs]
                command([*formatter, image])
                options = ['loop']
                if fs in {'ntfs', 'exfat', 'fat32'}:
                    options += [f'uid={os.getuid()}', f'gid={os.getgid()}', 'umask=0077']
                if fs == 'apfs': options.append('readwrite')
                args = ['sudo', '-n', 'mount', '-o', ','.join(options)]
                if fs == 'ntfs':
                    if ntfs_driver != 'ntfs-3g': args.append('-i')
                    args += ['-t',ntfs_driver]
                if fs in {'apfs', 'hammer2'}: args += ['-i', '-t', fs]
                command([*args, image, mount])
            deadline = time.monotonic() + 5
            while not os.path.ismount(mount) and time.monotonic() < deadline:
                time.sleep(.025)
            mounted = os.path.ismount(mount)
            if not mounted:
                raise RuntimeError('Backing mount missing; refusing to write to underlay')
            if fs not in {'ntfs', 'exfat', 'fat32'}:
                command(['sudo','-n','chown',f'{os.getuid()}:{os.getgid()}',mount]); mount.chmod(0o700)
            for backend in ('gocryptfs', 'cryfs', 'cryptomator'):
                run_case(backend, fs, root=mount, directory=True)
                last = result['cases'][-1]
                native = last.get('receipt', {})
                preserve |= native.get('status') != 'passed'
                if native.get('fixture') and not native.get('mount_remaining'):
                    evidence = backing / backend
                    evidence.mkdir(mode=0o700)
                    for entry in Path(native['fixture']).iterdir():
                        if entry.is_file() and (entry.suffix == '.log' or entry.name == 'result.json'):
                            shutil.copy2(entry, evidence / entry.name)
                    last['evidence'] = str(evidence)
                    save()
        except Exception as error:
            preserve = True
            result['cases'].append({'filesystem':fs,'route':'directory-backing-setup','status':'failed','error':str(error)})
            save()
        finally:
            if mounted:
                try:
                    if fs == 'zfs': command(['sudo','-n','zpool','export',pool])
                    else: command(['sudo','-n','umount',mount])
                    if preserve:
                        retained = Path.home() / '.cache/lurker/filesystem-round-retained'
                        retained.mkdir(parents=True, exist_ok=True)
                        destination = retained / (backing.name + '.img')
                        shutil.move(str(image), str(destination))
                        result.setdefault('retained_backings',[]).append(str(destination))
                    else:
                        image.unlink()
                except Exception as error:
                    result.setdefault('cleanup_errors',[]).append({'filesystem':fs,'error':str(error)})
    result['finished'] = time.time()
    result['status'] = 'terminal'
    result['counts'] = {'passed':sum(r.get('receipt',{}).get('status')=='passed' for r in result['cases']),
                        'failed':sum(r.get('receipt',{}).get('status')=='failed' or r.get('status')=='failed' or 'error' in r for r in result['cases'])}
    save()
    print(json.dumps({'receipt':str(receipt),'counts':result['counts'],'cleanup_errors':result.get('cleanup_errors',[])}))


if __name__ == '__main__':
    main()
