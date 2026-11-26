#!/usr/bin/env python3
"""Assess available native backing drivers using the shared Linux round."""
import importlib.util,os
from pathlib import Path
os.environ['PATH']='/usr/local/sbin:/usr/local/bin:/usr/sbin:/sbin:/usr/bin:/bin'
if os.uname().nodename!='opensuse-core-01':raise SystemExit('Owned openSUSE VM only')
spec=importlib.util.spec_from_file_location('shared_round',Path(__file__).with_name('test-linux-filesystem-round.py'))
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);ordinary=m.command

def command(args,**kwargs):
    args=list(map(str,args))
    if args[:3]==['sudo','-n','/usr/local/bin/fuse-ufs']:
        args=['sudo','-n','mount','-i','-t','ufs','-o','loop,rw,ufstype=ufs2',*args[-2:]]
    elif args[:3] in (['sudo','-n','zpool'],['sudo','-n','zfs']):
        args[2]='/usr/local/sbin/'+args[2]
    return ordinary(args,**kwargs)
m.command=command
if __name__=='__main__':m.main()
