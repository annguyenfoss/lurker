#!/usr/bin/env python3
"""Run the reviewed payload harness, retaining only generated fixture credentials.

The VM administrator credential is never read or retained by this wrapper.
The test secret stays mode0600 in its owned fixture and out of JSON/log evidence.
"""
import importlib.util
import os
from pathlib import Path
import secrets
import subprocess

harness = Path(__file__).with_name('test-filesystem-cli.py')
spec = importlib.util.spec_from_file_location('filesystem_case', harness)
h = importlib.util.module_from_spec(spec)
spec.loader.exec_module(h)
token = secrets.token_urlsafe(24)
h.secrets.token_urlsafe = lambda size: token
original = h.atomic_json


def retained_receipt(path, value):
    if 'fixture' in value:
        credential = Path(value['fixture']) / '.generated-test-credential'
        if not credential.exists():
            descriptor = os.open(credential, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(descriptor, 'w') as stream:
                stream.write('Lurker9!' + token)
    original(path, value)


h.atomic_json = retained_receipt
original_payload = h.payload_phase


def identified_payload(mountpoint, expected, phase, filesystem=None, backend=None, source=None):
    value = original_payload(mountpoint, expected, phase, filesystem, backend, source)
    if filesystem in {'ufs1', 'ufs2'}:
        native = value.get(phase + '_backing_mount', value[phase + '_mount'])
        command = (['/usr/bin/sudo', '-n'] if os.geteuid() else []) + [
            '/usr/sbin/blkid', '-p', native['source']]
        superblock = subprocess.check_output(command, text=True, timeout=20)
        if 'TYPE="ufs"' not in superblock or ('VERSION="' + filesystem[-1] + '"') not in superblock:
            raise RuntimeError('Independent UFS type/version differs before accepting payload phase')
        proof = {'superblock': superblock}
        if native['filesystem'] in {'fuse.ufs', 'fuse.rufs', 'fuse.fuse-ufs'}:
            proof['host_driver_sha256'] = h.file_hash(Path('/usr/local/bin/fuse-ufs'))
        value[phase + '_ufs_proof'] = proof
    return value


h.payload_phase = identified_payload
raise SystemExit(h.main())
