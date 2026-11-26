#!/usr/bin/env python3
"""Guarded Gentoo round-owned disk/bootstrap operations; no formatter or kernel build."""
import argparse,hashlib,importlib.util,json,os,re,stat,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('phase',choices=['mount-build','clear-passed-zfs','mount-ufs1','close-ufs1','schedule-old-kernel']);a=p.parse_args()
if os.geteuid()!=0 or os.uname().nodename!='gentoo-core-01':raise SystemExit('Owned Gentoo administrator required')
base=Path('/srv/lurker-native-fresh-20261127-gentoo');prefix=Path('/opt/lurker-native-fresh-20261127-gentoo-576');commit='576505d554d798422840a06322dfa93b899365bc';binary_sha='21959b16e303f1f586884d6374702813ef3a41c42948f19e6bbd41f0f71e315d';old='6.18.64-p1-gentoo-dist-bin';module=Path('/lib/modules')/old/'extra/ufs.ko';module_sha='1e0cee7875b995925c3f9737c335245d08baa99cd6c1084e3a4fc3ea76a76bd5';m=base/'supplemental-ufs1-reboot-native/mount'
r={'phase':a.phase,'kernel':os.uname().release,'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip(),'source_commit':commit,'operations':[],'status':'running'}
def sha(path):return hashlib.file_digest(Path(path).open('rb'),'sha256').hexdigest()
def command(argv,allowed=(0,)):
 c=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=120);r['operations'].append({'command':list(map(str,argv)),'exit_code':c.returncode,'stdout':c.stdout[-12000:],'stderr':c.stderr[-3000:]})
 if c.returncode not in allowed:raise RuntimeError(str(argv[0])+' failed: '+c.stderr[-1000:])
 return c.stdout
def identity(path,serial,size):
 d=Path(path).resolve(strict=True)
 if not stat.S_ISBLK(d.stat().st_mode):raise RuntimeError('Block device required')
 j=json.loads(command(['lsblk','--json','--tree','--bytes','-o','PATH,SERIAL,SIZE,TYPE,FSTYPE,UUID,MOUNTPOINTS',d]))['blockdevices'][0]
 if j.get('serial')!=serial or j.get('size')!=size or j.get('type')!='disk':raise RuntimeError('Owned disk identity differs')
 return d,j
def mount_record(path):
 text=command(['findmnt','--json','--mountpoint',path,'-o','SOURCE,FSTYPE,OPTIONS,TARGET']);return json.loads(text)['filesystems'][0]
def require_build():
 d,j=identity('/dev/disk/by-id/virtio-lurker-build-gentoo','lurker-build-gentoo',68719476736)
 if j.get('fstype')!='ext4' or j.get('uuid')!='0f324a9f-46f2-48d5-8194-67715bfac82e':raise RuntimeError('Build filesystem identity differs')
 if not os.path.ismount(base):raise RuntimeError('Verified build filesystem must be mounted')
 rec=mount_record(base)
 if rec['fstype']!='ext4' or Path(rec['source']).resolve()!=d:raise RuntimeError('Build mount source differs')
 return d,j
def no_usage(d,j):
 if any(j.get('mountpoints') or []) or list((Path('/sys/class/block')/d.name/'holders').iterdir()):raise RuntimeError('Scratch disk is in use')
 for c in j.get('children',[]):
  cp=Path(c['path'])
  if c.get('type')!='part' or c.get('children') or any(c.get('mountpoints') or []) or list((Path('/sys/class/block')/cp.name/'holders').iterdir()):raise RuntimeError('Scratch child is in use')
def existing_ufs():
 if os.uname().release!=old or module.stat().st_uid!=0 or module.stat().st_mode&0o022 or sha(module)!=module_sha:raise RuntimeError('Exact existing matching module required')
 if any(line.partition(' - ')[2].split()[:1]==['ufs'] for line in Path('/proc/self/mountinfo').read_text().splitlines()):raise RuntimeError('Existing UFS mounts prevent scoped module activation')
 metadata=command(['/usr/bin/env','--argv0=modinfo','/usr/bin/kmod',module]);v=next(line.partition(':')[2].strip().split()[0] for line in metadata.splitlines() if line.startswith('vermagic:'))
 if v!=old:raise RuntimeError('Module vermagic differs')
 if Path('/sys/module/ufs').exists():command(['/usr/bin/env','--argv0=modprobe','/usr/bin/kmod','-r','ufs'])
 command(['/usr/bin/env','--argv0=insmod','/usr/bin/kmod',module]);r['existing_module_sha256']=module_sha
