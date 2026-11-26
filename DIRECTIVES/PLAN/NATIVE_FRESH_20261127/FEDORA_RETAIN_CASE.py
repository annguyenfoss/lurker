#!/usr/bin/env python3
"""Run the reviewed payload harness, retaining only generated fixture credentials.

The VM administrator credential is never read or retained by this wrapper.
The test secret stays mode0600 in its owned fixture and out of JSON/log evidence.
"""
import importlib.util
import os
from pathlib import Path
import secrets

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
raise SystemExit(h.main())
