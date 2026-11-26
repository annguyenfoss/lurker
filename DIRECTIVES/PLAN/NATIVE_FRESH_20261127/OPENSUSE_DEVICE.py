#!/usr/bin/env python3
"""Destructive product cases fenced to the fresh openSUSE scratch serial."""
import argparse, hashlib, importlib.util, json, os, secrets, subprocess, time
from pathlib import Path

# sudo secure_path excludes locally installed, administrator-owned native tools.
os.environ["PATH"]="/usr/local/sbin:/usr/local/bin:/usr/sbin:/sbin:/usr/bin:/bin"

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for arg in ("binary","harness","payload","root","device"):p.add_argument("--"+arg,type=Path,required=True)
    p.add_argument("--commit",required=True)
    p.add_argument("--backends",default="luks,luks1,veracrypt")
    p.add_argument("--filesystems",default="ext4,btrfs,xfs,apfs,ufs1,ufs2,hammer2,ntfs,exfat,zfs")
    a=p.parse_args()
    if os.geteuid()!=0 or os.uname().nodename!="opensuse-core-01":p.error("Run with sudo on opensuse-core-01")
    identity=subprocess.check_output(["lsblk","-dn","-o","SERIAL,SIZE,TYPE",str(a.device)],text=True).strip()
    if identity.split()[0]!="lurker-fresh-opensuse" or identity.split()[-1]!="disk":p.error("Scratch disk serial/type mismatch")
    spec=importlib.util.spec_from_file_location("h",a.harness);h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
    a.root.mkdir(parents=True,exist_ok=True)
    expected={name:(a.payload/name).read_bytes() for name in ("bitcoin.pdf","ethereum.pdf","transformer.pdf","page.txt")}
    result={"host":os.uname().nodename,"kernel":os.uname().release,"commit":a.commit,"binary":str(a.binary),"binary_sha256":h.file_hash(a.binary),"device":str(a.device),"identity":identity,"cases":[],"status":"running","pid":os.getpid()}
    receipt=a.root/"device.json"
    def save():h.atomic_json(receipt,result)
    for fs in a.filesystems.split(","):
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
                if current!="lurker-fresh-opensuse":raise RuntimeError("Scratch serial changed")
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
                    dst=m/"payload"
                    if phase=="write":
                        dst.mkdir(mode=0o700)
                        for name,data in wanted.items():
                            with (dst/name).open("xb") as f:f.write(data);f.flush();os.fsync(f.fileno())
                    for name,data in wanted.items():
                        if (dst/name).read_bytes()!=data:raise RuntimeError("Payload mismatch "+name)
                    if phase=="append":
                        wanted["page.txt"]+=b"\nopenSUSE device reopen append.\n";wanted["second.txt"]=b"new file on reopened device\n"
                        for name in ("page.txt","second.txt"):
                            with (dst/name).open("wb") as f:f.write(wanted[name]);f.flush();os.fsync(f.fileno())
                    row[phase+"_hashes"]={name:hashlib.sha256((dst/name).read_bytes()).hexdigest() for name in wanted}
                    cli("unmount",[m],"auto")
                row["status"]="passed"
            except Exception as e:row.update(status="failed",error=str(e))
            finally:
                if os.path.ismount(m):
                    try:cli("unmount",[m],"auto")
                    except Exception as e:row["cleanup_error"]=str(e)
                row["mount_remaining"]=os.path.ismount(m);save()
                if row["mount_remaining"]:raise RuntimeError("Retaining live scratch disk after failed normal close")
                children=subprocess.check_output(["lsblk","-nr","-o","NAME",str(a.device)],text=True).splitlines()
                if len(children)!=1:raise RuntimeError("Retaining scratch device with active child mapping")
                if row["status"]=="failed":
                    failed=d/"failed-device.img"
                    copied=subprocess.run(["cp","--sparse=always","--",str(a.device),str(failed)],capture_output=True,text=True,timeout=180)
                    row["failed_fixture_copy"]={"path":str(failed),"exit_code":copied.returncode,"stderr":copied.stderr}
                    save()
                    if copied.returncode:raise RuntimeError("Cannot preserve failed scratch fixture; refusing another case")
    result["status"]="terminal";result["finished"]=time.time();save()
    print(json.dumps({"receipt":str(receipt),"counts":{s:sum(c["status"]==s for c in result["cases"]) for s in ("passed","failed")}}))
if __name__=="__main__":main()
