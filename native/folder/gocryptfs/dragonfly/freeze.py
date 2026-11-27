#!/usr/bin/env python3
"""Native, unprivileged pinned-source/ABI freeze. No mount/device adoption."""
import argparse,hashlib,json,os,pathlib,platform,shutil,subprocess,tarfile,time,zipfile
ENGINE='756cd453d7afe69fd2a00a7592fd2d9869a6f756'
FUSE='2d50fff207d78c197869fa492eff3a79bc1d8408'
VERSION='v2.12.1-0.20261018182834-2d50fff207d7'
ARCHIVE='6969e36aad99027ba87a1113eb9aff995af7457eb99b59f84ebd93ff42f84175'
def digest(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--base',type=pathlib.Path,required=True);p.add_argument('--workspace',type=pathlib.Path,required=True);p.add_argument('--evidence',type=pathlib.Path,required=True);a=p.parse_args()
 assert platform.system()=='DragonFly' and os.getuid()==os.geteuid()==1300
 assert not a.workspace.exists() and not a.evidence.exists();assert shutil.disk_usage(a.workspace.parent).free>768*1024**2 and shutil.disk_usage('/').free>768*1024**2
 a.workspace.mkdir();a.evidence.mkdir();r={'status':'freezing','pid':os.getpid(),'uid':os.getuid(),'started':time.time(),'workspace':str(a.workspace),'scope':'native userspace source/ABI only; no LKL, mount, FUSE FD adoption or product admission','engine_pin':ENGINE,'go_fuse_pin':FUSE,'commands':[],'inputs':[],'budget':{'new_bytes':256*1024**2,'work_reserve_bytes':512*1024**2,'root_reserve_bytes':768*1024**2}}
 def run(argv):
  z=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=45);r['commands'].append({'argv':list(map(str,argv)),'exit':z.returncode,'stdout':z.stdout,'stderr':z.stderr});assert z.returncode==0,z.stderr;return z.stdout
 try:
  archive=a.base/'gocryptfs-pinned-source.tar';assert digest(archive)==ARCHIVE
  mod=a.base/'go-native-cache/mod';zpath=mod/'cache/download/github.com/hanwen/go-fuse/v2/@v'/(VERSION+'.zip');h1=zpath.with_suffix('.ziphash').read_text().strip()
  engine=a.workspace/'engine';engine.mkdir()
  with tarfile.open(archive) as t:
   for m in t:
    if not m.isfile():continue
    rel=pathlib.PurePosixPath(m.name);assert not rel.is_absolute() and '..' not in rel.parts
    out=engine/rel;out.parent.mkdir(parents=True,exist_ok=True);out.write_bytes(t.extractfile(m).read())
  assert ('github.com/hanwen/go-fuse/v2 '+VERSION+' '+h1) in (engine/'go.sum').read_text()
  fuse=a.workspace/'go-fuse';fuse.mkdir();prefix='github.com/hanwen/go-fuse/v2@'+VERSION+'/'
  with zipfile.ZipFile(zpath) as z:
   for m in z.infolist():
    if m.is_dir():continue
    assert m.filename.startswith(prefix);rel=pathlib.PurePosixPath(m.filename[len(prefix):]);assert not rel.is_absolute() and '..' not in rel.parts
    out=fuse/rel;out.parent.mkdir(parents=True,exist_ok=True);out.write_bytes(z.read(m))
  for name,directory in [('engine',engine),('go-fuse',fuse)]:
   rows=[{'path':str(f.relative_to(directory)),'sha256':digest(f)} for f in sorted(directory.rglob('*')) if f.is_file()]
   (a.evidence/(name+'-original-files.json')).write_text(json.dumps(rows,indent=2)+'\n')
  original_mod=(engine/'go.mod').read_text();(a.workspace/'engine.mod').write_text(original_mod+'\nreplace github.com/hanwen/go-fuse/v2 => '+str(fuse)+'\n');shutil.copyfile(engine/'go.sum',a.workspace/'engine.sum')
  go='/usr/local/go126/bin/go';r['go_version']=run([go,'version']).strip();assert r['go_version']=='go version go1.26.11 dragonfly/amd64';r['compiler']=run(['cc','--version'])
  prior=a.base/'evidence/build/gocryptfs-native-assessment.json';assert json.loads(prior.read_text())['status']=='native-source-build-failed'
  headers=[pathlib.Path(x) for x in ['/usr/include/sys/dirent.h','/usr/include/sys/stat.h','/usr/include/sys/mount.h','/usr/include/sys/fcntl.h','/usr/include/sys/syscall.h','/usr/include/sys/syslimits.h']]+[a.base/'base-source/sys/vfs/fuse/fuse_abi.h']
  for f in [archive,zpath,zpath.with_suffix('.ziphash'),engine/'go.mod',engine/'go.sum',fuse/'go.mod',prior,*headers]:r['inputs'].append({'path':str(f),'sha256':digest(f)})
  r['go_fuse_go_sum_h1']=h1;r['stock_wire_source']='e17b9176356e3afb3427d71c21108a8a849cd4d5/sys/vfs/fuse/fuse_abi.h';r['wire_header_is_installed']=False
  source=pathlib.Path(__file__).with_name('abi-probe.c');binary=a.workspace/'abi-probe';run(['cc','-Wall','-Wextra','-Werror','-I'+str(a.base/'base-source/sys'),source,'-o',binary]);abi=json.loads(run([binary]));assert abi['sizeof.struct fuse_init_out']==64 and abi['FUSE_KERNEL_VERSION']==7 and abi['FUSE_KERNEL_MINOR_VERSION']==28
  (a.evidence/'native-abi.json').write_text(json.dumps(abi,indent=2)+'\n');r['probe_source_sha256']=digest(source);r['probe_binary_sha256']=digest(binary);r['status']='frozen-native-ABI-passed-no-transport'
 except Exception as x:r.update(status='failed',error=str(x))
 r['finished']=time.time();(a.evidence/'inputs.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'status':r['status'],'evidence':str(a.evidence),'workspace':str(a.workspace),'error':r.get('error')}));return r['status']=='failed'
if __name__=='__main__':raise SystemExit(main())
