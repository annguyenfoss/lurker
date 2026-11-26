#!/usr/bin/env python3
"""Read-only native crypto profile proof for this exact conformance cohort."""
import hashlib,json,os,re,subprocess,time
from pathlib import Path
if os.geteuid()!=0 or os.uname().nodename!='gentoo-core-01':raise SystemExit('Owned administrator observer required')
base=Path('/srv/lurker-native-fresh-20261127-gentoo');scope=base/'followup-conformance-package';out=base/'followup-native-profile-proof.json';rows=[];seen=set()
def command(argv):
 try:
  r=subprocess.run(argv,capture_output=True,text=True,timeout=15);return {'command':argv,'exit_code':r.returncode,'stdout':r.stdout,'stderr':r.stderr}
 except Exception as e:return {'command':argv,'error':str(e)}
def save():
 t=out.with_suffix('.tmp');t.write_text(json.dumps({'source_commit':'576505d554d798422840a06322dfa93b899365bc','kernel':os.uname().release,'observer_sha256':hashlib.file_digest(Path(__file__).open('rb'),'sha256').hexdigest(),'pid':os.getpid(),'observations':rows},indent=2)+'\n');t.replace(out)
def decode(s):return re.sub(r'\\([0-7]{3})',lambda m:chr(int(m[1],8)),s)
save()
while True:
 for line in Path('/proc/self/mountinfo').read_text().splitlines():
  left,sep,right=line.partition(' - ')
  if not sep:continue
  fields=left.split();native=right.split();point=Path(decode(fields[4]))
  if not str(point).startswith(str(scope)+'/') or point.name!='mount with spaces' or str(point) in seen:continue
  try:fixture=json.loads((point.parent/'result.json').read_text())
  except (FileNotFoundError,json.JSONDecodeError):continue
  if fixture.get('source_commit')!='576505d554d798422840a06322dfa93b899365bc':raise RuntimeError('Conformance artifact provenance differs')
  if fixture['backend'] not in {'luks','luks1','veracrypt'}:continue
  source=decode(native[1]);seen.add(str(point));row={'fixture':str(point.parent),'backend':fixture['backend'],'requested_cipher':fixture.get('cipher'),'authentication':fixture.get('authentication','password'),'privilege_route':fixture['privilege_route'],'binary_sha256':fixture['binary_sha256'],'native_mount':{'mount_id':fields[0],'mountpoint':str(point),'source':source,'filesystem':native[0]},'captured':time.time()};rows.append(row)
  if source.startswith('/dev/'):
   info=command(['dmsetup','info','--noheadings','-c','-o','name',source]);row['mapper_name']=info
   if info.get('exit_code')==0:
    status=command(['cryptsetup','status',info['stdout'].strip()]);row['crypto_status']=status
    cipher=re.search(r'^\s*cipher:\s*(\S+)',status.get('stdout',''),re.MULTILINE)
    bits=re.search(r'^\s*keysize:\s*(\d+)',status.get('stdout',''),re.MULTILINE)
    row['actual_cipher']=cipher[1] if cipher else None;row['key_bits']=int(bits[1]) if bits else None
    row['requested_profile_matches']=status.get('exit_code')==0 and row['actual_cipher']==fixture.get('cipher','aes')+'-xts-plain64' and row['key_bits']==512
   if fixture['backend'] in {'luks','luks1'}:
    row['header']=command(['cryptsetup','luksDump',*(['--dump-json-metadata'] if fixture['backend']=='luks' else []),fixture['root']+'/vault 密碼 with spaces.img'])
  else:row['qualification']='Userspace compatibility route; native mapper proof not applicable. Same restricted creator is covered by privileged profile controls.'
  save()
 marker=Path('/tmp/lurker-native-fresh-20261127-gentoo/followup-conformance-package.exit')
 if marker.exists():break
 time.sleep(.1)
save();print(json.dumps({'receipt':str(out),'observations':len(rows)}))
