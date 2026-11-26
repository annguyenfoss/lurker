#!/usr/bin/env python3
"""Run each fixed Linux cipher/credential profile with the downloaded payload."""
import argparse,hashlib,json,os,subprocess,time
from pathlib import Path

# sudo secure_path excludes locally installed, administrator-owned native tools.
os.environ["PATH"]="/usr/local/sbin:/usr/local/bin:/usr/sbin:/sbin:/usr/bin:/bin"

ROOT=Path('/var/tmp/lurker-native-fresh-20261127-opensuse')
PREFIX=Path('/usr/local/lib/lurker-native-fresh-20261127-opensuse')
COMMIT='8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d'

def main():
    if os.uname().nodename!='opensuse-core-01' or os.geteuid()!=0:
        raise SystemExit('Run the administrator-owned copy on opensuse-core-01 only')
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix',type=Path,default=PREFIX);parser.add_argument('--round-root',type=Path,default=ROOT);parser.add_argument('--payload-dir',type=Path,default=ROOT/'payload');parser.add_argument('--commit',default=COMMIT);parser.add_argument('--harness',type=Path,default=PREFIX/'harness/test-filesystem-cli-variations.py');a=parser.parse_args()
    root=a.round_root/'variations';root.mkdir(mode=0o700,exist_ok=True)
    cli=a.prefix/'bin/lurker';harness=a.harness
    result={'source_commit':a.commit,'harness_revision':'explicit frozen script input','harness_sha256':hashlib.sha256(harness.read_bytes()).hexdigest(),'binary_sha256':hashlib.sha256(cli.read_bytes()).hexdigest(),'host':os.uname().nodename,'kernel':os.uname().release,'pid':os.getpid(),'started':time.time(),'cases':[],'status':'running'}
    def save():
        p=root/'matrix.json';t=p.with_suffix('.tmp');t.write_text(json.dumps(result,indent=2)+'\n');t.replace(p)
    save()
    for backend in ('luks','luks1','veracrypt'):
        for cipher in ('aes','serpent','twofish'):
            for credential in ('password','keyfile'):
                row={'backend':backend,'cipher':cipher,'credential':credential,'filesystem':'btrfs','filesystem_role':'inner','source_kind':'container','route':'staged-product','auto_detect':True}
                log=root/(backend+'-'+cipher+'-'+credential+'.log')
                argv=['/usr/bin/python3',str(harness),'--binary',str(cli),'--backend',backend,'--cipher',cipher,'--credential',credential,'--auto-detect','--filesystem','btrfs','--root',str(root),'--commit',a.commit,'--size-gb','0.25','--payload-dir',str(a.payload_dir)]
                result['cases'].append(row);row.update(status='running',started=time.time(),log=str(log));save()
                try:
                    p=subprocess.run(argv,capture_output=True,text=True,timeout=900)
                    log.write_text(p.stdout+p.stderr);row['exit_code']=p.returncode
                    summary=json.loads(p.stdout.splitlines()[-1]);receipt=Path(summary['receipt']);native=json.loads(receipt.read_text());row.update(receipt=native,status=native['status'],receipt_path=str(receipt))
                    if native['status']=='passed' and not native.get('mount_remaining'):
                        source=Path(native['source'])
                        if source.parent==receipt.parent and source.name=='vault.img':source.unlink();row['passed_ciphertext_removed']=True
                except Exception as e:row.update(status='failed',error=str(e))
                row['finished']=time.time();save()
    result.update(status='terminal',finished=time.time());save()
    print(json.dumps({'receipt':str(root/'matrix.json'),'counts':{s:sum(r['status']==s for r in result['cases']) for s in ('passed','failed')}}))
if __name__=='__main__':main()
