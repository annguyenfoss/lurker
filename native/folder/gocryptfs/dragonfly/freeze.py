#!/usr/bin/env python3
"""Native, unprivileged pinned-source/ABI freeze. No mount/device adoption."""
import argparse,hashlib,json,os,pathlib,platform,shutil,subprocess,tarfile,time,zipfile
ENGINE='756cd453d7afe69fd2a00a7592fd2d9869a6f756'
FUSE='2d50fff207d78c197869fa492eff3a79bc1d8408'
VERSION='v2.12.1-0.20261018182834-2d50fff207d7'
ARCHIVE='6969e36aad99027ba87a1113eb9aff995af7457eb99b59f84ebd93ff42f84175'
def digest(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def durable(path,data):
 path=pathlib.Path(path);data=data.encode() if isinstance(data,str) else data
 with path.open('xb') as f:f.write(data);f.flush();os.fsync(f.fileno())
 syncdir(path.parent)
def syncdir(path):
 fd=os.open(path,os.O_RDONLY|os.O_DIRECTORY)
 try:os.fsync(fd)
 finally:os.close(fd)
def sync_tree(root):
 for path in sorted(root.rglob('*')):
  if path.is_file():
   fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW)
   try:os.fsync(fd)
   finally:os.close(fd)
 for path in sorted((p for p in root.rglob('*') if p.is_dir()),key=lambda p:len(p.parts),reverse=True):syncdir(path)
 syncdir(root)
