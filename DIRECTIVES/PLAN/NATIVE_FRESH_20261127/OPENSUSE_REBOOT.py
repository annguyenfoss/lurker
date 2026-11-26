#!/usr/bin/env python3
"""Exercise the actual helper JSON lifecycle and owned ordinary-reboot fixtures."""
import argparse,hashlib,importlib.util,json,os,secrets,subprocess,time
from pathlib import Path
ROOT=Path('/var/tmp/lurker-native-fresh-20261127-opensuse')
PREFIX=Path('/usr/local/lib/lurker-native-fresh-20261127-opensuse')
COMMIT='8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('phase',choices=['setup','verify']);p.add_argument('--prefix',type=Path,default=PREFIX);p.add_argument('--root',type=Path,default=ROOT/'reboot-fixtures');p.add_argument('--commit',default=COMMIT);p.add_argument('--payload',type=Path,default=ROOT/'payload');p.add_argument('--harness',type=Path,default=PREFIX/'mount-check/test-filesystem-cli.py');p.add_argument('--strict-json',action='store_true');p.add_argument('--direct-helper',action='store_true');p.add_argument('--include-directories',action='store_true');a=p.parse_args();prefix=a.prefix
    if os.uname().nodename!='opensuse-core-01' or os.geteuid()!=1300:p.error('Run as adjutant on the owned openSUSE VM')
    if a.include_directories and not a.direct_helper:p.error('Directory reboot fixtures must use the direct normal-user helper')
    base=a.root;receipt=base/'result.json';boot=Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    spec=importlib.util.spec_from_file_location('payload',a.harness);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
    capabilities=json.loads(subprocess.check_output([str(prefix/'bin/lurker-helper'),'capabilities'],text=True))
    assert capabilities['keyfile_authentication'] and capabilities['trusted_privileged_tools'] and capabilities['linux_loop_lease_protocol']==1
    if a.include_directories:assert capabilities['directory_backends'] and capabilities['folder_supervision_protocol']==1
    if a.phase=='setup':
        base.mkdir(mode=0o700,exist_ok=False)
        result={'host':os.uname().nodename,'source_commit':a.commit,'route':'staged-helper-json','stdout_json_mode':'strict-full-stdout' if a.strict_json else 'baseline-final-line-workaround','helper_elevation':'direct-normal-user' if a.direct_helper else 'explicit-sudo','helper_sha256':digest(prefix/'bin/lurker-helper'),'script_sha256':digest(Path(__file__)),'harness_sha256':digest(a.harness),'cli_sha256':digest(prefix/'bin/lurker'),'capabilities':capabilities,'kernel_before':os.uname().release,'boot_before':boot,'pid':os.getpid(),'started':time.time(),'cases':[],'status':'running'}
    else:
        result=json.loads(receipt.read_text());assert result['status']=='mounted-for-reboot' and result['boot_before']!=boot
        assert result['helper_sha256']==digest(prefix/'bin/lurker-helper') and result['cli_sha256']==digest(prefix/'bin/lurker') and result['source_commit']==a.commit
        result.update(kernel_after=os.uname().release,boot_after=boot)
    def save():h.atomic_json(receipt,result)
    def helper(row,action):
        log=base/(row['backend']+'-'+str(len(row['operations']))+'-'+action['action']+'.json')
        s=subprocess.run(([] if a.direct_helper else ['sudo','-n'])+[str(prefix/'bin/lurker-helper'),'run'],input=json.dumps(action).encode(),capture_output=True,timeout=300)
        log.write_bytes(s.stdout+s.stderr)
        responses=[json.loads(s.stdout)] if a.strict_json else [json.loads(line) for line in s.stdout.splitlines() if line.startswith(b'{')]
        if not responses:raise RuntimeError('Helper emitted no JSON response; see '+str(log))
        response=responses[-1]
        row['operations'].append({'action':action['action'],'log':str(log),'exit_code':s.returncode,'ok':response.get('ok')});save()
        if s.returncode or not response.get('ok'):raise RuntimeError('Helper action failed; see '+str(log))
    def authentication(row):return dict(passphrase=Path(row['passphrase_file']).read_text() if row.get('passphrase_file') else None,keyfile=row.get('keyfile'))
    def mount(row,kind='auto'):
        helper(row,dict(action='mount',source=row['source'],mountpoint=row['mountpoint'],volume_type=kind,tag=None,**authentication(row)))
    def close(row):helper(row,dict(action='unmount',target=row['mountpoint'],volume_type='auto',tag=None))
    try:
        if a.phase=='setup':
            profiles=[('luks','btrfs'),('luks1','ext4'),('veracrypt','exfat')]
            if a.include_directories:profiles += [(b,'ext4') for b in ['gocryptfs','cryfs','cryptomator']]
            for backend,fs in profiles:
                directory=backend in ['gocryptfs','cryfs','cryptomator'];d=base/(backend+'-'+fs);d.mkdir(mode=0o700);source=d/('encrypted' if directory else 'vault.img');m=d/'mount';m.mkdir(mode=0o700)
                key=None;passfile=None
                if directory:
                    passfile=d/'generated volume credential.txt';password='  Reboot 密碼 9!'+secrets.token_urlsafe(24)+'  ';assert len(password.encode())<64
                    with os.fdopen(os.open(passfile,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600),'w') as f:f.write(password)
                else:key=d/'key, Việt Nam.bin';key.write_bytes(b'\0\xff\n\r'+secrets.token_bytes(252));key.chmod(0o600)
                row={'backend':backend,'filesystem':fs,'filesystem_role':'backing' if directory else 'inner','source_kind':'directory' if directory else 'file','source':str(source),'mountpoint':str(m),'keyfile':str(key) if key else None,'keyfile_sha256':digest(key) if key else None,'passphrase_file':str(passfile) if passfile else None,'passphrase_sha256':digest(passfile) if passfile else None,'operations':[],'status':'running'};result['cases'].append(row);save()
                helper(row,dict(action='create',target=str(source),size_gb=None if directory else '0.25',force=False,source_kind=row['source_kind'],volume_type=backend,cipher=None if directory else 'aes',filesystem=None if directory else fs,**authentication(row)))
                if not directory:assert source.stat().st_uid==1300 and source.stat().st_mode&0o777==0o600
                expected=h.load_payload(a.payload)
                for phase in ['write','append','verify']:
                    mount(row,backend if phase=='write' else 'auto')
                    row.update(h.bounded_payload_call(lambda:h.payload_phase(m,expected,phase,fs,backend,source),30));save()
                    if phase=='write' and not directory:assert row['initial_root_metadata']['uid']==1300 and row['initial_root_metadata']['mode']=='0o700'
                    if phase=='append':expected['page.txt']+=h.APPEND;expected['second.txt']=h.SECOND
                    if phase!='verify':close(row)
                row['status']='mounted-for-reboot';save()
            result['status']='mounted-for-reboot'
        else:
            for row in result['cases']:
                m=Path(row['mountpoint']);source=Path(row['source']);expected=h.load_payload(a.payload);expected['page.txt']+=h.APPEND;expected['second.txt']=h.SECOND
                mount(row);row['after_reboot']=h.bounded_payload_call(lambda:h.payload_phase(m,expected,'verify',row['filesystem'],row['backend'],source),30)
                marker=m/'after-reboot.txt'
                def write_marker():
                    with marker.open('xb') as f:f.write(b'Writable after ordinary guest reboot.\n');f.flush();os.fsync(f.fileno())
                    return digest(marker)
                row['after_reboot_marker_sha256']=h.bounded_payload_call(write_marker,30);close(row);row['status']='passed';save()
            result['status']='passed'
    except Exception as e:
        result.update(status='failed',error=str(e))
        if hasattr(e,'worker'):row['payload_worker']=e.worker
        for row in result['cases']:
            if not row.get('payload_worker',{}).get('remaining') and os.path.ismount(row['mountpoint']):
                try:close(row)
                except Exception as err:row['cleanup_error']=str(err)
    result['updated']=time.time();save();print(json.dumps({'status':result['status'],'receipt':str(receipt),'cases':len(result['cases'])}))
    return 0 if result['status'] in ['passed','mounted-for-reboot'] else 1
if __name__=='__main__':raise SystemExit(main())
