#!/usr/bin/env python3
"""Destructive product cases fenced to the fresh Fedora scratch serial."""
import argparse, hashlib, importlib.util, json, os, secrets, shutil, subprocess, time
from pathlib import Path

# sudo secure_path excludes locally installed, administrator-owned native tools.
os.environ["PATH"]="/usr/local/sbin:/usr/local/bin:/usr/sbin:/sbin:/usr/bin:/bin"

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for arg in ("binary","harness","payload","root","device"):p.add_argument("--"+arg,type=Path,required=True)
    p.add_argument("--commit",required=True)
    p.add_argument("--filesystems",default="ext4,btrfs,xfs,apfs,ufs1,ufs2,hammer2,ntfs,exfat,zfs")
    p.add_argument("--serial",required=True,help="Observed guest serial recorded with the new domain attachment")
    p.add_argument("--size-bytes",type=int,default=1073741824)
    a=p.parse_args()
    supplied_device=str(a.device)
    a.device=a.device.resolve(strict=True)
    device_number=a.device.stat().st_rdev
    if os.geteuid()!=0 or os.uname().nodename!="fedora-core-01":p.error("Run with sudo on fedora-core-01")
    identity=subprocess.check_output(["lsblk","-dn","-o","SERIAL,SIZE,TYPE",str(a.device)],text=True).strip()
    if identity.split()[0]!=a.serial or a.serial not in {"lurker-fresh-fedora"} or identity.split()[-1]!="disk":p.error("Scratch disk serial/type mismatch")
    if int(subprocess.check_output(["blockdev","--getsize64",str(a.device)],text=True))!=a.size_bytes:p.error("Scratch size mismatch")
    spec=importlib.util.spec_from_file_location("h",a.harness);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
    a.root.mkdir(mode=0o711,parents=True,exist_ok=True)
    a.root.chmod(0o711)
    expected={name:(a.payload/name).read_bytes() for name in ("bitcoin.pdf","ethereum.pdf","transformer.pdf","page.txt")}
    result={"host":os.uname().nodename,"kernel":os.uname().release,"commit":a.commit,"binary":str(a.binary),"binary_sha256":h.file_hash(a.binary),"device":str(a.device),"supplied_device":supplied_device,"identity":identity,"cases":[],"status":"running","pid":os.getpid()}
    matrix_receipt=a.root/"device-directories.json"
    def save():h.atomic_json(matrix_receipt,result)
    def command(argv,**kw):
        completed=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=180,**kw)
        result.setdefault('native_operations',[]).append({'argv':list(map(str,argv)),'code':completed.returncode,'stdout':completed.stdout,'stderr':completed.stderr});save()
        if completed.returncode:raise RuntimeError('Native command failed: '+str(argv[0]))
        return completed.stdout
    assert subprocess.check_output(['lsblk','-dn','-o','SERIAL',str(a.device)],text=True).strip()==a.serial
    if len(subprocess.check_output(['lsblk','-nr','-o','NAME',str(a.device)],text=True).splitlines())!=1:raise RuntimeError('Scratch has children before owned partition setup')
    command(['sfdisk','--wipe','always','--wipe-partitions','always',a.device],input='label: gpt\n, , L\n')
    command(['udevadm','settle'])
    tree=json.loads(command(['lsblk','-J','-p','-o','NAME,TYPE,PKNAME',a.device]))['blockdevices'][0]
    children=tree.get('children',[])
    if len(children)!=1 or children[0]['type']!='part' or children[0].get('children') or Path(children[0]['pkname']).resolve()!=a.device.resolve():raise RuntimeError('Expected exactly one fresh scratch partition')
    partition=Path(children[0]['name']);result['directory_partition']=str(partition);save()
    for fs in a.filesystems.split(','):
        d=a.root/('directory-'+fs);d.mkdir(mode=0o711);d.chmod(0o711);m=d/'backing';m.mkdir();plain=d/'plaintext';plain.mkdir(mode=0o700);os.chown(plain,1300,1300)
        row={'filesystem':fs,'filesystem_role':'backing','source_kind':'device-partition','partition':str(partition),'route':'native-disk-backing-directory-engines','cases':[],'status':'running'};result.setdefault('directory_backings',[]).append(row);save()
        pool='lrkf_device_'+str(os.getpid())+'_'+fs;mounted=False;failed=False
        try:
            assert subprocess.check_output(['lsblk','-dn','-o','SERIAL',str(a.device)],text=True).strip()==a.serial
            if a.device.stat().st_rdev!=device_number:raise RuntimeError('Scratch device number changed')
            if subprocess.run(['findmnt','-rn','-S',str(partition)],capture_output=True).returncode==0:raise RuntimeError('Scratch partition is already mounted')
            command(['wipefs','-a',partition])
            if fs=='zfs':command(['zpool','create','-f','-o','cachefile=none','-m',m,pool,partition]);mounted=True
            else:
                if fs in {'ufs1','ufs2'}:
                    seed=d/'seed';seed.mkdir();size=command(['blockdev','--getsize64',partition]).strip()
                    command(['makefs-freebsd','-t','ffs','-B','little','-o','version='+('1' if fs=='ufs1' else '2')+',bsize=32768,fsize=4096','-s',size,partition,seed])
                else:
                    formatter={'ext4':['mkfs.ext4','-F','-t','ext4','-q'],'btrfs':['mkfs.btrfs','-f','-q'],'xfs':['mkfs.xfs','-f','-q'],'apfs':['mkapfs'],'hammer2':['newfs_hammer2','-L','DATA'],'ntfs':['mkntfs','-Q','-F'],'exfat':['mkfs.exfat']}[fs]
                    command([*formatter,partition])
                options=['rw']
                if fs in {'ufs1','ufs2'}:options+=['ufstype='+('44bsd' if fs=='ufs1' else 'ufs2')]
                if fs=='apfs':options+=['readwrite']
                if fs in {'ntfs','exfat'}:options+=['uid=1300','gid=1300','umask=0077']
                command(['mount','-i','-t',{'ufs1':'ufs','ufs2':'ufs','ntfs':'ntfs3'}.get(fs,fs),'-o',','.join(options),partition,m]);mounted=True
            actual=h.linux_mount_record(Path('/proc/self/mountinfo').read_text(),m);row['mount']=actual;row['identity_gate']=h.check_linux_filesystem(actual,fs)
            probe=subprocess.run(['blkid','-p',str(partition)],capture_output=True,text=True,timeout=30)
            row['superblock']=probe.stdout
            row['superblock_probe_code']=probe.returncode
            if fs in {'ufs1','ufs2'} and ('VERSION="'+('1' if fs=='ufs1' else '2')+'"') not in row['superblock']:raise RuntimeError('Independent backing UFS version mismatch')
            row['driver']=command(['modinfo',{'ufs1':'ufs','ufs2':'ufs','ntfs':'ntfs3'}.get(fs,fs)])
            if fs not in {'ntfs','exfat'}:os.chown(m,1300,1300);m.chmod(0o700)
            for backend in ['gocryptfs','cryfs','cryptomator']:
                argv=['sudo','-n','-u','adjutant','--','/usr/bin/python3',a.harness,'--binary',a.binary,'--backend',backend,'--filesystem',fs,'--commit',a.commit,'--root',m,'--mount-root',plain,'--payload-dir',a.payload,'--timeout','90']
                c=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=900)
                case={'backend':backend,'code':c.returncode,'stdout':c.stdout[-4000:],'stderr':c.stderr[-4000:]};row['cases'].append(case)
                summary=json.loads(c.stdout.splitlines()[-1]);case_receipt=Path(summary['receipt']);case['receipt']=json.loads(case_receipt.read_text());case['status']=case['receipt']['status'];failed|=case['status']!='passed'
                evidence=d/backend;evidence.mkdir()
                for entry in case_receipt.parent.iterdir():
                    if entry.is_file() and (entry.name=='result.json' or entry.suffix=='.log'):shutil.copy2(entry,evidence/entry.name)
                case['copied_evidence']=str(evidence);save()
                if case['receipt'].get('payload_worker',{}).get('remaining') or case['receipt'].get('mount_remaining'):raise RuntimeError('Directory case retains live worker or mount; preserve scratch')
            row['status']='failed' if failed else 'passed'
        except Exception as error:row.update(status='failed',error=str(error));failed=True
        finally:
            if mounted:
                try:command(['zpool','export',pool] if fs=='zfs' else ['umount',m])
                except Exception as error:row['cleanup_error']=str(error)
            row['mount_remaining']=os.path.ismount(m);save()
            if row['mount_remaining'] or row.get('cleanup_error'):raise RuntimeError('Retaining scratch after failed normal backing close')
            if failed:
                image=d/'failed-backing-device.img';command(['cp','--sparse=always','--',a.device,image]);row['failed_device_copy']=str(image);save()
    result["status"]="terminal";result["finished"]=time.time();save()
    print(json.dumps({"receipt":str(matrix_receipt),"counts":{s:sum(c["status"]==s for row in result.get("directory_backings",[]) for c in row["cases"]) for s in ("passed","failed")}}))
if __name__=="__main__":main()
