#!/usr/bin/env python3
"""Move only the closed Fedora round to its verified persistent fixture disk."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess

ROUND = Path('/home/adjutant/.cache/lurker/native-fresh-20261127')
STORE = Path('/var/lib/lurker-native-fresh-20261127-fedora-store')
UUID = '52bc440f-db46-4924-b708-1bd20bfc96c6'


def inventory(root):
    result = {}
    for directory, dirs, files in os.walk(root, followlinks=False):
        for name in dirs + files:
            p = Path(directory) / name
            s = p.lstat()
            row = dict(mode=stat.S_IMODE(s.st_mode), uid=s.st_uid, gid=s.st_gid)
            if stat.S_ISLNK(s.st_mode):
                row['symlink'] = os.readlink(p)
            elif stat.S_ISREG(s.st_mode):
                digest = hashlib.sha256()
                with p.open('rb') as stream:
                    for block in iter(lambda: stream.read(1024**2), b''):
                        digest.update(block)
                row.update(sha256=digest.hexdigest(), size=s.st_size)
            elif not stat.S_ISDIR(s.st_mode):
                raise RuntimeError('Unexpected special file: ' + str(p))
            result[str(p.relative_to(root))] = row
    return result


def main():
    if os.geteuid() != 0 or os.uname().nodename != 'fedora-core-01':
        raise RuntimeError('Run as root on the exclusively owned Fedora guest')
    matrix = json.loads((ROUND / 'final-container-576505d/matrix.json').read_text())
    if matrix['status'] != 'terminal':
        raise RuntimeError('Admission must stop at the completed case boundary')
    for line in Path('/proc/self/mountinfo').read_text().splitlines():
        point = re.sub(r'\\([0-7]{3})', lambda m: chr(int(m[1], 8)), line.split()[4])
        if Path(point).is_relative_to(ROUND):
            raise RuntimeError('Round still has a live mount: ' + point)
    loops = json.loads(subprocess.check_output(
        ['losetup', '--json', '--list', '--output', 'NAME,BACK-FILE'], text=True))['loopdevices']
    if any(str(row.get('back-file', '')).startswith(str(ROUND) + '/') for row in loops):
        raise RuntimeError('Round still has a live loop backing source')
    if not os.path.ismount(STORE):
        raise RuntimeError('Fixture store is not mounted')
    device = subprocess.check_output(['findmnt', '-rn', '-M', str(STORE), '-o', 'SOURCE'], text=True).strip()
    if subprocess.check_output(['blkid', '-s', 'UUID', '-o', 'value', device], text=True).strip() != UUID:
        raise RuntimeError('Fixture filesystem UUID differs')
    if subprocess.check_output(['lsblk', '-dn', '-o', 'SERIAL', device], text=True).strip() != 'lrkf-fedora-store':
        raise RuntimeError('Fixture disk serial differs')
    target = STORE / 'round'
    if target.exists():
        raise RuntimeError('Refuse to overwrite an existing migration target')
    before = inventory(ROUND)
    (STORE / 'migration-inventory-before.json').write_text(json.dumps(before, indent=2) + '\n')
    subprocess.run(['rsync', '-aHAXS', '--numeric-ids', str(ROUND) + '/', str(target) + '/'], check=True)
    after = inventory(target)
    if before != after:
        raise RuntimeError('Closed-source copy differs; retain original and copy')
    backup = ROUND.with_name(ROUND.name + '.closed-before-fixture-store')
    if backup.exists():
        raise RuntimeError('Original-path backup already exists')
    fstab = Path('/etc/fstab')
    previous = fstab.read_text()
    fstab_backup = Path('/etc/fstab.lurker-native-fresh-20261127-fixture-store.before')
    if fstab_backup.exists() or str(STORE) in previous or str(ROUND) in previous:
        raise RuntimeError('Refuse ambiguous existing fixture mount configuration')
    shutil.copy2(fstab, fstab_backup)
    fstab_backup.chmod(0o600)
    addition = ('\n# Lurker native-fresh-20261127 Fedora owned fixture storage\n'
                f'UUID={UUID} {STORE} ext4 defaults,nodev,nosuid 0 2\n'
                f'{target} {ROUND} none bind,rprivate,x-systemd.requires-mounts-for={STORE} 0 0\n')
    with fstab.open('a') as stream:
        stream.write(addition)
        stream.flush()
        os.fsync(stream.fileno())
    ROUND.rename(backup)
    ROUND.mkdir(mode=0o700)
    os.chown(ROUND, 1300, 1300)
    subprocess.run(['mount', '--bind', str(target), str(ROUND)], check=True)
    subprocess.run(['mount', '--make-rprivate', str(ROUND)], check=True)
    if inventory(ROUND) != before:
        raise RuntimeError('Bound path differs; keep original backup')
    receipt = dict(status='passed', source=str(backup), target=str(target), bind=str(ROUND),
                   filesystem_uuid=UUID, serial='lrkf-fedora-store', verified_entries=len(before),
                   copied_regular_bytes=sum(x.get('size', 0) for x in before.values()),
                   source_closed=True, file_hashes_owners_modes_verified=True,
                   boot_configuration=str(fstab), original_backup_retained=True)
    (STORE / 'migration.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt), flush=True)


if __name__ == '__main__':
    main()
