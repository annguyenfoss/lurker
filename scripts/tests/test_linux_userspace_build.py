"""Regression tests for native-source preparation; no download/build is needed."""
import importlib.util
import hashlib
import io
import tarfile
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'build-linux-userspace.py'


class NativeSourcePreparation(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location('userspace_builder', SCRIPT)
        self.builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.builder)
        self.temporary = tempfile.TemporaryDirectory(prefix='lurker-build-test-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.builder.ROOT = self.root
        self.builder.SOURCES = {'example': ('example/upstream', 'fixed-revision')}
        patches = self.root / 'native/linux-userspace/patches'
        patches.mkdir(parents=True)
        self.patch = patches / 'example.patch'
        self.patch.write_text('--- a/example.c\n+++ b/example.c\n@@ -1 +1 @@\n-upstream\n+patched\n')
        self.cache = self.root / 'cache'
        self.source = self.cache / 'example'
        self.source.mkdir(parents=True)
        (self.source / 'example.c').write_text('upstream\n')
        self.archive = self.cache / 'example-fixed-revision.tar.gz'
        with tarfile.open(self.archive, 'w:gz') as archive:
            member = tarfile.TarInfo('example-fixed/example.c')
            member.size = len(b'upstream\n')
            archive.addfile(member, io.BytesIO(b'upstream\n'))
        self.builder.SOURCE_ARCHIVE_SHA256 = {'example': hashlib.sha256(self.archive.read_bytes()).hexdigest()}

    def prepare(self):
        return self.builder.prepare('example', self.cache, offline=True)

    def test_pristine_sources_are_patched_and_stamped_cache_is_reusable(self):
        self.assertEqual(self.prepare(), self.source.resolve())
        self.assertEqual((self.source / 'example.c').read_text(), 'patched\n')
        self.assertEqual(self.prepare(), self.source.resolve())
        self.assertEqual((self.source / 'example.c').read_text(), 'patched\n')

    def test_a_stamp_cannot_hide_missing_source_changes(self):
        self.prepare()
        (self.source / 'example.c').write_text('upstream\n')
        with self.assertRaisesRegex(RuntimeError, 'do not match their patch'):
            self.prepare()

    def test_symlinked_package_sources_return_the_canonical_build_path(self):
        actual = self.root / 'actual-source'
        self.source.rename(actual)
        self.source.symlink_to(actual, target_is_directory=True)
        self.assertEqual(self.prepare(), actual)
        self.assertEqual((actual / 'example.c').read_text(), 'patched\n')

    def test_corrupt_archive_is_rejected_even_when_source_cache_exists(self):
        self.archive.write_bytes(b'not the pinned source archive')
        with self.assertRaisesRegex(RuntimeError, 'checksum mismatch'):
            self.prepare()

    def test_unpack_uses_a_verified_archive(self):
        import shutil
        shutil.rmtree(self.source)
        self.assertEqual(self.prepare(), self.source.resolve())
        self.assertEqual((self.source / 'example.c').read_text(), 'patched\n')

    def test_changed_patches_refuse_reusing_old_generated_sources(self):
        self.prepare()
        self.patch.write_text(self.patch.read_text().replace('+patched', '+new-patch'))
        with self.assertRaisesRegex(RuntimeError, 'Patch changed'):
            self.prepare()


if __name__ == '__main__':
    unittest.main()
