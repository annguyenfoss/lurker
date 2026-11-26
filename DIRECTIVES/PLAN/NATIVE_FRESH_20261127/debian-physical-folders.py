#!/usr/bin/env python3
"""Debian-owned native scratch backing coverage; direct folder/block cells are NA."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('root','prefix','harness','qualifier','round-runner','device'):
        p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--serial',required=True)
    p.add_argument('--commit',required=True)
    p.add_argument('--filesystems',required=True)
    a=p.parse_args()
    assert os.getuid()==1300 and 'debian' in Path('/etc/os-release').read_text()
    assert a.serial=='lurker-fresh-debian'
    os.environ['PATH']='/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:'+os.environ.get('PATH','')
    def load(name,path):
        spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
    h=load('harness',a.harness);q=load('qualifier',a.qualifier);rr=load('round_runner',a.round_runner)
    a.root.mkdir(mode=0o700,parents=True,exist_ok=True)
    def command(argv,timeout=90):
        c=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=timeout)
        return {'argv':list(map(str,argv)),'exit':c.returncode,'stdout':c.stdout[-12000:],'stderr':c.stderr[-12000:]}
    def guard():
        assert a.device.is_block_device()
        size,serial=subprocess.check_output(['lsblk','-dnbo','SIZE,SERIAL',str(a.device)],text=True).strip().split()
        assert int(size)==1073741824 and serial==a.serial,(size,serial)
        assert not subprocess.check_output(['lsblk','-nro','MOUNTPOINT',str(a.device)],text=True).strip()
        for dev in [a.device.resolve(),*a.device.resolve().parent.glob(a.device.resolve().name+'[0-9]*')]:
            assert not list((Path('/sys/class/block')/dev.name/'holders').iterdir()),dev
        return {'path':str(a.device.resolve()),'size':int(size),'serial':serial}
    for fs in a.filesystems.split(','):
        assert fs in q.FILESYSTEMS
        d=a.root/fs
        if (d/'result.json').exists():continue
        d.mkdir(mode=0o700);mount=d/'backing';mount.mkdir(mode=0o700)
        pool='lrk_debian_physical_'+fs
        row={'host':'debian','source_commit':a.commit,'kernel':os.uname().release,'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip(),'binary_sha256':h.file_hash(a.prefix/'bin/lurker'),'qualifier_sha256':h.file_hash(Path(__file__)),'common_harness_sha256':h.file_hash(a.harness),'filesystem':fs,'role':'backing','backing_source_kind':'device','source_kind':'directory','direct_block_source_status':'not-applicable','direct_block_source_reason':'Directory engines encrypt a directory on this native filesystem; they do not format or encrypt a whole block device','operations':[],'cases':[],'status':'running','started':time.time(),
             'tool_versions':json.loads((Path.home()/'.cache/lurker/native-fresh-20261127-debian/tools.json').read_text())}
        def save():
            t=d/'result.tmp';t.write_text(json.dumps(row,indent=2)+'\n');t.replace(d/'result.json')
        def run(argv,timeout=90):
            observed=command(argv,timeout);row['operations'].append(observed);save()
            if observed['exit']:raise RuntimeError(observed)
            return observed
        try:
            row['device_identity']=guard();save()
            run(['sudo','-n','wipefs','--all',a.device]);run(['sudo','-n','blockdev','--rereadpt',a.device])
            if fs=='zfs':
                run(['sudo','-n','zpool','create','-f','-o','cachefile=none','-m',mount,pool,a.device])
            elif fs in {'ufs1','ufs2'}:
                image=d/'native-seed.img';seed=d/'seed';seed.mkdir(mode=0o700)
                run(['makefs-freebsd','-t','ffs','-B','little','-o',f'version={1 if fs=="ufs1" else 2},bsize=32768,fsize=4096','-s',str(512*1024**2),image,seed])
                guard();run(['sudo','-n','dd','if='+str(image),'of='+str(a.device),'bs=4M','conv=fsync','status=none'])
                row['native_seed_sha256']=h.file_hash(image)
                if fs=='ufs1':run(['sudo','-n','mount','-i','-t','ufs','-o','rw,ufstype=44bsd',a.device,mount])
                else:run(['sudo','-n','/usr/local/bin/fuse-ufs','-o',f'rw,allow_other,fsname={a.device}',a.device,mount])
            else:
                formatter={'ext4':['mkfs.ext4','-q','-F'],'btrfs':['mkfs.btrfs','-q','-f'],'xfs':['mkfs.xfs','-q','-f'],'apfs':['mkapfs'],'hammer2':['newfs_hammer2','-L','DATA'],'ntfs':['mkntfs','-Q','-F'],'exfat':['mkfs.exfat']}[fs]
                guard();run(['sudo','-n',*formatter,a.device])
                kind=rr.ntfs_mount_driver() if fs=='ntfs' else fs
                options=['rw']
                if fs in {'ntfs','exfat'}:options += ['uid=1300','gid=1300','umask=0077']
                if fs=='apfs':options += ['readwrite']
                run(['sudo','-n','mount',*(['-i'] if kind!='ntfs-3g' else []),'-t',kind,'-o',','.join(options),a.device,mount])
            deadline=time.monotonic()+5
            while not os.path.ismount(mount) and time.monotonic()<deadline:time.sleep(.025)
            assert os.path.ismount(mount)
            row['native_mount']=q.require_native_filesystem(mount,fs)
            if fs not in {'ntfs','exfat'}:run(['sudo','-n','chown','1300:1300',mount]);mount.chmod(0o700)
            row['native_features']=h.bounded_payload_call(lambda:q.observe_features(mount,True),30);save()
            for backend in q.DIRECTORIES:
                c=command(['python3',a.harness,'--binary',a.prefix/'bin/lurker','--backend',backend,'--filesystem',fs,'--commit',a.commit,'--root',mount,'--mount-root',d,'--payload-dir',Path.home()/'.cache/lurker/native-fresh-20261127-debian/payload','--timeout','90'],900)
                item={'backend':backend,'command':c};row['cases'].append(item)
                summary=json.loads(c['stdout'].splitlines()[-1]);receipt=Path(summary['receipt']);native=json.loads(receipt.read_text());item['receipt']=native
                evidence=d/backend;evidence.mkdir(mode=0o700)
                for entry in Path(native['fixture']).iterdir():
                    if entry.is_file() and (entry.suffix=='.log' or entry.name=='result.json'):shutil.copy2(entry,evidence/entry.name)
                item['status']=native['status'];save()
                if native.get('mount_remaining'):raise RuntimeError('Folder plaintext mount remains; preserve native backing')
            row['status']='passed' if all(c['status']=='passed' for c in row['cases']) else 'failed'
        except Exception as error:row.update(status='failed',error=str(error))
        finally:
            if os.path.ismount(mount):
                try:run(['sudo','-n','zpool','export',pool] if fs=='zfs' else ['sudo','-n','umount',mount])
                except Exception as error:row['cleanup_error']=str(error)
            row['mount_remaining']=os.path.ismount(mount)
            if row['status']!='passed' and not row['mount_remaining']:
                try:
                    guard();snapshot=d/'failed-device.img'
                    with snapshot.open('xb') as out:
                        snapshot.chmod(0o600);subprocess.run(['sudo','-n','cat',str(a.device)],stdout=out,check=True,timeout=120)
                    row['failed_device_snapshot_sha256']=h.file_hash(snapshot)
                except Exception as error:row['snapshot_error']=str(error)
            row['finished']=time.time();save();print(json.dumps({'filesystem':fs,'status':row['status'],'receipt':str(d/'result.json')}),flush=True)
        if row['mount_remaining'] or row.get('cleanup_error') or row.get('snapshot_error'):return 1
    return 0

if __name__=='__main__':raise SystemExit(main())
