#!/usr/bin/env python3
"""Focused native tests for pinned inputs and bounded LKL diagnostic jobs."""
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("netbsd_lkl", Path(__file__).with_name("build-netbsd-lkl.py"))
BUILDER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILDER)


class BuildInputs(unittest.TestCase):
    def test_cached_archive_is_rehashed(self):
        with tempfile.TemporaryDirectory() as tmp:
            archive = Path(tmp) / "cached.tar.gz"
            archive.write_bytes(b"original")
            expected = hashlib.sha256(b"original").hexdigest()
            with mock.patch.object(BUILDER, "SOURCE_SHA256", expected):
                self.assertEqual(BUILDER.fetch(archive), archive)
                archive.write_bytes(b"replacement")
                with self.assertRaisesRegex(RuntimeError, "SHA-256"):
                    BUILDER.fetch(archive)

    def test_modified_upstream_file_prevents_resume(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            source = work / "source"
            source.mkdir()
            (source / "input.c").write_bytes(b"original")
            archive = work / "input.tar"
            with tarfile.open(archive, "w") as tar:
                item = tarfile.TarInfo(f"linux-{BUILDER.REVISION}/input.c")
                item.size = len(b"original")
                tar.addfile(item, io.BytesIO(b"original"))
            self.assertEqual(BUILDER.verify_source(archive, source), 1)
            (source / "input.c").write_bytes(b"changed")
            with self.assertRaisesRegex(RuntimeError, "Modified upstream input"):
                BUILDER.verify_source(archive, source)

    def test_archive_traversal_prevents_resume(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            archive = work / "input.tar"
            with tarfile.open(archive, "w") as tar:
                item = tarfile.TarInfo(f"linux-{BUILDER.REVISION}/../escape")
                tar.addfile(item, io.BytesIO())
            with self.assertRaisesRegex(RuntimeError, "Unsafe source archive"):
                BUILDER.verify_source(archive, work)

    def test_source_symlink_is_not_a_regular_input(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "real").write_bytes(b"original")
            (work / "input.c").symlink_to("real")
            archive = work / "input.tar"
            with tarfile.open(archive, "w") as tar:
                item = tarfile.TarInfo(f"linux-{BUILDER.REVISION}/input.c")
                item.size = 8
                tar.addfile(item, io.BytesIO(b"original"))
            with self.assertRaisesRegex(RuntimeError, "Missing regular upstream input"):
                BUILDER.verify_source(archive, work)

    def test_source_directory_cannot_redirect_outside_cache(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "outside").mkdir()
            (work / "subdir").symlink_to("outside", target_is_directory=True)
            archive = work / "input.tar"
            with tarfile.open(archive, "w") as tar:
                item = tarfile.TarInfo(f"linux-{BUILDER.REVISION}/subdir")
                item.type = tarfile.DIRTYPE
                tar.addfile(item)
            with self.assertRaisesRegex(RuntimeError, "Modified upstream directory"):
                BUILDER.verify_source(archive, work)

    def test_safe_tar_symlink_normalization_is_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "subdir").mkdir()
            (work / "subdir/link").symlink_to("../target")
            archive = work / "input.tar"
            with tarfile.open(archive, "w") as tar:
                item = tarfile.TarInfo(f"linux-{BUILDER.REVISION}/subdir/link")
                item.type = tarfile.SYMTYPE
                item.linkname = ".././target"
                tar.addfile(item)
            self.assertEqual(BUILDER.verify_source(archive, work), 0)

    def test_timeout_terminates_owned_make_process_group(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            marker = work / "escaped-child"
            child = "import time,pathlib;time.sleep(2);pathlib.Path(%r).touch()" % str(marker)
            parent = "import subprocess,sys,time;subprocess.Popen([sys.executable,'-c',%r]);time.sleep(30)" % child
            result = BUILDER.run_build([sys.executable, "-c", parent], work, os.environ.copy(), work / "build.log", 0.2)
            self.assertTrue(result["timed_out"])
            import time
            time.sleep(2.1)
            self.assertFalse(marker.exists())


if __name__ == "__main__":
    unittest.main()
