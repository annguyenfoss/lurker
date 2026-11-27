#!/usr/bin/env python3
"""Native, memory-only first gate. This command cannot mount a filesystem."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import time
import traceback

from generate import (write_c_include, validate_profile, write_bindings,
                      write_memory_test, write_functions)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cc', default='cc')
    parser.add_argument('--java-home', type=Path,
                        help='Also compile/run native-memory FFM round trips; no mounts')
    args = parser.parse_args()
    if (platform.system() != 'FreeBSD' or platform.machine() != 'amd64'
            or not os.getuid() or os.geteuid() != os.getuid()):
        raise SystemExit('This probe requires a genuine native FreeBSD amd64 login user')
    source = Path(__file__).resolve().parent
    output = args.output.absolute()
    output.mkdir(mode=0o700)
    sha = lambda p: hashlib.file_digest(open(p, 'rb'), 'sha256').hexdigest()
    report = {'schema': 1, 'status': 'running', 'uid': os.getuid(),
              'euid': os.geteuid(), 'uname': list(platform.uname()),
              'pid': os.getpid(), 'controller_sha256': sha(__file__),
              'source_files': {p.name: sha(p) for p in source.iterdir() if p.is_file()},
              'mount_authorized': False, 'commands': []}

    def save():
        (output / 'probe-result.json').write_text(json.dumps(report, indent=2) + '\n')

    def run(label, argv):
        if shutil.disk_usage(output).free < 2 * 1024**3:
            raise RuntimeError('Native workvolume free space below 2 GiB')
        log = output / (label + '.log')
        with log.open('wb') as stream:
            result = subprocess.run(argv, cwd=output, stdout=stream,
                                    stderr=subprocess.STDOUT, timeout=90)
        row = {'label': label, 'argv': argv, 'exit': result.returncode,
               'log': str(log), 'sha256': sha(log)}
        report['commands'].append(row)
        save()
        if result.returncode:
            raise RuntimeError('Native probe stage failed: ' + label)
        return log

    save()
    try:
        shutil.copy2(source / 'abi-probe.c', output / 'abi-probe.c')
        write_c_include(output / 'abi-fields.inc')
        includes = shlex.split(subprocess.check_output(
            ['pkg-config', '--cflags', 'fuse3'], text=True))
        libs = shlex.split(subprocess.check_output(
            ['pkg-config', '--libs', 'fuse3'], text=True))
        report['host_fuse_version'] = subprocess.check_output(
            ['pkg-config', '--modversion', 'fuse3'], text=True).strip()
        common = [args.cc, '-std=gnu11', '-Wall', '-Wextra', '-Werror',
                  '-Wno-unused-function', *includes, 'abi-probe.c']
        run('compile-probe', [*common, '-o', 'abi-probe', *libs])
        profile_log = run('native-profile', [str(output / 'abi-probe')])
        profile = json.loads(profile_log.read_text())
        validate_profile(profile)
        (output / 'abi-profile.json').write_text(json.dumps(profile, indent=2) + '\n')
        run('compile-memory-control', [*common, '-DABI_SHARED', '-shared', '-fPIC',
                                      '-o', 'libjfuse-abi-test.so', *libs])
        write_bindings(profile, output / 'generated')
        write_functions(output / 'generated')
        write_memory_test(profile, output / 'generated')
        if args.java_home:
            java = args.java_home.resolve(strict=True)
            sources = sorted((output / 'generated').rglob('*.java'))
            units = output / 'java-sources.txt'
            units.write_text(''.join('"' + str(p) + '"\n' for p in sources))
            classes = output / 'classes'
            classes.mkdir()
            run('compile-ffm-memory-tests', [str(java / 'bin/javac'), '--release',
                '25', '-d', str(classes), '@' + str(units)])
            result = run('native-ffm-memory-tests', [str(java / 'bin/java'),
                '--enable-native-access=ALL-UNNAMED', '-cp', str(classes),
                'org.lurker.freebsdjfuse.tests.NativeMemoryTest',
                str(output / 'libjfuse-abi-test.so')])
            report['memory_tests'] = json.loads(result.read_text())
            if report['memory_tests']['failures']:
                raise RuntimeError('Native FFM memory controls failed')
        report['generated_files'] = {str(p.relative_to(output)): sha(p)
                                     for p in output.rglob('*') if p.is_file()
                                     and p.name != 'probe-result.json'}
        report.update(status='passed-native-header-and-ffm' if args.java_home
                      else 'passed-native-header-probe',
                      abi_profile_sha256=sha(output / 'abi-profile.json'))
    except BaseException as error:
        report.update(status='failed', error=repr(error), traceback=traceback.format_exc())
        raise
    finally:
        report['finished_epoch'] = time.time()
        save()
    print(json.dumps({'status': report['status'], 'result': str(output / 'probe-result.json')}))


if __name__ == '__main__':
    main()
