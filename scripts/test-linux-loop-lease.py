#!/usr/bin/env python3
"""Bounded Linux loop-descriptor lifetime tests on fresh private /tmp files.

Root in a disposable VM only. No encryption, mounts, existing devices, forced
unmounts or detach-by-name cleanup. SIGKILL targets recorded owned processes.
"""
import argparse
import array
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pwd
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time


def identity(pid):
    proc = Path('/proc') / str(pid)
    fields = (proc / 'stat').read_text().rsplit(') ', 1)[1].split()
    uid = next(line.split()[1:] for line in (proc / 'status').read_text().splitlines() if line.startswith('Uid:'))
    return {'birth': fields[19], 'uids': list(map(int, uid)), 'state': fields[0]}


def exact(stream, count):
    result = b''
    while len(result) < count:
        chunk = stream.recv(count - len(result))
        if not chunk: raise RuntimeError('Loop helper closed before reply')
        result += chunk
    return result


def worker(root, helper, route, readonly, path_only):
    root, helper = Path(root), Path(helper)
    source = root / 'source.img'; metadata = source.stat()
    descriptor = os.open(source, (os.O_PATH if path_only else os.O_RDONLY) | os.O_CLOEXEC)
    if not path_only: fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
    endpoint = root / 'control.sock'
    server = socket.socket(socket.AF_UNIX); server.bind(str(endpoint)); server.listen(1); server.settimeout(10)
    command = (['sudo', '-n'] if route == 'user' else []) + [str(helper), 'loop-lease', str(endpoint)]
    with (root / 'helper.log').open('wb') as log:
        child = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        connection, _ = server.accept(); connection.settimeout(10)
        pid, uid, gid = struct.unpack('3i', connection.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
        assert uid == 0
        request = json.dumps({'protocol': 1, 'device': metadata.st_dev, 'inode': metadata.st_ino, 'readonly': readonly}).encode()
        connection.sendmsg([struct.pack('!I', len(request)) + request], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array('i', [descriptor]))])
        reply = json.loads(exact(connection, struct.unpack('!I', exact(connection, 4))[0]))
        print(json.dumps({'reply': reply, 'helper_pid': pid, 'helper_identity': identity(pid), 'caller_pid': os.getpid()}), flush=True)
        sys.stdin.buffer.read(1)
        connection.close(); server.close(); os.close(descriptor)
        child.wait(timeout=10)


