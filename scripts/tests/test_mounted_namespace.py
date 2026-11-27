"""Admission checks for the mounted namespace integration probe."""

import importlib.util
import os
from pathlib import Path
import signal
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "mounted_namespace", Path(__file__).resolve().parents[1] / "test-mounted-namespace.py")
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class AdmissionTests(unittest.TestCase):
    def test_escaped_mount_and_both_readonly_option_fields(self):
        for vfs, superblock, readonly in (
                ("rw,relatime", "rw,errors=remount-ro", False),
                ("rw,relatime", "ro", True),
                ("ro,relatime", "rw", True)):
            record = probe.mount_record(
                f"42 1 7:0 / /test\\040mount {vfs} - ext4 /dev/loop0 {superblock}\n",
                Path("/test mount"))
            if readonly:
                with self.assertRaisesRegex(RuntimeError, "not read/write"):
                    probe.validate_record(record, "42", "/dev/loop0", "ext4")
            else:
                probe.validate_record(record, "42", "/dev/loop0", "ext4")

    def test_exact_mount_identity_and_overmount_ambiguity_are_rejected(self):
        line = "42 1 7:0 / /test rw - ext4 /dev/loop0 rw\n"
        record = probe.mount_record(line, Path("/test"))
        for expected in (("43", "/dev/loop0", "ext4"),
                         ("42", "/dev/loop1", "ext4"),
                         ("42", "/dev/loop0", "xfs")):
            with self.assertRaisesRegex(RuntimeError, "differs"):
                probe.validate_record(record, *expected)
        with self.assertRaisesRegex(RuntimeError, "exactly one"):
            probe.mount_record(line + line.replace("42 ", "43 "), Path("/test"))
        with self.assertRaisesRegex(RuntimeError, "exactly one"):
            probe.mount_record(line, Path("/test/underlay"))

    def test_wrong_caller_rejected_before_path_or_descriptor_access(self):
        args = SimpleNamespace(expect_euid=1301)
        with patch.object(probe.os, "geteuid", return_value=1300), \
                patch.object(probe.os, "open", side_effect=AssertionError("unexpected path access")):
            with self.assertRaisesRegex(RuntimeError, "effective UID"):
                probe.MountedNamespace(args, {})


class WorkerTerminationTests(unittest.TestCase):
    def test_passed_receipt_cannot_hide_nonzero_or_signal_exit(self):
        class PassedReceipt:
            def __init__(self, args, receipt):
                self.receipt = receipt
                self.fd = os.open(os.devnull, os.O_RDONLY)

            def run(self):
                self.receipt["status"] = "passed"

        real_exit = os._exit
        for expected in (1, -signal.SIGTERM):
            def abnormal_exit(_):
                if expected < 0:
                    os.kill(os.getpid(), -expected)
                real_exit(expected)

            with tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                receipt = directory / "result.json"
                args = ["probe", str(directory / "mount"), "--expect-mount-id", "1",
                        "--expect-source", "test", "--expect-fstype", "test",
                        "--expect-euid", str(os.geteuid()), "--receipt", str(receipt)]
                with patch.object(probe, "MountedNamespace", PassedReceipt), \
                        patch.object(probe.sys, "argv", args), \
                        patch.object(probe.os, "_exit", abnormal_exit):
                    self.assertEqual(probe.main(), 1)
                result = probe.json.loads(receipt.read_text())
                self.assertEqual(result["status"], "failed-worker-exit")
                self.assertEqual(result["worker_exit_code"], expected)
                self.assertFalse(result["worker_exited_zero"])
                self.assertTrue(result["preserve_unexpected_remnants"])


if __name__ == "__main__":
    unittest.main()
