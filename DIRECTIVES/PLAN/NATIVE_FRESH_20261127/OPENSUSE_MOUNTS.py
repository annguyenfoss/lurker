#!/usr/bin/env python3
"""Record actual mount/superblock identity only for this round's owned fixtures."""
import json,os,re,subprocess,time
from pathlib import Path
ROOT=Path('/var/tmp/lurker-native-fresh-20261127-opensuse')
def decode(v):return re.sub(r'\\([0-7]{3})',lambda m:chr(int(m[1],8)),v)
def main():
    if os.geteuid()!=0 or os.uname().nodename!='opensuse-core-01':raise SystemExit('Run the trusted copy on the owned openSUSE VM')
    seen=set();output=ROOT/'mount-identities.jsonl'
    with output.open('a',buffering=1) as f:
        while not (ROOT/'stop-mount-observer').exists():
            for line in Path('/proc/self/mountinfo').read_text().splitlines():
                left,sep,right=line.partition(' - ');a=left.split();b=right.split()
                if not sep or len(a)<6 or len(b)<2:continue
                point=decode(a[4]);source=decode(b[1]);key=(a[0],point,source)
                if not point.startswith(str(ROOT)+'/') or key in seen:continue
                seen.add(key);r={'host':os.uname().nodename,'kernel':os.uname().release,'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip(),'timestamp':time.time(),'mount_id':a[0],'mountpoint':point,'filesystem':b[0],'source':source,'options':a[5].split(',')}
                # The owned mountpoint scopes the query. Never discover or probe
                # unrelated block devices, and never modify any resource.
                if source.startswith('/dev/mapper/lurker_') or source.startswith('/dev/loop') or source.startswith(str(ROOT)+'/'):
                    try:
                        p=subprocess.run(['/usr/sbin/blkid','-p','-o','export',source],capture_output=True,text=True,timeout=3)
                        r['superblock']={'exit_code':p.returncode,'stdout':p.stdout[-8000:],'stderr':p.stderr[-2000:]}
                    except Exception as e:r['superblock_error']=str(e)
                f.write(json.dumps(r)+'\n')
            time.sleep(.1)
if __name__=='__main__':main()
