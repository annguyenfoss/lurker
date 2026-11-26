#!/usr/bin/env python3
"""Fresh UFS1 write control on only the existing matching Gentoo prerequisite."""
import argparse,hashlib,importlib.util,json,os,pwd,subprocess,tempfile,time
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for name in ('root','payload'):p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args()
expected_kernel='6.18.64-p1-gentoo-dist-bin';module=Path('/lib/modules')/expected_kernel/'extra/ufs.ko';expected_sha='1e0cee7875b995925c3f9737c335245d08baa99cd6c1084e3a4fc3ea76a76bd5'
if os.geteuid()!=0 or os.uname().nodename!='gentoo-core-01' or os.uname().release!=expected_kernel:raise SystemExit('Exact owned matching-kernel administrator control required')
if module.stat().st_uid!=0 or module.stat().st_mode&0o022 or hashlib.file_digest(module.open('rb'),'sha256').hexdigest()!=expected_sha:raise SystemExit('Existing module identity differs')
s=importlib.util.spec_from_file_location('h',Path(__file__).with_name('test-filesystem-cli.py'));h=importlib.util.module_from_spec(s);s.loader.exec_module(h)
f=importlib.util.spec_from_file_location('features',Path(__file__).with_name('test-gentoo-native-fresh.py'));feature=importlib.util.module_from_spec(f);f.loader.exec_module(feature)
login=pwd.getpwnam('adjutant');a.root.mkdir(mode=0o700,parents=True,exist_ok=False);os.chown(a.root,login.pw_uid,login.pw_gid);image=a.root/'native-ufs1.img';mount=a.root/'mount';mount.mkdir(mode=0o700);seed=a.root/'seed';seed.mkdir(mode=0o700);os.chown(seed,login.pw_uid,login.pw_gid)
r={'route':'native filesystem control; pre-existing supplemental configured module','kernel':os.uname().release,'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip(),'module':str(module),'module_sha256':expected_sha,'harness_sha256':h.file_hash(Path(__file__)),'fresh_build_executed':False,'image':str(image),'mountpoint':str(mount),'status':'running','operations':[]}
def save():h.atomic_json(a.root/'result.json',r)
def command(argv):
 c=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=120);r['operations'].append({'command':list(map(str,argv)),'exit_code':c.returncode,'stdout':c.stdout[-5000:],'stderr':c.stderr[-2000:]});save()
 if c.returncode:raise RuntimeError(str(argv[0])+' failed')
 return c.stdout
save()
try:
 if any(line.partition(' - ')[2].split()[:1]==['ufs'] for line in Path('/proc/self/mountinfo').read_text().splitlines()):raise RuntimeError('Existing UFS mount prevents scoped module selection')
 selected=command(['/usr/bin/env','--argv0=modinfo','/usr/bin/kmod','-n','ufs']).strip()
 r['normal_module_selection']=selected
 r['module_metadata']=command(['/usr/bin/env','--argv0=modinfo','/usr/bin/kmod',str(module)])
 vermagic=next(line.partition(':')[2].strip().split()[0] for line in r['module_metadata'].splitlines() if line.startswith('vermagic:'))
 if vermagic!=expected_kernel:raise RuntimeError('Existing module vermagic differs from running kernel')
 if Path('/sys/module/ufs').exists():command(['/usr/bin/env','--argv0=modprobe','/usr/bin/kmod','-r','ufs'])
 command(['/usr/bin/env','--argv0=insmod','/usr/bin/kmod',str(module)])
 r['activation']='Normal insmod of exact existing matching module; no force flags; normal resolver remains stock'
 command(['makefs-freebsd','-t','ffs','-B','little','-o','version=1,bsize=32768,fsize=4096','-s','268435456',image,seed])
 for phase in ('write','append','verify'):
  command(['mount','-i','-t','ufs','-o','loop,rw,ufstype=44bsd',image,mount])
  identity=h.linux_mount_record(Path('/proc/self/mountinfo').read_text(),mount);h.check_linux_filesystem(identity,'ufs1');r[phase+'_native_mount']=identity
  block=command(['blkid','-p',identity['source']]);r[phase+'_superblock']=block
  if 'VERSION="1"' not in block:raise RuntimeError('Independent UFS1 superblock proof differs')
  if phase=='write':r['features']=feature.features(mount)
  check=subprocess.run(['/usr/bin/sudo','-n','-u',login.pw_name,'--','/home/adjutant/.local/bin/python3',str(Path(__file__).with_name('gentoo-mounted.py')),phase,str(mount),str(a.payload)],capture_output=True,text=True,timeout=90);log=a.root/(phase+'-payload.log');log.write_text(check.stdout+check.stderr);r[phase+'_payload_log']=str(log)
  if check.returncode:raise RuntimeError('Login-user native UFS1 payload '+phase+' failed')
  r[phase+'_sha256']=json.loads(check.stdout);command(['umount',mount]);save()
 r['status']='passed'
except Exception as e:r.update(status='failed',error=str(e))
finally:
 if os.path.ismount(mount):
  try:command(['umount',mount])
  except Exception as e:r['cleanup_error']=str(e)
 r.update(mount_remaining=os.path.ismount(mount),finished=time.time());save()
print(json.dumps({'status':r['status'],'receipt':str(a.root/'result.json')}));raise SystemExit(0 if r['status']=='passed' else 1)
