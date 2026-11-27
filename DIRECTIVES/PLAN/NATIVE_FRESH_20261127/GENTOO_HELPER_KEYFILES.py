#!/usr/bin/env python3
"""Strict real-helper JSON and binary-keyfile payload lifecycles on owned Gentoo."""
import argparse,hashlib,importlib.util,json,os,secrets,subprocess,time
from pathlib import Path

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['prefix','root','payload','harness']:p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--commit',required=True);p.add_argument('--device',type=Path);a=p.parse_args()
    if os.uname().nodename!='gentoo-core-01' or os.geteuid()!=1300:p.error('Run as the normal owned Gentoo login user')
    helper=a.prefix/'bin/lurker-helper';assert helper.stat().st_uid==0 and not helper.stat().st_mode&0o022
    def scratch_guard():
        if not a.device:return None
        device=a.device.resolve(strict=True)
        if str(a.device)!='/dev/disk/by-id/virtio-lurker-fresh-gentoo' or not device.is_block_device():raise RuntimeError('Only the identified round scratch device is authorized')
        row=json.loads(subprocess.check_output(['lsblk','--json','--tree','--bytes','-o','PATH,SERIAL,SIZE,TYPE,MOUNTPOINTS',str(device)]))['blockdevices'][0]
        holders=list((Path('/sys/class/block')/device.name/'holders').iterdir())
        if row.get('serial')!='lurker-fresh-gentoo' or row.get('size')!=2147483648 or row.get('type')!='disk' or row.get('children') or any(row.get('mountpoints') or []) or holders:raise RuntimeError('Scratch identity or busy guard failed')
        return row
    device_identity=scratch_guard()
    a.root.mkdir(mode=0o700,parents=True,exist_ok=False)
    spec=importlib.util.spec_from_file_location('h',a.harness);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
    j={'commit':a.commit,'host':os.uname().nodename,'kernel':os.uname().release,'uid':os.geteuid(),'helper':str(helper),'helper_sha256':h.file_hash(helper),'harness_sha256':h.file_hash(a.harness),'pid':os.getpid(),'started':time.time(),'status':'running','device_identity':device_identity,'cases':[]}
    def save():h.atomic_json(a.root/'result.json',j)
    save()
    for backend in ['luks','luks1','veracrypt']:
        d=a.root/backend;d.mkdir(mode=0o700);key=d/'binary key Việt Nam.bin';key.write_bytes(b'\0\xff\n\r'+secrets.token_bytes(252));key.chmod(0o600);source=a.device if a.device else d/'vault.img';m=d/'mount';m.mkdir(mode=0o700)
        row={'backend':backend,'filesystem':'btrfs','filesystem_role':'inner','source_kind':'block' if a.device else 'container','source':str(source),'keyfile_sha256':h.file_hash(key),'operations':[],'status':'running'};j['cases'].append(row);save()
        def action(request):
            c=subprocess.run(['sudo','-n',str(helper),'run'],input=json.dumps(request).encode(),capture_output=True,timeout=300)
            name=str(len(row['operations']))+'-'+request['action'];(d/(name+'.stdout')).write_bytes(c.stdout);(d/(name+'.stderr')).write_bytes(c.stderr)
            row['operations'].append({'action':request['action'],'exit_code':c.returncode,'stdout':str(d/(name+'.stdout'))});save()
            response=json.loads(c.stdout);row['operations'][-1].update(strict_full_stdout_json=True,ok=response.get('ok'));save()
            if c.returncode or not response.get('ok'):raise RuntimeError('Helper action failed: '+name)
        def close():action(dict(action='unmount',target=str(m),volume_type='auto',tag=None))
        try:
            scratch_guard()
            action(dict(action='create',target=str(source),size_gb=None if a.device else '0.25',force=bool(a.device),source_kind='block' if a.device else 'file',volume_type=backend,cipher='aes',filesystem='btrfs',passphrase=None,keyfile=str(key)))
            if not a.device:assert source.stat().st_uid==1300 and source.stat().st_mode&0o777==0o600
            expected=h.load_payload(a.payload)
            for phase in ['write','append','verify']:
                action(dict(action='mount',source=str(source),mountpoint=str(m),volume_type=backend if phase=='write' else 'auto',tag=None,passphrase=None,keyfile=str(key)))
                row.update(h.bounded_payload_call(lambda:h.payload_phase(m,expected,phase,'btrfs',backend,source),90))
                native=h.linux_mount_record(Path('/proc/self/mountinfo').read_text(),m);h.check_linux_filesystem(native,'btrfs');row[phase+'_native_mount']=native
                if phase=='write':
                    proof=subprocess.run(['sudo','-n','blkid','-p',native['source']],capture_output=True,text=True,timeout=20);row['superblock']={'exit_code':proof.returncode,'stdout':proof.stdout,'stderr':proof.stderr}
                    if proof.returncode or 'TYPE="btrfs"' not in proof.stdout:raise RuntimeError('Independent Btrfs identity missing')
                save()
                if phase=='write':assert row['initial_root_metadata']['uid']==1300 and row['initial_root_metadata']['mode']=='0o700'
                if phase=='append':expected['page.txt']+=h.APPEND;expected['second.txt']=h.SECOND
                close()
            row['status']='passed'
            if not a.device and not subprocess.check_output(['sudo','-n','/usr/bin/losetup','-j',str(source)],text=True).strip():
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
            if a.device and row['status']!='passed':raise RuntimeError('Retain failed scratch state; stop before another destructive case')
    j.update(status='passed' if all(r['status']=='passed' for r in j['cases']) else 'failed',finished=time.time());save();print(json.dumps({'status':j['status'],'receipt':str(a.root/'result.json')}));return int(j['status']!='passed')
if __name__=='__main__':raise SystemExit(main())
