"""Owned fixture reclamation policy; no encryption tools or mounts execute."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("package_runner", Path(__file__).resolve().parents[1] / "test-linux-package.py")
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class ReclaimPolicy(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lurker-matrix-reclaim-test-")
        self.addCleanup(self.temporary.cleanup)
        self.job = Path(self.temporary.name).resolve()
        self.root = self.job / "lurker-conformance-owned"
        self.root.mkdir()
        self.image = self.root / "vault 密碼 with spaces.img"
        self.image.write_bytes(b"closed encrypted fixture")
        self.result = {"root": str(self.root), "status": "passed", "mount_present": False}

    def test_closed_image_and_hardlink_are_hashed_before_removal(self):
        alias = self.root / "source alias"
        os.link(self.image, alias)
        unrelated = self.root / "keep.img"
        unrelated.write_text("retain")
        log = self.root / "result.json"
        log.write_text("evidence")
        expected = runner.digest(self.image)
        files = runner.reclaim_passed_image(self.result, self.job)
        self.assertEqual([expected, expected], [item["sha256"] for item in files])
        self.assertEqual([len(b"closed encrypted fixture")] * 2, [item["bytes"] for item in files])
        self.assertFalse(self.image.exists())
        self.assertFalse(alias.exists())
        self.assertEqual("evidence", log.read_text())
        self.assertEqual("retain", unrelated.read_text())

    def test_failure_or_remaining_mount_retains_image(self):
        for status, mounted in (("failed", False), ("passed", True)):
            self.result.update(status=status, mount_present=mounted)
            self.assertEqual([], runner.reclaim_passed_image(self.result, self.job))
            self.assertTrue(self.image.exists())

    def test_directory_backend_keeps_ciphertext_and_its_intentional_alias(self):
        self.image.unlink()
        self.image.mkdir()
        ciphertext = self.image / "encrypted-block"
        ciphertext.write_bytes(b"retained ciphertext")
        alias = self.root / "source alias"
        alias.symlink_to(self.image, target_is_directory=True)
        for backend in ("gocryptfs", "cryfs", "cryptomator"):
            with self.subTest(backend=backend):
                self.result["backend"] = backend
                self.assertEqual([], runner.reclaim_passed_image(self.result, self.job))
                self.assertEqual(b"retained ciphertext", ciphertext.read_bytes())
                self.assertTrue(alias.is_symlink())

    def test_unexpected_fixture_parent_is_refused(self):
        with self.assertRaisesRegex(ValueError, "Unexpected conformance fixture"):
            runner.reclaim_passed_image(self.result, self.job.parent)
        self.assertTrue(self.image.exists())

    def test_symlink_cannot_redirect_reclamation(self):
        self.image.unlink()
        outside = self.job / "outside"
        outside.write_text("keep")
        self.image.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "Unexpected symlink"):
            runner.reclaim_passed_image(self.result, self.job)
        self.assertEqual("keep", outside.read_text())

    def test_kernel_mount_or_loop_lease_blocks_pruning(self):
        original = Path.read_text
        for mode in ("mount", "loop"):
            def read(path, *args, **kwargs):
                if str(path) == "/proc/self/mountinfo":
                    return str(self.root) + "/mount" if mode == "mount" else ""
                if path == self.job / "fake-loop-backing":
                    return str(self.image)
                return original(path, *args, **kwargs)
            with self.subTest(mode=mode), mock.patch.object(Path, "read_text", read), mock.patch.object(Path, "glob", return_value=[self.job / "fake-loop-backing"]):
                with self.assertRaisesRegex(ValueError, "still owns a"):
                    runner.reclaim_passed_image(self.result, self.job)
            self.assertTrue(self.image.exists())


if __name__ == "__main__":
    unittest.main()
