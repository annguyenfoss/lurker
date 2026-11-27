#!/usr/bin/env python3
"""Native OpenBSD primitive controls. Never mount, format or install anything."""
import argparse, hashlib, importlib.util, json, os, pathlib, platform, subprocess, time

def sha(path):
    with path.open('rb') as stream: return hashlib.file_digest(stream,'sha256').hexdigest()

def main():
    p=argparse.ArgumentParser();p.add_argument('--output',type=pathlib.Path,required=True)
    p.add_argument('--controls',required=True);args=p.parse_args()
    assert platform.system()=='OpenBSD' and platform.machine()=='amd64'
    assert os.getuid()==1300 and os.geteuid()==1300
    root=pathlib.Path('/srv/lurker-openbsd-lkl-build/adapted-first')
    source=root/'linux-d0f76a77e8c2ca68bd7d96190b8da09040eab072'
    own=pathlib.Path(__file__).resolve().parent
    library=source/'tools/lkl/liblkl.a';expected='d07dbeda5039932129d5a66dd232fd8d92c5d4e0aafecdbb98f718fe0070d4d3'
    assert sha(library)==expected
    args.output.mkdir(mode=0o700)
    files=[x for x in own.iterdir() if x.is_file()]
    receipt={'started':time.time(),'uid':os.getuid(),'scope':'native primitives/RAM only; no host disk registration, FUSE mount, formatter or install','library_sha256':expected,'inputs':{x.name:sha(x) for x in files}}
    (args.output/'inputs-before-execution.json').write_text(json.dumps(receipt,indent=2)+'\n')
    spec=importlib.util.spec_from_file_location('producer','/srv/lurker-openbsd-lkl-build/preview/scripts/build-openbsd-lkl.py')
    producer=importlib.util.module_from_spec(spec);spec.loader.exec_module(producer)
    receipt['controller_sha256']=sha(pathlib.Path(spec.origin))
    receipt['host_mounts_before']=subprocess.check_output(['/sbin/mount'],text=True)
    results=[]
    for name in args.controls.split(','):
        assert name in ('caller','abi','credential','disk','shutdown')
        binary=args.output/(name+'-control')
        cmd=['/usr/local/bin/egcc','-std=gnu11','-O2','-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-unused-parameter','-pie','-Wl,-z,noexecstack','-I'+str(root/'openbsd-include'),'-I'+str(source/'tools/lkl/include'),'-I'+str(own),str(own/(name+'-control.c'))]
        if name=='credential':cmd+=[str(library)]
        cmd+=['-lpthread','-o',str(binary)]
        r={'name':name,'command':cmd}
        c=subprocess.run(cmd,stdout=(args.output/(name+'-compile.stdout')).open('xb'),stderr=(args.output/(name+'-compile.stderr')).open('xb'));r['compile_returncode']=c.returncode
        if c.returncode==0:
            r['binary_sha256']=sha(binary)
            hdr=subprocess.check_output(['/usr/local/bin/greadelf','-l',str(binary)],text=True)
            (args.output/(name+'-readelf.txt')).write_text(hdr);assert '/usr/libexec/ld.so' in hdr
            cmd=[str(binary)]
            if name in ('credential','disk'):cmd+=[str(args.output/(name+'-fixture'))]
            r.update(producer.run_build(cmd,args.output,dict(os.environ),args.output/(name+'-run.log'),45))
        results.append(r)
    receipt['results']=results;receipt['finished']=time.time()
    receipt['host_mounts_after']=subprocess.check_output(['/sbin/mount'],text=True)
    receipt['library_sha256_after']=sha(library)
    receipt['status']='passed' if all(x.get('returncode')==0 and not x.get('timed_out') for x in results) and receipt['host_mounts_before']==receipt['host_mounts_after'] and receipt['library_sha256_after']==expected else 'failed'
    (args.output/'result.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps({'output':str(args.output),'status':receipt['status'],'results':results},indent=2))
if __name__=='__main__':main()
