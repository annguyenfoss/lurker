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
        (self.source / 'version.h').write_text('old-version\n')
        self.archive = self.cache / 'example-fixed-revision.tar.gz'
        with tarfile.open(self.archive, 'w:gz') as archive:
            member = tarfile.TarInfo('example-fixed/example.c')
            member.size = len(b'upstream\n')
            archive.addfile(member, io.BytesIO(b'upstream\n'))
            member = tarfile.TarInfo('example-fixed/version.h')
            member.size = len(b'old-version\n')
            archive.addfile(member, io.BytesIO(b'old-version\n'))
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


    def test_legacy_old_tree_plus_new_archive_cannot_be_relabeled(self):
        self.prepare()
        (self.source / '.lurker-source-inputs.json').unlink()
        self.builder.SOURCES['example'] = ('example/upstream', 'new-revision')
        self.archive = self.cache / 'example-new-revision.tar.gz'
        with tarfile.open(self.archive, 'w:gz') as archive:
            for name, contents in [('example.c', b'upstream\n'), ('version.h', b'new-version\n')]:
                entry = tarfile.TarInfo('new-source/' + name)
                entry.size = len(contents)
                archive.addfile(entry, io.BytesIO(contents))
        self.builder.SOURCE_ARCHIVE_SHA256['example'] = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        with self.assertRaisesRegex(RuntimeError, 'does not match pinned source inputs.*version.h'):
            self.prepare()
        self.assertFalse((self.source / '.lurker-source-inputs.json').exists())

    def test_bound_cache_rejects_a_changed_revision_before_adoption(self):
        self.prepare()
        self.builder.SOURCES['example'] = ('example/upstream', 'new-revision')
        new = self.cache / 'example-new-revision.tar.gz'
        new.write_bytes(self.archive.read_bytes())
        with self.assertRaisesRegex(RuntimeError, 'Source cache inputs changed'):
            self.prepare()

    def test_binding_cannot_hide_unpatched_or_extra_patched_file_changes(self):
        self.prepare()
        for name in ['version.h', 'example.c']:
            original = (self.source / name).read_bytes()
            (self.source / name).write_bytes(original + b'outside-patch-tampering\n')
            with self.assertRaisesRegex(RuntimeError, 'does not match pinned source inputs'):
                self.prepare()
            (self.source / name).write_bytes(original)
        self.prepare()

    def test_tar_filter_symlink_normalization_preserves_source_identity(self):
        with tarfile.open(self.cache / 'linked.tar.gz', 'w:gz') as archive:
            for name, contents in [('example.c', b'upstream\n'), ('version.h', b'old-version\n')]:
                entry = tarfile.TarInfo('source/' + name)
                entry.size = len(contents)
                archive.addfile(entry, io.BytesIO(contents))
            entry = tarfile.TarInfo('source/source-alias')
            entry.type = tarfile.SYMTYPE
            entry.linkname = './example.c'
            archive.addfile(entry)
        (self.cache / 'linked.tar.gz').replace(self.archive)
        self.builder.SOURCE_ARCHIVE_SHA256['example'] = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        (self.source / 'source-alias').symlink_to('example.c')
        self.prepare()

    def test_legacy_deleted_upstream_source_is_not_mistaken_for_generated_output(self):
        self.prepare()
        (self.source / '.lurker-source-inputs.json').unlink()
        with tarfile.open(self.cache / 'new.tar.gz', 'w:gz') as archive:
            entry = tarfile.TarInfo('new-source/example.c')
            entry.size = len(b'upstream\n')
            archive.addfile(entry, io.BytesIO(b'upstream\n'))
        (self.cache / 'new.tar.gz').replace(self.archive)
        self.builder.SOURCE_ARCHIVE_SHA256['example'] = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        with self.assertRaisesRegex(RuntimeError, 'version.h.*unexpected source input'):
            self.prepare()

    def test_legitimate_generated_outputs_are_not_source_inputs(self):
        self.prepare()
        self.builder.GENERATED_INPUTS['example'] = {'build-output/config.h'}
        (self.source / 'example.o').write_bytes(b'generated object code')
        (self.source / 'build-output').mkdir()
        (self.source / 'build-output/config.h').write_text('generated')
        self.prepare()
        import json
        binding = json.loads((self.source / '.lurker-source-inputs.json').read_text())
        self.assertEqual(binding['revision'], 'fixed-revision')
        self.assertEqual(binding['verified_source_files'], 2)


if __name__ == '__main__':
    unittest.main()
