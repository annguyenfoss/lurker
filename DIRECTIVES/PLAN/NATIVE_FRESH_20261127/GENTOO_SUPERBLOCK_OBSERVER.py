#!/usr/bin/env python3
"""Record independent superblocks for only this round's native container mounts."""
import hashlib,json,os,subprocess,time
from pathlib import Path
if os.geteuid()!=0 or os.uname().nodename!='gentoo-core-01':raise SystemExit('Owned Gentoo administrator observer required')
base=Path('/srv/lurker-native-fresh-20261127-gentoo');scope=str(base/'followup-containers');out=base/'followup-container-superblocks.json';rows=[];seen=set()
def sha(path):return hashlib.file_digest(Path(path).open('rb'),'sha256').hexdigest()
identity={'source_commit':'576505d554d798422840a06322dfa93b899365bc','observer_sha256':sha(__file__),'driver_sha256':sha('/usr/local/bin/fuse-ufs'),'kernel':os.uname().release,'pid':os.getpid()}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps({**identity,'observations':rows},indent=2)+'\n');tmp.replace(out)
save()
while True:
 for line in Path('/proc/self/mountinfo').read_text().splitlines():
  left,sep,right=line.partition(' - ')
  if not sep:continue
  fields=left.split();native=right.split();point=fields[4];key=(fields[0],point)
  if not point.startswith(scope+'/') or key in seen:continue
  source=native[1]
  if not (source.startswith('/dev/') or source.startswith(scope+'/')):continue
  seen.add(key);r=subprocess.run(['blkid','-p',source],capture_output=True,text=True,timeout=10)
  rows.append({'mount_id':fields[0],'mountpoint':point,'source':source,'fstype':native[0],'options':fields[5],'time':time.time(),'command':['blkid','-p',source],'exit_code':r.returncode,'stdout':r.stdout,'stderr':r.stderr});save()
 try:
  if json.loads((base/'followup-containers/matrix.json').read_text()).get('status')=='terminal':break
 except FileNotFoundError:pass
 time.sleep(.1)
identity['status']='terminal';save();print(json.dumps({'receipt':str(out),'observations':len(rows)}))
