"""Source-input and refusal controls for the standalone native formatter builder.

Run on the owned native remote host. These tests never execute a formatter.
"""
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest import mock

SCRIPT = Path(__file__).resolve().parents[1] / "build-macos-xfsprogs.py"
SPEC = importlib.util.spec_from_file_location("build_macos_xfsprogs", SCRIPT)
builder = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(builder)


class FormatterBuilderTests(unittest.TestCase):
    def test_non_native_host_is_rejected_before_creating_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with mock.patch("sys.argv", [str(SCRIPT), "--cache-dir", str(root / "cache"),
                                        "--output-dir", str(root / "output")]), \
                 mock.patch.object(builder.platform, "system", return_value="Linux"), \
                 self.assertRaises(SystemExit):
                builder.main()
            self.assertFalse((root / "output").exists())

    def test_elevated_native_build_is_rejected_before_creating_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with mock.patch("sys.argv", [str(SCRIPT), "--cache-dir", str(root / "cache"),
                                        "--output-dir", str(root / "output")]), \
                 mock.patch.object(builder.platform, "system", return_value="Darwin"), \
                 mock.patch.object(builder.platform, "machine", return_value="arm64"), \
                 mock.patch.object(builder.os, "getuid", return_value=0), \
                 self.assertRaises(SystemExit):
                builder.main()
            self.assertFalse((root / "output").exists())

    def archive(self, root, names):
        archive = root / "source.tar.gz"
        with tarfile.open(archive, "w:gz") as stream:
            for name, kind in names:
                info = tarfile.TarInfo(name)
                if kind == "symlink":
                    info.type = tarfile.SYMTYPE
                    info.linkname = "/etc/passwd"
                    stream.addfile(info)
                else:
                    info.size = 4
                    stream.addfile(info, io.BytesIO(b"data"))
        return archive

    def test_archive_traversal_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = self.archive(root, [("source/../../outside", "file")])
            with self.assertRaisesRegex(ValueError, "Unsafe source"):
                builder.extract(archive, root / "out")
            self.assertFalse((root / "outside").exists())

    def test_source_symlinks_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = self.archive(root, [("source/include/xfs.h", "symlink")])
            with self.assertRaisesRegex(ValueError, "Non-file"):
                builder.extract(archive, root / "out")

    def test_mixed_source_roots_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = self.archive(root, [("one/main.c", "file"), ("two/header.h", "file")])
            with self.assertRaisesRegex(ValueError, "single source"):
                builder.extract(archive, root / "out")

    def test_cached_tampering_is_rejected_even_offline(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "xfsprogs.tar.gz").write_bytes(b"altered")
            with self.assertRaisesRegex(ValueError, "Source hash mismatch"):
                builder.fetch("xfsprogs.tar.gz", root, True)

    def test_missing_offline_input_never_uses_network(self):
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
            builder.urllib.request, "urlopen", side_effect=AssertionError("network")
        ):
            with self.assertRaisesRegex(ValueError, "Missing verified offline"):
                builder.fetch("xfsprogs.tar.gz", Path(temporary), True)

    def test_broken_cache_symlink_is_rejected_before_download_or_write(self):
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
            builder.urllib.request, "urlopen", side_effect=AssertionError("network")
        ):
            root = Path(temporary)
            cache = root / "cache"
            cache.mkdir()
            outside = root / "unrelated"
            (cache / "xfsprogs.tar.gz").symlink_to(outside)
            with self.assertRaisesRegex(ValueError, "source-cache symlink"):
                builder.fetch("xfsprogs.tar.gz", cache, False)
            self.assertFalse(outside.exists())

    def test_replaced_git_content_inventory_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            names = [f"file{i:03d}.c" for i in range(738)]
            (root / ".gitcensus").write_text("\n".join(names) + "\n")
            for name in names:
                (root / name).write_text("modified source\n")
            with self.assertRaisesRegex(ValueError, "authenticated pinned Git commit"):
                builder.verify_source_tree(root)

    def test_libxfs_inventory_rejects_shell_or_path_syntax(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "libxfs").mkdir()
            (root / "libxfs/Makefile").write_text("CFILES = init.c $(shell bad)\n\n")
            with self.assertRaisesRegex(ValueError, "source-list syntax"):
                builder.libxfs_sources(root)


if __name__ == "__main__":
    unittest.main()
