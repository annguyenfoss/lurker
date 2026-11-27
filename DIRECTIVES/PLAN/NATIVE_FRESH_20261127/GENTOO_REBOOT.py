#!/usr/bin/env python3
"""Owned final-package fixtures spanning one ordinary Gentoo reboot."""
import argparse,importlib.util,json,os,secrets,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('phase',choices=['prepare','resume'])
for name in ('root','prefix','payload'):p.add_argument('--'+name,type=Path,required=True)
p.add_argument('--commit',required=True);p.add_argument('--login-disk',action='store_true');p.add_argument('--representative',action='store_true');p.add_argument('--ufs1-supplemental',action='store_true');p.add_argument('--directory-backing-root',type=Path);a=p.parse_args()
if os.geteuid()!=1300 or os.uname().nodename!='gentoo-core-01':p.error('Owned Gentoo login user required')
helper=a.prefix/'bin/lurker-helper';binary=a.prefix/'bin/lurker';harness=a.prefix/'harness/test-filesystem-cli.py'
s=importlib.util.spec_from_file_location('h',harness);h=importlib.util.module_from_spec(s);s.loader.exec_module(h)
boot=Path('/proc/sys/kernel/random/boot_id').read_text().strip();receipt=a.root/'reboot-batch.json'
marker=b'\nWritten and synced before an ordinary final-candidate OS reboot.\n'
if a.phase=='prepare':
 if a.ufs1_supplemental and a.representative:raise SystemExit('Representative and supplemental cohorts are separate')
 a.root.mkdir(mode=0o700,parents=True,exist_ok=False)
 configs=([(backend,'ufs1') for backend in ('luks','luks1','veracrypt','gocryptfs','cryfs','cryptomator')] if a.ufs1_supplemental else [('luks',fs) for fs in ('ext4','btrfs','xfs','apfs','ufs2','hammer2','ntfs','exfat','zfs')]+[('luks1','btrfs'),('veracrypt','btrfs')]+[(engine,'ext4') for engine in ('gocryptfs','cryfs','cryptomator')])
 if a.representative:configs=[(backend,'btrfs') for backend in ('luks','luks1','veracrypt')]+[(engine,'ext4') for engine in ('gocryptfs','cryfs','cryptomator')]
 if a.ufs1_supplemental:
  if not a.directory_backing_root or not os.path.ismount(a.directory_backing_root):raise SystemExit('Verified supplemental native UFS1 physical mount required')
  h.check_linux_filesystem(h.linux_mount_record(Path('/proc/self/mountinfo').read_text(),a.directory_backing_root),'ufs1')
 j={'source_commit':a.commit,'login_disk':a.login_disk,'representative':a.representative,'binary_sha256':h.file_hash(binary),'helper_sha256':h.file_hash(helper),'harness_sha256':h.file_hash(Path(__file__)),'prepared_boot_id':boot,'prepared_kernel':os.uname().release,'pid':os.getpid(),'status':'preparing','cases':[]}
 for backend,fs in configs:
  d=a.root/(backend+'-'+fs);d.mkdir(mode=0o700);folder=backend in h.DIRECTORIES;source=(a.directory_backing_root/(backend+'-reboot-encrypted')) if folder and a.ufs1_supplemental else d/('encrypted' if folder else 'vault.img');m=d/'mount';m.mkdir(mode=0o700)
  if folder:
   credential=d/'test-password';credential.write_text('Lurker9!'+secrets.token_urlsafe(24));credential.chmod(0o600)
  else:
   credential=d/'binary key Việt Nam.bin';credential.write_bytes(b'\0\xff\n\r'+secrets.token_bytes(252));credential.chmod(0o600)
  j['cases'].append({'backend':backend,'filesystem':fs,'role':'backing' if folder else 'inner','source':str(source),'fixture':str(d),'mountpoint':str(m),'credential':str(credential),'source_kind':'directory' if folder else 'file','route':'normal-user product CLI with native elevation where required' if folder or a.login_disk else 'administrator product CLI','authentication':'password' if folder else 'binary-keyfile','operations':[],'status':'pending'})
else:
 j=json.loads(receipt.read_text())
 if j['prepared_boot_id']==boot:raise SystemExit('OS boot ID did not change')
 if j['source_commit']!=a.commit or j['binary_sha256']!=h.file_hash(binary) or j['helper_sha256']!=h.file_hash(helper):raise SystemExit('Reboot artifact identity differs')
 j.update(resumed_boot_id=boot,resumed_kernel=os.uname().release)
