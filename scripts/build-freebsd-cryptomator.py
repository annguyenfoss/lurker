#!/usr/bin/env python3
"""Build a native FreeBSD Cryptomator platform candidate, without installing/mounting.

The shared helper's FreeBSD admission remains unchanged. First run all native
header/FFM controls, then rebuild genuine platform/provider modules and probe
their registration. Never import Linux generated layouts or bundle host FUSE.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import signal
import shutil
import subprocess
import sys
import time
import traceback
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'native/folder/cryptomator'
PORT = SOURCE / 'freebsd'


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def native_host():
    return (platform.system() == 'FreeBSD' and platform.machine() == 'amd64'
            and os.getuid() != 0 and os.getuid() == os.geteuid()
            and os.getgid() == os.getegid())


def verified_download(record, cache, offline):
    path = cache / record['url'].rsplit('/', 1)[-1]
    if not path.exists():
        if offline:
            raise RuntimeError('Missing verified source cache input: ' + path.name)
        temporary = path.with_suffix(path.suffix + '.download')
        with urllib.request.urlopen(record['url'], timeout=45) as source:
            with temporary.open('xb') as output:
                shutil.copyfileobj(source, output)
        if digest(temporary) != record['sha256']:
            raise RuntimeError('Downloaded source checksum differs: ' + path.name)
        temporary.replace(path)
    if path.is_symlink() or not path.is_file() or digest(path) != record['sha256']:
        raise RuntimeError('Verified source checksum differs: ' + path.name)
    return path


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def preserve_build_sources(root, manifest, destination):
    """Retain every exact build input, including recipe/POM/lock/notice files."""
    destination.mkdir(parents=True)
    for relative, expected in manifest.items():
        name = Path(relative)
        if name.is_absolute() or '..' in name.parts:
            raise RuntimeError('Unsafe source-kit path')
        source = root / name
        if source.is_symlink() or digest(source) != expected:
            raise RuntimeError('Source changed before source-kit publication: ' + relative)
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        if digest(target) != expected:
            raise RuntimeError('Source-kit copied bytes differ: ' + relative)
    return dict(manifest)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--java-home', type=Path, required=True)
    parser.add_argument('--maven', default='mvn')
    parser.add_argument('--cc', default='cc')
    parser.add_argument('--source-cache', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--offline', action='store_true')
    args = parser.parse_args()
    if not native_host():
        parser.error('Use a native FreeBSD amd64 login user; no root/cross/masquerade build')
    output = args.output.absolute()
    output.mkdir(mode=0o700)
    cache = args.source_cache.absolute()
    cache.mkdir(parents=True, exist_ok=True)
    java = args.java_home.resolve(strict=True)
    for name in ['java', 'javac', 'jar', 'jlink']:
        if not (java / 'bin' / name).is_file():
            parser.error('Native JDK tool missing: ' + name)
    env = dict(os.environ, JAVA_HOME=str(java),
               PATH=str(java / 'bin') + os.pathsep + os.environ.get('PATH', ''),
               PYTHONDONTWRITEBYTECODE='1', MAVEN_OPTS='-Xmx768m')
    for name in ['JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS', 'CLASSPATH']:
        env.pop(name, None)
    source_manifest = {str(p.relative_to(ROOT)): digest(p)
                       for p in SOURCE.rglob('*') if p.is_file()
                       and 'target' not in p.relative_to(SOURCE).parts
                       and '__pycache__' not in p.parts}
    source_manifest['scripts/build-freebsd-cryptomator.py'] = digest(__file__)
    source_manifest['scripts/build-cryptomator.py'] = digest(ROOT / 'scripts/build-cryptomator.py')
    report = {'schema': 1, 'role': 'FreeBSD Cryptomator source candidate only',
              'status': 'running', 'uid': os.getuid(), 'euid': os.geteuid(),
              'pid': os.getpid(), 'uname': list(platform.uname()),
              'source_files': source_manifest, 'commands': [],
              'mount_authorized': False, 'product_admission_changed': False}

    def save():
        (output / 'build-result.json').write_text(json.dumps(report, indent=2) + '\n')

    def run(label, command, *, cwd=output, timeout=600):
        if min(shutil.disk_usage(output).free, shutil.disk_usage(cache).free) < 2 * 1024**3:
            raise RuntimeError('Workvolume free space below 2 GiB')
        log = output / (label + '.log')
        row = {'label': label, 'argv': list(map(str, command)),
               'log': str(log), 'started_epoch': time.time()}
        report['commands'].append(row)
        save()
        with log.open('wb') as stream:
            child = subprocess.Popen(row['argv'], cwd=cwd, env=env, stdout=stream,
                                     stderr=subprocess.STDOUT, start_new_session=True)
            row['pid'] = child.pid
            save()
            deadline = time.monotonic() + timeout
            bound = None
            while child.poll() is None:
                if min(shutil.disk_usage(output).free, shutil.disk_usage(cache).free) < 2 * 1024**3:
                    bound = 'free space below 2 GiB'
                if time.monotonic() > deadline:
                    bound = 'stage deadline exceeded'
                if bound:
                    for sig in [signal.SIGTERM, signal.SIGKILL]:
                        if child.poll() is not None:
                            break
                        os.killpg(child.pid, sig)
                        try:
                            child.wait(timeout=2)
                        except subprocess.TimeoutExpired:
                            pass
                    row.update(bound=bound, worker_still_live=child.poll() is None)
                    break
                time.sleep(0.2)
            row.update(exit=child.poll(), sha256=digest(log), finished_epoch=time.time())
        save()
        if bound or row['exit'] != 0:
            raise RuntimeError('Native build stage failed: ' + label)
        return log

    def compile_module(label, sources, path, destination):
        classes = output / (label + '-classes')
        classes.mkdir()
        units = output / (label + '-sources.txt')
        units.write_text(''.join('"' + str(p) + '"\n' for p in sorted(sources.rglob('*.java'))))
        run(label + '-javac', [java / 'bin/javac', '--release', '25',
            '--module-path', os.pathsep.join(map(str, path)), '-d', classes, '@' + str(units)])
        run(label + '-jar', [java / 'bin/jar', '--create', '--date=2026-11-27T00:00:00Z',
                            '--file', destination, '-C', classes, '.'])

    save()
    try:
        configuration = json.loads((PORT / 'sources.json').read_text())
        inputs = {name: verified_download(record, cache, args.offline)
                  for name, record in configuration['archives'].items()}
        run('native-abi', [sys.executable, PORT / 'probe.py', '--output', output / 'abi',
                          '--java-home', java, '--cc', args.cc])
        report['abi'] = json.loads((output / 'abi/probe-result.json').read_text())
        if report['abi']['status'] != 'passed-native-header-and-ffm':
            raise RuntimeError('Native ABI gate incomplete')
        shared = load_module('cryptomator_shared_builder', ROOT / 'scripts/build-cryptomator.py')
        sys.path.insert(0, str(PORT))
        import prepare
        import generate
        project = output / 'original-helper-source'
        shutil.copytree(SOURCE, project, ignore=shutil.ignore_patterns('target', '__pycache__'))
        maven = [args.maven, '-B', '-q'] + (['--offline'] if args.offline else [])
        run('original-helper-build', [*maven, '-f', project / 'pom.xml', '-DskipTests', 'package'])
        modules = project / 'target/mods'
        shared.verify_dependencies(modules)
        report['locked_runtime_jars'] = {p.name: digest(p) for p in sorted(modules.glob('*.jar'))}
        image = output / 'image'
        image.mkdir()
        shutil.copytree(modules, image / 'mods')
        shutil.copy2(project / 'target/lurker-cryptomator-1.0.0.jar', image / 'mods')
        preserved = image / 'source/base-jars'
        preserved.mkdir(parents=True)
        for name in ['jfuse-api-0.7.3.jar', 'fuse-nio-adapter-5.1.0.jar']:
            shutil.move(image / 'mods' / name, preserved / name)
        derived = image / 'source/derived'
        api, platform_source, provider = [derived / n for n in ['jfuse-api', 'jfuse-freebsd-amd64', 'fuse-nio-adapter']]
        prepare.prepare_api(inputs['jfuse-api'], api)
        prepare.prepare_platform(inputs['jfuse-linux-amd64'], platform_source, PORT)
        profile = json.loads((output / 'abi/abi-profile.json').read_text())
        generate.write_bindings(profile, platform_source)
        generate.write_functions(platform_source)
        prepare.prepare_provider(inputs['fuse-nio-adapter'], provider, PORT)
        path = [image / 'mods', inputs['annotations']]
        compile_module('jfuse-api', api, path, image / 'mods/jfuse-api-0.7.3.jar')
        compile_module('jfuse-freebsd-amd64', platform_source, path,
                       image / 'mods/jfuse-freebsd-amd64-0.7.3.jar')
        compile_module('fuse-nio-freebsd', provider, path, image / 'mods/fuse-nio-adapter-5.1.0.jar')
        run('jlink-native-runtime', [java / 'bin/jlink', *shared.runtime_link_options(java),
            '--add-modules', 'java.base,java.compiler,java.naming,java.xml,jdk.unsupported,jdk.net',
            '--no-header-files', '--no-man-pages', '--strip-debug', '--compress', 'zip-6',
            '--output', image / 'runtime'])
        runtime_java = image / 'runtime/bin/java'
        run('private-runtime-jdk-net', [runtime_java, '--describe-module', 'jdk.net'])
        lifecycle = output / 'lifecycle-test-classes'
        lifecycle.mkdir()
        run('compile-lifecycle-controls', [java / 'bin/javac', '--release', '25',
            '--module-path', os.pathsep.join(map(str, path)), '--patch-module',
            'org.cryptomator.jfuse.freebsd.amd64=' + str(PORT),
            '-d', lifecycle, PORT / 'LifecycleProbe.java', PORT / 'WorkerProtocolProbe.java'])
        worker_evidence = output / 'lifecycle-worker-evidence'
        worker_evidence.mkdir(mode=0o700)
        lifecycle_log = run('native-lifecycle-controls', [runtime_java,
            '-Djava.io.tmpdir=' + str(worker_evidence),
            '--enable-native-access=org.cryptomator.jfuse.freebsd.amd64',
            '--module-path', image / 'mods', '--patch-module',
            'org.cryptomator.jfuse.freebsd.amd64=' + str(lifecycle),
            '-m', 'org.cryptomator.jfuse.freebsd.amd64/org.cryptomator.jfuse.freebsd.amd64.LifecycleProbe',
            runtime_java, image / 'mods', lifecycle, output / 'lifecycle-controls.stderr.log'])
        report['lifecycle_controls'] = json.loads(lifecycle_log.read_text())
        report['lifecycle_diagnostics'] = {'path': str(output / 'lifecycle-controls.stderr.log'),
                                           'sha256': digest(output / 'lifecycle-controls.stderr.log')}
        if (report['lifecycle_controls']['failures']
                or report['lifecycle_controls']['native_mount_or_channel_operations']):
            raise RuntimeError('Memory-only lifecycle controls failed')
        adapter_tests = output / 'adapter-test-classes'
        adapter_tests.mkdir()
        run('compile-adapter-lifecycle-controls', [java / 'bin/javac', '--release', '25',
            '--module-path', os.pathsep.join(map(str, path)), '--patch-module',
            'org.cryptomator.frontend.fuse=' + str(PORT),
            '-d', adapter_tests, PORT / 'AdapterLifecycleProbe.java'])
        adapter_log = run('native-adapter-lifecycle-controls', [runtime_java,
            '--enable-native-access=org.cryptomator.jfuse.freebsd.amd64',
            '--module-path', image / 'mods', '--patch-module',
            'org.cryptomator.frontend.fuse=' + str(adapter_tests),
            '-m', 'org.cryptomator.frontend.fuse/org.cryptomator.frontend.fuse.mount.AdapterLifecycleProbe'])
        report['adapter_lifecycle_controls'] = json.loads(adapter_log.read_text())
        if (report['adapter_lifecycle_controls']['failures']
                or report['adapter_lifecycle_controls']['native_mount_or_channel_operations']):
            raise RuntimeError('Memory-only adapter lifecycle controls failed')
        tests = output / 'provider-test-classes'
        tests.mkdir()
        run('compile-provider-probe', [java / 'bin/javac', '--release', '25',
            '--module-path', image / 'mods', '--add-modules', 'ALL-MODULE-PATH',
            '-d', tests, PORT / 'ProviderProbe.java'])
        log = run('native-provider-probe', [runtime_java,
            '--enable-native-access=org.cryptomator.jfuse.freebsd.amd64',
            '--module-path', image / 'mods', '--add-modules', 'ALL-MODULE-PATH',
            '--add-exports', 'org.cryptomator.frontend.fuse/org.cryptomator.frontend.fuse=ALL-UNNAMED',
            '--add-opens', 'org.cryptomator.frontend.fuse/org.cryptomator.frontend.fuse=ALL-UNNAMED',
            '--add-exports', 'org.cryptomator.jfuse.freebsd.amd64/org.cryptomator.jfuse.freebsd.amd64=ALL-UNNAMED',
            '-cp', tests, 'org.lurker.freebsdjfuse.tests.ProviderProbe', str(output)])
        report['provider'] = json.loads(log.read_text())
        if report['provider']['fsyncdir_registered']:
            raise RuntimeError('Unexpected upstream fsyncdir contract; reassess source')
        notices = image / 'licenses'
        notices.mkdir()
        shutil.copy2(SOURCE / 'LICENSE.txt', notices / 'LURKER-CRYPTOMATOR-AGPL-3.0.txt')
        for name, expected in configuration['license_texts'].items():
            if digest(PORT / name) != expected['sha256']:
                raise RuntimeError('Port license notice checksum differs')
            shutil.copy2(PORT / name, notices / name)
        shutil.copytree(PORT, image / 'source/freebsd-port', ignore=shutil.ignore_patterns('__pycache__'))
        shutil.copytree(SOURCE / 'src', image / 'source/helper')
        upstream = image / 'source/upstream'
        upstream.mkdir()
        for name, archive in inputs.items():
            shutil.copy2(archive, upstream / archive.name)
        shutil.copy2(output / 'abi/abi-profile.json', image / 'source/abi-profile.json')
        source_kit = preserve_build_sources(ROOT, source_manifest, image / 'source/build-inputs')
        manifest = {'schema': 1, 'role': 'native FreeBSD Cryptomator platform candidate',
            'os': platform.system(), 'architecture': platform.machine(),
            'product_admission_changed': False, 'native_fs_fuse_bundled': False,
            'abi_tests': report['abi']['memory_tests'], 'provider': report['provider'],
            'memory_only_lifecycle_controls': report['lifecycle_controls'],
            'memory_only_adapter_controls': report['adapter_lifecycle_controls'],
            'native_unmount_runtime_exercised': False,
            'source_files': source_manifest, 'port_inputs': configuration,
            'complete_build_source_kit': source_kit,
            'original_runtime_dependencies': report['locked_runtime_jars'],
            'derivations': {n: {'original_sha256': report['locked_runtime_jars'][n],
                               'derived_sha256': digest(image / 'mods' / n)}
                            for n in ['jfuse-api-0.7.3.jar', 'fuse-nio-adapter-5.1.0.jar']},
            'crypto_algorithms_changed': False, 'fsyncdir': 'upstream operation not registered; no stub added',
            'qualification_limit': 'Provider registration uses a memory-only constructor fixture; the upstream FileStore factory and all mounted behavior require separate native qualification.',
            'files': {str(p.relative_to(image)): digest(p) for p in sorted(image.rglob('*')) if p.is_file()}}
        (image / 'candidate-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        for path, expected in source_manifest.items():
            if digest(ROOT / path) != expected:
                raise RuntimeError('Build source changed during native execution: ' + path)
        report.update(status='passed-native-source-candidate-no-mount',
                      manifest_sha256=digest(image / 'candidate-manifest.json'),
                      no_mount_or_install_performed=True)
    except BaseException as error:
        report.update(status='failed', error=repr(error), traceback=traceback.format_exc())
        raise
    finally:
        report['finished_epoch'] = time.time()
        save()
    print(json.dumps({'status': report['status'], 'result': str(output / 'build-result.json')}))


if __name__ == '__main__':
    main()
