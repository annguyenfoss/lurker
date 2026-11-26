#!/usr/bin/env python3
"""Build ordinary external drivers for the installed OBS kernel, never a kernel."""
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
ROOT=Path('/var/tmp/lurker-native-fresh-20261127-opensuse')
KERNEL='7.3.6-lp160.4.gf857b30-default'
ZFS_PREFIX=Path('/opt/lurker-native-fresh-20261127-zfs73')
PINS={'apfs':'6cf6de8ad6fa4cb634a700b5d0436d73ee01942c','hammer2':'09c130bf8bac021b0bb6a8ed9c9685a5bd9bbbd0','zfs':'3b6d727ddd8d6d005774605964f044f364ee6456'}
def main():
    if os.uname().nodename!='opensuse-core-01' or os.geteuid()!=1300:raise SystemExit('Build as adjutant on the owned openSUSE VM')
    os.environ['PATH']='/usr/local/sbin:/usr/local/bin:/usr/sbin:/sbin:/usr/bin:/bin'
    result={'host':os.uname().nodename,'running_kernel':os.uname().release,'target_kernel':KERNEL,'pid':os.getpid(),'started':time.time(),'kind':'external driver modules; no kernel build','sources':PINS,'cases':[],'status':'running'}
    dest=ROOT/'driver-build';dest.mkdir(mode=0o700,exist_ok=True)
    def save():
        t=dest/'result.tmp';t.write_text(json.dumps(result,indent=2));t.replace(dest/'result.json')
    def run(args,name,cwd=None,env=None,timeout=7200):
        r={'name':name,'argv':list(map(str,args)),'log':str(dest/(name+'.log'))};result['cases'].append(r);save()
        with Path(r['log']).open('wb') as f:
            p=subprocess.Popen(list(map(str,args)),cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT,stdin=subprocess.DEVNULL);r['pid']=p.pid;save();r['exit_code']=p.wait(timeout=timeout)
        save()
        if r['exit_code']:raise RuntimeError(name+' failed; see '+r['log'])
    def install_module(name,module):
        target=Path('/lib/modules')/KERNEL/'extra'/(name+'.ko')
        if target.exists():
            backup=dest/(name+'-previous.ko');shutil.copy2(target,backup);result.setdefault('previous_modules',{})[name]={'path':str(backup),'sha256':hashlib.sha256(backup.read_bytes()).hexdigest()}
        run(['sudo','-n','install','-D','-m644',module,target],name+'-install')
        result.setdefault('modules',{})[name]={'path':str(target),'sha256':hashlib.sha256(target.read_bytes()).hexdigest(),'source_commit':PINS.get(name,PINS['zfs']),'vermagic':subprocess.check_output(['/sbin/modinfo','-F','vermagic',str(target)],text=True).strip()};save()
    save()
    for name in ('apfs','hammer2','zfs'):
        source=ROOT/'drivers'/name
        try:
            assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=source,text=True).strip()==PINS[name]
            subprocess.run(['git','diff','--exit-code'],cwd=source,check=True,stdout=subprocess.DEVNULL)
            if name=='apfs':
                run(['make','-j1','KERNEL_DIR=/lib/modules/'+KERNEL+'/build'],'apfs-build',source);install_module(name,source/'apfs.ko')
            elif name=='hammer2':
                run(['make','-j1','KDIR=/lib/modules/'+KERNEL+'/build'],'hammer2-build',source);install_module(name,source/'src/sys/fs/hammer2/hammer2.ko')
            else:
                env=os.environ.copy();env['LDFLAGS']='-Wl,-rpath,'+str(ZFS_PREFIX/'lib')
                run(['sh','autogen.sh'],'zfs-autogen',source,env)
                run(['./configure','--prefix='+str(ZFS_PREFIX),'--libdir='+str(ZFS_PREFIX/'lib'),'--with-linux='+str((Path('/lib/modules')/KERNEL/'source').resolve()),'--with-linux-obj='+str((Path('/lib/modules')/KERNEL/'build').resolve()),'--with-config=all','--disable-pyzfs'],'zfs-configure',source,env)
                run(['make','-j1'],'zfs-build',source,env)
                stage=dest/'zfs-stage';stage.mkdir(exist_ok=True)
                # Mirror this host's /lib -> usr/lib layout inside DESTDIR.
                # Kernel modules_install uses usr/lib; OpenZFS cleanup uses lib.
                (stage/'usr/lib').mkdir(parents=True,exist_ok=True)
                if not os.path.lexists(stage/'lib'):(stage/'lib').symlink_to('usr/lib')
                run(['make','install','DESTDIR='+str(stage)],'zfs-stage',source,env)
                staged=stage/str(ZFS_PREFIX).lstrip('/')
                if not staged.is_dir() or ZFS_PREFIX.exists():raise RuntimeError('Missing staged prefix or refusing to overwrite an existing private prefix')
                run(['sudo','-n','cp','-a',staged,ZFS_PREFIX],'zfs-prefix-copy')
                run(['sudo','-n','chown','-R','root:root',ZFS_PREFIX],'zfs-prefix-owner')
                install_module('spl',source/'module/spl.ko')
                install_module(name,source/'module/zfs.ko')
                result['zfs_prefix']=str(ZFS_PREFIX)
            result.setdefault('outcomes',{})[name]='built-and-staged; not running until kernel boot and native tests'
        except Exception as e:result.setdefault('outcomes',{})[name]={'status':'failed','error':str(e)}
        save()
    run(['sudo','-n','depmod','-a',KERNEL],'target-depmod')
    result.update(status='terminal',finished=time.time());save();print(json.dumps({'receipt':str(dest/'result.json'),'outcomes':result.get('outcomes')}))
if __name__=='__main__':main()
