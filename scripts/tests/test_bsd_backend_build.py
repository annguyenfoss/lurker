"""BSD build provenance tests, without SSH, compilation or storage mounts."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "build-bsd-backends.py"
spec = importlib.util.spec_from_file_location("bsd_builder", SCRIPT)
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class SourceProvenance(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lurker-bsd-build-test-")
        self.addCleanup(self.temporary.cleanup)
        self.cache = Path(self.temporary.name)
        self.archive = self.cache / "example-fixed.tar.gz"
        self.pin = {"revision": "fixed", "url": "https://example.invalid/pinned.tar.gz", "sha256": "pending"}
        self.patch = patch.dict(builder.PINS, {"example": self.pin}, clear=True)
        self.patch.start()
        self.addCleanup(self.patch.stop)

    def archive_file(self, contents=b"source", name="upstream/file.c", symlink=False):
        with tarfile.open(self.archive, "w:gz") as tar:
            member = tarfile.TarInfo(name)
            if symlink:
                member.type = tarfile.SYMTYPE
                member.linkname = "/etc/passwd"
            else:
                member.size = len(contents)
            tar.addfile(member, None if symlink else io.BytesIO(contents))
        self.pin["sha256"] = builder.digest(self.archive)

    def test_verified_cache_reuse_rejects_changed_source_and_archive(self):
        self.archive_file()
        source = builder.prepare("example", self.cache, offline=True)
        self.assertEqual((source / "file.c").read_bytes(), b"source")
        self.assertEqual(builder.prepare("example", self.cache, offline=True), source)
        (source / "file.c").write_bytes(b"tampered")
        with self.assertRaisesRegex(RuntimeError, "Modified source input"):
            builder.prepare("example", self.cache, offline=True)
        self.archive.write_bytes(b"untrusted archive")
        with self.assertRaisesRegex(RuntimeError, "SHA-256"):
            builder.prepare("example", self.cache, offline=True)

    def test_offline_missing_sources_and_unstamped_trees_fail(self):
        with self.assertRaisesRegex(RuntimeError, "Missing pinned archive"):
            builder.prepare("example", self.cache, offline=True)
        self.archive_file()
        (self.cache / "example").mkdir()
        with self.assertRaisesRegex(RuntimeError, "Unverified source cache"):
            builder.prepare("example", self.cache, offline=True)

    def test_archive_link_cannot_escape_cache(self):
        self.archive_file(symlink=True)
        with self.assertRaisesRegex(RuntimeError, "Unexpected .* archive layout"):
            builder.prepare("example", self.cache, offline=True)

    def test_ldd_parses_both_native_output_forms_and_rejects_missing_libraries(self):
        freebsd = "libfuse.so.2 => /usr/local/lib/libfuse.so.2 (0x10)\nlibc.so.7 => /lib/libc.so.7 (0x20)\n"
        openbsd = "0000 0001 rlib 0 1 0 /usr/lib/libfuse.so.4.0\n0001 0002 rlib 0 1 0 /usr/lib/libc.so.103.0\n"
        with patch.object(builder.subprocess, "check_output", return_value=freebsd):
            self.assertIn(Path("/usr/local/lib/libfuse.so.2"), builder.linked_libraries(Path("backend")))
        with patch.object(builder.subprocess, "check_output", return_value=openbsd):
            self.assertEqual(len(builder.linked_libraries(Path("backend"))), 2)
        with patch.object(builder.subprocess, "check_output", return_value="libfuse.so.2 => not found"):
            with self.assertRaisesRegex(RuntimeError, "unresolved"):
                builder.linked_libraries(Path("backend"))

    def test_wx_cache_requires_exact_specification_and_artifact_hashes(self):
        artifact = self.cache / "libwx.a"
        artifact.write_bytes(b"compiled wx")
        stamp = self.cache / "wx-build.json"
        spec = {"compiler": "qualified compiler"}
        stamp.write_text(json.dumps({"specification": spec, "files": {str(artifact): builder.digest(artifact)}}))
        self.assertTrue(builder.wx_cached(stamp, spec))
        self.assertFalse(builder.wx_cached(stamp, {"compiler": "different"}))
        artifact.write_bytes(b"stale build")
        self.assertFalse(builder.wx_cached(stamp, spec))

    def test_reviewed_patch_is_repeatable_and_rejects_extra_source_changes(self):
        # A reduced real upstream fixture retains the complete reviewed hunk.
        original = ('prefix\n\tvoid UserInterface::DisplayVolumeProperties (const VolumeInfoList &volumes) const\n'
            '\t{\n\t\tif (volumes.size() < 1)\n\t\t\tthrow_err (LangString["NO_VOLUMES_MOUNTED"]);\n\n\t\twxString prop;\n'
            '#ifndef TC_WINDOWS\n'
            '\t\t\tprop << LangString["VIRTUAL_DEVICE"] << L": " << wstring (volume.VirtualDevice) << L\'\\n\';\n'
            '#endif\n'
            '\t\t\tprop << LangString["MOUNT_POINT"] << L": " << wstring (volume.MountPoint) << L\'\\n\';\n'
            '\t\t\tprop << LangString["SIZE"] << L": " << SizeToString (volume.Size) << L\'\\n\';\n'
            '\t\t\tprop << LangString["TYPE"] << L": " << VolumeTypeToString (volume.Type, volume.Protection) << L\'\\n\';\n'
            'suffix\n').encode()
        archive = self.cache / "veracrypt-fixed.tar.gz"
        with tarfile.open(archive, "w:gz") as tar:
            member = tarfile.TarInfo("upstream/" + builder.PATCH_TARGET)
            member.size = len(original)
            tar.addfile(member, io.BytesIO(original))
        pin = {"revision": "fixed", "url": "https://example.invalid/source", "sha256": builder.digest(archive)}
        with patch.dict(builder.PINS, {"veracrypt": pin}):
            source = builder.prepare("veracrypt", self.cache, offline=True)
            self.assertIn(b"Lurker Auxiliary Directory:", (source / builder.PATCH_TARGET).read_bytes())
            self.assertEqual(builder.prepare("veracrypt", self.cache, offline=True), source)
            with (source / builder.PATCH_TARGET).open("ab") as stream:
                stream.write(b"unreviewed source modification")
            with self.assertRaisesRegex(RuntimeError, "Modified source input"):
                builder.prepare("veracrypt", self.cache, offline=True)


if __name__ == "__main__":
    unittest.main()
