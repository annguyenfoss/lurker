#!/usr/bin/env python3
"""Read-only Linux CLI imports from independently formatted PDF/text fixtures."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import secrets
import subprocess
import tempfile
import time


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--harness', type=Path, required=True)
    p.add_argument('--fixture', type=Path, required=True)
    p.add_argument('--payload', type=Path, required=True, help='JSON map of root filename to SHA256')
    p.add_argument('--filesystem', choices=('ufs1', 'apfs', 'hammer2'), required=True)
    p.add_argument('--commit', required=True)
    p.add_argument('--backends', default='luks,luks1,veracrypt')
    p.add_argument('--root', type=Path, required=True)
    a = p.parse_args()
    if os.geteuid() != 0:
        p.error('Install this script in an administrator-controlled path and run with sudo')
    spec = importlib.util.spec_from_file_location('filesystem_payload', a.harness)
    h = importlib.util.module_from_spec(spec); spec.loader.exec_module(h)
    expected = json.loads(a.payload.read_text())
    if set(expected) != {'paper.pdf', 'page.txt'}:
        p.error('The independent fixture must contain paper.pdf and page.txt')
    a.root.mkdir(mode=0o700, parents=True, exist_ok=True)
    result = {'commit':a.commit, 'binary':str(a.binary), 'binary_sha256':h.file_hash(a.binary),
              'fixture':str(a.fixture), 'fixture_sha256':h.file_hash(a.fixture),
              'filesystem':a.filesystem, 'started':time.time(), 'cases':[], 'status':'running'}
    receipt = a.root/'imports.json'

    def save(): h.atomic_json(receipt,result)

    def native(args, password, data=None):
        proc = subprocess.run(list(map(str,args)),input=data,capture_output=True)
        if proc.returncode:
            raise RuntimeError(str(args[0])+' failed: '+h.redact(proc.stderr or proc.stdout,[password])[-4000:])
        return proc.stdout.decode(errors='replace').strip()

    save()
    backends = a.backends.split(',')
    if any(backend not in {'luks','luks1','veracrypt'} for backend in backends):
        p.error('Unsupported import encryption backend')
    for backend in backends:
        root = Path(tempfile.mkdtemp(prefix='ro-'+backend+'-',dir=a.root))
        source=root/'vault.img';mount=root/'mount';mount.mkdir(mode=0o700)
        password='Lurker9!'+secrets.token_urlsafe(24)
        mapper_name='lrkf_import_'+str(os.getpid())+'_'+str(len(result['cases']))
        mapper=Path('/dev/mapper')/mapper_name
        row={'backend':backend,'fixture':str(root),'actions':[],'status':'running'}
        result['cases'].append(row);save();raw_open=False;mounted=False

        def cli(action,operands):
            log=root/(str(len(row['actions']))+'-'+action+'.log')
            status=h.terminal([str(a.binary),action,'--type',backend,'--color=never',*map(str,operands)],password,120,log)
            row['actions'].append({'action':action,'log':str(log),**status});save()
            if status['exit_code'] or status['timed_out']:raise RuntimeError('CLI '+action+' failed')

        try:
            # Create only a fresh encrypted file through the unchanged AES /
            # password profile; replace its temporary inner filesystem with
            # the independent fixture before the real read-only CLI import.
            cli('create',['--filesystem','ext4',source,'0.75' if a.filesystem=='hammer2' else '0.25'])
            if mapper.exists():raise RuntimeError('Private import mapper already exists')
            args=['/usr/sbin/cryptsetup' if Path('/usr/sbin/cryptsetup').exists() else '/usr/bin/cryptsetup','open','--batch-mode']
            if backend=='veracrypt':args+=['--type','tcrypt','--veracrypt'];secret=(password+'\n').encode()
            else:args+=['--type','luks','--key-file','-'];secret=password.encode()
            native([*args,source,mapper_name],password,secret);raw_open=True
            native(['/usr/sbin/wipefs' if Path('/usr/sbin/wipefs').exists() else '/usr/bin/wipefs','-a',mapper],password)
            with a.fixture.open('rb') as incoming, mapper.open('r+b',buffering=0) as target:
                while block:=incoming.read(1024*1024):target.write(block)
                os.fsync(target.fileno())
            native([args[0],'close',mapper_name],password);raw_open=False
            before=h.file_hash(source)
            cli('mount',['--readonly',source,mount]);mounted=True
            if not os.path.ismount(mount):raise RuntimeError('Read-only filesystem mount missing')
            view=native(['findmnt','-rn','-M',mount,'-o','SOURCE,FSTYPE,OPTIONS'],password)
            row['mount']=view
            fields=view.split()
            if len(fields)!=3 or 'ro' not in fields[2].split(','):raise RuntimeError('Filesystem is not read-only')
            row['crypto_readonly']=native(['blockdev','--getro',fields[0]],password)
            if row['crypto_readonly']!='1':raise RuntimeError('Crypto mapper is not read-only')
            row['payload_sha256']={name:h.file_hash(mount/name) for name in expected}
            if row['payload_sha256']!=expected:raise RuntimeError('Imported PDF/text hash mismatch')
            st=mount.stat();row['root_metadata']={'uid':st.st_uid,'gid':st.st_gid,'mode':oct(st.st_mode&0o777)}
            cli('unmount',[mount]);mounted=False
            row['ciphertext_sha256_before']=before;row['ciphertext_sha256_after']=h.file_hash(source)
            if row['ciphertext_sha256_after']!=before:raise RuntimeError('Read-only import changed ciphertext')
            row['status']='passed';source.unlink();row['passed_ciphertext_removed']=True
        except Exception as error:row['status']='failed';row['error']=str(error)
        finally:
            if mounted or os.path.ismount(mount):
                try:cli('unmount',[mount])
                except Exception as error:row['cleanup_error']=str(error)
            if raw_open:
                try:native([args[0],'close',mapper_name],password)
                except Exception as error:row['raw_cleanup_error']=str(error)
            row['mount_remaining']=os.path.ismount(mount);save()
    result['status']='terminal';result['finished']=time.time()
    result['counts']={'passed':sum(r['status']=='passed' for r in result['cases']),
                      'failed':sum(r['status']=='failed' for r in result['cases'])}
    save();print(json.dumps({'receipt':str(receipt),'counts':result['counts']}))
    return 1 if result['counts']['failed'] else 0


if __name__=='__main__':raise SystemExit(main())
