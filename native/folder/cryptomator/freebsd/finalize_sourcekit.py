#!/usr/bin/env python3
"""Packaging-only derivation of a passed candidate; never run Java or mount.

Keep the original image and receipts intact. Copy every verified original build
input into a new image and retain this independent packaging recipe separately.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import time


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--original-source', required=True, type=Path)
    parser.add_argument('--original-build', required=True, type=Path)
    parser.add_argument('--original-image', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--producer', required=True, type=Path)
    args = parser.parse_args()
    if (platform.system() != 'FreeBSD' or platform.machine() != 'amd64'
            or os.getuid() == 0 or os.getuid() != os.geteuid()):
        parser.error('Use the native FreeBSD login user')
    build = json.loads(args.original_build.read_text())
    original_manifest = args.original_image / 'candidate-manifest.json'
    manifest = json.loads(original_manifest.read_text())
    if (build['status'] != 'passed-native-source-candidate-no-mount'
            or sha(original_manifest) != build['manifest_sha256']
            or manifest['os'] != 'FreeBSD' or manifest['architecture'] != 'amd64'
            or manifest['native_fs_fuse_bundled'] is not False):
        raise RuntimeError('Original candidate identity/gate differs')
    for relative, expected in manifest['files'].items():
        path = Path(relative)
        if path.is_absolute() or '..' in path.parts:
            raise RuntimeError('Unsafe original manifest path')
        if sha(args.original_image / path) != expected:
            raise RuntimeError('Original image bytes differ: ' + relative)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if shutil.disk_usage(args.output.parent).free < 2 * 1024**3 + sum(p.stat().st_size for p in args.original_image.rglob('*') if p.is_file()):
        raise RuntimeError('Insufficient native source-kit workspace')
    shutil.copytree(args.original_image, args.output)
    spec = importlib.util.spec_from_file_location('new_sourcekit_producer', args.producer)
    producer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(producer)
    source_kit = producer.preserve_build_sources(
        args.original_source, build['source_files'], args.output / 'source/build-inputs')
    derived = args.output / 'source/packaging-derivation'
    derived.mkdir()
    shutil.copy2(__file__, derived / 'finalize_sourcekit.py')
    shutil.copy2(args.producer, derived / 'build-freebsd-cryptomator.py')
    manifest['complete_build_source_kit'] = source_kit
    manifest['packaging_derivation'] = {
        'original_manifest_sha256': sha(original_manifest),
        'original_build_result_sha256': sha(args.original_build),
        'producer_sha256': sha(args.producer), 'finalizer_sha256': sha(__file__),
        'operation': 'Complete source-kit copy only; all original compiled/runtime bytes unchanged',
        'no_compile_runtime_or_mount_operation': True,
    }
    for relative, expected in json.loads(original_manifest.read_text())['files'].items():
        if sha(args.output / relative) != expected:
            raise RuntimeError('Source-kit derivation changed original compiled/runtime bytes')
    manifest['files'] = {str(p.relative_to(args.output)): sha(p)
                         for p in sorted(args.output.rglob('*')) if p.is_file()
                         and p != args.output / 'candidate-manifest.json'}
    (args.output / 'candidate-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    receipt = {'status': 'passed-packaging-only-sourcekit-derivation',
               'uid': os.getuid(), 'euid': os.geteuid(), 'epoch': time.time(),
               'original_image': str(args.original_image), 'output': str(args.output),
               'manifest_sha256': sha(args.output / 'candidate-manifest.json'),
               'source_inputs_retained': len(source_kit),
               **manifest['packaging_derivation']}
    (args.output.parent / (args.output.name + '-sourcekit-result.json')).write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))


if __name__ == '__main__':
    main()
