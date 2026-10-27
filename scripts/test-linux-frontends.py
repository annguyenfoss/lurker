#!/usr/bin/env python3
"""Exercise CLI terminal prompts and desktop helper JSON on disposable images.
Run outside a filesystem sandbox:
  python scripts/test-linux-frontends.py target/release --unprivileged
  python scripts/test-linux-frontends.py target/release  # cached/NOPASSWD sudo
Never accesses an existing vault. All test sources live in one fresh /tmp directory.
"""
import errno
import contextlib
import shutil
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import subprocess
import sys
import tempfile
import termios
import time

BIN = Path(sys.argv[1] if len(sys.argv) > 1 else 'target/release').resolve()
UNPRIVILEGED = '--unprivileged' in sys.argv[2:]
PASSWORD = '  Frontend 密碼 test passphrase 123456789!  '

def terminal(args, cwd, cancel=False):
    if UNPRIVILEGED:
        args = [args[0], '--unprivileged', *args[1:]]
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(cwd)
        os.execv(str(BIN / 'lurker'), ['lurker', *args])
    output = b''
    pending = b''
    prompts = 0
    deadline = time.monotonic() + 180
    try:
        while time.monotonic() < deadline:
            ready, _, _ = select.select([fd], [], [], 1)
            if ready:
                try:
                    data = os.read(fd, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        break
                    raise
                if not data:
                    break
                output += data
                pending += data
                if re.search(rb'Type[^\r\n]*yes[^\r\n]*capital[^\r\n]*: *$', pending, re.I):
                    os.write(fd, b'YES\n')
                    pending = b''
                elif re.search(rb'(?:password|passphrase)[^\r\n]*: *$', pending, re.I):
                    if UNPRIVILEGED:
                        assert not termios.tcgetattr(fd)[3] & termios.ECHO, 'prompt was shown before echo was disabled'
                    prompts += 1
                    assert prompts <= 3, output.decode(errors='replace')
                    os.write(fd, b'\x03' if cancel else (PASSWORD + '\n').encode())
                    pending = b''
            completed, status = os.waitpid(pid, os.WNOHANG)
            if completed:
                assert (os.waitstatus_to_exitcode(status) != 0 if cancel else os.waitstatus_to_exitcode(status) == 0), output.decode(errors='replace')
                pid = None
                break
        else:
            raise AssertionError('CLI timed out: ' + output.decode(errors='replace'))
        if pid is not None:
            _, status = os.waitpid(pid, 0)
            pid = None
            assert (os.waitstatus_to_exitcode(status) != 0 if cancel else os.waitstatus_to_exitcode(status) == 0), output.decode(errors='replace')
        if cancel:
            assert termios.tcgetattr(fd)[3] & termios.ECHO, 'terminal echo was not restored after cancellation'
        assert PASSWORD.encode() not in output, 'passphrase was echoed by terminal'
        return prompts
    finally:
        if pid is not None:
            os.killpg(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
        os.close(fd)

def helper_command():
    return ([] if UNPRIVILEGED else ['sudo', '-n']) + [str(BIN / 'lurker-helper'), 'run']

def helper(action):
    if UNPRIVILEGED:
        action = dict(action, privilege_mode='unprivileged')
    result = subprocess.run(helper_command(), input=json.dumps(action).encode(), capture_output=True, timeout=180)
    assert result.returncode == 0, result.stderr.decode()
    response = json.loads(result.stdout)
    assert PASSWORD not in result.stdout.decode(), 'passphrase in helper logs'
    return response

def must_succeed(response):
    assert response['ok'], response

@contextlib.contextmanager
def fixture():
    root = Path(tempfile.mkdtemp(prefix='lurker-frontends-', dir='/tmp'))
    try:
        yield root
    finally:
        live = [line.split()[4] for line in Path('/proc/self/mountinfo').read_text().splitlines()
                if line.split()[4].startswith(str(root) + '/')]
        if live:
            print(f'Retaining live test fixture {root}: {live}', file=sys.stderr)
        else:
            shutil.rmtree(root)

def main():
    if UNPRIVILEGED:
        assert os.geteuid() != 0, 'Run unprivileged tests without sudo'
    else:
        subprocess.run(['sudo', '-n', 'true'], check=True)
    with fixture() as name:
        root = Path(name)
        at = root / 'mount with spaces'
        at.mkdir()
        if UNPRIVILEGED:
            runtime = root / 'runtime'
            runtime.mkdir(mode=0o700)
            os.environ['XDG_RUNTIME_DIR'] = str(runtime)
            deny = root / 'deny-elevation'; deny.mkdir()
            for program in ['sudo', 'doas', 'pkexec', 'mount', 'umount', 'losetup', 'dmsetup', 'cryptsetup']:
                script = deny / program
                script.write_text('#!/bin/sh\necho "Forbidden privileged tool invoked" >&2\nexit 91\n')
                script.chmod(0o755)
            os.environ['PATH'] = str(deny) + os.pathsep + os.environ.get('PATH', '')
        if UNPRIVILEGED:
            assert terminal(['create', 'cancelled.hc', '0.25'], root, cancel=True) == 1
            assert not (root / 'cancelled.hc').exists()
        kinds = [('veracrypt', 'createvc')] if UNPRIVILEGED else [('luks', 'create'), ('veracrypt', 'createvc')]
        for ty, verb in kinds:
            source = root / f'{ty} vault.img'
            try:
                # Bare relative filename verifies the current-directory create regression.
                prompts = terminal([verb, source.name, '0.25'], root)
                assert prompts == (2 if UNPRIVILEGED else 3), (ty, prompts)
                assert source.stat().st_mode & 0o777 == 0o600
                assert terminal(['mount', str(source), str(at)], root) == 1
                if not UNPRIVILEGED:
                    subprocess.run(['sudo', '-n', 'chown', f'{os.getuid()}:{os.getgid()}', str(at)], check=True)
                (at / 'cli-data').write_text('CLI persisted data')
                terminal(['unmount', str(source)], root)
                assert terminal(['mount', '--readonly', str(source), str(at)], root) == 1
                assert (at / 'cli-data').read_text() == 'CLI persisted data'
                try:
                    (at / 'forbidden').write_text('must fail')
                    raise AssertionError('readonly CLI mount allowed writes')
                except OSError as error:
                    assert error.errno in (errno.EROFS, errno.EACCES), error
                terminal(['unmount', str(at)], root)
                # Desktop helper receives passphrases on stdin as JSON and preserves its response contract.
                action = dict(action='mount', source=str(source), mountpoint=str(at), volume_type='auto', tag=None, passphrase=PASSWORD, readonly=True)
                must_succeed(helper(action))
                must_succeed(helper(dict(action='unmount', target=str(at), volume_type='auto', tag=None)))
                action['passphrase'] = 'incorrect password'
                assert not helper(action)['ok']
                action['passphrase'] = None
                assert not helper(action)['ok']
                invalid = subprocess.run(helper_command(), input=b'{invalid', capture_output=True, timeout=10)
                assert not json.loads(invalid.stdout)['ok']
                print(f'PASS {ty}: interactive CLI create/mount/readonly/unmount; helper JSON success and failures', flush=True)
            finally:
                # Only our fresh mountpoint/source can be passed to cleanup.
                helper(dict(action='unmount', target=str(at), volume_type='auto', tag=None))
        print(f'PASS: CLI and desktop helper frontend scenarios ({"unprivileged" if UNPRIVILEGED else "privileged"}); mounts survive terminal-session exit', flush=True)

if __name__ == '__main__':
    main()
