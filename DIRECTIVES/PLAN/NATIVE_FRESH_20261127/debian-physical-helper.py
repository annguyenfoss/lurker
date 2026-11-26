"""Normal-user installed helper block-source controls and one ordinary reboot."""
import argparse,importlib.util,json,os,secrets,subprocess,time
from pathlib import Path
r=Path.home()/'.cache/lurker/native-fresh-20261127-debian'
s=Path('/var/tmp/lurker-native-fresh-20261127-debian-storage/final-physical-helper-576505d')
prefix=Path('/usr/lib/lurker');commit='576505d554d798422840a06322dfa93b899365bc'
def load(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
h=load('harness',r/'tools/test-filesystem-cli-576505d.py');q=load('qualifier',r/'tools/debian-qualify-canonical-device.py')
p=argparse.ArgumentParser(description=__doc__);p.add_argument('phase',choices=['prepare','verify']);a=p.parse_args()
assert os.getuid()==1300
alias=Path('/dev/disk/by-id/virtio-lurker-fresh-debian');serial='lurker-fresh-debian';identity=q.scratch_device_identity(alias,serial);device=Path(identity['path']);boot=Path('/proc/sys/kernel/random/boot_id').read_text().strip()
if a.phase=='prepare':
 s.mkdir(mode=0o700)
 j={'host':'debian','source_commit':commit,'route':'installed normal-user helper, native whole block source','source_kind':'device','identity':identity,'helper_sha256':h.file_hash(prefix/'bin/lurker-helper'),'binary_sha256':h.file_hash(prefix/'bin/lurker'),'harness_sha256':h.file_hash(Path(__file__)),'kernel_before':os.uname().release,'boot_before':boot,'started':time.time(),'status':'preparing','cases':[]}
else:
 j=json.loads((s/'result.json').read_text());assert j['status']=='reboot_ready' and j['boot_before']!=boot
 assert j['helper_sha256']==h.file_hash(prefix/'bin/lurker-helper') and j['binary_sha256']==h.file_hash(prefix/'bin/lurker')
 j.update(status='verifying',boot_after=boot,kernel_after=os.uname().release)
def save():
 t=s/'result.tmp';t.write_text(json.dumps(j,indent=2)+'\n');t.replace(s/'result.json')
def action(row,d,request):
 c=subprocess.run([str(prefix/'bin/lurker-helper'),'run'],input=json.dumps(request).encode(),capture_output=True,timeout=300)
 name=f'{len(row["operations"]):02d}-{request["action"]}-{a.phase}'
 (d/(name+'.stdout')).write_bytes(c.stdout);(d/(name+'.stderr')).write_bytes(c.stderr)
 response=json.loads(c.stdout);row['operations'].append({'action':request['action'],'exit':c.returncode,'strict_full_stdout_json':True,'response':response});save();assert c.returncode==0 and response['ok'],response
save()
try:
 if a.phase=='prepare':
  for backend in ['luks1','veracrypt','luks']:
   q.scratch_device_identity(alias,serial)
   d=s/backend;d.mkdir(mode=0o700);key=d/'binary key Việt Nam.bin';key.write_bytes(b'\0\xff\n\r'+secrets.token_bytes(252));key.chmod(0o600);mount=d/'mount';mount.mkdir(mode=0o700)
   row={'backend':backend,'filesystem':'btrfs','role':'inner','source_kind':'device','source':str(device),'device_identity':identity,'private_fixture_credential':str(key),'mountpoint':str(mount),'operations':[],'status':'running'};j['cases'].append(row);save()
   try:
    action(row,d,dict(action='create',target=str(device),size_gb=None,force=True,source_kind='block',volume_type=backend,cipher='aes',filesystem='btrfs',passphrase=None,keyfile=str(key)))
    expected=h.load_payload(r/'payload')
    for phase in ['write','append','verify']:
     action(row,d,dict(action='mount',source=str(device),mountpoint=str(mount),volume_type=backend if phase=='write' else 'auto',tag=None,passphrase=None,keyfile=str(key)))
     row.update(h.bounded_payload_call(lambda:h.payload_phase(mount,expected,phase,'btrfs',backend,device),90))
     action(row,d,dict(action='unmount',target=str(mount),volume_type='auto',tag=None))
     if phase=='append':expected['page.txt']+=h.APPEND;expected['second.txt']=h.SECOND
    row['status']='passed';save()
    if backend=='luks':
     q.scratch_device_identity(alias,serial)
     action(row,d,dict(action='mount',source=str(device),mountpoint=str(mount),volume_type='auto',tag=None,passphrase=None,keyfile=str(key)))
     row['before_boot_sha256']=h.bounded_payload_call(lambda:h.verify_payload(mount,expected),90)
     assert row['before_boot_sha256']==row['verify_sha256'];row['left_for_reboot']=True;save()
   finally:
    if os.path.ismount(mount) and not row.get('left_for_reboot'):action(row,d,dict(action='unmount',target=str(mount),volume_type='auto',tag=None))
  subprocess.run(['/usr/bin/sync'],check=True,timeout=120);j['status']='reboot_ready'
 else:
  row=j['cases'][-1];assert row['backend']=='luks' and row['left_for_reboot'];d=s/row['backend'];mount=Path(row['mountpoint']);key=Path(row['private_fixture_credential']);assert not key.stat().st_mode&0o077
  action(row,d,dict(action='mount',source=str(device),mountpoint=str(mount),volume_type='auto',tag=None,passphrase=None,keyfile=str(key)))
  try:
   expected=h.load_payload(r/'payload');expected['page.txt']+=h.APPEND;expected['second.txt']=h.SECOND
   row['after_boot_mount']=q.require_native_filesystem(mount,'btrfs');row['after_boot_sha256']=h.bounded_payload_call(lambda:h.verify_payload(mount,expected),90);assert row['after_boot_sha256']==row['before_boot_sha256']
  finally:action(row,d,dict(action='unmount',target=str(mount),volume_type='auto',tag=None))
  row['left_for_reboot']=False;row['mount_remaining']=os.path.ismount(mount);assert not row['mount_remaining'];j['status']='passed'
except Exception as error:
 j.update(status='failed',error=str(error))
 if isinstance(error,h.PayloadWorkerError):j.setdefault('payload_workers',[]).append(error.worker)
j['updated_at']=time.time();save();print(json.dumps({'status':j['status'],'receipt':str(s/'result.json')}));raise SystemExit(j['status'] not in ['passed','reboot_ready'])