def save():h.atomic_json(receipt,j)
save()
for row in j['cases']:
 backend=row['backend'];fs=row['filesystem'];source=Path(row['source']);m=Path(row['mountpoint']);d=Path(row['fixture']);folder=backend in h.DIRECTORIES;credential=Path(row['credential']);password=credential.read_text() if folder else None
 expected=h.load_payload(a.payload,None)
 def cli(action,operands):
  argv=([] if folder or j.get('login_disk',False) else ['/usr/bin/sudo','-n'])+[str(binary),action,'--type',backend if action=='create' else 'auto','--color=never']
  if not folder and action in {'create','mount'}:argv+=['--key-file',str(credential)]
  log=d/(str(len(row['operations']))+'-'+action+'.log');r=h.terminal([*argv,*map(str,operands)],password,180,log);row['operations'].append({'action':action,'log':str(log),**r});save()
  if r['exit_code'] or r['timed_out']:raise RuntimeError('CLI '+action+' failed')
 def phase(name):
  try:result=h.bounded_payload_call(lambda:h.payload_phase(m,expected,name,fs,backend,source),90)
  except h.PayloadWorkerError as e:row['payload_worker']=e.worker;raise
  row.update(result);save()
 try:
  if a.phase=='prepare':
   operands=[] if folder else ['--filesystem',fs];operands.append(source)
   if not folder:operands.append('0.5' if fs in {'xfs','hammer2'} else '0.25')
   cli('create',operands)
   for name in ('write','append','verify'):
    cli('mount',[source,m]);phase(name)
    if name=='append':expected['page.txt']+=h.APPEND;expected['second.txt']=h.SECOND
    if name!='verify':cli('unmount',[m])
   identity=h.linux_mount_record(Path('/proc/self/mountinfo').read_text(),source if folder else m);row['pre_reboot_native_mount']=identity;h.check_linux_filesystem(identity,fs)
   if fs in {'ufs1','ufs2'}:
    result=subprocess.run(['sudo','-n','blkid','-p',identity['source']],capture_output=True,text=True,timeout=20);row['superblock']={'exit_code':result.returncode,'stdout':result.stdout,'stderr':result.stderr}
    if result.returncode or ('VERSION="'+('1' if fs=='ufs1' else '2')+'"') not in result.stdout:raise RuntimeError('Independent UFS superblock proof missing')
   h.bounded_payload_call(lambda:h.payload_write(m/'Lurker payload/page.txt',marker,append=True),90);expected['page.txt']+=marker
   row['before_reboot_sha256']=h.bounded_payload_call(lambda:h.verify_payload(m,expected),90);row['status']='prepared-mounted-for-reboot'
  else:
   if row['status']!='prepared-mounted-for-reboot':continue
   expected['page.txt']+=h.APPEND+marker;expected['second.txt']=h.SECOND
   if os.path.ismount(m):raise RuntimeError('Unexpected mount survived OS boot identity change')
   cli('mount',[source,m]);phase('verify');row['after_reboot_sha256']=row['verify_sha256']
   if row['after_reboot_sha256']!=row['before_reboot_sha256']:raise RuntimeError('Five reboot payload hashes differ')
   cli('unmount',[m]);row['status']='passed'
 except Exception as e:
  row.update(status='failed',error=str(e))
 finally:
  if row['status'] not in {'prepared-mounted-for-reboot'} and not row.get('payload_worker',{}).get('remaining') and os.path.ismount(m):
   try:cli('unmount',[m])
   except Exception as e:row['cleanup_error']=str(e)
  row['mount_remaining']=os.path.ismount(m);save()
  if row.get('payload_worker',{}).get('remaining'):raise SystemExit('Live payload worker retained; reconcile before reboot')
j.update(status=('prepared-mounted-for-reboot' if a.phase=='prepare' else 'passed') if all(r['status']==('prepared-mounted-for-reboot' if a.phase=='prepare' else 'passed') for r in j['cases']) else 'failed',finished=time.time());save();print(json.dumps({'status':j['status'],'receipt':str(receipt),'cases':len(j['cases'])}));raise SystemExit(0 if j['status'] in {'prepared-mounted-for-reboot','passed'} else 1)
