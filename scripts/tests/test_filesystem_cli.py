"""Regress terminal password handling with actual sudo control-sequence shapes."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import time
import unittest

spec = importlib.util.spec_from_file_location("filesystem_cli", Path(__file__).resolve().parents[1] / "test-filesystem-cli.py")
suite = importlib.util.module_from_spec(spec)
spec.loader.exec_module(suite)


class PromptTests(unittest.TestCase):
    def test_osc_session_prefix_and_colored_prompt(self):
        # A child with its own TTY must receive the secret only after echo is
        # disabled, including when OSC control text precedes a native prompt.
        child = r'''
import os, sys, termios
fd = sys.stdin.fileno()
attrs = termios.tcgetattr(fd)
attrs[3] &= ~termios.ECHO
termios.tcsetattr(fd, termios.TCSAFLUSH, attrs)
os.write(1, b'\x1b]3008;start=test;comm=sudo\x1b\\\x1b[32mEnter passphrase for /tmp/vault:\x1b[0m ')
secret = sys.stdin.readline().rstrip('\n')
sys.exit(0 if secret == 'test-secret' else 2)
'''
        with tempfile.TemporaryDirectory() as root:
            result = suite.terminal([sys.executable, "-c", child], "test-secret", 5, Path(root) / "terminal.log")
            self.assertEqual(result["exit_code"], 0)
            self.assertFalse(result["timed_out"])
            self.assertEqual(result["prompts"], 1)
            self.assertNotIn("test-secret", (Path(root) / "terminal.log").read_text())


class PayloadTests(unittest.TestCase):
    def test_downloaded_payload_requires_every_paper_and_records_exact_bytes(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            for name in ("bitcoin.pdf", "ethereum.pdf", "transformer.pdf"):
                (directory / name).write_bytes(b"%PDF-1.4\n" + name.encode())
            (directory / "page.txt").write_bytes(b"Text including \x00 binary boundary\n")
            expected = suite.load_payload(directory)
            self.assertEqual(set(expected), {"bitcoin.pdf", "ethereum.pdf", "transformer.pdf", "page.txt"})
            self.assertEqual(expected["page.txt"], (directory / "page.txt").read_bytes())
            (directory / "transformer.pdf").unlink()
            with self.assertRaises(ValueError):
                suite.load_payload(directory)

    def test_invalid_paper_and_ambiguous_input_are_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            (directory / "bitcoin.pdf").write_bytes(b"<html>download failed</html>")
            with self.assertRaises(ValueError):
                suite.load_payload(directory)
            with self.assertRaises(ValueError):
                suite.load_payload(directory, directory / "paper.pdf")


class PayloadWorkerTests(unittest.TestCase):
    def test_worker_returns_result_and_reports_io_failure(self):
        self.assertEqual(suite.bounded_payload_call(lambda: {"sha256": "verified"}, 2),
                         {"sha256": "verified"})

        def failed_io():
            raise OSError("native filesystem write failed")

        with self.assertRaises(suite.PayloadWorkerError) as failure:
            suite.bounded_payload_call(failed_io, 2)
        self.assertIn("native filesystem write failed", str(failure.exception))
        self.assertFalse(failure.exception.worker["remaining"])

    def test_stalled_worker_is_bounded_and_reaped(self):
        started = time.monotonic()
        with self.assertRaises(suite.PayloadWorkerError) as failure:
            suite.bounded_payload_call(lambda: time.sleep(30), 0.1)
        self.assertLess(time.monotonic() - started, 4)
        self.assertIn("timed out", str(failure.exception))
        self.assertFalse(failure.exception.worker["remaining"])
        with self.assertRaises(ChildProcessError):
            suite.os.waitpid(failure.exception.worker["pid"], suite.os.WNOHANG)


class NativeFilesystemTests(unittest.TestCase):
    def test_superblock_readonly_rejects_a_writable_vfs_view(self):
        for vfs, superblock, readonly in (
                ("rw,relatime", "ro,ufstype=44bsd", True),
                ("ro,relatime", "rw", True),
                ("rw,relatime", "rw,errors=remount-ro", False)):
            mounts = f"1 0 7:0 / /native {vfs} - ufs /dev/loop0 {superblock}\n"
            record = suite.linux_mount_record(mounts, Path("/native"))
            self.assertEqual(record["options"], vfs.split(","))
            self.assertEqual(record["superblock_options"], superblock.split(","))
            if readonly:
                with self.assertRaisesRegex(RuntimeError, "not read/write"):
                    suite.check_linux_filesystem(record, "ufs1")
            else:
                self.assertEqual(suite.check_linux_filesystem(record, "ufs1"),
                                 "driver-or-superblock-proof-required")

    def test_selected_ext4_cannot_pass_when_formatter_produces_ext2(self):
        record = {"filesystem": "ext2", "options": ["rw", "relatime"]}
        with self.assertRaisesRegex(RuntimeError, "Requested ext4.*ext2"):
            suite.check_linux_filesystem(record, "ext4")
        record["filesystem"] = "ext4"
        self.assertEqual(suite.check_linux_filesystem(record, "ext4"), "native-type-verified")
        record["options"] = ["ro"]
        with self.assertRaisesRegex(RuntimeError, "not read/write"):
            suite.check_linux_filesystem(record, "ext4")

    def test_backing_identity_uses_longest_native_mount_and_decodes_escapes(self):
        mounts = ("1 0 8:0 / / rw - ext4 /dev/root rw\n"
                  "2 1 7:0 / /test\\040backing rw - xfs /dev/loop0 rw\n"
                  "3 2 0:2 / /plaintext rw - fuse.gocryptfs gocryptfs rw\n")
        backing = suite.linux_mount_record(mounts, Path("/test backing/vault"))
        plaintext = suite.linux_mount_record(mounts, Path("/plaintext"))
        self.assertEqual(backing["filesystem"], "xfs")
        self.assertEqual(backing["mountpoint"], "/test backing")
        self.assertEqual(plaintext["filesystem"], "fuse.gocryptfs")
        self.assertEqual(suite.check_linux_filesystem(backing, "xfs"), "native-type-verified")

    def test_generic_fuse_is_not_full_format_identity_proof(self):
        record = {"filesystem": "fuseblk", "options": ["rw"]}
        self.assertEqual(suite.check_linux_filesystem(record, "ntfs"),
                         "driver-or-superblock-proof-required")
        for filesystem in ("ufs", "fuse.ufs"):
            record["filesystem"] = filesystem
            self.assertEqual(suite.check_linux_filesystem(record, "ufs2"),
                             "driver-or-superblock-proof-required")


if __name__ == "__main__":
    unittest.main()