def main():
 p=argparse.ArgumentParser();p.add_argument('--base',type=pathlib.Path,required=True);p.add_argument('--workspace',type=pathlib.Path,required=True);p.add_argument('--evidence',type=pathlib.Path,required=True);a=p.parse_args()
 assert platform.system()=='DragonFly' and os.getuid()==os.geteuid()==1300
 assert not a.workspace.exists() and not a.evidence.exists()
 resources={str(p):{'device':os.stat(p).st_dev,'free_bytes':shutil.disk_usage(p).free} for p in [pathlib.Path('/'),a.workspace.parent,a.evidence.parent,pathlib.Path('/tmp'),a.base/'go-native-cache/build',a.base/'go-native-cache/mod']}
 assert resources[str(a.workspace.parent)]['free_bytes']>768*1024**2 and resources['/']['free_bytes']>768*1024**2 and resources[str(a.evidence.parent)]['free_bytes']>768*1024**2
 assert resources['/tmp']['free_bytes']>64*1024**2
 a.workspace.mkdir();a.evidence.mkdir();r={'status':'freezing','pid':os.getpid(),'uid':os.getuid(),'started':time.time(),'workspace':str(a.workspace),'scope':'native userspace source/ABI only; no LKL, mount, FUSE FD adoption or product admission','engine_pin':ENGINE,'go_fuse_pin':FUSE,'commands':[],'inputs':[],'budget':{'new_bytes':256*1024**2,'work_reserve_bytes':512*1024**2,'root_reserve_bytes':768*1024**2}}
 r['resource_preflight']=resources;r['refresh_scope']='New evidence only; earlier missing001 remains missing, never reconstructed.'
 source=pathlib.Path(__file__).with_name('abi-probe.c')
 for p in [pathlib.Path(__file__),source]:durable(a.evidence/p.name,p.read_bytes())
 temp=a.workspace/'tmp';temp.mkdir();syncdir(a.workspace)
 env=os.environ|{'TMPDIR':str(temp),'GOMAXPROCS':'1','GOTOOLCHAIN':'local','GOTELEMETRY':'off','GOENV':'off','GOPROXY':'off','GOSUMDB':'off','CGO_ENABLED':'0','PYTHONDONTWRITEBYTECODE':'1'}
 def run(argv):
  z=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=45,env=env);r['commands'].append({'argv':list(map(str,argv)),'exit':z.returncode,'stdout':z.stdout,'stderr':z.stderr});assert z.returncode==0,z.stderr;return z.stdout
 try:
  archive=a.base/'gocryptfs-pinned-source.tar';assert digest(archive)==ARCHIVE
  mod=a.base/'go-native-cache/mod';zpath=mod/'cache/download/github.com/hanwen/go-fuse/v2/@v'/(VERSION+'.zip');h1=zpath.with_suffix('.ziphash').read_text().strip();assert digest(zpath)=='36c636fdcff1b570b10cfd68cdd02e32be67614688e0a9560508fc582c55c049'
  engine=a.workspace/'engine';engine.mkdir()
  with tarfile.open(archive) as t:
   for m in t:
    if not m.isfile():continue
    rel=pathlib.PurePosixPath(m.name);assert not rel.is_absolute() and '..' not in rel.parts
    out=engine/rel;out.parent.mkdir(parents=True,exist_ok=True);durable(out,t.extractfile(m).read())
  assert ('github.com/hanwen/go-fuse/v2 '+VERSION+' '+h1) in (engine/'go.sum').read_text()
  fuse=a.workspace/'go-fuse';fuse.mkdir();prefix='github.com/hanwen/go-fuse/v2@'+VERSION+'/'
  with zipfile.ZipFile(zpath) as z:
   for m in z.infolist():
    if m.is_dir():continue
    assert m.filename.startswith(prefix);rel=pathlib.PurePosixPath(m.filename[len(prefix):]);assert not rel.is_absolute() and '..' not in rel.parts
    out=fuse/rel;out.parent.mkdir(parents=True,exist_ok=True);durable(out,z.read(m))
  for name,directory in [('engine',engine),('go-fuse',fuse)]:
   rows=[{'path':str(f.relative_to(directory)),'sha256':digest(f)} for f in sorted(directory.rglob('*')) if f.is_file()]
   durable(a.evidence/(name+'-original-files.json'),json.dumps(rows,indent=2)+'\n');sync_tree(directory)
  original_mod=(engine/'go.mod').read_text();durable(a.workspace/'engine.mod',original_mod+'\nreplace github.com/hanwen/go-fuse/v2 => '+str(fuse)+'\n');durable(a.workspace/'engine.sum',(engine/'go.sum').read_bytes())
  go='/usr/local/go126/bin/go';r['go_version']=run([go,'version']).strip();assert r['go_version']=='go version go1.26.11 dragonfly/amd64';r['compiler']=run(['cc','--version'])
  prior=a.base/'evidence/build/gocryptfs-native-assessment.json';assert json.loads(prior.read_text())['status']=='native-source-build-failed'
  headers=[pathlib.Path(x) for x in ['/usr/include/sys/dirent.h','/usr/include/sys/stat.h','/usr/include/sys/mount.h','/usr/include/sys/fcntl.h','/usr/include/sys/syscall.h','/usr/include/sys/syslimits.h']]+[a.base/'base-source/sys/vfs/fuse/fuse_abi.h']
  assert digest(headers[-1])=='0f74fd0c9e3b84c454029777b75cf7b5aac207bef5d10dff2a5af16043fb9936'
  snapshots=a.evidence/'headers';snapshots.mkdir()
  for f in headers:durable(snapshots/f.name,f.read_bytes())
  for f in [pathlib.Path(go),pathlib.Path(shutil.which('cc')),pathlib.Path(__file__),source,archive,zpath,zpath.with_suffix('.ziphash'),engine/'go.mod',engine/'go.sum',fuse/'go.mod',prior,*headers]:r['inputs'].append({'path':str(f),'sha256':digest(f)})
  r['go_fuse_go_sum_h1']=h1;r['stock_wire_source']='e17b9176356e3afb3427d71c21108a8a849cd4d5/sys/vfs/fuse/fuse_abi.h';r['wire_header_is_installed']=False
  binary=a.workspace/'abi-probe';run(['cc','-Wall','-Wextra','-Werror','-I'+str(a.base/'base-source/sys'),source,'-o',binary]);abi=json.loads(run([binary]));assert abi['sizeof.struct fuse_init_out']==64 and abi['FUSE_KERNEL_VERSION']==7 and abi['FUSE_KERNEL_MINOR_VERSION']==28
  durable(a.evidence/'native-abi.json',json.dumps(abi,indent=2)+'\n');sync_tree(a.workspace);r['probe_source_sha256']=digest(source);r['probe_binary_sha256']=digest(binary);r['status']='frozen-native-ABI-passed-no-transport'
 except Exception as x:r.update(status='failed',error=str(x))
 r['finished']=time.time();r['durability']='Every created source/manifest/probe file and directories fsynced before terminal output';durable(a.evidence/'inputs.json',json.dumps(r,indent=2)+'\n');syncdir(a.evidence.parent);syncdir(a.workspace.parent);print(json.dumps({'status':r['status'],'evidence':str(a.evidence),'workspace':str(a.workspace),'error':r.get('error')}));return r['status']=='failed'
if __name__=='__main__':raise SystemExit(main())