try:
 if sha(prefix/'bin/lurker')!=binary_sha:raise RuntimeError('Frozen product changed')
 if a.phase=='mount-build':
  d,j=identity('/dev/disk/by-id/virtio-lurker-build-gentoo','lurker-build-gentoo',68719476736)
  if j.get('fstype')!='ext4' or j.get('uuid')!='0f324a9f-46f2-48d5-8194-67715bfac82e':raise RuntimeError('Build filesystem differs')
  if not os.path.ismount(base):command(['mount','-t','ext4',d,base])
  require_build()
 else:
  require_build()
  if a.phase=='clear-passed-zfs':
   old_receipt=base/'followup-device-backing-resumed/device-backing-matrix.json';j=json.loads(old_receipt.read_text());row=j['cases'][-1]
   if j['source_commit']!=commit or j['binary_sha256']!=binary_sha or row['filesystem']!='zfs' or row['status']!='passed' or row['mount_remaining'] or any(c['status']!='passed' or c.get('mount_remaining') for c in row['engines']):raise RuntimeError('Passed exported ZFS backing receipt differs')
   d,disk=identity('/dev/disk/by-id/virtio-lurker-fresh-gentoo','lurker-fresh-gentoo',2147483648);no_usage(d,disk);pool='lrkf_phys_'+Path(row['fixture']).name.replace('-','_')
   pools=command(['zpool','list','-H','-o','name'],allowed=(0,1)).splitlines()
   if pool in pools:raise RuntimeError('Owned test pool still imported')
   children=[Path(c['path']) for c in disk.get('children',[])];labels=[]
   for part in children or [d]:
    text=command(['blkid','-p','-o','export',part],allowed=(0,2));props=dict(line.split('=',1) for line in text.splitlines() if '=' in line)
    if props.get('TYPE')=='zfs_member':
     if props.get('LABEL')!=pool:raise RuntimeError('Foreign ZFS label; refuse wipe')
     labels.append(str(part))
    elif props.get('TYPE'):raise RuntimeError('Unexpected filesystem on owned test child')
   if not labels:raise RuntimeError('Exact passed test-pool label missing')
   r['passed_pool']=pool;r['label_devices']=labels
   for part in children:command(['wipefs','--all',part])
   command(['wipefs','--all',d]);command(['blockdev','--rereadpt',d]);post_d,post=identity(d,'lurker-fresh-gentoo',2147483648);no_usage(post_d,post)
   if post.get('children'):raise RuntimeError('Partition devices remain; do not format')
   r['post_clear_identity']=post
  elif a.phase=='mount-ufs1':
   proof=json.loads((base/'supplemental-device-backing/device-backing-matrix.json').read_text());row=proof['cases'][-1]
   if proof['source_commit']!=commit or row['filesystem']!='ufs1' or row['status']!='passed' or row['mount_remaining']:raise RuntimeError('Supplemental raw UFS1 backing receipt differs')
   if os.path.ismount(m):raise RuntimeError('Reboot UFS1 backing already mounted; inspect before reuse')
   d,disk=identity('/dev/disk/by-id/virtio-lurker-fresh-gentoo','lurker-fresh-gentoo',2147483648);no_usage(d,disk)
   expected=next(re.search(r'UUID="([^"]+)"',x['stdout'])[1] for x in row['superblock_observations'] if x.get('exit_code')==0 and 'VERSION="1"' in x.get('stdout',''))
   text=command(['blkid','-p',d])
   if 'VERSION="1"' not in text or 'UUID="'+expected+'"' not in text:raise RuntimeError('Supplemental UFS1 superblock identity differs')
   existing_ufs();m.parent.mkdir(mode=0o700,exist_ok=True);os.chown(m.parent,1300,1300);m.mkdir(mode=0o700,exist_ok=True);command(['mount','-i','-t','ufs','-o','rw,ufstype=44bsd',d,m])
   spec=importlib.util.spec_from_file_location('h',prefix/'harness/test-filesystem-cli.py');h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h);rec=h.linux_mount_record(Path('/proc/self/mountinfo').read_text(),m);h.check_linux_filesystem(rec,'ufs1')
   if rec['mountpoint']!=str(m) or Path(rec['source']).resolve()!=d:raise RuntimeError('Native UFS1 mount source differs')
   r['native_mount']=rec;r['superblock']=text
  elif a.phase=='close-ufs1':
   d,disk=identity('/dev/disk/by-id/virtio-lurker-fresh-gentoo','lurker-fresh-gentoo',2147483648);rec=mount_record(m)
   if Path(rec['source']).resolve()!=d or rec['fstype']!='ufs':raise RuntimeError('Not the owned native UFS1 mount')
   command(['umount',m]);d,disk=identity(d,'lurker-fresh-gentoo',2147483648);no_usage(d,disk)
  elif a.phase=='schedule-old-kernel':
   qualified="GRUB_DEFAULT='Advanced options for Gentoo GNU/Linux>Gentoo GNU/Linux, with Linux 7.3.6-gentoo-dist-bin'"
   if qualified not in Path('/etc/default/grub').read_text().splitlines():raise RuntimeError('Configured qualified default differs')
   title='Advanced options for Gentoo GNU/Linux>Gentoo GNU/Linux, with Linux '+old
   if 'menuentry \'Gentoo GNU/Linux, with Linux '+old+"'" not in Path('/boot/grub/grub.cfg').read_text():raise RuntimeError('Existing old kernel entry missing')
   command(['/usr/sbin/grub-reboot',title]);r['grub_environment']=command(['/usr/sbin/grub-editenv','list']);r['configured_default_unchanged']=qualified
 r['status']='passed'
except Exception as e:r.update(status='failed',error=str(e))
finally:
 r['harness_sha256']=sha(__file__);r['finished']=time.time()
 if os.path.ismount(base):
  output=base/('host-control-'+a.phase+'-'+r['boot_id']+'-'+str(time.time_ns())+'.json');output.write_text(json.dumps(r,indent=2)+'\n');output.chmod(0o600);os.chown(output,1300,1300)
print(json.dumps(r));raise SystemExit(0 if r['status']=='passed' else 1)
