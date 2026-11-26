#!/usr/bin/env python3
"""Gentoo-owned native physical backing × folder engines, with serial guards."""
import argparse,importlib.util,json,os,pwd,secrets,stat,subprocess,tempfile,time
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
for n in ('device','binary','root','payload-dir'):p.add_argument('--'+n,type=Path,required=True)
p.add_argument('--commit',required=True);p.add_argument('--filesystems',required=True);p.add_argument('--skip-terminal-ledger',type=Path);a=p.parse_args()
if os.geteuid()!=0:raise SystemExit('Administrator-controlled harness required')
login=pwd.getpwnam('adjutant');device=a.device.resolve(strict=True)
if not stat.S_ISBLK(device.stat().st_mode):raise SystemExit('Block device required')
s=importlib.util.spec_from_file_location('h',Path(__file__).with_name('test-filesystem-cli.py'));h=importlib.util.module_from_spec(s);s.loader.exec_module(h)
f=importlib.util.spec_from_file_location('features',Path(__file__).with_name('test-gentoo-native-fresh.py'));feature=importlib.util.module_from_spec(f);f.loader.exec_module(feature)
a.root.mkdir(mode=0o700,parents=True,exist_ok=True);os.chown(a.root,login.pw_uid,login.pw_gid)
rows=[];receipt=a.root/'device-backing-matrix.json'
def command(argv,**kw):
 r=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=180,**kw)
 if 'backing' in globals():backing.setdefault('native_commands',[]).append({'command':list(map(str,argv)),'exit_code':r.returncode,'stdout':r.stdout[-4000:],'stderr':r.stderr[-2000:]})
 if r.returncode:raise RuntimeError(str(argv[0])+' failed: '+(r.stderr or r.stdout)[-3000:])
 return r.stdout
def guard():
 identity=json.loads(command(['lsblk','--json','--bytes','-o','PATH,SERIAL,SIZE,TYPE,MOUNTPOINTS',device]))['blockdevices'][0]
 if identity.get('serial')!='lurker-fresh-gentoo' or identity.get('size')!=2147483648 or identity.get('type')!='disk':raise RuntimeError('Physical scratch identity differs')
 if identity.get('children') or any(identity.get('mountpoints') or []) or list((Path('/sys/class/block')/device.name/'holders').iterdir()):raise RuntimeError('Physical scratch is busy; refusing format')
 return identity
identity=guard()
skip=set()
if a.skip_terminal_ledger:
 prior=json.loads(a.skip_terminal_ledger.read_text())
 if prior['source_commit']!=a.commit or prior['binary_sha256']!=h.file_hash(a.binary) or prior['device'].get('serial')!='lurker-fresh-gentoo':raise SystemExit('Prior same-candidate backing ledger differs')
 rows.extend(dict(r,prior_ledger=str(a.skip_terminal_ledger),prior_harness_sha256=prior['harness_sha256']) for r in prior['cases'] if r['status']=='passed' or (r['filesystem']=='ufs1' and r['status']=='failed' and 'not read/write' in r.get('error','')))
 skip={r['filesystem'] for r in rows}
