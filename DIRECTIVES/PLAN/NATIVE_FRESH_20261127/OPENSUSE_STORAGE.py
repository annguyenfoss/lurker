#!/usr/bin/env python3
"""Move closed owned artifacts to the identified utility disk, retaining original paths."""
import hashlib,json,os,shutil,stat,subprocess,time
from pathlib import Path
ROOT=Path('/var/tmp/lurker-native-fresh-20261127-opensuse');DATA=Path('/var/lib/lurker-native-fresh-20261127-data')
def main():
    assert os.uname().nodename=='opensuse-core-01' and os.geteuid()==0 and os.path.ismount(DATA)
    disk=Path('/dev/disk/by-id/virtio-lkr-build-opensuse').resolve(strict=True)
    assert subprocess.check_output(['lsblk','-dn','-o','SERIAL',str(disk)],text=True).strip()=='lkr-build-opensuse'
    assert int(subprocess.check_output(['blockdev','--getsize64',str(disk)],text=True))==68719476736
    j={'pid':os.getpid(),'started':time.time(),'status':'running','moves':[]};receipt=ROOT/'storage-move.json'
    def save():
        t=receipt.with_suffix('.tmp');t.write_text(json.dumps(j,indent=2));t.replace(receipt)
    def inventory(root):
        result={}
        for p in [root,*root.rglob('*')]:
            s=p.lstat();r={'mode':stat.S_IMODE(s.st_mode),'uid':s.st_uid,'gid':s.st_gid}
            if p.is_symlink():r['symlink']=os.readlink(p)
            elif p.is_file():
                with p.open('rb') as f:r['sha256']=hashlib.file_digest(f,'sha256').hexdigest()
                r['size']=s.st_size
            elif not p.is_dir():raise RuntimeError('Unexpected special file: '+str(p))
            result[str(p.relative_to(root))]=r
        return result
    save()
    sources=[ROOT/'final-containers',ROOT/'final-validation',Path('/home/adjutant/.cache/lurker/remote-test/cargo/3d217e192b0807560b01/target')]
    for source in sources:
        if source==sources[1]:assert set(p.name for p in source.iterdir())=={'post.json'}
        mountinfo=Path('/proc/self/mountinfo').read_text();assert not any(str(source) in line for line in mountinfo.splitlines())
        for loop in Path('/sys/class/block').glob('loop*/loop/backing_file'):assert not loop.read_text().strip().startswith(str(source)+'/')
        for proc in Path('/proc').iterdir():
            if not proc.name.isdigit() or proc.name==str(os.getpid()):continue
            try:
                assert proc.joinpath('comm').read_text().strip() not in {'cargo','rustc'},'Active Rust build'
                refs=[proc/'cwd',*proc.joinpath('fd').iterdir()]
                for ref in refs:
                    try:value=os.readlink(ref)
                    except OSError:continue
                    assert not value.startswith(str(source)+'/'),'Open artifact reference: '+str(ref)
            except (FileNotFoundError,ProcessLookupError,PermissionError):pass
        destination=DATA/source.name;assert not destination.exists();row={'source':str(source),'destination':str(destination),'stage':'copying'};j['moves'].append(row);save()
        subprocess.run(['cp','-a','--sparse=always','--',str(source),str(destination)],check=True)
        row['stage']='verifying-complete-content';save();before=inventory(source);after=inventory(destination);assert before==after,'Copy differs'
        row.update(entries=len(before),inventory_sha256=hashlib.sha256(json.dumps(before,sort_keys=True).encode()).hexdigest());backup=source.with_name(source.name+'.native-fresh-terminal-original');assert not backup.exists();mode=source.stat().st_mode&0o777;uid=source.stat().st_uid;gid=source.stat().st_gid
        source.rename(backup);source.mkdir(mode=mode);os.chown(source,uid,gid);subprocess.run(['mount','--bind',str(destination),str(source)],check=True)
        entry=' '.join([str(destination),str(source),'none','bind,nofail,x-systemd.requires-mounts-for='+str(DATA),'0','0'])
        with Path('/etc/fstab').open('a') as f:f.write(entry+chr(10))
        row['stage']='bound-and-verified';save();assert inventory(source)==before;shutil.rmtree(backup);row['stage']='moved-path-preserved';save()
    subprocess.run(['systemctl','daemon-reload'],check=True);j.update(status='passed',finished=time.time());save();print(json.dumps({'status':'passed','receipt':str(receipt)}))
if __name__=='__main__':main()