def trusted(path):
    path = Path(path).resolve(strict=True)
    for component in [path, *path.parents]:
        metadata = component.stat()
        if metadata.st_uid != 0 or metadata.st_mode & 0o022: raise ValueError('Administrator-controlled test executable required: ' + str(component))
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--helper', type=Path, required=True)
    parser.add_argument('--cli', type=Path, help='Also kill the real CLI before its read-only header query')
    parser.add_argument('--result', type=Path, required=True)
    parser.add_argument('--source-commit', required=True)
    options = parser.parse_args()
    if os.geteuid() != 0 or os.uname().nodename.split('.')[0] == 'endeavour': parser.error('Use the disposable Linux test VM as root')
    if not hasattr(os, 'pidfd_open') or not hasattr(signal, 'pidfd_send_signal'): parser.error('Linux pidfd support is required')
    helper = trusted(options.helper); user = pwd.getpwnam('adjutant')
    root = Path(tempfile.mkdtemp(prefix='lurker-loop-lifetime-', dir='/tmp')); root.chmod(0o755)
    report = {'schema': 1, 'status': 'running', 'source_commit': options.source_commit,
              'helper_sha256': hashlib.sha256(helper.read_bytes()).hexdigest(), 'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'root': str(root), 'cases': [], 'started': time.time()}
    def save():
        temporary = options.result.with_suffix('.json.tmp'); temporary.write_text(json.dumps(report, indent=2) + '\n'); temporary.chmod(0o600); temporary.replace(options.result)
    def rows(source):
        metadata = source.stat()
        result = json.loads(subprocess.check_output(['losetup', '--json', '--list', '--output', 'NAME,AUTOCLEAR,RO,BACK-INO,BACK-MAJ:MIN'], text=True))['loopdevices']
        return [row for row in result if row['back-ino'] == metadata.st_ino and row['back-maj:min'] == f'{os.major(metadata.st_dev)}:{os.minor(metadata.st_dev)}']
    def gone(source):
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if not rows(source): return
            time.sleep(.05)
        raise RuntimeError('Owned source still has a loop attachment; retained without detach')
    def kill_owned(pid, expected):
        assert pid not in [0, 1, os.getpid(), os.getppid()]
        descriptor = os.pidfd_open(pid)
        try:
            actual = identity(pid); assert actual['birth'] == expected['birth'] and actual['uids'] == expected['uids']
            signal.pidfd_send_signal(descriptor, signal.SIGKILL)
            import select
            assert select.select([descriptor], [], [], 10)[0], 'Owned process did not exit'
        finally: os.close(descriptor)
    plans = [('root-rw-release', 'root', False, 'release'), ('root-ro-release', 'root', True, 'release'),
             ('root-caller-sigkill', 'root', False, 'caller-kill'), ('root-helper-sigkill', 'root', False, 'helper-kill'),
             ('user-rw-release', 'user', False, 'release'), ('user-ro-release', 'user', True, 'release'),
             ('user-caller-sigkill', 'user', False, 'caller-kill'), ('user-helper-sigkill', 'user', False, 'helper-kill'),
             ('path-only-source-rejected', 'root', True, 'path-only')]
    active = None
    try:
        for name, route, readonly, action in plans:
            entry = {'name': name, 'status': 'running'}; report['cases'].append(entry); save(); start = time.monotonic()
            directory = root / name; directory.mkdir(mode=0o700)
            source = directory / 'source.img'
            with source.open('xb') as stream: stream.truncate(16 * 1024 * 1024)
            if route == 'user':
                os.chown(directory, user.pw_uid, user.pw_gid); os.chown(source, user.pw_uid, user.pw_gid)
            command = (['runuser', '-u', 'adjutant', '--'] if route == 'user' else []) + [sys.executable, str(Path(__file__).resolve()), '--worker', str(directory), str(helper), route, str(int(readonly)), str(int(action == 'path-only'))]
            env = os.environ.copy(); env.pop('XDG_RUNTIME_DIR', None)
            with (directory / 'worker.log').open('wb') as log:
                child = subprocess.Popen(command, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log)
                active = child
                import select
                assert select.select([child.stdout], [], [], 15)[0], 'Owned caller did not complete lease setup'
                line = child.stdout.readline(8192)
                if not line: raise RuntimeError('Owned caller setup failed; ' + str(directory / 'worker.log'))
                ready = json.loads(line); entry['caller_pid'] = ready['caller_pid']; entry['helper_pid'] = ready['helper_pid']
                observed = rows(source)
                if action == 'path-only':
                    assert ready['reply']['error'] and not observed
                else:
                    assert ready['reply']['error'] is None and len(observed) == 1
                    assert observed[0]['autoclear'] and bool(observed[0]['ro']) == readonly
                    entry['autoclear_from_setup'] = True
                if action == 'caller-kill':
                    kill_owned(ready['caller_pid'], identity(ready['caller_pid']))
                elif action == 'helper-kill':
                    kill_owned(ready['helper_pid'], ready['helper_identity'])
                if action != 'caller-kill': child.stdin.write(b'x'); child.stdin.flush()
                child.stdin.close(); child.wait(timeout=15); active = None
                gone(source)
                # Last FD closure must release the inherited inode flock, too.
                with source.open('rb') as stream: fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
                entry.update(status='passed', duration_seconds=round(time.monotonic() - start, 3)); save()
        if options.cli:
            cli = trusted(options.cli)
            directory = root / 'cli-before-header'; directory.mkdir(mode=0o700)
            source = directory / 'source.img'
            with source.open('xb') as stream: stream.truncate(16 * 1024 * 1024)
            key = directory / 'key.bin'; key.write_bytes(os.urandom(32)); key.chmod(0o600)
            point = directory / 'mount'; point.mkdir(mode=0o700)
            tools = options.result.parent / 'loop-fault-tools'; tools.mkdir(mode=0o755)
            wrapper = tools / 'cryptsetup'
            wrapper.write_text("#!/usr/bin/python\nimport json,os,sys,time\nfrom pathlib import Path\np=Path(os.environ['LURKER_LOOP_GATE'])\np.joinpath('ready.json').write_text(json.dumps({'pid':os.getpid(),'parent':os.getppid()}))\nparent=os.getppid();deadline=time.monotonic()+20\nwhile os.getppid()==parent and time.monotonic()<deadline:time.sleep(.01)\nraise SystemExit(71)\n")
            wrapper.chmod(0o755); trusted(wrapper)
            env = os.environ.copy(); env.update(PATH=str(tools)+':/usr/bin:/usr/sbin', LURKER_LOOP_GATE=str(directory))
            with (directory/'cli.log').open('wb') as log:
                active = subprocess.Popen([str(cli),'mount','--type','luks1','--key-file',str(key),str(source),str(point)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log)
                deadline=time.monotonic()+15
                while not (directory/'ready.json').exists():
                    if active.poll() is not None:raise RuntimeError('CLI exited before gated header query')
                    if time.monotonic()>deadline:raise RuntimeError('CLI did not reach gated header query')
                    time.sleep(.01)
                assert len(rows(source))==1 and rows(source)[0]['autoclear']
                kill_owned(active.pid,identity(active.pid));active.wait(timeout=10);active=None
                gone(source)
                with source.open('rb') as stream:fcntl.flock(stream,fcntl.LOCK_EX|fcntl.LOCK_NB)
            report['cases'].append({'name':'real-cli-sigkill-before-header-query','status':'passed','no_crypto_tool_executed':True})
        report['status'] = 'passed' 
    except Exception as error:
        report.update(status='failed', diagnostic=str(error))
        if report['cases']: report['cases'][-1]['status'] = 'failed'
    finally:
        if active is not None and active.poll() is None:
            try:
                if active.stdin is not None and not active.stdin.closed: active.stdin.close()
                active.wait(timeout=25)
            except (OSError, subprocess.TimeoutExpired): report['retained_caller_pid'] = active.pid
        report.update(finished=time.time()); report['duration_seconds'] = round(report['finished'] - report['started'], 3); save()
    print(json.dumps({'status': report['status'], 'passed': sum(x['status'] == 'passed' for x in report['cases']), 'required': len(plans)+int(options.cli is not None), 'result': str(options.result)}))
    return report['status'] != 'passed'


if __name__ == '__main__':
    if len(sys.argv) == 7 and sys.argv[1] == '--worker':
        worker(sys.argv[2], sys.argv[3], sys.argv[4], bool(int(sys.argv[5])), bool(int(sys.argv[6])))
    else: raise SystemExit(main())
