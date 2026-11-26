#!/usr/bin/env python3
"""Read/write only a verified mounted fixture, with the case's original uid."""
import hashlib, json, os, sys
from pathlib import Path

mount = Path(sys.argv[1])
expected = json.loads(sys.argv[2])
phase = sys.argv[3]
if not os.path.ismount(mount): raise RuntimeError('Missing OS mount; refusing underlay access')
folder = mount / 'Lurker payload'
found = {name: hashlib.sha256((folder / name).read_bytes()).hexdigest() for name in expected}
if found != expected: raise RuntimeError('Reboot payload hashes differ')
if phase == 'append':
    for name, data in [('page.txt', (folder / 'page.txt').read_bytes() + b'\nVerified after ordinary VM reboot.\n'),
                       ('reboot.txt', b'Created and synced after ordinary VM reboot.\n')]:
        with (folder / name).open('wb') as f:
            f.write(data); f.flush(); os.fsync(f.fileno())
        expected[name] = hashlib.sha256(data).hexdigest()
elif phase != 'verify':
    raise RuntimeError('Unknown payload phase')
print(json.dumps(expected))