def save():h.atomic_json(receipt,{'source_commit':a.commit,'binary_sha256':h.file_hash(a.binary),'helper_sha256':h.file_hash(a.binary.with_name('lurker-helper')),'harness_sha256':h.file_hash(Path(__file__)),'kernel':os.uname().release,'device':identity,'pid':os.getpid(),'cases':rows})
def present(path):return any(line.split()[4]==str(path) for line in Path('/proc/self/mountinfo').read_text().splitlines())
save()
for fs in a.filesystems.split(','):
 if fs in skip:continue
 guard();d=Path(tempfile.mkdtemp(prefix='physical-'+fs+'-',dir=a.root));os.chown(d,login.pw_uid,login.pw_gid)
 mount=d/'backing';mount.mkdir();pool='lrkf_phys_'+d.name.replace('-','_');active=False;failed=False
 backing={'filesystem':fs,'role':'backing','source_kind':'native-physical-filesystem','device':str(device),'fixture':str(d),'status':'running','engines':[],'native_mounts':[]};rows.append(backing);save()
 def mount_backing():
  global active
  if fs=='zfs':command(['zpool','import','-d','/dev/disk/by-id','-o','cachefile=none',pool])
  elif fs=='ufs2':command(['/usr/local/bin/fuse-ufs','-o','rw,allow_other,fsname='+str(device),device,mount])
  else:
   args=['mount','-i','-t',{'ufs1':'ufs','ntfs':'ntfs3'}.get(fs,fs)]
   opts=['rw']
   if fs=='ufs1':opts+=['ufstype=44bsd']
   if fs=='apfs':opts+=['readwrite']
   if fs in {'ntfs','exfat'}:opts+=['uid='+str(login.pw_uid),'gid='+str(login.pw_gid),'umask=0077']
   command([*args,'-o',','.join(opts),device,mount])
  deadline=time.monotonic()+5
  while not present(mount) and time.monotonic()<deadline:time.sleep(.025)
  active=present(mount)
  if not active:backing['mount_readiness_failed']=True;raise RuntimeError('Physical native backing mount absent after bounded readiness wait')
  record=h.linux_mount_record(Path('/proc/self/mountinfo').read_text(),mount);backing['native_mounts'].append(record);save();h.check_linux_filesystem(record,fs)
 def close_backing():
  global active
  if fs=='zfs':command(['zpool','export',pool])
  else:command(['umount',mount])
  active=present(mount)
  if active:raise RuntimeError('Physical backing did not close')
 try:
  command(['wipefs','--all',device])
  if fs=='zfs':
   command(['zpool','create','-f','-o','cachefile=none','-m',mount,pool,device]);active=present(mount)
   record=h.linux_mount_record(Path('/proc/self/mountinfo').read_text(),mount);h.check_linux_filesystem(record,fs);backing['native_mounts'].append(record)
  elif fs in {'ufs1','ufs2'}:
   seed=d/'seed';seed.mkdir();os.chown(seed,login.pw_uid,login.pw_gid);seed.chmod(0o700)
   command(['makefs-freebsd','-t','ffs','-B','little','-o','version='+('1' if fs=='ufs1' else '2')+',bsize=32768,fsize=4096','-s','2147483648',device,seed]);mount_backing()
  else:
   formatter={'ext4':['mkfs.ext4','-t','ext4','-F','-q'],'btrfs':['mkfs.btrfs','-f','-q'],'xfs':['mkfs.xfs','-f','-q'],'apfs':['mkapfs'],'hammer2':['newfs_hammer2','-L','DATA'],'ntfs':['mkntfs','-Q','-F'],'exfat':['mkfs.exfat']}[fs]
   command([*formatter,device]);mount_backing()
  backing['superblock_observations']=[feature.probe(['blkid','-p',device]),feature.probe(['file','-s',device])]
  if fs not in {'ntfs','exfat'}:os.chown(mount,login.pw_uid,login.pw_gid);mount.chmod(0o700)
  backing['features']=feature.features(mount);save()
  for engine in ('gocryptfs','cryfs','cryptomator'):
   plain=d/(engine+'-plain');plain.mkdir(mode=0o700);os.chown(plain,login.pw_uid,login.pw_gid);source=mount/(engine+'-encrypted');password='Lurker9!'+secrets.token_urlsafe(24)
   row={'backend':engine,'filesystem':fs,'role':'backing','source_kind':'native-physical-filesystem','route':'product-helper-login-user','source':str(source),'mountpoint':str(plain),'backing_device':str(device),'crypto_source_kind':'directory','status':'running','operations':[]};backing['engines'].append(row);save()
   def request(action):
    payload={'action':action,'volume_type':engine}
    if action=='create':payload.update(target=str(source),size_gb=None,force=False,source_kind='directory',cipher='aes',passphrase=password)
    elif action=='mount':payload.update(source=str(source),mountpoint=str(plain),tag=None,passphrase=password)
    else:payload.update(target=str(plain),tag=None)
    r=subprocess.run(['/usr/bin/sudo','-n','-H','-u',login.pw_name,'--',str(a.binary.with_name('lurker-helper')),'run'],input=json.dumps(payload).encode(),capture_output=True,timeout=180)
    if password.encode() in r.stdout+r.stderr:raise RuntimeError('Helper exposed credential')
    log=d/(engine+'-'+str(len(row['operations']))+'-'+action+'.log');log.write_bytes(r.stdout+r.stderr);answer=json.loads(r.stdout);row['operations'].append({'action':action,'exit_code':r.returncode,'ok':answer.get('ok'),'log':str(log)});save()
    if r.returncode or not answer.get('ok'):raise RuntimeError(engine+' '+action+' failed')
   try:
    request('create')
    for phase in ('write','append','verify'):
     request('mount')
     if not present(plain):raise RuntimeError('Folder plaintext mount absent; underlay refused')
     check=subprocess.run(['/usr/bin/sudo','-n','-H','-u',login.pw_name,'--','/home/adjutant/.local/bin/python3',str(Path(__file__).with_name('gentoo-mounted.py')),phase,str(plain),str(a.payload_dir)],capture_output=True,text=True,timeout=90)
     log=d/(engine+'-'+phase+'-payload.log');log.write_text(check.stdout+check.stderr);row[phase+'_payload_log']=str(log)
     if check.returncode:raise RuntimeError('Payload '+phase+' failed')
     row[phase+'_sha256']=json.loads(check.stdout);request('unmount');close_backing();mount_backing();save()
    row['status']='passed'
   except Exception as error:row.update(status='failed',error=str(error));failed=True
   finally:
    if present(plain):
     try:request('unmount')
     except Exception as error:row['cleanup_error']=str(error)
    row['mount_remaining']=present(plain);save()
    if row['mount_remaining']:raise RuntimeError('Folder mount remains; preserve physical backing')
  backing['status']='failed' if failed else 'passed'
 except Exception as error:
  backing.update(status='failed',error=str(error));failed=True
  for engine in ('gocryptfs','cryfs','cryptomator'):
   if not any(r['backend']==engine for r in backing['engines']):backing['engines'].append({'backend':engine,'filesystem':fs,'role':'backing','source_kind':'native-physical-filesystem','status':'blocked','error':'Native physical backing unavailable: '+str(error)})
 finally:
  if active or present(mount):
   try:close_backing()
   except Exception as error:backing['cleanup_error']=str(error)
  backing['mount_remaining']=present(mount)
  if failed and not backing['mount_remaining']:
   target=d/'failed-physical-backing.img'
   try:command(['dd','if='+str(device),'of='+str(target),'bs=4M','conv=sparse','status=none']);backing['retained_image']=str(target);backing['retained_sha256']=h.file_hash(target)
   except Exception as error:backing['retention_error']=str(error)
  backing['finished']=time.time();save()
  if backing['mount_remaining'] or backing.get('retention_error') or backing.get('mount_readiness_failed') or any(r.get('mount_remaining') for r in backing['engines']):raise SystemExit('Stopped with busy/retention failure; reconcile storage')
print(json.dumps({'receipt':str(receipt),'passed':sum(r['status']=='passed' for r in rows),'failed':sum(r['status']=='failed' for r in rows)}))
