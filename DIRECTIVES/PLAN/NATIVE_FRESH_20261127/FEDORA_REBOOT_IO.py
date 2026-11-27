#!/usr/bin/env python3
"""Read/write only a verified mounted fixture, with the case's original uid."""
import hashlib, importlib.util, json, os, sys
from pathlib import Path

mount = Path(sys.argv[1])
expected = json.loads(sys.argv[2])
phase = sys.argv[3]
spec = importlib.util.spec_from_file_location('h', Path(__file__).with_name('test-filesystem-cli.py'))
h = importlib.util.module_from_spec(spec); spec.loader.exec_module(h)


def payload():
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
    return expected


try:
    print(json.dumps({'ok': True, 'hashes': h.bounded_payload_call(payload, 60)}))
except Exception as error:
    print(json.dumps({'ok': False, 'error': str(error), 'payload_worker': getattr(error, 'worker', None)}))
    raise SystemExit(1)
