#!/usr/bin/env python3
"""Run the OpenBSD diagnostic producer checks natively, without any mount."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tarfile
import tempfile
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("openbsd_lkl", ROOT / "scripts/build-openbsd-lkl.py")
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)


class SourceInputs(unittest.TestCase):
    def test_cached_archive_corruption_is_retained_and_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "archive"
            path.write_bytes(b"corrupted input")
            with self.assertRaisesRegex(RuntimeError, "cache hash mismatch"):
                build.fetch(path)
            self.assertEqual(path.read_bytes(), b"corrupted input")

    def test_cached_archive_symlink_is_refused_before_read(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "archive"
            path.symlink_to("missing")
            with self.assertRaisesRegex(RuntimeError, "cannot be a symlink"):
                build.fetch(path)
            self.assertEqual(os.readlink(path), "missing")

    def test_exact_pinned_cache_does_not_download(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "archive"
            path.write_bytes(b"exact pinned input")
            with patch.object(build, "SOURCE_SHA256", hashlib.sha256(path.read_bytes()).hexdigest()), \
                 patch.object(build.urllib.request, "urlopen", side_effect=AssertionError("network forbidden")):
                self.assertEqual(build.fetch(path), path)

    def source_archive(self, root, relative="file.c"):
        archive = root / "source.tar"
        with tarfile.open(archive, "w") as tar:
            item = tarfile.TarInfo("linux-test/" + relative)
            item.size = 4
            tar.addfile(item, io.BytesIO(b"code"))
        return archive

    def test_changed_source_is_refused_without_overwrite(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive = self.source_archive(root)
            source = root / "source"
            source.mkdir()
            path = source / "file.c"
            path.write_bytes(b"code")
            with patch.object(build, "REVISION", "test"):
                self.assertEqual(build.verify_source(archive, source), 1)
                path.write_bytes(b"user change")
                with self.assertRaisesRegex(RuntimeError, "Modified upstream input"):
                    build.verify_source(archive, source)
            self.assertEqual(path.read_bytes(), b"user change")

    def test_source_symlink_cannot_substitute_regular_input(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive = self.source_archive(root)
            source = root / "source"
            source.mkdir()
            (root / "foreign").write_bytes(b"code")
            (source / "file.c").symlink_to(root / "foreign")
            with patch.object(build, "REVISION", "test"), self.assertRaisesRegex(RuntimeError, "Missing regular upstream input"):
                build.verify_source(archive, source)

    def test_archive_traversal_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive = self.source_archive(root, "../outside")
            with patch.object(build, "REVISION", "test"), self.assertRaisesRegex(RuntimeError, "Unsafe source archive path"):
                build.verify_source(archive, root)


class OwnedBuildProcess(unittest.TestCase):
    def test_success_preserves_exact_log_and_does_not_run_shell(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            r = build.run_build([sys.executable, "-c", "print('native diagnostic')"], root,
                                os.environ.copy(), root / "build.log", 5)
            self.assertEqual(r["returncode"], 0)
            self.assertFalse(r["timed_out"])
            self.assertEqual((root / "build.log").read_text(), "native diagnostic\n")

    def test_existing_log_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            log = root / "build.log"
            log.write_text("old diagnostic")
            with self.assertRaises(FileExistsError):
                build.run_build(["/nonexistent"], root, os.environ.copy(), log, 5)
            self.assertEqual(log.read_text(), "old diagnostic")

    def test_deadline_kills_child_that_ignores_term_after_leader_exits(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            child_code = ("import os,signal,time;from pathlib import Path;"
                          "signal.signal(signal.SIGTERM,signal.SIG_IGN);"
                          f"Path({str(root / 'grandchild')!r}).write_text(str(os.getpid()));"
                          "time.sleep(30)")
            parent_code = ("import subprocess,sys,time;from pathlib import Path;"
                           f"subprocess.Popen([sys.executable,'-c',{child_code!r}]);"
                           "time.sleep(30)")
            r = build.run_build([sys.executable, "-c", parent_code], root,
                                os.environ.copy(), root / "build.log", 1, termination_grace=0.1)
            self.assertTrue(r["timed_out"])
            self.assertTrue((root / "grandchild").exists())
            grandchild = int((root / "grandchild").read_text())
            # A transient reparented zombie cannot execute; no compiler may
            # survive the bounded whole-session cancellation.
            output = subprocess.check_output(["/bin/ps", "-ax", "-o", "pid,pgid,state"], text=True)
            live = [line for line in output.splitlines()[1:] if len(line.split()) == 3
                    and int(line.split()[1]) == r["pid"] and not line.split()[2].startswith("Z")]
            self.assertEqual(live, [], (grandchild, output))


if __name__ == "__main__":
    if platform.system() != "OpenBSD":
        raise SystemExit("Run these checks on native OpenBSD")
    unittest.main()
