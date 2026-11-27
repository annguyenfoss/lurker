#!/usr/bin/env python3
"""Repair only the owned fixture bind after preserving three ZFS close refusals."""
import copy, hashlib, importlib.util, json, os, shutil, subprocess
from pathlib import Path

R = Path('/home/adjutant/.cache/lurker/native-fresh-20261127')
STORE = Path('/var/lib/lurker-native-fresh-20261127-fedora-store')
P = Path('/usr/local/lib/lurker-native-fresh-20261127-fedora-package-67894f6')
Q = P / 'qualification'


def main():
    if os.geteuid() or os.uname().nodename != 'fedora-core-01':
        raise RuntimeError('Exclusive Fedora administrator required')
    spec = importlib.util.spec_from_file_location('h', Q / 'test-filesystem-cli.py')
    h = importlib.util.module_from_spec(spec); spec.loader.exec_module(h)
    original = json.loads((R / 'reboot-verify-678/reboot.json').read_text())
    failed = [c for c in original['cases'] if c['status'] == 'failed']
    passed = [c for c in original['cases'] if c['status'] == 'passed']
    assert original['status'] == 'terminal' and len(passed) == 63 and len(failed) == 3
    assert all(c['filesystem'] == 'zfs' and c['route'] == 'privileged' for c in failed)
    assert not original.get('backing_errors') and not original.get('cleanup_errors')
    creation = json.loads((R / 'reboot-cohort-678/matrix.json').read_text())
    rows = {c['backend']: c for c in creation['cases'] if c['filesystem'] == 'zfs' and c['route'] == 'privileged'}
    reference = next(c for c in passed if c['route'] == 'privileged')
    wanted = reference['after_reopen_hashes']
    assert 'reboot.txt' in wanted and len(wanted) == 6
    expected_mounts = {Path(rows[c['backend']]['receipt']['mountpoint']) for c in failed}
    info = Path('/proc/self/mountinfo').read_text()
    assert {Path(line.split(' - ')[0].split()[4]) for line in info.splitlines()
            if line.split(' - ')[0].split()[4].startswith(str(R) + '/')} == expected_mounts
    out = dict(status='running', runtime_commit=original['runtime_commit'],
               failure_receipt=str(R / 'reboot-verify-678/reboot.json'),
               first_boot_passes_retained=63, cases=[], native_operations=[])
    receipt = R / 'logs/fixture-bind-rprivate-repair.json'
    def save(): h.atomic_json(receipt, out)
    def native(argv):
        c = subprocess.run(list(map(str, argv)), capture_output=True, text=True, timeout=120)
        out['native_operations'].append(dict(argv=list(map(str, argv)), code=c.returncode, stdout=c.stdout, stderr=c.stderr)); save()
        if c.returncode: raise RuntimeError('Native repair command failed: ' + str(argv[0]))
        return c.stdout.strip()
    def payload(mount, expected):
        c = subprocess.run(['python3', str(Q / 'FEDORA_REBOOT_IO.py'), str(mount), json.dumps(expected), 'verify'], capture_output=True, text=True, timeout=90)
        value = json.loads(c.stdout)
        if c.returncode or not value['ok']: raise RuntimeError('Bounded repair payload verification failed: ' + c.stdout)
        return value['hashes']
    save()
    # Prove the live failures and their exact mirrored aliases before changing propagation.
    for failure in failed:
        old = rows[failure['backend']]['receipt']; mount = Path(old['mountpoint'])
        alias = STORE / 'round' / mount.relative_to(R)
        assert failure['verified_hashes'] == reference['verified_hashes']
        root = h.linux_mount_record(info, mount); mirror = h.linux_mount_record(info, alias)
        assert root['mountpoint'] == str(mount) and mirror['mountpoint'] == str(alias)
        assert root['filesystem'] == mirror['filesystem'] == 'zfs' and root['source'] == mirror['source']
        out['cases'].append(dict(backend=failure['backend'], source=old['source'], mountpoint=str(mount),
                                 alias=str(alias), root_before=root, alias_before=mirror,
                                 first_attempt_after_append_hashes=payload(mount, wanted)))
        save()
    fstab = Path('/etc/fstab'); old_text = fstab.read_text()
    before = f'{STORE}/round {R} none bind,x-systemd.requires-mounts-for={STORE} 0 0\n'
    after = before.replace('none bind,', 'none bind,rprivate,')
    assert old_text.count(before) == 1
    backup = Path('/etc/fstab.lurker-native-fresh-20261127-before-rprivate')
    assert not backup.exists(); shutil.copy2(fstab, backup); backup.chmod(0o600)
    temporary = Path('/etc/.fstab-lurker-native-fresh-20261127-rprivate')
    assert not temporary.exists()
    with temporary.open('x') as stream:
        stream.write(old_text.replace(before, after)); stream.flush(); os.fsync(stream.fileno())
    temporary.chmod(fstab.stat().st_mode & 0o777); os.replace(temporary, fstab)
    native(['restorecon', '/etc/fstab']); native(['systemctl', 'daemon-reload'])
    unit = native(['systemd-escape', '--path', '--suffix=mount', R])
    generated = (Path('/run/systemd/generator') / unit).read_text()
    assert 'Options=bind,rprivate,' in generated
    native(['mount', '--make-rprivate', R])
    assert native(['findmnt', '-rn', '-M', R, '-o', 'PROPAGATION']) == 'private'
    out.update(fstab_backup=str(backup), fstab_sha256=h.file_hash(fstab), generated_mount_unit=generated)
    baseline_rows = []
    for item in out['cases']:
        row = rows[item['backend']]; old = row['receipt']; mount = Path(item['mountpoint'])
        password = (Path(old['fixture']) / '.generated-test-credential').read_text()
        def cli(action):
            operands = [old['source'], str(mount)] if action == 'mount' else [str(mount)]
            log = R / ('logs/zfs-bind-repair-' + item['backend'] + '-' + str(len(item.get('operations', []))) + '-' + action + '.log')
            value = h.terminal([str(P / 'bin/lurker'), action, '--type', 'auto', '--color=never', *operands], password, 180, log)
            item.setdefault('operations', []).append(dict(action=action, log=str(log), **value)); save()
            if value['exit_code'] or value['timed_out']: raise RuntimeError('Normal product repair lifecycle failed')
        native(['umount', item['alias']])
        assert h.linux_mount_record(Path('/proc/self/mountinfo').read_text(), mount)['mountpoint'] == str(mount)
        cli('unmount')
        assert not native(['losetup', '-j', old['source']])
        backup_dir = R / 'reboot-zfs-first-attempt-failures' / item['backend']; backup_dir.mkdir(mode=0o700, parents=True)
        image = backup_dir / 'vault.img'
        source_hash = h.file_hash(Path(old['source']))
        native(['cp', '--sparse=always', '--', old['source'], image]); image.chmod(0o600)
        assert h.file_hash(image) == source_hash == h.file_hash(Path(old['source']))
        shutil.copy2(Path(old['fixture']) / '.generated-test-credential', backup_dir / '.generated-test-credential')
        item['closed_failure_image'] = dict(path=str(image), sha256=source_hash)
        cli('mount'); payload(mount, wanted)
        def retain_first_attempt_file():
            folder = mount / 'Lurker payload'; target = folder / 'reboot-first-attempt.txt'
            assert not target.exists(); (folder / 'reboot.txt').rename(target)
            fd = os.open(folder, os.O_RDONLY)
            try: os.fsync(fd)
            finally: os.close(fd)
            return {'retained_first_attempt_file': str(target)}
        item.update(h.bounded_payload_call(retain_first_attempt_file, 60))
        baseline = {('reboot-first-attempt.txt' if name == 'reboot.txt' else name): value for name, value in wanted.items()}
        item['targeted_reboot_baseline_hashes'] = payload(mount, baseline)
        cli('unmount')
        updated = copy.deepcopy(row)
        updated['receipt']['original_creation_verify_sha256'] = old['verify_sha256']
        updated['receipt']['verify_sha256'] = baseline
        updated['receipt']['reboot_baseline_provenance'] = 'First failure image preserved closed; original generated reboot file retained by normal rename; expected hashes verified after normal product reopen.'
        baseline_rows.append(updated); item['status'] = 'passed'; save()
    target = R / 'reboot-zfs-private-cohort-678'; target.mkdir(mode=0o700); os.chown(target, 1300, 1300)
    matrix = target / 'matrix.json'
    matrix.write_text(json.dumps(dict(cases=baseline_rows, backing_records=[], runtime_commit=original['runtime_commit'],
                                      baseline_boot_id=original['after_boot_id'], repair_receipt=str(receipt)), indent=2) + '\n')
    matrix.chmod(0o600); os.chown(matrix, 1300, 1300)
    out.update(status='passed', targeted_cohort=str(matrix)); save()
    print(json.dumps(dict(status=out['status'], targeted_cases=len(baseline_rows), receipt=str(receipt))))


if __name__ == '__main__': main()
