import argparse,errno,fcntl,hashlib,json,os,pathlib,subprocess,sys,tempfile,time
if len(sys.argv)==3 and sys.argv[1]=='child':
 r={'path':sys.argv[2],'uid':os.getuid(),'euid':os.geteuid()}
 try:
  fd=os.open(sys.argv[2],os.O_RDWR|os.O_NOFOLLOW|os.O_CLOEXEC);st=os.fstat(fd);r['opened']={'dev':st.st_dev,'ino':st.st_ino};fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB);r['claimed']=True;os.close(fd)
 except OSError as x:r.update(claimed=False,errno=x.errno,error=str(x))
 print(json.dumps(r));sys.exit(0)
assert os.getuid()==os.geteuid()==1300
parser=argparse.ArgumentParser();parser.add_argument('--work',type=pathlib.Path,required=True);parser.add_argument('--receipt',type=pathlib.Path,required=True);args=parser.parse_args();d=pathlib.Path(tempfile.mkdtemp(prefix='source-claim-',dir=args.work));base=d/'source';fd=os.open(base,os.O_CREAT|os.O_EXCL|os.O_RDWR|os.O_CLOEXEC,0o600);os.ftruncate(fd,524288);hard=d/'hardlink';os.link(base,hard);alias=d/'symlink';alias.symlink_to(base);fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB);r={'status':'running','uid':os.getuid(),'euid':os.geteuid(),'started':time.time(),'scope':'native OS source-flock/O_NOFOLLOW primitive; final new client duplicate tests await review','harness_sha256':hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),'cases':[]}
for p in [base,hard,alias]:
 z=subprocess.run([sys.executable,__file__,'child',str(p)],capture_output=True,text=True,timeout=10);assert z.returncode==0,z.stderr;row=json.loads(z.stdout);r['cases'].append(row);assert not row['claimed'];assert row['errno'] in ([errno.EWOULDBLOCK,errno.EAGAIN] if p!=alias else [errno.ELOOP,errno.EMLINK])
fcntl.flock(fd,fcntl.LOCK_UN);os.close(fd);z=subprocess.run([sys.executable,__file__,'child',str(base)],capture_output=True,text=True,timeout=10);row=json.loads(z.stdout);assert row['claimed'];r['after_release']=row
try:
 fd=os.open('/dev/null',os.O_RDONLY|os.O_CLOEXEC)
 try:fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB);r['character_fd_claim']='supported on /dev/null'
 except OSError as x:r['character_fd_claim']={'errno':x.errno,'error':str(x),'limit':'Character/mapped-device route needs its separately reviewed exclusive lease; strict client fails closed.'}
 finally:os.close(fd)
except OSError as x:r['character_probe_error']=str(x)
r.update(status='passed',finished=time.time());p=args.receipt;p.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'status':r['status'],'receipt':str(p),'cases':r['cases'],'character':r.get('character_fd_claim')}))
