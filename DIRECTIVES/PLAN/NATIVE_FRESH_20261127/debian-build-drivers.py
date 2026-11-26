#!/usr/bin/env python3
"""Build verified ordinary external Debian filesystem prerequisites as login user.

No kernel configuration or kernel source is changed. Install reviewed copies
into administrator-controlled driver paths, preserving replaced driver bytes.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--storage', required=True, type=Path)
    args = p.parse_args()
    if os.uname().nodename != 'debian-core-01' or os.geteuid() == 0:
        p.error('Use the assigned Debian login user')
    storage = args.storage.resolve(strict=True)
    if not os.path.ismount(storage):
        p.error('The identified fixture/build storage disk must be mounted')
    inputs = storage / 'inputs'
    build = storage / 'driver-build'
    build.mkdir(mode=0o700)
    backups = build / 'original-drivers'
    backups.mkdir(mode=0o700)
    receipt = build / 'result.json'
    results = []
    env = dict(os.environ, PATH=str(Path.home() / '.cargo/bin') + ':/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin', CARGO_BUILD_JOBS='1', CARGO_INCREMENTAL='0')

    def save():
        temporary = receipt.with_suffix('.tmp')
        temporary.write_text(json.dumps(results, indent=2) + '\n')
        temporary.replace(receipt)

    for name in ('apfs', 'hammer2', 'ufs2'):
        row = {'driver': name, 'status': 'running', 'commands': []}
        results.append(row)
        save()

        def run(argv, cwd=None):
            log = build / f'{name}-{len(row["commands"]):02d}.log'
            with log.open('wb') as output:
                observed = subprocess.run(list(map(str, argv)), cwd=cwd, env=env, stdout=output, stderr=subprocess.STDOUT, timeout=1800)
            row['commands'].append({'argv': list(map(str, argv)), 'code': observed.returncode, 'log': str(log)})
            save()
            if observed.returncode:
                raise RuntimeError('Command failed; see ' + str(log))

        try:
            meta = json.loads((inputs / (name + '-source.json')).read_text())
            archive = inputs / (name + '-current.tar.gz')
            if digest(archive) != meta['sha256']:
                raise RuntimeError('Source archive hash mismatch')
            row['source'] = meta
            source = build / name
            source.mkdir(mode=0o700)
            with tarfile.open(archive) as tar:
                tar.extractall(source, filter='data')
            source = next(path for path in source.iterdir() if path.is_dir())
            if name == 'ufs2':
                patch = inputs / meta['patch']
                if digest(patch) != meta['patch_sha256']:
                    raise RuntimeError('Driver patch hash mismatch')
                run(['git', 'apply', '--unidiff-zero', patch], cwd=source)
                env['CARGO_TARGET_DIR'] = str(build / 'ufs2-target')
                run(['cargo', 'test', '--locked', '-p', 'rufs', '--lib'], cwd=source)
                run(['cargo', 'build', '--locked', '--release', '--bin', 'fuse-ufs'], cwd=source)
                artifact = build / 'ufs2-target/release/fuse-ufs'
                target = Path('/usr/local/bin/fuse-ufs')
                mounts = subprocess.run(['findmnt', '-rn', '-t', 'fuse.ufs'], capture_output=True, text=True)
                if mounts.stdout.strip():
                    raise RuntimeError('UFS2 sessions remain; preserve them before driver installation')
            else:
                kernel = os.uname().release if name == 'apfs' else '7.3.4+deb13-cloud-amd64'
                headers = Path('/lib/modules') / kernel / 'build'
                if not headers.is_dir():
                    raise RuntimeError('Matching official kernel headers unavailable')
                run(['make', '-j1', ('KERNEL_DIR=' if name == 'apfs' else 'KDIR=') + str(headers)], cwd=source)
                artifact = source / ('apfs.ko' if name == 'apfs' else 'src/sys/fs/hammer2/hammer2.ko')
                run(['/sbin/modinfo', artifact])
                target = Path('/lib/modules') / kernel / 'extra' / (name + '.ko')
                row['kernel'] = kernel
                if name == 'apfs':
                    mounts = subprocess.run(['findmnt', '-rn', '-t', 'apfs'], capture_output=True, text=True)
                    if mounts.stdout.strip():
                        raise RuntimeError('APFS sessions remain; preserve them before module installation')
                    if Path('/sys/module/apfs').exists():
                        run(['/usr/bin/sudo', '-n', '/sbin/modprobe', '-r', 'apfs'])
            if target.exists():
                backup = backups / (name + '-' + digest(target))
                shutil.copy2(target, backup)
                row['previous'] = {'path': str(backup), 'sha256': digest(backup)}
            row['artifact_sha256'] = digest(artifact)
            run(['/usr/bin/sudo', '-n', '/usr/bin/install', '-D', '-m', '755' if name == 'ufs2' else '644', artifact, target])
            row['installed'] = {'path': str(target), 'sha256': digest(target)}
            if row['installed']['sha256'] != row['artifact_sha256']:
                raise RuntimeError('Installed driver differs from built artifact')
            if name != 'ufs2':
                run(['/usr/bin/sudo', '-n', '/sbin/depmod', '-a', row['kernel']])
                if name == 'apfs':
                    run(['/usr/bin/sudo', '-n', '/sbin/modprobe', 'apfs'])
            row['status'] = 'passed'
        except Exception as error:
            row.update(status='failed', error=str(error))
        save()
        print(json.dumps({'driver': name, 'status': row['status'], 'receipt': str(receipt)}), flush=True)


if __name__ == '__main__':
    main()
