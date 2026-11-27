#!/usr/bin/env python3
"""Opt-in host-only rebuild against the exact qualified kernel/generated ABI.

No formatter, device registration, FUSE mount or installation is performed.
"""
import argparse,hashlib,importlib.util,json,os,pathlib,platform,shlex,subprocess,time
BASELINE='d07dbeda5039932129d5a66dd232fd8d92c5d4e0aafecdbb98f718fe0070d4d3'
SOURCE='7f8cd122383ebca3ba5f497c6eddfcf302a35c6ed108be56b7823f2528dd370a'
PATCH='2079b42fad7998d69d8fa4aa9739c68ea614f33318b8fbeab1713af2929c444f'
def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def main():
    a=argparse.ArgumentParser();a.add_argument('--baseline',type=pathlib.Path,required=True);a.add_argument('--output',type=pathlib.Path,required=True);args=a.parse_args()
    assert platform.system()=='OpenBSD' and platform.machine()=='amd64' and os.getuid()==1300 and os.geteuid()==1300
    own=pathlib.Path(__file__).resolve().parent;src=args.baseline;lib=src/'tools/lkl';assert sha(lib/'liblkl.a')==BASELINE and sha(lib/'lib/virtio_blk.c')==SOURCE and sha(own/'flush.patch')==PATCH
    args.output.mkdir(mode=0o700);out=args.output
    env={'HOME':str(pathlib.Path.home()),'PATH':'/bin:/usr/bin:/usr/local/bin','LC_ALL':'C'}
    inputs={str(x):sha(x) for x in [own/'build-flush-host.py',own/'flush.patch',own/'flush-codec-control.c',lib/'lib/lkl.o',lib/'lib/liblkl-in.o',lib/'lib/.liblkl-in.o.cmd']}
    sdk={str(p.relative_to(lib/'include')):sha(p) for p in (lib/'include').rglob('*') if p.is_file()}
    receipt={'started':time.time(),'scope':'host-only virtio transport codec/library/RAM checks; no device registration/mount/format/install','baseline_sha256':BASELINE,'inputs':inputs,'generated_sdk':sdk,'commands':[]}
    (out/'inputs-before-execution.json').write_text(json.dumps(receipt,indent=2)+'\n')
    def run(argv,name,cwd=None):
        receipt['commands'].append({'name':name,'argv':[str(x) for x in argv]})
        (out/'progress.json').write_text(json.dumps(receipt,indent=2)+'\n')
        with (out/(name+'.stdout')).open('xb') as so,(out/(name+'.stderr')).open('xb') as se:
            r=subprocess.run([str(x) for x in argv],cwd=cwd,env=env,stdout=so,stderr=se)
        if r.returncode:raise RuntimeError(f'{name} failed: {r.returncode}; retained')
    # Verify original object membership against the immutable qualified archive.
    for name in ['lkl.o','liblkl-in.o']:
        p=subprocess.Popen(['/usr/local/bin/gar','p',str(lib/'liblkl.a'),name],stdout=subprocess.PIPE)
        with p.stdout as stream:h=hashlib.file_digest(stream,'sha256').hexdigest()
        assert p.wait()==0 and h==sha(lib/'lib'/name)
    # Re-link unchanged host objects first, proving the individual inputs reproduce
    # the exact host member of d07. The large kernel member is only read/copied.
    original=shlex.split((lib/'lib/.liblkl-in.o.cmd').read_text().split(' := ',1)[1].split('\n',1)[0])
    assert original[0]=='/usr/bin/ld' and original[1:3]==['-r','-o']
    objects=[pathlib.Path(x) for x in original[4:]]
    assert len(objects)==9 and all(x.parent==lib/'lib' and x.is_file() for x in objects)
    receipt['host_objects']={str(x):sha(x) for x in objects}
    run(original[:3]+[out/'baseline-host-recheck.o']+objects,'relink-baseline')
    assert sha(out/'baseline-host-recheck.o')==sha(lib/'lib/liblkl-in.o')
    # Exact native patch application in a private one-file tree.
    tree=out/'source/tools/lkl/lib';tree.mkdir(parents=True);(tree/'virtio_blk.c').write_bytes((lib/'lib/virtio_blk.c').read_bytes())
    run(['/usr/bin/patch','-p1','-i',own/'flush.patch'],'apply-host-patch',out/'source')
    compile=['/usr/local/bin/egcc','-include','endian.h','-I'+str(src.parent/'openbsd-include'),'-I'+str(lib/'include'),'-I'+str(lib/'lib'),'-fPIC','-pthread','-Wall','-g','-O2','-Wextra','-Wno-unused-parameter','-Wno-missing-field-initializers','-fno-strict-aliasing','-DBUILD_STR(s)=#s']
    run(compile+['-c',tree/'virtio_blk.c','-o',out/'virtio_blk.o'],'compile-transport')
    revised=[out/'virtio_blk.o' if p.name=='virtio_blk.o' else p for p in objects]
    run(original[:3]+[out/'liblkl-in.o']+revised,'relink-host')
    run(['/usr/local/bin/gar','rcs',out/'liblkl-flush.a',out/'liblkl-in.o',lib/'lib/lkl.o'],'archive-library')
    run(compile[:1]+['-I'+str(tree)]+compile[1:]+[own/'flush-codec-control.c','-o',out/'flush-codec-control'],'compile-codec')
    run([out/'flush-codec-control'],'run-codec')
    receipt.update(library_sha256=sha(out/'liblkl-flush.a'),kernel_object_sha256=sha(lib/'lib/lkl.o'),host_member_sha256=sha(out/'liblkl-in.o'),codec_sha256=sha(out/'flush-codec-control'),patched_source_sha256=sha(tree/'virtio_blk.c'),finished=time.time(),status='passed')
    assert sha(lib/'liblkl.a')==BASELINE and all(sha(lib/'include'/name)==h for name,h in sdk.items())
    receipt['baseline_and_generated_abi_unchanged']=True
    (out/'result.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({k:receipt[k] for k in ['status','library_sha256','kernel_object_sha256','codec_sha256','baseline_and_generated_abi_unchanged']},indent=2))
if __name__=='__main__':main()
