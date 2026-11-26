#!/usr/bin/env python3
"""Reboot the final verified Fedora helper/partition sample without formatting it."""
import argparse, hashlib, importlib.util, json, os, subprocess
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ['prefix', 'helper-fixture', 'harness', 'output']:
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--before-boot-id', required=True)
    p.add_argument('--prepare', action='store_true')
    a = p.parse_args()
    if os.geteuid() != 1300 or os.uname().nodename != 'fedora-core-01':
        p.error('Run as the exclusive Fedora login user')
    boot = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    if (boot == a.before_boot_id) != a.prepare:
        p.error('Boot identity does not match requested phase')
    old = json.loads((a.helper_fixture / 'result.json').read_text())
    row = next(x for x in old['cases'] if x['backend'] == 'veracrypt')
    if old['status'] != 'passed' or row['status'] != 'passed':
        p.error('The retained helper fixture is not a completed pass')
    source = Path(row['source']).resolve(strict=True)
    parent = subprocess.check_output(['lsblk', '-dn', '-o', 'PKNAME', source], text=True).strip()
    identity = subprocess.check_output(['lsblk', '-bdn', '-o', 'SIZE,SERIAL,TYPE', '/dev/' + parent], text=True).split()
    if identity != ['1073741824', 'lurker-fresh-fedora', 'disk']:
        p.error('Partition parent is not the identified scratch disk')
    spec = importlib.util.spec_from_file_location('h', a.harness)
    h = importlib.util.module_from_spec(spec); spec.loader.exec_module(h)
    helper = a.prefix / 'bin/lurker-helper'
    if h.file_hash(helper) != old['helper_sha256']:
        p.error('Helper artifact changed')
    key = a.helper_fixture / 'veracrypt/binary key Việt Nam.bin'
    if h.file_hash(key) != row['keyfile_sha256']:
        p.error('Retained binary key changed')
    a.output.mkdir(mode=0o700, exist_ok=False)
    mount = a.output / 'mount'; mount.mkdir(mode=0o700)
    result = dict(status='running', phase='prepare' if a.prepare else 'verify',
                  before_boot_id=a.before_boot_id, after_boot_id=boot,
                  helper_sha256=old['helper_sha256'], source=str(source), operations=[])
    def save(): h.atomic_json(a.output / 'result.json', result)
    def action(verb):
        request = dict(action=verb, volume_type='auto', tag=None)
        if verb == 'mount': request.update(source=str(source), mountpoint=str(mount), passphrase=None, keyfile=str(key))
        else: request['target'] = str(mount)
        response = subprocess.run([str(helper), 'run'], input=json.dumps(request).encode(), capture_output=True, timeout=300)
        value = json.loads(response.stdout)
        result['operations'].append(dict(action=verb, exit_code=response.returncode, strict_stdout_json=True, ok=value.get('ok'))); save()
        if response.returncode or not value.get('ok'): raise RuntimeError('Helper ' + verb + ' failed')
    def payload(wanted, append=False):
        if not os.path.ismount(mount): raise RuntimeError('Refuse underlay payload access')
        native = h.linux_mount_record(Path('/proc/self/mountinfo').read_text(), mount)
        h.check_linux_filesystem(native, 'btrfs')
        folder = mount / 'Lurker payload'
        for name, expected in wanted.items():
            if hashlib.sha256((folder / name).read_bytes()).hexdigest() != expected: raise RuntimeError('Physical reboot payload mismatch')
        if append:
            with (folder / 'page.txt').open('ab') as stream:
                stream.write(b'\nFedora ordinary physical-helper reboot append.\n'); stream.flush(); os.fsync(stream.fileno())
            with (folder / 'physical-reboot.txt').open('xb') as stream:
                stream.write(b'New file after ordinary OS reboot.\n'); stream.flush(); os.fsync(stream.fileno())
            wanted = {**wanted, 'physical-reboot.txt': ''}
        return dict(hashes={name: hashlib.sha256((folder / name).read_bytes()).hexdigest() for name in wanted}, mount=native)
    stalled = False; save()
    try:
        action('mount')
        observed = h.bounded_payload_call(lambda: payload(row['verify_sha256']), 60)
        result['initial_verified'] = observed
        if not a.prepare:
            changed = h.bounded_payload_call(lambda: payload(observed['hashes'], True), 60)
            action('unmount'); action('mount')
            result['after_append_reopen'] = h.bounded_payload_call(lambda: payload(changed['hashes']), 60)
            action('unmount')
        result['status'] = 'passed'
    except Exception as error:
        result.update(status='failed', error=str(error))
        if hasattr(error, 'worker'): result['payload_worker'] = error.worker; stalled = error.worker['remaining']
    finally:
        if not stalled and os.path.ismount(mount) and not (a.prepare and result['status'] == 'passed'):
            try: action('unmount')
            except Exception as error: result['cleanup_error'] = str(error)
        result['mount_remaining'] = True if stalled else os.path.ismount(mount); save()
    print(json.dumps(dict(status=result['status'], receipt=str(a.output / 'result.json'))))
    return int(result['status'] != 'passed')


if __name__ == '__main__': raise SystemExit(main())
