#!/usr/bin/env python3
"""Owned Gentoo scratch-device lifecycle using the shared PTY/payload primitives."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import secrets
import stat
import subprocess
import tempfile
import pwd
import time


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--device',type=Path,required=True)
    p.add_argument('--serial',required=True)
    p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--root',type=Path,required=True)
    p.add_argument('--payload-dir',type=Path,required=True)
    p.add_argument('--commit',required=True)
    p.add_argument('--filesystems',required=True)
    p.add_argument('--skip-passed-ledger',type=Path)
    a=p.parse_args()
    if os.geteuid()!=0: p.error('Use administrator-controlled harness and binary through sudo')
    device=a.device.resolve(strict=True)
    if not stat.S_ISBLK(device.stat().st_mode): p.error('Expected identified scratch block device')
    identity=json.loads(subprocess.check_output(['lsblk','--json','--tree','--bytes','-o','PATH,SERIAL,SIZE,TYPE,MOUNTPOINTS',str(device)]))['blockdevices'][0]
    if identity.get('serial')!=a.serial or a.serial!='lurker-fresh-gentoo' or identity.get('type')!='disk' or identity.get('size')!=2147483648:
        p.error('Scratch identity differs; refusing destructive commands')
    spec=importlib.util.spec_from_file_location('payload',Path(__file__).with_name('test-filesystem-cli.py'))
    h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
    fspec=importlib.util.spec_from_file_location('features',Path(__file__).with_name('test-gentoo-native-fresh.py'))
    feature=importlib.util.module_from_spec(fspec);fspec.loader.exec_module(feature)
    ts=importlib.util.spec_from_file_location('device_terminal',Path(__file__).with_name('gentoo-device-terminal.py'))
    terminal=importlib.util.module_from_spec(ts);ts.loader.exec_module(terminal)
    reused=[]; skip=set()
    if a.skip_passed_ledger:
        prior=json.loads(a.skip_passed_ledger.read_text())
        if prior['source_commit']!=a.commit or prior['binary_sha256']!=h.file_hash(a.binary) or prior['device_identity'].get('serial')!=a.serial:
            raise SystemExit('Prior in-round ledger identity differs')
        reused=[dict(r,prior_ledger=str(a.skip_passed_ledger),prior_harness_sha256=prior['harness_sha256']) for r in prior['cases'] if r['status']=='passed']
        skip={(r['filesystem'],r['backend']) for r in reused}
    a.root.mkdir(parents=True,exist_ok=True,mode=0o700); os.chown(a.root,pwd.getpwnam('adjutant').pw_uid,pwd.getpwnam('adjutant').pw_gid)
    rows=list(reused)
    login=pwd.getpwnam('adjutant')
    out=a.root/'device-matrix.json'
    def save(): h.atomic_json(out,{'schema':1,'source_commit':a.commit,'binary':str(a.binary),'binary_sha256':h.file_hash(a.binary),'device_identity':identity,'harness_sha256':h.file_hash(Path(__file__)),'kernel':os.uname().release,'pid':os.getpid(),'cases':rows})
    save()
    def child_mounted(path):
        return any(line.split()[4]==str(path) for line in Path('/proc/self/mountinfo').read_text().splitlines())
    for fs in a.filesystems.split(','):
        for backend in ('luks','luks1','veracrypt'):
            if (fs,backend) in skip:continue
            d=Path(tempfile.mkdtemp(prefix=backend+'-'+fs+'-',dir=a.root));os.chown(d,login.pw_uid,login.pw_gid);mount=d/'mount';mount.mkdir()
            row={'backend':backend,'filesystem':fs,'role':'inner','source_kind':'block','route':'product-cli','device':str(device),'fixture':str(d),'status':'running','operations':[]}
            rows.append(row);save()
            password='Lurker9!'+secrets.token_urlsafe(24)
            expected=h.load_payload(a.payload_dir,None)
            def cli(action,operands):
                argv=[str(a.binary),action,'--type',backend,'--color=never',*map(str,operands)]
                log=d/(str(len(row['operations']))+'-'+action+'.log')
                receipt=terminal.terminal(argv,password,180,log,authorize_quick=backend=='veracrypt' and action=='create' and '--force' in argv and str(device) in argv)
                row['operations'].append({'action':action,'log':str(log),**receipt});save()
                if receipt['exit_code'] or receipt['timed_out']: raise RuntimeError('CLI '+action+' failed')
            try:
                # Re-check holders and mounts before every destructive create.
                current=json.loads(subprocess.check_output(['lsblk','--json','--tree','-o','PATH,SERIAL,TYPE,MOUNTPOINTS',str(device)]))['blockdevices'][0]
                holders=list((Path('/sys/class/block')/device.name/'holders').iterdir())
                if current.get('serial')!=a.serial or current.get('children') or any(current.get('mountpoints') or []) or holders:
                    row['identity_or_busy_guard_failed']=True
                    raise RuntimeError('Scratch disk became busy or changed identity; refusing create')
                cli('create',['--filesystem',fs,'--force',device])
                if backend in {'luks','luks1'}:
                    row['encryption_header']=feature.probe(['cryptsetup','luksDump',*(['--dump-json-metadata'] if backend=='luks' else []),device]);save()
                nested={}
                for phase in ('write','append','verify'):
                    cli('mount',[device,mount])
                    if not os.path.ismount(mount): raise RuntimeError('Mount absent; underlay write refused')
                    mount_identity=h.linux_mount_record(Path('/proc/self/mountinfo').read_text(),mount)
                    row[phase+'_native_mount']=mount_identity; row[phase+'_filesystem_identity']=h.check_linux_filesystem(mount_identity,fs)
                    if phase=='write':
                        row['features']=feature.features(mount)
                        row['superblock_observations']=[feature.probe(['blkid','-p',mount_identity['source']]),feature.probe(['file','-s',mount_identity['source']])]
                        row['native_crypto_status']=feature.probe(['cryptsetup','status',mount_identity['source']])
                    mounted_program=Path(__file__).with_name('gentoo-mounted.py')
                    check=subprocess.run(['/usr/bin/sudo','-n','-u',login.pw_name,'--','/home/adjutant/.local/bin/python3',str(mounted_program),phase,str(mount),str(a.payload_dir)],capture_output=True,text=True,timeout=90)
                    payload_log=d/(phase+'-payload.log');payload_log.write_text(check.stdout+check.stderr);row[phase+'_payload_log']=str(payload_log)
                    if check.returncode:raise RuntimeError('Login-user device payload '+phase+' failed')
                    row[phase+'_sha256']=json.loads(check.stdout)
                    for engine,child in nested.items():
                        result=child['result']
                        if result['status']=='failed':continue
                        def nested_cli(action,operands):
                            payload={'action':action,'volume_type':engine}
                            if action=='create':payload.update(target=str(child['source']),size_gb=None,force=False,source_kind='directory',cipher='aes',passphrase=child['password'])
                            elif action=='mount':payload.update(source=str(child['source']),mountpoint=str(child['mount']),tag=None,passphrase=child['password'])
                            else:payload.update(target=str(child['mount']),tag=None)
                            argv=['/usr/bin/sudo','-n','-u',login.pw_name,'--',str(a.binary.with_name('lurker-helper')),'run']
                            log=d/(engine+'-'+str(len(result['operations']))+'-'+action+'.log')
                            process=subprocess.run(argv,input=json.dumps(payload).encode(),capture_output=True,timeout=180)
                            output=process.stdout+process.stderr
                            if child['password'].encode() in output:raise RuntimeError('Helper exposed fixture credential')
                            log.write_bytes(output);answer=json.loads(process.stdout);result['operations'].append({'action':action,'log':str(log),'exit_code':process.returncode,'ok':answer.get('ok')});save()
                            if process.returncode or not answer.get('ok'):raise RuntimeError(engine+' '+action+' failed')
                        try:
                            if phase=='write':nested_cli('create',[child['source']])
                            nested_cli('mount',[child['source'],child['mount']])
                            if not child_mounted(child['mount']):raise RuntimeError('Directory mount absent; underlay writes refused')
                            mounted_program=Path(__file__).with_name('gentoo-mounted.py')
                            check=subprocess.run(['/usr/bin/sudo','-n','-u',login.pw_name,'--','/home/adjutant/.local/bin/python3',str(mounted_program),phase,str(child['mount']),str(a.payload_dir)],capture_output=True,text=True,timeout=90)
                            payload_log=d/(engine+'-'+phase+'-payload.log');payload_log.write_text(check.stdout+check.stderr);result[phase+'_payload_log']=str(payload_log)
                            if check.returncode:raise RuntimeError(engine+' payload '+phase+' failed')
                            result[phase+'_sha256']=json.loads(check.stdout);nested_cli('unmount',[child['mount']])
                            if phase=='verify':result['status']='passed'
                        except Exception as error:result.update(status='failed',error=str(error))
                        finally:
                            if child_mounted(child['mount']):
                                try:nested_cli('unmount',[child['mount']])
                                except Exception as error:result['cleanup_error']=str(error)
                            result['mount_remaining']=child_mounted(child['mount']);save()
                    cli('unmount',[mount])
                    if os.path.ismount(mount):raise RuntimeError('Normal close left mount present')
                row['status']='passed'
            except Exception as e:row.update(status='failed',error=str(e))
            finally:
                if os.path.ismount(mount):
                    try:cli('unmount',[mount])
                    except Exception as e:row['cleanup_error']=str(e)
                row['mount_remaining']=os.path.ismount(mount)
                for child_case in row.get('directory_backing_cases',[]):
                    if child_case['status']=='running':child_case.update(status='blocked',error='Backing-device lifecycle did not complete child verification')
                    child_case['backing_lifecycle_status']=row['status']
                if (row['status']=='failed' or any(c['status']!='passed' for c in row.get('directory_backing_cases',[]))) and not row['mount_remaining'] and not row.get('identity_or_busy_guard_failed'):
                    retained=d/'failed-device.img'
                    try:
                        subprocess.run(['dd','if='+str(device),'of='+str(retained),'bs=4M','conv=sparse','status=none'],check=True,timeout=180)
                        row['retained_device_image']=str(retained);row['retained_device_sha256']=h.file_hash(retained)
                    except Exception as e:row['retention_error']=str(e)
                row['finished']=time.time();save()
                if row.get('identity_or_busy_guard_failed') or row.get('retention_error') or row['mount_remaining']:
                    raise SystemExit('Device job stopped with retained/busy storage; reconcile before continuing')
    print(json.dumps({'receipt':str(out),'passed':sum(r['status']=='passed' for r in rows),'failed':sum(r['status']=='failed' for r in rows)}))

if __name__=='__main__':main()
