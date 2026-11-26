#!/usr/bin/env python3
"""Strict real-helper JSON and binary-keyfile payload lifecycles on owned openSUSE."""
import argparse,hashlib,importlib.util,json,os,secrets,subprocess,time
from pathlib import Path

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['prefix','root','payload','harness']:p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--commit',required=True);p.add_argument('--device',type=Path);p.add_argument('--device-parent',type=Path);p.add_argument('--serial');a=p.parse_args()
    if os.uname().nodename!='opensuse-core-01' or os.geteuid()!=1300:p.error('Run as the normal owned openSUSE login user')
    helper=a.prefix/'bin/lurker-helper';assert helper.stat().st_uid==0 and not helper.stat().st_mode&0o022
    def device_identity():
        tree=json.loads(subprocess.check_output(['lsblk','-J','-b','-p','-o','NAME,TYPE,SERIAL,SIZE,PKNAME',str(a.device_parent)],text=True))['blockdevices'][0]
        assert a.serial in {'lurker-fresh-opensuse','lurker-fresh-opensus'} and tree['serial']==a.serial and tree['size']==1073741824 and tree['type']=='disk'
        children=tree.get('children',[]);assert len(children)==1 and children[0]['type']=='part' and Path(children[0]['name']).resolve()==a.device.resolve() and not children[0].get('children')
        assert not subprocess.run(['findmnt','-rn','-S',str(a.device)],capture_output=True).returncode==0
        return tree
    if a.device:
        if not a.device_parent or not a.serial:p.error('Device mode requires its independently identified scratch parent and serial')
        a.device=a.device.resolve(strict=True);a.device_parent=a.device_parent.resolve(strict=True);device_identity()
    a.root.mkdir(mode=0o700,parents=True,exist_ok=False)
    spec=importlib.util.spec_from_file_location('h',a.harness);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
    j={'commit':a.commit,'host':os.uname().nodename,'kernel':os.uname().release,'uid':os.geteuid(),'route':'normal-login-user-direct-helper-with-native-elevation','helper':str(helper),'helper_sha256':h.file_hash(helper),'harness_sha256':h.file_hash(a.harness),'script_sha256':h.file_hash(Path(__file__)),'pid':os.getpid(),'started':time.time(),'status':'running','cases':[]}
    def save():h.atomic_json(a.root/'result.json',j)
    save()
    for backend in ['luks','luks1','veracrypt']:
        d=a.root/backend;d.mkdir(mode=0o700);key=d/'binary key Việt Nam.bin';key.write_bytes(b'\0\xff\n\r'+secrets.token_bytes(252));key.chmod(0o600);source=a.device if a.device else d/'vault.img';m=d/'mount';m.mkdir(mode=0o700)
        row={'backend':backend,'filesystem':'btrfs','filesystem_role':'inner','source_kind':'device-partition' if a.device else 'container','source':str(source),'keyfile_sha256':h.file_hash(key),'operations':[],'status':'running'};j['cases'].append(row);save()
        def action(request):
            c=subprocess.run([str(helper),'run'],input=json.dumps(request).encode(),capture_output=True,timeout=300)
            name=str(len(row['operations']))+'-'+request['action'];(d/(name+'.stdout')).write_bytes(c.stdout);(d/(name+'.stderr')).write_bytes(c.stderr)
            row['operations'].append({'action':request['action'],'exit_code':c.returncode,'stdout':str(d/(name+'.stdout'))});save()
            response=json.loads(c.stdout);row['operations'][-1].update(strict_full_stdout_json=True,ok=response.get('ok'));save()
            if c.returncode or not response.get('ok'):raise RuntimeError('Helper action failed: '+name)
        def close():action(dict(action='unmount',target=str(m),volume_type='auto',tag=None))
        try:
            if a.device:row['device_identity']=device_identity();save()
            action(dict(action='create',target=str(source),size_gb=None if a.device else '0.25',force=bool(a.device),source_kind='block' if a.device else 'file',volume_type=backend,cipher='aes',filesystem='btrfs',passphrase=None,keyfile=str(key)))
            if not a.device:assert source.stat().st_uid==1300 and source.stat().st_mode&0o777==0o600
            expected=h.load_payload(a.payload)
            for phase in ['write','append','verify']:
                action(dict(action='mount',source=str(source),mountpoint=str(m),volume_type=backend if phase=='write' else 'auto',tag=None,passphrase=None,keyfile=str(key)))
                row.update(h.bounded_payload_call(lambda:h.payload_phase(m,expected,phase,'btrfs',backend,source),45));save()
                if phase=='write':assert row['initial_root_metadata']['uid']==1300 and row['initial_root_metadata']['mode']=='0o700'
                if phase=='append':expected['page.txt']+=h.APPEND;expected['second.txt']=h.SECOND
                close()
            row['status']='passed'
            if not a.device and not subprocess.check_output(['sudo','-n','/usr/sbin/losetup','-j',str(source)],text=True).strip():
                row['passed_ciphertext_removed']={'sha256':h.file_hash(source),'size':source.stat().st_size};source.unlink()
        except Exception as e:
            row.update(status='failed',error=str(e))
            if hasattr(e,'worker'):row['payload_worker']=e.worker
        finally:
            if not row.get('payload_worker',{}).get('remaining') and os.path.ismount(m):
                try:close()
                except Exception as e:row['cleanup_error']=str(e)
            row['mount_remaining']=os.path.ismount(m);save()
            if row['mount_remaining'] or row.get('payload_worker',{}).get('remaining'):raise RuntimeError('Retain live helper fixture and worker')
            if a.device:
                device_identity()
                if row['status']!='passed':
                    failed=d/'failed-parent-device.img';c=subprocess.run(['sudo','-n','cp','--sparse=always','--',str(a.device_parent),str(failed)],capture_output=True,text=True,timeout=180);row['failed_device_copy']={'path':str(failed),'exit_code':c.returncode,'stderr':c.stderr};save()
                    if c.returncode:raise RuntimeError('Retain scratch without overwriting after failed fixture copy')
    j.update(status='passed' if all(r['status']=='passed' for r in j['cases']) else 'failed',finished=time.time());save();print(json.dumps({'status':j['status'],'receipt':str(a.root/'result.json')}));return int(j['status']!='passed')
if __name__=='__main__':raise SystemExit(main())
