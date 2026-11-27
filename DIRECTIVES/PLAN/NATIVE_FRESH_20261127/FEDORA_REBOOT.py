#!/usr/bin/env python3
"""Reopen the retained Fedora final sweep after an observed ordinary reboot."""
import argparse, hashlib, importlib.util, json, os, re, subprocess, time
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for key in ('matrix', 'binary', 'harness', 'output'):
        p.add_argument('--' + key, type=Path, required=True)
    p.add_argument('--before-boot-id', required=True)
    p.add_argument('--runtime-commit', required=True)
    p.add_argument('--creation-artifact', action='append', default=[], help='Explicit prior COMMIT:SHA256 identity for unchanged encrypted fixtures')
    p.add_argument('--prepare', action='store_true', help='Verify and leave owned fixtures mounted before reboot')
    a = p.parse_args()
    if os.geteuid() == 0 or os.uname().nodename != 'fedora-core-01':
        p.error('Run as the Fedora login user')
    if not re.fullmatch(r'[0-9a-f]{40}', a.runtime_commit): p.error('Full runtime commit is required')
    prior = []
    for identity in a.creation_artifact:
        if not re.fullmatch(r'[0-9a-f]{40}:[0-9a-f]{64}', identity): p.error('Creation identity must be full COMMIT:SHA256')
        prior.append(tuple(identity.split(':')))
    boot = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    if not a.prepare and boot == a.before_boot_id: p.error('Boot ID did not change')
    if a.prepare and boot != a.before_boot_id: p.error('Unexpected boot before preparation')
    spec = importlib.util.spec_from_file_location('h', a.harness)
    h = importlib.util.module_from_spec(spec); spec.loader.exec_module(h)
    matrix = json.loads(a.matrix.read_text())
    ntfs_driver = matrix.get('ntfs_driver', 'ntfs-3g')
    a.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    result = dict(host=os.uname().nodename, kernel=os.uname().release,
        phase='prepare' if a.prepare else 'verify', before_boot_id=a.before_boot_id, after_boot_id=boot,
        binary_sha256=h.file_hash(a.binary), runtime_commit=a.runtime_commit, explicit_prior_creation_artifacts=prior, cases=[], status='running')
    receipt = a.output / 'reboot.json'

    def save(): h.atomic_json(receipt, result)

    def native(argv):
        r = subprocess.run(['sudo', '-n', *map(str, argv)], capture_output=True, text=True, timeout=180)
        if r.returncode: raise RuntimeError(str(argv[0]) + ': ' + r.stderr[-1200:])
        return r.stdout

    def case(row):
        old = row.get('receipt', {})
        if old.get('status') != 'passed': return
        privileged = row['route'] == 'privileged'
        directory = row['route'] == 'directory-on-filesystem'
        keyfile = Path(old['keyfile']) if old.get('keyfile') else None
        new = dict(backend=row['backend'], filesystem=row['filesystem'], route=row['route'],
                   source=old['source'], credential=old.get('credential', 'password'), creation_commit=old['commit'], creation_binary_sha256=old['binary_sha256'], runtime_commit=a.runtime_commit, status='running', operations=[])
        result['cases'].append(new); save()
        mountpoint = Path(old['mountpoint'])
        credential_path = Path(old['fixture']) / '.generated-test-credential'
        password = ''
        folder = mountpoint / 'Lurker payload'
        prefix = ['/usr/bin/sudo', '-n'] if privileged else []
        def cli(action, operand):
            log = a.output / (str(len(result['cases'])) + '-' + str(len(new['operations'])) + '-' + action + '.log')
            credential_args = ['--key-file', str(keyfile)] if action == 'mount' and keyfile else []
            r = h.terminal([*prefix, str(a.binary), action, '--type', 'auto', '--color=never', *credential_args, *map(str, operand)], password, 180, log)
            new['operations'].append(dict(action=action, log=str(log), **r)); save()
            if r['exit_code'] or r['timed_out']: raise RuntimeError('CLI ' + action + ' failed')
        def payload(phase, wanted):
            args = ['/usr/bin/python3', str(Path(__file__).with_name('FEDORA_REBOOT_IO.py')),
                    str(mountpoint), json.dumps(wanted), phase]
            if privileged:
                return json.loads(native(args))
            process = subprocess.run(args, capture_output=True, text=True, timeout=60)
            if process.returncode: raise RuntimeError(process.stderr[-1200:])
            return json.loads(process.stdout)
        try:
            if keyfile:
                if h.file_hash(keyfile) != old['keyfile_sha256']:
                    raise RuntimeError('Retained binary keyfile identity differs')
                new['keyfile_sha256'] = old['keyfile_sha256']
            else:
                password = native(['cat', credential_path]) if privileged else credential_path.read_text()
            creation_identity = (old['commit'], old['binary_sha256'])
            if creation_identity not in [(a.runtime_commit, result['binary_sha256']), *prior]:
                raise RuntimeError('Creation/runtime artifact transition was not explicitly identified')
            cli('mount', [old['source'], mountpoint])
            identity_path = Path(old['source']) if directory else mountpoint
            identity = h.linux_mount_record(Path('/proc/self/mountinfo').read_text(), identity_path)
            new['native_filesystem'] = identity
            new['filesystem_identity_gate'] = h.check_linux_filesystem(identity, row['filesystem'])
            if row['filesystem'] in {'ufs1', 'ufs2', 'ntfs'}:
                superblock = native(['blkid', '-p', identity['source']])
                new['native_superblock'] = superblock
                expected_marker = ('VERSION=\"' + ('1' if row['filesystem'] == 'ufs1' else '2') + '\"') if row['filesystem'] in {'ufs1', 'ufs2'} else 'TYPE=\"ntfs\"'
                if expected_marker not in superblock: raise RuntimeError('Independent filesystem superblock differs')
            if identity['filesystem'] in {'fuse.ufs', 'fuse.rufs', 'fuse.fuse-ufs'}:
                new['fuse_ufs_driver_sha256'] = h.file_hash(Path('/usr/local/bin/fuse-ufs'))
            expected = old['verify_sha256']
            new['verified_hashes'] = payload('verify', expected)
            if a.prepare:
                new['status'] = 'passed'
                return
            expected = payload('append', expected)
            cli('unmount', [mountpoint]); cli('mount', [old['source'], mountpoint])
            new['after_reopen_hashes'] = payload('verify', expected)
            cli('unmount', [mountpoint]); new['status'] = 'passed'
        except Exception as e: new.update(status='failed', error=str(e))
        finally:
            if os.path.ismount(mountpoint) and not (a.prepare and new['status'] == 'passed'):
                try: cli('unmount', [mountpoint])
                except Exception as e: new['cleanup_error'] = str(e)
            new['mount_remaining'] = os.path.ismount(mountpoint); save()

    save()
    for row in matrix['cases']:
        if row['route'] in {'privileged', 'login-user'}: case(row)
    for backing in matrix.get('backing_records', []):
        fs = backing['filesystem']; image = Path(backing['image']); mount = Path(backing['mountpoint'])
        try:
            if fs == 'zfs': native(['zpool', 'import', '-d', image.parent, backing['pool']])
            elif fs == 'ufs2': native(['/usr/local/bin/fuse-ufs', '-o', 'rw,allow_other,fsname=' + str(image), image, mount])
            else:
                options = ['loop']
                if fs in {'ntfs', 'exfat', 'fat32'}: options += ['uid=1300', 'gid=1300', 'umask=0077']
                if fs == 'apfs': options += ['readwrite']
                if fs == 'ufs1': options += ['rw', 'ufstype=44bsd']
                args = ['mount', '-o', ','.join(options)]
                if fs == 'ntfs':
                    if ntfs_driver != 'ntfs-3g': args.append('-i')
                    args += ['-t', ntfs_driver]
                if fs in {'apfs', 'hammer2'}: args += ['-i', '-t', fs]
                if fs == 'ufs1': args += ['-i', '-t', 'ufs']
                native([*args, image, mount])
            if not os.path.ismount(mount): raise RuntimeError('Backing mount missing')
            for row in matrix['cases']:
                if row['route'] == 'directory-on-filesystem' and row['filesystem'] == fs: case(row)
        except Exception as e:
            result.setdefault('backing_errors', []).append(dict(filesystem=fs, error=str(e))); save()
        finally:
            if os.path.ismount(mount) and not a.prepare:
                try: native(['zpool', 'export', backing['pool']] if fs == 'zfs' else ['umount', mount])
                except Exception as e: result.setdefault('cleanup_errors', []).append(str(e)); save()
    result['status'] = 'terminal'; result['finished'] = time.time(); save()
    print(json.dumps(dict(receipt=str(receipt), counts={s: sum(c['status'] == s for c in result['cases']) for s in ('passed', 'failed')})))

if __name__ == '__main__': main()
