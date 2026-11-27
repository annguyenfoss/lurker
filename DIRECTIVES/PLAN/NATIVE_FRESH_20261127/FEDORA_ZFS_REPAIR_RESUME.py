#!/usr/bin/env python3
"""Finish only the three unadmitted directory baselines after exact bind repair."""
import copy, importlib.util, json, os, subprocess
from pathlib import Path


def main():
    if os.geteuid() or os.uname().nodename != 'fedora-core-01': raise RuntimeError('Owned Fedora administrator required')
    r = Path('/home/adjutant/.cache/lurker/native-fresh-20261127')
    p = Path('/usr/local/lib/lurker-native-fresh-20261127-fedora-package-67894f6'); q = p / 'qualification'
    spec = importlib.util.spec_from_file_location('h', q / 'test-filesystem-cli.py')
    h = importlib.util.module_from_spec(spec); spec.loader.exec_module(h)
    receipt = r / 'logs/fixture-bind-rprivate-repair.json'; out = json.loads(receipt.read_text())
    assert out['status'] == 'running' and len(out['cases']) == 6
    assert all(x['status'] == 'passed' for x in out['cases'][:3])
    assert subprocess.check_output(['findmnt', '-rn', '-M', str(r), '-o', 'PROPAGATION'], text=True).strip() == 'private'
    assert 'none bind,rprivate,' in Path('/etc/fstab').read_text()
    creation = json.loads((r / 'reboot-cohort-678/matrix.json').read_text())
    first = json.loads((r / 'reboot-verify-678/reboot.json').read_text())
    wanted = next(x['after_reopen_hashes'] for x in first['cases'] if x['status'] == 'passed' and x['route'] == 'privileged')
    backing = next(b for b in creation['backing_records'] if b['filesystem'] == 'zfs')
    mounted = h.linux_mount_record(Path('/proc/self/mountinfo').read_text(), Path(backing['mountpoint']))
    assert mounted['mountpoint'] == backing['mountpoint'] and mounted['source'] == backing['pool']
    out['resume_harness_sha256'] = h.file_hash(Path(__file__))
    def save(): h.atomic_json(receipt, out)
    def payload(mount, expected):
        c = subprocess.run(['/usr/bin/sudo', '-n', '-u', 'adjutant', '--', 'python3', str(q / 'FEDORA_REBOOT_IO.py'), str(mount), json.dumps(expected), 'verify'], capture_output=True, text=True, timeout=90)
        value = json.loads(c.stdout)
        if c.returncode or not value['ok']: raise RuntimeError('Bounded directory baseline verification failed: ' + c.stdout)
        return value['hashes']
    baseline_rows = []
    for item in out['cases']:
        directory = item.get('role') == 'directory-on-filesystem'
        row = next(x for x in creation['cases'] if x['filesystem'] == 'zfs' and x['backend'] == item['backend'] and (x['route'] == 'directory-on-filesystem') == directory)
        old = row['receipt']; updated = copy.deepcopy(row)
        if not directory:
            updated['receipt']['original_creation_verify_sha256'] = old['verify_sha256']
            updated['receipt']['verify_sha256'] = item['targeted_reboot_baseline_hashes']
            updated['receipt']['reboot_baseline_provenance'] = 'Preserved failed image; generated first-attempt file retained by rename; normal product reopen/close verified.'
            baseline_rows.append(updated); continue
        assert not item.get('status') and all(op['exit_code'] == 127 for op in item.get('operations', []))
        for op in item.get('operations', []): op['superseded_before_cli_execution'] = True
        password = (Path(old['fixture']) / '.generated-test-credential').read_text(); mount = Path(old['mountpoint'])
        def cli(action):
            operands = [old['source'], str(mount)] if action == 'mount' else [str(mount)]
            log = r / ('logs/zfs-bind-directory-resume-' + item['backend'] + '-' + action + '.log')
            value = h.terminal(['/usr/bin/sudo', '-n', '-u', 'adjutant', '--', str(p / 'bin/lurker'), action, '--type', 'auto', '--color=never', *operands], password, 180, log)
            item.setdefault('operations', []).append(dict(action=action, log=str(log), **value)); save()
            if value['exit_code'] or value['timed_out']: raise RuntimeError('Normal directory product lifecycle failed')
        cli('mount'); payload(mount, wanted)
        def retain_file():
            os.setgroups([]); os.setgid(1300); os.setuid(1300)
            folder = mount / 'Lurker payload'; target = folder / 'reboot-first-attempt.txt'
            assert not target.exists(); (folder / 'reboot.txt').rename(target)
            fd = os.open(folder, os.O_RDONLY)
            try: os.fsync(fd)
            finally: os.close(fd)
            return str(target)
        item['retained_first_attempt_file'] = h.bounded_payload_call(retain_file, 60)
        baseline = {('reboot-first-attempt.txt' if name == 'reboot.txt' else name): value for name, value in wanted.items()}
        item['targeted_reboot_baseline_hashes'] = payload(mount, baseline); cli('unmount')
        updated['receipt']['original_creation_verify_sha256'] = old['verify_sha256']
        updated['receipt']['verify_sha256'] = baseline
        updated['receipt']['reboot_baseline_provenance'] = 'Backing failure image preserved closed; generated first-attempt file retained by rename; normal product reopen/close verified.'
        baseline_rows.append(updated); item['status'] = 'passed'; save()
    subprocess.run(['zpool', 'export', backing['pool']], check=True)
    target = r / 'reboot-zfs-private-cohort-678'; target.mkdir(mode=0o700); os.chown(target, 1300, 1300)
    matrix = target / 'matrix.json'
    matrix.write_text(json.dumps(dict(cases=baseline_rows, backing_records=[backing], runtime_commit=out['runtime_commit'],
                                      baseline_boot_id=first['after_boot_id'], repair_receipt=str(receipt)), indent=2) + '\n')
    matrix.chmod(0o600); os.chown(matrix, 1300, 1300)
    out.update(status='passed', targeted_cohort=str(matrix)); save()
    print(json.dumps(dict(status='passed', targeted_cases=len(baseline_rows), receipt=str(receipt))))


if __name__ == '__main__': main()
