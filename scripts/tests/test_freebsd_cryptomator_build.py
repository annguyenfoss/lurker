"""Run on the FreeBSD owner host; no mount, install or encrypted fixture."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
PORT = ROOT / 'native/folder/cryptomator/freebsd'
sys.path.insert(0, str(PORT))
import generate
import prepare

spec = importlib.util.spec_from_file_location('freebsd_cryptomator_builder', ROOT / 'scripts/build-freebsd-cryptomator.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class NativeBuilderGuards(unittest.TestCase):
    def test_native_host_rejects_root_and_foreign_system(self):
        with patch.object(builder.platform, 'system', return_value='Linux'):
            self.assertFalse(builder.native_host())
        with patch.object(builder.os, 'getuid', return_value=0):
            self.assertFalse(builder.native_host())

    def test_offline_cache_rejects_unverified_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache = Path(temporary)
            (cache / 'source.jar').write_bytes(b'changed')
            record = {'url': 'https://example.invalid/source.jar',
                      'sha256': hashlib.sha256(b'original').hexdigest()}
            with self.assertRaisesRegex(RuntimeError, 'checksum differs'):
                builder.verified_download(record, cache, True)

    def test_verified_cache_never_downloads_offline(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache = Path(temporary)
            source = cache / 'source.jar'
            source.write_bytes(b'original')
            record = {'url': 'https://example.invalid/source.jar',
                      'sha256': hashlib.sha256(b'original').hexdigest()}
            with patch.object(builder.urllib.request, 'urlopen', side_effect=AssertionError('network forbidden')):
                self.assertEqual(builder.verified_download(record, cache, True), source)

    def test_source_archive_cannot_escape_owned_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'sources.jar'
            with zipfile.ZipFile(archive, 'w') as stream:
                stream.writestr('../escaped.java', 'invalid')
            with self.assertRaisesRegex(ValueError, 'Unsafe source archive'):
                prepare.java_sources(archive, root / 'output')
            self.assertFalse((root / 'escaped.java').exists())

    def test_pinned_source_replacement_rejects_duplicate_anchor(self):
        with self.assertRaisesRegex(ValueError, 'anchor differs'):
            prepare.replace_once('target target', 'target', 'changed')

    def test_native_profile_rejects_foreign_or_incomplete_schema(self):
        for profile in [{}, {'schema': 1, 'os': 'Linux', 'architecture': 'amd64', 'uid': 1300, 'euid': 1300}]:
            with self.assertRaises(ValueError):
                generate.validate_profile(profile)

    def test_native_callback_prototype_inventory_is_complete(self):
        self.assertEqual(set(generate.C_ARGS), set(generate.CALLBACKS))
        for name, (_, signature) in generate.CALLBACKS.items():
            self.assertEqual(len(signature), len(generate.C_ARGS[name]), name)

    def test_complete_source_kit_retains_recipe_and_exact_lock_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            (root/'recipe.py').write_bytes(b'build recipe\n')
            (root/'lock.json').write_bytes(b'{"pinned":true}\n')
            manifest={n:builder.digest(root/n) for n in ['recipe.py','lock.json']}
            result=builder.preserve_build_sources(root,manifest,root/'published')
            self.assertEqual(result,manifest)
            self.assertEqual((root/'published/lock.json').read_bytes(),b'{"pinned":true}\n')
            self.assertEqual(builder.digest(root/'published/recipe.py'),manifest['recipe.py'])

    def test_source_kit_refuses_changed_input_and_escape(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            (root/'source').write_bytes(b'changed')
            with self.assertRaisesRegex(RuntimeError,'Source changed'):
                builder.preserve_build_sources(root,{'source':'0'*64},root/'bad')
            with self.assertRaisesRegex(RuntimeError,'Unsafe source-kit'):
                builder.preserve_build_sources(root,{'../source':'0'*64},root/'escape')


class RetainedNativeInputs(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cache = Path(os.environ['LURKER_FREEBSD_CRYPTO_SOURCE_CACHE'])
        cls.profile = json.loads(Path(os.environ['LURKER_FREEBSD_ABI_PROFILE']).read_text())
        cls.inputs = json.loads((PORT / 'sources.json').read_text())

    def test_native_profile_preserves_actual_freebsd_layout(self):
        generate.validate_profile(self.profile)
        self.assertEqual(self.profile['structs']['stat']['size'], 224)
        self.assertEqual(self.profile['structs']['stat']['fields']['st_size']['offset'], 112)
        broken = copy.deepcopy(self.profile)
        broken['structs']['stat']['fields']['st_size']['offset'] = 224
        with self.assertRaisesRegex(ValueError, 'outside'):
            generate.validate_profile(broken)

    def test_native_profile_rejects_linux_mode_width(self):
        broken = copy.deepcopy(self.profile)
        broken['types']['mode_t'] = 4
        with self.assertRaisesRegex(ValueError, 'widths'):
            generate.validate_profile(broken)

    def test_source_transform_has_genuine_platform_and_no_linux_abi(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            def archive(name):
                return builder.verified_download(self.inputs['archives'][name], self.cache, True)
            prepare.prepare_api(archive('jfuse-api'), output / 'api')
            prepare.prepare_platform(archive('jfuse-linux-amd64'), output / 'platform', PORT)
            enum = (output / 'api/org/cryptomator/jfuse/api/platforms/OperatingSystem.java').read_text()
            self.assertIn('OS_NAME.equals("freebsd")', enum)
            self.assertIn('return FREEBSD;', enum)
            api_module = (output / 'api/module-info.java').read_text()
            self.assertEqual(api_module.count(generate.PACKAGE), 2)
            self.assertFalse(any('extr' in p.parts for p in (output / 'platform').rglob('*.java')))
            sources = '\n'.join(p.read_text() for p in (output / 'platform').rglob('*.java'))
            self.assertIn('OperatingSystem.FREEBSD', sources)
            self.assertNotIn('OperatingSystem.LINUX', sources)
            self.assertNotIn('org.cryptomator.jfuse.linux.amd64', sources)
            self.assertIn('fuse_config.direct_io(cfg, 1)', sources)
            self.assertIn('Short.toUnsignedInt(mode)', sources)

    def test_genuine_provider_is_registered_in_rebuilt_descriptor(self):
        with tempfile.TemporaryDirectory() as temporary:
            archive = builder.verified_download(self.inputs['archives']['fuse-nio-adapter'], self.cache, True)
            prepare.prepare_provider(archive, Path(temporary) / 'provider', PORT)
            module = (Path(temporary) / 'provider/module-info.java').read_text()
            self.assertIn('provides MountService with org.cryptomator.frontend.fuse.mount.FreeBsdFuseMountProvider,', module)
            self.assertIn('requires org.cryptomator.jfuse.freebsd.amd64;', module)


if __name__ == '__main__':
    unittest.main()
