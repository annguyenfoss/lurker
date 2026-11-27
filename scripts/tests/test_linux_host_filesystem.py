"""Retained host-driver verification never rebuilds or executes an artifact."""
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('host_filesystem', ROOT / 'native/linux-userspace/build-host-filesystem.py')
host = importlib.util.module_from_spec(spec)
spec.loader.exec_module(host)


class RetainedHostDriver(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(); self.addCleanup(temporary.cleanup)
        self.output = Path(temporary.name)
        builder = host.source_builder()
        (self.output / 'lurker-fs').write_bytes(b'unchanged native filesystem helper')
        (self.output / 'build-config').write_bytes(b'CONFIG_BTRFS_FS=y\n')
        inputs = {'source_archive_sha256': builder.SOURCE_ARCHIVE_SHA256['lkl']}
        (self.output / 'verified-source-inputs.json').write_text(json.dumps(inputs))
        self.manifest = {'schema': 1, 'role': 'external-host-filesystem-prerequisite', 'protocol': 5,
            'os': platform.system(), 'architecture': platform.machine(), 'source': list(builder.SOURCES['lkl']),
            'source_archive_sha256': builder.SOURCE_ARCHIVE_SHA256['lkl'], 'stable_patch': builder.LKL_STABLE_PATCH,
            'integration_patch_sha256': hashlib.sha256((ROOT / 'native/linux-userspace/patches/lkl.patch').read_bytes()).hexdigest(),
            'host_io_error_patch_sha256': hashlib.sha256((ROOT / 'native/linux-userspace/patches/host-io-error.patch').read_bytes()).hexdigest(), 'sticky_io_errors': True,
            'config_sha256': hashlib.sha256((self.output / 'build-config').read_bytes()).hexdigest(),
            'source_inputs': inputs, 'files': {'lurker-fs': {'sha256': hashlib.sha256((self.output / 'lurker-fs').read_bytes()).hexdigest()}}}
        self.write_manifest()

    def write_manifest(self):
        (self.output / 'host-filesystem-source.json').write_text(json.dumps(self.manifest))

    def test_verify_is_static_and_binds_config_and_executable(self):
        with mock.patch.object(host.subprocess, 'Popen', side_effect=AssertionError('must not execute')):
            self.assertEqual(host.verify_output(self.output), self.manifest)
        (self.output / 'lurker-fs').write_bytes(b'changed')
        with self.assertRaisesRegex(RuntimeError, 'checksum differs'):
            host.verify_output(self.output)

    def test_wrong_protocol_config_and_unsafe_inventory_are_rejected(self):
        self.manifest['protocol'] = 3; self.write_manifest()
        with self.assertRaisesRegex(RuntimeError, 'identity differs'):
            host.verify_output(self.output)
        self.manifest['protocol'] = 5; self.write_manifest()
        (self.output / 'build-config').write_bytes(b'changed config')
        with self.assertRaisesRegex(RuntimeError, 'config/source verification differs'):
            host.verify_output(self.output)
        self.manifest['files'] = {'../escape': {'sha256': '0' * 64}}; self.write_manifest()
        with self.assertRaisesRegex(RuntimeError, 'Unsafe'):
            host.verify_output(self.output)

    def test_kernel_cache_upgrade_rejects_unknown_object_before_patching(self):
        source = self.output / 'lkl-6.12.118'
        (source / 'tools/lkl/lib').mkdir(parents=True)
        (source / 'tools/lkl/lib/lkl.o').write_bytes(b'unqualified object')
        (source / '.config').write_bytes(b'CONFIG_BTRFS_FS=y\n')
        proof = self.output / 'proof.json'
        proof.write_text(json.dumps({'schema': 1, 'architecture': platform.machine(),
            'kernel_object_sha256': '0' * 64, 'config_sha256': '0' * 64}))
        builder = host.source_builder()
        with mock.patch.object(builder, 'unpack_lkl_stable_patch'), mock.patch.object(host.subprocess, 'run', side_effect=AssertionError('must not patch')):
            with self.assertRaisesRegex(RuntimeError, 'kernel object/config provenance differs'):
                host.prepare_host_source(builder, self.output, True, proof)
        self.assertEqual((source / 'tools/lkl/lib/lkl.o').read_bytes(), b'unqualified object')

    def test_cache_safety_delta_cannot_change_kernel_configuration(self):
        root = self.output / 'recipe'
        patch = root / 'native/linux-userspace/patches/host-io-error.patch'
        patch.parent.mkdir(parents=True)
        patch.write_text('--- a/arch/lkl/configs/defconfig\n+++ b/arch/lkl/configs/defconfig\n')
        with mock.patch.object(host, 'ROOT', root):
            with self.assertRaisesRegex(RuntimeError, 'changes kernel/configuration'):
                host.prepare_host_source(mock.Mock(), self.output, True, None)


if __name__ == '__main__':
    unittest.main()
