#!/usr/bin/env python3
"""Build the standalone protocol-5 host filesystem prerequisite; never install it.

This administrator-installed facility is NOT an application package artifact.
Build as a normal user, then review/install its output as an administrator.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import time
import tempfile
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def source_builder():
    spec = importlib.util.spec_from_file_location('linux_sources', ROOT / 'scripts/build-linux-userspace.py')
    builder = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(builder)
    return builder


def prepare_host_source(builder, cache, offline, kernel_provenance):
    base_patch = ROOT / 'native/linux-userspace/patches/lkl.patch'
    safety_patch = ROOT / 'native/linux-userspace/patches/host-io-error.patch'
    # A cache upgrade is host-only: the proven kernel object/config remain exact.
    allowed = {'tools/lkl/include/lkl.h', 'tools/lkl/lib/virtio_blk.c', 'tools/lkl/lklfuse.c'}
    changed = {line[6:].split('\t')[0] for line in safety_patch.read_text().splitlines()
               if line.startswith(('--- a/', '+++ b/'))}
    if not changed or not changed <= allowed:
        raise RuntimeError('Host safety patch changes kernel/configuration inputs')
    with tempfile.TemporaryDirectory(prefix='lurker-host-stable-', dir=cache) as temporary:
        stable = builder.unpack_lkl_stable_patch(cache, offline, Path(temporary) / 'stable.patch')
        source = cache / ('lkl-' + builder.LKL_STABLE_PATCH['version'])
        proof = json.loads(kernel_provenance.read_text()) if kernel_provenance else None
        if proof:
            kernel = source / 'tools/lkl/lib/lkl.o'
            if (proof.get('schema') != 1 or proof.get('architecture') != platform.machine()
                    or hashlib.sha256(kernel.read_bytes()).hexdigest() != proof['kernel_object_sha256']
                    or hashlib.sha256((source / '.config').read_bytes()).hexdigest() != proof['config_sha256']):
                raise RuntimeError('Qualified kernel object/config provenance differs')
            binding = source / '.lurker-source-inputs.json'
            old = json.loads(binding.read_text())
            if old.get('patch_sha256') == hashlib.sha256(base_patch.read_bytes()).hexdigest():
                builder.verify_source_cache('lkl', source, cache / ('lkl-' + builder.SOURCES['lkl'][1] + '.tar.gz'), (stable, base_patch))
                if json.loads(binding.read_text()) != proof['base_source_inputs']:
                    raise RuntimeError('Qualified base source verification differs')
                subprocess.run(['patch', '-p1', '--forward', '--force', '--batch', '--fuzz=0', '-i', str(safety_patch)], cwd=source, check=True)
                saved = cache / 'protocol4-base-source-inputs.json'
                if saved.exists():
                    raise RuntimeError('Base cache upgrade receipt already exists; inspect checkpoint')
                binding.rename(saved)
                # Independently derive every new input from archive+all patches;
                # never adopt the new stamp merely because patch returned zero.
                builder.verify_source_cache('lkl', source, cache / ('lkl-' + builder.SOURCES['lkl'][1] + '.tar.gz'), (stable, base_patch, safety_patch))
                stamp = hashlib.sha256((builder.LKL_STABLE_PATCH['sha256'] + ':' + hashlib.sha256(safety_patch.read_bytes()).hexdigest()).encode()).hexdigest()
                (source / '.lurker-patch-sha256').write_text(stamp + '\n')
        source = builder.prepare_source('lkl', cache, offline, (stable, base_patch, safety_patch))
        return source, proof


def verify_output(output):
    """Check retained output/input records without rebuilding or executing it."""
    builder = source_builder()
    manifest = json.loads((output / 'host-filesystem-source.json').read_text())
    patch = hashlib.sha256((ROOT / 'native/linux-userspace/patches/lkl.patch').read_bytes()).hexdigest()
    if (manifest.get('schema') != 1 or manifest.get('role') != 'external-host-filesystem-prerequisite'
            or manifest.get('protocol') != 5 or manifest.get('os') != platform.system()
            or manifest.get('architecture') != platform.machine()
            or manifest.get('source') != list(builder.SOURCES['lkl'])
            or manifest.get('source_archive_sha256') != builder.SOURCE_ARCHIVE_SHA256['lkl']
            or manifest.get('stable_patch') != builder.LKL_STABLE_PATCH
            or manifest.get('integration_patch_sha256') != patch
            or manifest.get('host_io_error_patch_sha256') != hashlib.sha256((ROOT / 'native/linux-userspace/patches/host-io-error.patch').read_bytes()).hexdigest()
            or manifest.get('sticky_io_errors') is not True):
        raise RuntimeError('Retained host filesystem source/protocol identity differs')
    for name, record in manifest['files'].items():
        relative = Path(name)
        if relative.is_absolute() or '..' in relative.parts or (output / relative).is_symlink():
            raise RuntimeError('Unsafe retained host filesystem inventory')
        if hashlib.sha256((output / relative).read_bytes()).hexdigest() != record['sha256']:
            raise RuntimeError('Retained host filesystem checksum differs: ' + name)
    if ('lurker-fs' not in manifest['files']
            or hashlib.sha256((output / 'build-config').read_bytes()).hexdigest() != manifest['config_sha256']
            or json.loads((output / 'verified-source-inputs.json').read_text()) != manifest['source_inputs']):
        raise RuntimeError('Retained host filesystem config/source verification differs')
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-dir', type=Path)
    parser.add_argument('--output-dir', type=Path)
    parser.add_argument('--verify-output', type=Path, help='Verify retained output/config/source records without rebuilding or executing')
    parser.add_argument('--jobs', type=int, default=2)
    parser.add_argument('--offline', action='store_true')
    parser.add_argument('--kernel-provenance', type=Path, help='Optional qualified original kernel-object/config/source receipt for a host-only cache upgrade')
    args = parser.parse_args()
    if platform.system() != 'Linux' or os.geteuid() == 0 or args.jobs < 1:
        parser.error('Build on Linux as a normal user with positive --jobs')
    if args.verify_output:
        verify_output(args.verify_output.resolve())
        print(json.dumps({'status': 'verified-retained-output', 'rebuilt': False, 'executed': False}))
        return
    if args.source_dir is None or args.output_dir is None:
        parser.error('--source-dir and --output-dir are required for a build')
    output = args.output_dir.absolute()
    cache = args.source_dir.resolve()
    if not cache.is_dir():
        parser.error('Create the source-cache directory first')
    if shutil.disk_usage(cache).free < 2 * 1024**3:
        parser.error('Keep at least 2 GiB free before preparing host sources')
    if output.exists():
        parser.error('Use a new output directory; existing prerequisites are never overwritten')
    for tool in ('make', 'gcc', 'g++', 'flex', 'bison', 'bc', 'perl', 'pkg-config', 'strip', 'readelf'):
        if not shutil.which(tool):
            parser.error('Required host build tool missing: ' + tool)
    subprocess.run(['pkg-config', '--exists', 'fuse3'], check=True)
    builder = source_builder()
    source, kernel_proof = prepare_host_source(builder, cache, args.offline, args.kernel_provenance)
    if shutil.disk_usage(cache).free < 2 * 1024**3:
        parser.error('Prepared sources retained; recover the 2 GiB free-space floor before compilation')
    # Same reviewed fixed profile as the original protocol-4 helper. No UFS1
    # configuration, host-kernel build, virtualization or network transport.
    if not kernel_proof:
        (source / '.config').unlink(missing_ok=True)
    kernel = source / 'tools/lkl/lib/lkl.o'
    command = ['make', '-C', str(source / 'tools/lkl')]
    if kernel_proof:
        command += ['-o', str(kernel)]
    command += [f'-j{args.jobs}', str(source / 'tools/lkl/lklfuse')]
    child = subprocess.Popen(command, start_new_session=True)
    try:
        while child.poll() is None:
            if shutil.disk_usage(cache).free < 2 * 1024**3:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
                raise RuntimeError('Build checkpoint retained: recover the 2 GiB free-space floor before resuming')
            time.sleep(1)
        if child.returncode:
            raise subprocess.CalledProcessError(child.returncode, command)
    finally:
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
    if kernel_proof and (hashlib.sha256(kernel.read_bytes()).hexdigest() != kernel_proof['kernel_object_sha256']
            or hashlib.sha256((source / '.config').read_bytes()).hexdigest() != kernel_proof['config_sha256']):
        raise RuntimeError('Host-only rebuild changed the qualified kernel object/configuration')
    output.mkdir(mode=0o755, parents=True)
    binary = output / 'lurker-fs'
    shutil.copyfile(source / 'tools/lkl/lklfuse', binary)
    binary.chmod(0o755)
    subprocess.run(['strip', '--strip-unneeded', str(binary)], check=True)
    version = subprocess.check_output([str(binary), '--lurker-userspace-version'], text=True, timeout=5).strip()
    if version != 'lurker-userspace-5':
        raise RuntimeError('Host filesystem helper protocol differs')
    shutil.copyfile(source / '.config', output / 'build-config')
    shutil.copyfile(source / '.lurker-source-inputs.json', output / 'verified-source-inputs.json')
    licenses = output / 'licenses'
    licenses.mkdir()
    shutil.copyfile(source / 'COPYING', licenses / 'Linux.txt')
    shutil.copytree(source / 'LICENSES', licenses / 'Linux')
    patch = ROOT / 'native/linux-userspace/patches/lkl.patch'
    manifest = {'schema': 1, 'role': 'external-host-filesystem-prerequisite',
        'protocol': 5, 'os': platform.system(), 'architecture': platform.machine(),
        'source': builder.SOURCES['lkl'], 'source_archive_sha256': builder.SOURCE_ARCHIVE_SHA256['lkl'],
        'stable_patch': builder.LKL_STABLE_PATCH,
        'integration_patch_sha256': hashlib.sha256(patch.read_bytes()).hexdigest(),
        'host_io_error_patch_sha256': hashlib.sha256((ROOT / 'native/linux-userspace/patches/host-io-error.patch').read_bytes()).hexdigest(),
        'sticky_io_errors': True,
        'config_sha256': hashlib.sha256((source / '.config').read_bytes()).hexdigest(),
        'source_inputs': json.loads((source / '.lurker-source-inputs.json').read_text()),
        'host_fuse': subprocess.check_output(['pkg-config', '--modversion', 'fuse3'], text=True).strip(),
        'elf_dynamic': subprocess.check_output(['readelf', '-d', str(binary)], text=True),
        'kernel_object_reuse': kernel_proof, 'build_command': command,
        'compiler': subprocess.check_output(['gcc', '--version'], text=True).splitlines()[0],
        'files': {str(path.relative_to(output)): {'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
                  for path in sorted(output.rglob('*')) if path.is_file()}}
    (output / 'host-filesystem-source.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    print(json.dumps({'output': str(output), 'binary_sha256': manifest['files']['lurker-fs']['sha256'],
        'installed': False, 'qualification': 'native lifecycle and host-backing fsync/error controls remain required'}))


if __name__ == '__main__':
    main()
