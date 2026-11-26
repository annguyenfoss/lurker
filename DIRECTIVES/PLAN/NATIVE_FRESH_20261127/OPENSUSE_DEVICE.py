#!/usr/bin/env python3
"""Destructive product cases fenced to the fresh openSUSE scratch serial."""
import argparse, hashlib, importlib.util, json, os, secrets, shutil, subprocess, time
from pathlib import Path

# sudo secure_path excludes locally installed, administrator-owned native tools.
os.environ["PATH"]="/usr/local/sbin:/usr/local/bin:/usr/sbin:/sbin:/usr/bin:/bin"

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for arg in ("binary","harness","payload","root","device"):p.add_argument("--"+arg,type=Path,required=True)
    p.add_argument("--commit",required=True)
    p.add_argument("--backends",default="luks,luks1,veracrypt")
    p.add_argument("--filesystems",default="ext4,btrfs,xfs,apfs,ufs1,ufs2,hammer2,ntfs,exfat,zfs")
    p.add_argument("--serial",required=True,help="Observed guest serial recorded with the new domain attachment")
    p.add_argument("--size-bytes",type=int,default=1073741824)
    p.add_argument("--skip-block",action="store_true")
    p.add_argument("--skip-directory",action="store_true")
    a=p.parse_args()
    if os.geteuid()!=0 or os.uname().nodename!="opensuse-core-01":p.error("Run with sudo on opensuse-core-01")
    identity=subprocess.check_output(["lsblk","-dn","-o","SERIAL,SIZE,TYPE",str(a.device)],text=True).strip()
    if identity.split()[0]!=a.serial or a.serial not in {"lurker-fresh-opensuse","lurker-fresh-opensus"} or identity.split()[-1]!="disk":p.error("Scratch disk serial/type mismatch")
    if int(subprocess.check_output(["blockdev","--getsize64",str(a.device)],text=True))!=a.size_bytes:p.error("Scratch size mismatch")
    spec=importlib.util.spec_from_file_location("h",a.harness);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
    a.root.mkdir(parents=True,exist_ok=True)
    expected={name:(a.payload/name).read_bytes() for name in ("bitcoin.pdf","ethereum.pdf","transformer.pdf","page.txt")}
    result={"host":os.uname().nodename,"kernel":os.uname().release,"commit":a.commit,"binary":str(a.binary),"binary_sha256":h.file_hash(a.binary),"device":str(a.device),"identity":identity,"cases":[],"status":"running","pid":os.getpid()}
    receipt=a.root/"device.json"
    def save():h.atomic_json(receipt,result)
    for fs in ([] if a.skip_block else a.filesystems.split(",")):
        for backend in a.backends.split(","):
            if backend not in {"luks","luks1","veracrypt"}:raise RuntimeError("Unknown backend")
            d=a.root/(fs+"-"+backend);d.mkdir();m=d/"mount";m.mkdir()
            row={"filesystem":fs,"backend":backend,"role":"inner","source_kind":"device","status":"running","operations":[]};result["cases"].append(row);save()
            secret="Lurker9!"+secrets.token_urlsafe(24)
            def cli(action,operands,kind=None):
                log=d/(str(len(row["operations"]))+"-"+action+".log")
                r=h.terminal([str(a.binary),action,"--type",kind or backend,"--color=never",*map(str,operands)],secret,180,log)
                row["operations"].append({"action":action,"log":str(log),**r});save()
                if r["exit_code"] or r["timed_out"]:raise RuntimeError("CLI "+action+" failed")
            try:
                current=subprocess.check_output(["lsblk","-dn","-o","SERIAL",str(a.device)],text=True).strip()
                if current!=a.serial:raise RuntimeError("Scratch serial changed")
                if subprocess.run(["findmnt","-rn","-S",str(a.device)],capture_output=True).returncode==0:raise RuntimeError("Scratch device still mounted")
                children=subprocess.check_output(["lsblk","-nr","-o","NAME",str(a.device)],text=True).splitlines()
                if len(children)!=1:raise RuntimeError("Scratch device has active child mapping; refusing overwrite")
                cli("create",["--filesystem",fs,a.device,"--force"])
                wanted=dict(expected)
                for phase in ("write","append","verify"):
                    cli("mount",[a.device,m],"auto")
                    if not os.path.ismount(m):raise RuntimeError("OS mount missing; refusing underlay writes")
                    view=subprocess.check_output(["findmnt","-rn","-M",str(m),"-o","SOURCE,FSTYPE,OPTIONS"],text=True).strip();row.setdefault("mounts",[]).append(view)
                    if "rw" not in view.split()[-1].split(","):raise RuntimeError("Mount is not read/write")
                    proof=subprocess.run(["blkid","-p",view.split()[0]],capture_output=True,text=True,timeout=15)
                    row.setdefault("superblocks",[]).append({"source":view.split()[0],"code":proof.returncode,"stdout":proof.stdout,"stderr":proof.stderr})
                    if fs in {"ufs1","ufs2"} and (proof.returncode or ('VERSION="'+("1" if fs=="ufs1" else "2")+'"') not in proof.stdout):raise RuntimeError("Independent UFS superblock version mismatch")
                    actual=view.split()[1]
                    driver={'mount_filesystem':actual,'running_kernel':os.uname().release}
                    if fs=='ufs2' and actual.startswith('fuse'):
                        binary=Path('/usr/local/bin/fuse-ufs');assert binary.stat().st_uid==0 and not binary.stat().st_mode&0o022
                        driver.update(kind='host-fuse-ufs',binary=str(binary),binary_sha256=h.file_hash(binary))
                    else:
                        module={'ufs1':'ufs','ufs2':'ufs','ntfs':'ntfs3'}.get(fs,fs)
                        driver.update(kind='kernel',module=module,modinfo=subprocess.run(['modinfo',module],capture_output=True,text=True,timeout=15).stdout)
                    row['filesystem_driver']=driver
                    row.update(h.bounded_payload_call(lambda:h.payload_phase(m,wanted,phase,fs,backend,a.device),45))
                    if phase=="append":wanted['page.txt']+=h.APPEND;wanted['second.txt']=h.SECOND
                    cli("unmount",[m],"auto")
                row["status"]="passed"
            except Exception as e:
                row.update(status="failed",error=str(e))
                if hasattr(e,"worker"):row["payload_worker"]=e.worker
            finally:
                if row.get("payload_worker",{}).get("remaining"):
                    row["mount_remaining"]=os.path.ismount(m);save()
                    raise RuntimeError("Retaining uninterruptible payload worker and owned scratch storage")
                if os.path.ismount(m):
                    try:cli("unmount",[m],"auto")
                    except Exception as e:row["cleanup_error"]=str(e)
                row["mount_remaining"]=os.path.ismount(m);save()
                if row.get("payload_worker",{}).get("remaining"):raise RuntimeError("Retaining uninterruptible payload worker and owned scratch storage")
                if row["mount_remaining"]:raise RuntimeError("Retaining live scratch disk after failed normal close")
                children=subprocess.check_output(["lsblk","-nr","-o","NAME",str(a.device)],text=True).splitlines()
                if len(children)!=1:raise RuntimeError("Retaining scratch device with active child mapping")
                if row["status"]=="failed":
                    failed=d/"failed-device.img"
                    copied=subprocess.run(["cp","--sparse=always","--",str(a.device),str(failed)],capture_output=True,text=True,timeout=180)
                    row["failed_fixture_copy"]={"path":str(failed),"exit_code":copied.returncode,"stderr":copied.stderr}
                    save()
                    if copied.returncode:raise RuntimeError("Cannot preserve failed scratch fixture; refusing another case")
    if not a.skip_directory:
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
                row['superblock']=command(['blkid','-p',partition])
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
    print(json.dumps({"receipt":str(receipt),"counts":{s:sum(c["status"]==s for c in result["cases"]) for s in ("passed","failed")}}))
if __name__=="__main__":main()
