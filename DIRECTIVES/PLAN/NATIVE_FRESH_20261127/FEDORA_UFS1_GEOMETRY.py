#!/usr/bin/env python3
"""Native LUKS2/UFS1 geometry control; keeps the approved AES/Argon profile."""
import argparse, hashlib, json, os, secrets, subprocess, time
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, required=True)
    p.add_argument('--payload', type=Path, required=True)
    a = p.parse_args()
    if os.geteuid() != 0 or os.uname().nodename != 'fedora-core-01':
        p.error('Run with sudo on the owned Fedora VM')
    os.umask(0o077)
    a.root.mkdir(parents=True, exist_ok=False)
    image = a.root / 'luks2-ufs1.img'
    image.touch(mode=0o600)
    with image.open('r+b') as stream:
        stream.truncate(512 * 1024**2)
    mountpoint = a.root / 'mount'; mountpoint.mkdir()
    seed = a.root / 'seed'; seed.mkdir()
    name = 'lrkf_fedora_ufs1_' + str(os.getpid())
    mapper = Path('/dev/mapper') / name
    password = secrets.token_bytes(64)
    wanted = {name: (a.payload / name).read_bytes() for name in
              ('bitcoin.pdf', 'ethereum.pdf', 'transformer.pdf', 'page.txt')}
    result = dict(host=os.uname().nodename, kernel=os.uname().release,
                  route='native geometry research, not product pass',
                  backend='luks2', filesystem='ufs1', role='inner',
                  started=time.time(), operations=[], status='running')
    opened = mounted = False

    def save():
        temp = a.root / 'receipt.tmp'
        temp.write_text(json.dumps(result, indent=2) + '\n')
        temp.replace(a.root / 'receipt.json')

    def run(argv, secret=None):
        r = subprocess.run(list(map(str, argv)), input=secret, capture_output=True, timeout=180)
        result['operations'].append(dict(argv=list(map(str, argv)), code=r.returncode,
            stdout=r.stdout.decode(errors='replace')[-6000:], stderr=r.stderr.decode(errors='replace')[-6000:]))
        save()
        if r.returncode:
            raise RuntimeError(str(argv[0]) + ' failed')
        return r.stdout.decode().strip()

    def close():
        nonlocal opened, mounted
        if mounted:
            run(['umount', mountpoint]); mounted = False
        if opened:
            run(['cryptsetup', 'close', name]); opened = False

    try:
        run(['cryptsetup', 'luksFormat', '--type', 'luks2', '-c', 'aes-xts-plain64',
             '-s', '512', '--pbkdf', 'argon2id', '--pbkdf-memory', '1048576',
             '-i', '5000', '--use-urandom', '--sector-size', '512',
             '--batch-mode', '--key-file', '-', image], password)
        result['luks_metadata'] = json.loads(run(['cryptsetup', 'luksDump', '--dump-json-metadata', image]))
        for phase in ('write', 'append', 'verify'):
            run(['cryptsetup', 'open', '--key-file', '-', image, name], password); opened = True
            result['logical_sector_bytes'] = run(['blockdev', '--getss', mapper])
            if result['logical_sector_bytes'] != '512': raise RuntimeError('Unexpected sector geometry')
            if phase == 'write':
                size = run(['blockdev', '--getsize64', mapper])
                run(['makefs-freebsd', '-t', 'ffs', '-B', 'little', '-o',
                     'version=1,bsize=32768,fsize=4096', '-s', size, mapper, seed])
            run(['mount', '-i', '-t', 'ufs', '-o', 'rw,ufstype=44bsd', mapper, mountpoint])
            mounted = os.path.ismount(mountpoint)
            if not mounted: raise RuntimeError('Missing native mount; refusing underlay writes')
            view = run(['findmnt', '-rn', '-M', mountpoint, '-o', 'SOURCE,FSTYPE,OPTIONS'])
            if 'rw' not in view.split()[-1].split(','): raise RuntimeError('Mount is not writable')
            target = mountpoint / 'payload'
            if phase == 'write':
                target.mkdir()
                for filename, data in wanted.items():
                    with (target / filename).open('xb') as f: f.write(data); f.flush(); os.fsync(f.fileno())
            for filename, data in wanted.items():
                if (target / filename).read_bytes() != data: raise RuntimeError('Payload mismatch: ' + filename)
            if phase == 'append':
                wanted['page.txt'] += b'\nNative LUKS2 512-byte geometry reopen.\n'
                wanted['second.txt'] = b'New file after native reopen.\n'
                for filename in ('page.txt', 'second.txt'):
                    with (target / filename).open('wb') as f: f.write(wanted[filename]); f.flush(); os.fsync(f.fileno())
            result[phase + '_hashes'] = {n: hashlib.sha256((target / n).read_bytes()).hexdigest() for n in wanted}
            run(['sync', '-f', mountpoint]); close()
        result['status'] = 'passed'
    except Exception as e:
        result.update(status='failed', error=str(e))
    finally:
        try: close()
        except Exception as e: result['cleanup_error'] = str(e)
        result['mount_remaining'] = os.path.ismount(mountpoint)
        result['finished'] = time.time(); save()
    print(json.dumps(dict(status=result['status'], receipt=str(a.root / 'receipt.json'))))

if __name__ == '__main__':
    main()
