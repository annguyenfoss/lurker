"""Verify native wrappers reject modified and unsafe stage archives."""

import io
import json
from pathlib import Path
import platform
import sys
import tarfile
import tempfile
import unittest


sys.path.insert(0, str(Path(__file__).parents[1]))
from unix_package import STAGE, unpack_archive


class PackageArchive(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.prefix = self.root / "lurker"
        self.archive = self.root / "package.tar.gz"
        self.scratch = self.root / "scratch"
        self.scratch.mkdir()
        self.commit = "a" * 40
        (self.prefix / "bin").mkdir(parents=True)
        self.program = self.prefix / "bin/lurker"
        self.program.write_bytes(b"immutable executable bytes")
        self.program.chmod(0o755)
        (self.prefix / "bin/alias").symlink_to("lurker")
        share = self.prefix / "share/lurker"
        self.notices = share / "licenses/rust"
        self.notices.mkdir(parents=True)
        (self.notices / "packages.json").write_text(json.dumps({
            "source_commit": self.commit, "products": ["lurker"], "packages": [{"name": "a-crate"}],
        }))
        self.provenance = share / "provenance/rust-binaries.json"
        self.provenance.parent.mkdir()
        self.provenance.write_text(json.dumps({
            "backend": "binaries", "source_commit": self.commit,
            "files": {"lurker": {"sha256": STAGE.digest(self.program)}},
            "rust_notices": {"cargo_log_sha256": "b" * 64, "files": STAGE.file_manifest(self.notices)},
        }))
        self.manifest = {
            "schema": 1, "source_commit": self.commit, "architecture": platform.machine(),
            "os": platform.system(),
        }
        self.write_manifest()

    def write_manifest(self):
        metadata = "share/lurker/manifest.json"
        self.manifest["files"] = STAGE.file_manifest(self.prefix, exclude=(metadata,))
        (self.prefix / metadata).write_text(json.dumps(self.manifest))

    def build(self, extra=None):
        with tarfile.open(self.archive, "w:gz") as source:
            def normalize_directory(member):
                if member.isdir():
                    member.mode = 0o755
                return member
            source.add(self.prefix, arcname="lurker", filter=normalize_directory)
            if extra is not None:
                payload = b"extra"
                member = tarfile.TarInfo(extra)
                member.size = len(payload)
                source.addfile(member, io.BytesIO(payload))
        return STAGE.digest(self.archive)

    def test_preserves_verified_files_and_symlinks(self):
        prefix, manifest = unpack_archive(self.archive, self.build(), self.commit, self.scratch)
        self.assertEqual(manifest, self.manifest)
        self.assertTrue((prefix / "bin/alias").is_symlink())
        self.assertEqual((prefix / "bin/alias").read_bytes(), self.program.read_bytes())

    def test_rejects_wrong_archive_digest(self):
        self.build()
        with self.assertRaisesRegex(ValueError, "archive SHA-256"):
            unpack_archive(self.archive, "f" * 64, self.commit, self.scratch)
        self.assertFalse(list(self.scratch.iterdir()))

    def test_rejects_corrupt_payload(self):
        self.program.write_bytes(b"changed bytes")
        with self.assertRaisesRegex(ValueError, "file inventory differs"):
            unpack_archive(self.archive, self.build(), self.commit, self.scratch)

    def test_rejects_missing_notice_catalog_in_consistent_archive(self):
        (self.notices / "packages.json").unlink()
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, "require Rust dependency notices"):
            unpack_archive(self.archive, self.build(), self.commit, self.scratch)

    def test_rejects_notices_detached_from_binary_provenance(self):
        (self.notices / "packages.json").write_text("{}")
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, "Rust notices differ"):
            unpack_archive(self.archive, self.build(), self.commit, self.scratch)

    def test_rejects_detached_product_binary(self):
        self.program.write_bytes(b"an unrelated build")
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, "Rust product differs"):
            unpack_archive(self.archive, self.build(), self.commit, self.scratch)

    def test_rejects_symlink_ancestor_before_extraction(self):
        digest = self.build("lurker/bin/alias/new")
        with self.assertRaisesRegex(ValueError, "non-directory ancestor"):
            unpack_archive(self.archive, digest, self.commit, self.scratch)
        self.assertFalse(list(self.scratch.iterdir()))

    def test_rejects_parent_traversal_before_extraction(self):
        digest = self.build("lurker/../escape")
        with self.assertRaisesRegex(ValueError, "unsafe relative path"):
            unpack_archive(self.archive, digest, self.commit, self.scratch)
        self.assertFalse(list(self.scratch.iterdir()))


if __name__ == "__main__":
    unittest.main()
