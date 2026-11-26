#!/usr/bin/env python3
"""Exercise the actual helper JSON lifecycle and owned ordinary-reboot fixtures."""
import argparse,hashlib,importlib.util,json,os,secrets,subprocess,time
from pathlib import Path
ROOT=Path('/var/tmp/lurker-native-fresh-20261127-opensuse')
PREFIX=Path('/usr/local/lib/lurker-native-fresh-20261127-opensuse')
COMMIT='8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('phase',choices=['setup','verify']);a=p.parse_args()
    if os.uname().nodename!='opensuse-core-01' or os.geteuid()!=1300:p.error('Run as adjutant on the owned openSUSE VM')
    base=ROOT/'reboot-fixtures';receipt=base/'result.json';boot=Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    spec=importlib.util.spec_from_file_location('payload',PREFIX/'mount-check/test-filesystem-cli.py');h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
    capabilities=json.loads(subprocess.check_output([str(PREFIX/'bin/lurker-helper'),'capabilities'],text=True))
    assert capabilities['keyfile_authentication'] and capabilities['trusted_privileged_tools'] and capabilities['linux_loop_lease_protocol']==1
    if a.phase=='setup':
        base.mkdir(mode=0o700,exist_ok=False)
        result={'host':os.uname().nodename,'source_commit':COMMIT,'route':'staged-helper-json','baseline_stdout_workaround':'Final JSON line accepted after known cryptsetup verbose stdout; final candidate requires strict full-stdout JSON','helper_sha256':digest(PREFIX/'bin/lurker-helper'),'cli_sha256':digest(PREFIX/'bin/lurker'),'capabilities':capabilities,'kernel_before':os.uname().release,'boot_before':boot,'pid':os.getpid(),'started':time.time(),'cases':[],'status':'running'}
    else:
        result=json.loads(receipt.read_text());assert result['status']=='mounted-for-reboot' and result['boot_before']!=boot
        result.update(kernel_after=os.uname().release,boot_after=boot)
    def save():h.atomic_json(receipt,result)
    def helper(row,action):
        log=base/(row['backend']+'-'+str(len(row['operations']))+'-'+action['action']+'.json')
        s=subprocess.run(['sudo','-n',str(PREFIX/'bin/lurker-helper'),'run'],input=json.dumps(action).encode(),capture_output=True,timeout=300)
        log.write_bytes(s.stdout+s.stderr)
        responses=[json.loads(line) for line in s.stdout.splitlines() if line.startswith(b'{')]
        if not responses:raise RuntimeError('Helper emitted no JSON response; see '+str(log))
        response=responses[-1]
        row['operations'].append({'action':action['action'],'log':str(log),'exit_code':s.returncode,'ok':response.get('ok')});save()
        if s.returncode or not response.get('ok'):raise RuntimeError('Helper action failed; see '+str(log))
    def mount(row,kind='auto'):
        helper(row,dict(action='mount',source=row['source'],mountpoint=row['mountpoint'],volume_type=kind,tag=None,passphrase=None,keyfile=row['keyfile']))
    def close(row):helper(row,dict(action='unmount',target=row['mountpoint'],volume_type='auto',tag=None))
    try:
        if a.phase=='setup':
            for backend,fs in [('luks','btrfs'),('luks1','ext4'),('veracrypt','exfat')]:
                d=base/(backend+'-'+fs);d.mkdir(mode=0o700);source=d/'vault.img';m=d/'mount';m.mkdir(mode=0o700);key=d/'key, Việt Nam.bin';key.write_bytes(b'\0\xff\n\r'+secrets.token_bytes(252));key.chmod(0o600)
                row={'backend':backend,'filesystem':fs,'filesystem_role':'inner','source_kind':'file','source':str(source),'mountpoint':str(m),'keyfile':str(key),'keyfile_sha256':digest(key),'operations':[],'status':'running'};result['cases'].append(row);save()
                helper(row,dict(action='create',target=str(source),size_gb='0.25',force=False,source_kind='file',volume_type=backend,cipher='aes',filesystem=fs,passphrase=None,keyfile=str(key)))
                assert source.stat().st_uid==1300 and source.stat().st_mode&0o777==0o600
                expected=h.load_payload(ROOT/'payload')
                for phase in ['write','append','verify']:
                    mount(row,backend if phase=='write' else 'auto')
                    row.update(h.bounded_payload_call(lambda:h.payload_phase(m,expected,phase,fs,backend,source),30));save()
                    if phase=='write':assert row['initial_root_metadata']['uid']==1300 and row['initial_root_metadata']['mode']=='0o700'
                    if phase=='append':expected['page.txt']+=h.APPEND;expected['second.txt']=h.SECOND
                    if phase!='verify':close(row)
                row['status']='mounted-for-reboot';save()
            result['status']='mounted-for-reboot'
        else:
            for row in result['cases']:
                m=Path(row['mountpoint']);source=Path(row['source']);expected=h.load_payload(ROOT/'payload');expected['page.txt']+=h.APPEND;expected['second.txt']=h.SECOND
                mount(row);row['after_reboot']=h.bounded_payload_call(lambda:h.payload_phase(m,expected,'verify',row['filesystem'],row['backend'],source),30)
                marker=m/'after-reboot.txt'
                with marker.open('xb') as f:f.write(b'Writable after ordinary guest reboot.\n');f.flush();os.fsync(f.fileno())
                row['after_reboot_marker_sha256']=digest(marker);close(row);row['status']='passed';save()
            result['status']='passed'
    except Exception as e:
        result.update(status='failed',error=str(e))
        for row in result['cases']:
            if os.path.ismount(row['mountpoint']):
                try:close(row)
                except Exception as err:row['cleanup_error']=str(err)
    result['updated']=time.time();save();print(json.dumps({'status':result['status'],'receipt':str(receipt),'cases':len(result['cases'])}))
    return 0 if result['status'] in ['passed','mounted-for-reboot'] else 1
if __name__=='__main__':raise SystemExit(main())
