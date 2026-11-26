#!/usr/bin/env python3
"""openSUSE fresh-round native backing controls; runs only on the owned VM."""
import argparse, hashlib, json, os, shutil, subprocess, time
from pathlib import Path

# sudo secure_path excludes locally installed, administrator-owned native tools.
os.environ["PATH"]="/usr/local/sbin:/usr/local/bin:/usr/sbin:/sbin:/usr/bin:/bin"

FILES = ("ext4", "btrfs", "xfs", "apfs", "ufs1", "ufs2", "hammer2", "ntfs", "exfat", "zfs")

def run(argv, **kw):
    p = subprocess.run(list(map(str, argv)), capture_output=True, text=True, timeout=180, **kw)
    return {"argv": list(map(str, argv)), "code": p.returncode, "stdout": p.stdout[-8000:], "stderr": p.stderr[-8000:]}

def main():
    a = argparse.ArgumentParser(description=__doc__)
    a.add_argument("--root", required=True, type=Path)
    a.add_argument("--payload", required=True, type=Path)
    a.add_argument("--device", type=Path)
    a.add_argument("--serial")
    a.add_argument("--filesystems", default=",".join(FILES))
    x = a.parse_args()
    if os.uname().nodename != "opensuse-core-01" or os.geteuid() != 0:
        a.error("Run with sudo on opensuse-core-01 only")
    if x.device:
        if x.serial != "lurker-fresh-opensuse": a.error("Expected round-specific serial")
        identity = run(["lsblk", "-dn", "-o", "SERIAL,SIZE,TYPE", x.device])
        if identity["code"] or identity["stdout"].split()[0] != x.serial or identity["stdout"].split()[-1] != "disk":
            a.error("Scratch device identity mismatch")
        if run(["findmnt", "-rn", "-S", x.device])["code"] == 0: a.error("Scratch device is mounted")
    x.root.mkdir(parents=True, exist_ok=True)
    result = {"host":os.uname().nodename,"kernel":os.uname().release,"pid":os.getpid(),"started":time.time(),"cases":[]}
    receipt = x.root / "native.json"
    def save():
        temp=receipt.with_suffix(".tmp");temp.write_text(json.dumps(result,indent=2)+"\n");temp.replace(receipt)
    expected={p.name:p.read_bytes() for p in x.payload.iterdir() if p.name in {"bitcoin.pdf","ethereum.pdf","transformer.pdf","page.txt"}}
    if len(expected)!=4: a.error("Missing common downloaded payload")
    for fs in x.filesystems.split(","):
        d=x.root/fs;d.mkdir(exist_ok=True);m=d/"mount";m.mkdir(exist_ok=True)
        image=x.device or d/"native.img"
        row={"filesystem":fs,"role":"native filesystem control","source_kind":"device" if x.device else "container","source":str(image),"operations":[],"status":"running"}
        if x.device:row["identity"]=identity
        result["cases"].append(row);save();mounted=False
        pool="lrkf_opensuse_"+str(os.getpid())+"_"+fs
        def cmd(argv, required=True):
            r=run(argv);row["operations"].append(r);save()
            if required and r["code"]:raise RuntimeError(str(argv[0])+" failed: "+r["stderr"][-1500:])
            return r
        def mount():
            nonlocal mounted
            if fs=="zfs":cmd(["zpool","import","-d",str(image.parent),pool])
            elif fs=="ufs2":cmd(["/usr/local/bin/fuse-ufs","-o","rw,allow_other,fsname="+str(image),image,m])
            else:
                opts=[] if x.device else ["loop"]
                if fs=="apfs":opts += ["readwrite"]
                if fs=="ufs1":opts += ["rw","ufstype=44bsd"]
                c=["mount",*(["-i"] if fs!="ntfs" else []),"-t",{"ufs1":"ufs","ntfs":"ntfs-3g"}.get(fs,fs)]
                if opts:c += ["-o",",".join(opts)]
                cmd(c+[image,m])
            for _ in range(100):
                if os.path.ismount(m):break
                time.sleep(.02)
            mounted=os.path.ismount(m)
            if not mounted:raise RuntimeError("Missing OS mount; refusing underlay writes")
            cmd(["findmnt","-rn","-M",m,"-o","SOURCE,TARGET,FSTYPE,OPTIONS"])
        def close():
            nonlocal mounted
            cmd(["zpool","export",pool] if fs=="zfs" else ["umount",m]);mounted=False
        try:
            if not x.device:
                with image.open("xb") as f:f.truncate(768*1024**2)
            else:cmd(["wipefs","-a",image])
            if fs=="zfs":
                cmd(["zpool","create","-f","-o","cachefile=none","-m",m,pool,image]);mounted=True;close()
            elif fs in {"ufs1","ufs2"}:
                seed=d/"seed";seed.mkdir(exist_ok=True)
                size=cmd(["blockdev","--getsize64",image])["stdout"].strip() if x.device else str(image.stat().st_size)
                cmd(["makefs-freebsd","-t","ffs","-B","little","-o","version="+("1" if fs=="ufs1" else "2")+",bsize=32768,fsize=4096","-s",size,image,seed])
            else:
                formatter={"ext4":["mkfs.ext4","-F","-q"],"btrfs":["mkfs.btrfs","-f","-q"],"xfs":["mkfs.xfs","-f","-q"],"apfs":["mkapfs"],"hammer2":["newfs_hammer2","-L","DATA"],"ntfs":["mkntfs","-Q","-F"],"exfat":["mkfs.exfat"]}[fs]
                cmd(formatter+[image])
            wanted=dict(expected)
            for phase in ("write","append","verify"):
                mount();p=m/"payload"
                if phase=="write":
                    p.mkdir(mode=0o700)
                    for name,data in expected.items():
                        with (p/name).open("xb") as f:f.write(data);f.flush();os.fsync(f.fileno())
                    probe=m/"feature-probe";probe.write_text("feature probe\n");os.chmod(probe,0o600)
                    row["features"]={"acl":cmd(["setfacl","-m","u:1300:rw",probe],False),"chmod":cmd(["chmod","640",probe],False),"chown":cmd(["chown","1300:1300",probe],False),"ownership_after":cmd(["stat","-c","%u:%g %a",probe],False),"reflink":cmd(["cp","--reflink=always",probe,m/"reflink-probe"],False)}
                    if fs=="ext4":row["features"]["journaling"]=cmd(["tune2fs","-l",image],False)
                    elif fs=="xfs":row["features"]["journaling"]=cmd(["xfs_info",m],False)
                    elif fs=="zfs":
                        row["features"]["properties"]=cmd(["zfs","get","sync,acltype,xattr,compression",pool],False)
                        row["features"]["enable_scoped_acl"]=cmd(["zfs","set","acltype=posixacl",pool],False)
                        row["features"]["configured_acl"]=cmd(["setfacl","-m","u:1300:rw",probe],False)
                    elif fs in {"btrfs","apfs","hammer2"}:row["features"]["journaling"]="not-applicable: CoW filesystem; no journal assertion"
                    else:row["features"]["journaling"]="unknown"
                for name,data in wanted.items():
                    if (p/name).read_bytes()!=data:raise RuntimeError("Payload mismatch "+name)
                if phase=="append":
                    wanted["page.txt"] += b"\nFresh openSUSE normal reopen append.\n";wanted["second.txt"]=b"second mounted file\n"
                    for name in ("page.txt","second.txt"):
                        with (p/name).open("wb") as f:f.write(wanted[name]);f.flush();os.fsync(f.fileno())
                row[phase+"_hashes"]={n:hashlib.sha256((p/n).read_bytes()).hexdigest() for n in wanted}
                cmd(["sync","-f",m]);close()
            row["status"]="passed"
            if not x.device:image.unlink();row["passed_fixture_removed"]=True
        except Exception as e:row.update(status="failed",error=str(e))
        finally:
            if mounted or os.path.ismount(m):
                try:close()
                except Exception as e:row["cleanup_error"]=str(e)
            row["mount_remaining"]=os.path.ismount(m);save()
    result["finished"]=time.time();result["status"]="terminal";save()
    print(json.dumps({"receipt":str(receipt),"counts":{s:sum(r["status"]==s for r in result["cases"]) for s in ("passed","failed")}}))
if __name__=="__main__":main()
