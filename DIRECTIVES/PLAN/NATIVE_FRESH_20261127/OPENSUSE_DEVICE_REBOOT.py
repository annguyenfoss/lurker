#!/usr/bin/env python3
"""Verify the last identified physical VeraCrypt helper sample across ordinary reboot."""
import argparse,hashlib,importlib.util,json,os,subprocess,time
from pathlib import Path
ROOT=Path('/var/tmp/lurker-native-fresh-20261127-opensuse')
PREFIX=Path('/usr/lib/lurker')
COMMIT='576505d554d798422840a06322dfa93b899365bc'
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('phase',choices=['setup','verify']);p.add_argument('--prefix',type=Path,default=PREFIX);p.add_argument('--base',type=Path,default=ROOT/'device-576');p.add_argument('--commit',default=COMMIT);p.add_argument('--harness',type=Path,default=Path('/usr/local/lib/lurker-native-fresh-20261127-opensuse-final-576505d/harness/test-filesystem-cli-device.py'));a=p.parse_args()
    if os.uname().nodename!='opensuse-core-01' or os.geteuid()!=1300:p.error('Normal owned openSUSE login user only')
    base=a.base;receipt=base/'physical-reboot.json';helper=a.prefix/'bin/lurker-helper';boot=Path('/proc/sys/kernel/random/boot_id').read_text().strip();prior=json.loads((base/'helper-physical/result.json').read_text());assert prior['status']=='passed' and prior['commit']==a.commit
    assert helper.stat().st_uid==0 and not helper.stat().st_mode&0o022
    digest=lambda f:hashlib.sha256(f.read_bytes()).hexdigest()
    assert digest(helper)==prior['helper_sha256'];vc=next(r for r in prior['cases'] if r['backend']=='veracrypt');key=base/'helper-physical/veracrypt/binary key Việt Nam.bin';assert digest(key)==vc['keyfile_sha256']
    disks=json.loads(subprocess.check_output(['lsblk','-J','-b','-p','-o','NAME,TYPE,SERIAL,SIZE'],text=True))['blockdevices'];match=[d for d in disks if d.get('serial')=='lurker-fresh-opensus'];assert len(match)==1;disk=match[0];assert disk['size']==1073741824 and disk['type']=='disk';children=disk.get('children',[]);assert len(children)==1 and children[0]['type']=='part' and not children[0].get('children');source=Path(children[0]['name'])
    assert subprocess.run(['findmnt','-rn','-S',str(source)],capture_output=True).returncode!=0
    harness=a.harness;spec=importlib.util.spec_from_file_location('h',harness);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
    m=base/'physical-reboot-mount'
    if a.phase=='setup':
        m.mkdir(mode=0o700);j={'commit':a.commit,'helper_sha256':digest(helper),'script_sha256':digest(Path(__file__)),'harness_sha256':digest(harness),'host':os.uname().nodename,'boot_before':boot,'kernel_before':os.uname().release,'identity_before':disk,'keyfile_sha256':digest(key),'mountpoint':str(m),'source_before':str(source),'operations':[],'status':'running','started':time.time()}
    else:
        j=json.loads(receipt.read_text());assert j['status']=='mounted-for-reboot' and j['boot_before']!=boot and j['helper_sha256']==digest(helper) and j['commit']==a.commit;j.update(boot_after=boot,kernel_after=os.uname().release,identity_after=disk,source_after=str(source))
    def save():h.atomic_json(receipt,j)
    def action(value):
        c=subprocess.run([str(helper),'run'],input=json.dumps(value).encode(),capture_output=True,timeout=300);n=str(len(j['operations']))+'-'+value['action'];log=base/('physical-reboot-'+n+'.json');log.write_bytes(c.stdout+c.stderr);response=json.loads(c.stdout);j['operations'].append({'action':value['action'],'exit_code':c.returncode,'ok':response.get('ok'),'strict_full_stdout_json':True,'log':str(log)});save();assert c.returncode==0 and response.get('ok')
    save()
    try:
        action(dict(action='mount',source=str(source),mountpoint=str(m),volume_type='auto',tag=None,passphrase=None,keyfile=str(key)))
        expected=h.load_payload(ROOT/'payload');expected['page.txt']+=h.APPEND;expected['second.txt']=h.SECOND
        j[a.phase+'_payload']=h.bounded_payload_call(lambda:h.payload_phase(m,expected,'verify','btrfs','veracrypt',source),45)
        if a.phase=='setup':j['status']='mounted-for-reboot'
        else:
            def marker():
                f=m/'physical-after-reboot.txt'
                with f.open('xb') as stream:stream.write(b'Physical scratch volume writable after ordinary reboot.\n');stream.flush();os.fsync(stream.fileno())
                return digest(f)
            j['after_reboot_marker_sha256']=h.bounded_payload_call(marker,30);action(dict(action='unmount',target=str(m),volume_type='auto',tag=None));j['status']='passed'
    except Exception as e:
        j.update(status='failed',error=str(e))
        if hasattr(e,'worker'):j['payload_worker']=e.worker
        if not j.get('payload_worker',{}).get('remaining') and os.path.ismount(m):
            try:action(dict(action='unmount',target=str(m),volume_type='auto',tag=None))
            except Exception as err:j['cleanup_error']=str(err)
    j.update(updated=time.time(),mount_remaining=os.path.ismount(m));save();print(json.dumps({'status':j['status'],'receipt':str(receipt)}));return int(j['status'] not in ['mounted-for-reboot','passed'])
if __name__=='__main__':raise SystemExit(main())
