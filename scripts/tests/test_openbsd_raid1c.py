"""Pure safety checks for the opt-in CLI suite; no native storage operations."""
import importlib.util
from pathlib import Path
import unittest
import time
import struct
import tempfile
from types import SimpleNamespace
from unittest import mock

SPEC = importlib.util.spec_from_file_location("openbsd_raid1c_conformance", Path(__file__).parents[1] / "test-openbsd-raid1c.py")
SUITE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SUITE)


class Raid1cHarnessTests(unittest.TestCase):
    def test_owned_resource_scope_resists_prefix_and_parent_traversal(self):
        root = Path("/tmp/lurker-owned")
        self.assertTrue(SUITE.contains_owned_path({"mirror": {"members": [{"path": str(root / "member")}] }}, root))
        for outside in ["/tmp/lurker-owned-other/member", "/tmp/lurker-owned/../foreign", "/critical", "relative"]:
            self.assertFalse(SUITE.contains_owned_path({"source": outside}, root))

    def test_quiet_window_gate_rejects_foreign_arrays_and_raw_vnodes(self):
        idle = {"vnconfig": "vnd0: not in use\nvnd1: not in use\n", "bioctl": "", "sessions": {}}
        SUITE.require_idle(idle)
        for delta in [{"vnconfig": "vnd0: covering /foreign.img on sd0a, inode 42\n"},
                      {"bioctl": "softraid0 0 Online 1 sd1 CRYPTO"}, {"sessions": {"owned.json": {}}},
                      {"vnconfig": "unrecognized response"}]:
            with self.subTest(delta=delta), self.assertRaises(AssertionError):
                SUITE.require_idle(dict(idle, **delta))

    def test_cli_modes_keep_degraded_recovery_explicit(self):
        argv = SUITE.arguments("/owned/lurker", "create", "/tmp/source", peer="/tmp/peer")
        self.assertIn("--member", argv)
        self.assertNotIn("--degraded", argv)
        mount = SUITE.arguments("/owned/lurker", "mount", "/tmp/peer", kind="raid1c", degraded=True, mountpoint="/tmp/mnt")
        self.assertIn("--degraded", mount)
        self.assertEqual(mount[mount.index("--type") + 1], "raid1c")
        auto = SUITE.arguments("/owned/lurker", "mount", "/tmp/source", mountpoint="/tmp/mnt")
        self.assertNotIn("--degraded", auto)
        self.assertEqual(auto[auto.index("--type") + 1], "auto")
        with self.assertRaises(ValueError):
            SUITE.arguments("/owned/lurker", "mount", "/tmp/source")

    def test_close_can_identify_a_missing_logical_primary_without_statting_it(self):
        suite = object.__new__(SUITE.MirrorSuite)
        suite.options = SimpleNamespace(binary=Path("/owned/lurker"), size_gb="0.125")
        suite.mountpoint = Path("/tmp/lurker-owned/mounted")
        suite.cli = mock.Mock()
        suite.footprint = lambda: {"vnconfig": "vnd0: not in use\n", "bioctl": "", "sessions": {}}
        missing = Path("/tmp/lurker-owned/missing-primary.img")
        with mock.patch.object(SUITE.os.path, "ismount", return_value=False):
            suite.close(target=missing)
        self.assertEqual(suite.cli.call_args.args[0][-1], str(missing))
        self.assertFalse(suite.mounted)

    def test_failure_preservation_does_not_launch_cleanup_commands(self):
        suite = object.__new__(SUITE.MirrorSuite)
        suite.options = SimpleNamespace(preserve_failure=True)
        suite.mountpoint = Path("/tmp/lurker-owned/mounted")
        suite.other_mount = Path("/tmp/lurker-owned/other")
        suite.result = {"status": "running", "started": time.time(), "cases": []}
        suite.save = mock.Mock()
        attempted = []
        def fail_first(name, _function):
            attempted.append(name)
            suite.result["cases"].append({"status": "failed"})
            raise AssertionError("synthetic failure before storage")
        suite.case = fail_first
        with mock.patch.object(SUITE.os.path, "ismount", return_value=False):
            result = suite.run()
        self.assertEqual(attempted, ["quiet-native-window"])
        self.assertEqual(result["cleanup_policy"], "preserved-on-failure")
        self.assertEqual(result["status"], "failed")

    def test_invoking_owner_context_is_explicit_only_on_create(self):
        suite = object.__new__(SUITE.InvokingOwnerSuite)
        suite.options = SimpleNamespace(binary=Path("/owned/lurker"), size_gb="0.125")
        suite.owner = SimpleNamespace(pw_uid=1300, pw_gid=1300)
        suite.source = Path("/tmp/lurker-owned/primary.img")
        suite.peer = Path("/tmp/lurker-owned/peer.img")
        create = suite.create_args()
        self.assertEqual(create[:3], ["/usr/bin/env", "SUDO_UID=1300", "SUDO_GID=1300"])
        self.assertIn("--member", create)
        mount = suite.command("mount", mountpoint=Path("/tmp/lurker-owned/mnt"))
        self.assertNotIn("/usr/bin/env", mount)
        self.assertFalse(any("SUDO_UID" in argument for argument in mount))
        compile(SUITE.USER_WORKLOAD, "normal-user-workload.py", "exec")

    def test_firefox_requires_the_invoking_owner_fixture(self):
        with tempfile.TemporaryDirectory(prefix="lurker-raid1c-browser-", dir="/tmp") as directory:
            binary = Path(directory) / "executable"
            binary.write_text("#!/bin/sh\nexit 0\n")
            binary.chmod(0o700)
            args = ["--binary", str(binary), "--commit", "a" * 40, "--firefox", str(binary)]
            with mock.patch.object(SUITE.platform, "system", return_value="OpenBSD"), \
                 mock.patch.object(SUITE.os, "geteuid", return_value=0), \
                 mock.patch.object(SUITE, "private_binary"):
                with self.assertRaises(SystemExit):
                    SUITE.parse_args(args)
                options = SUITE.parse_args([*args, "--owner-user", "fixture"])
                self.assertEqual(options.firefox, binary)

    def test_retained_header_capture_is_bounded_and_preserves_conflicting_roles(self):
        with tempfile.TemporaryDirectory(prefix="lurker-raid1c-header-", dir="/tmp") as directory:
            path = Path(directory) / "member.img"
            value = bytearray(8192 + 168)
            struct.pack_into("=Q", value, 8192, 0x4d4152436372616d)
            for offset, number in [(8, 6), (32, 2), (36, 0), (52, 0x1c)]:
                struct.pack_into("=I", value, 8192 + offset, number)
            struct.pack_into("=Q", value, 8192 + 152, 12)
            path.write_bytes(value)
            before = path.read_bytes()
            header = SUITE.native_headers([path])[path.name]
            self.assertEqual(header["member_index"], 0)
            self.assertEqual(header["generation"], 12)
            self.assertEqual(path.read_bytes(), before)
            path.write_bytes(b"truncated")
            with self.assertRaises(AssertionError): SUITE.native_headers([path])

    def test_long_encrypted_parent_does_not_lengthen_native_mountpoint(self):
        root = Path("/tmp/lurker-conformance-example")
        parent = SUITE.member_parent(root, True)
        self.assertEqual(len(parent.name), 120)
        self.assertTrue(parent.is_relative_to(root))
        self.assertGreater(len(str(parent / "primary.img")), 90)
        self.assertLess(len(str(root / "mount with spaces")), 90)
        self.assertEqual(SUITE.member_parent(root, False), root)

    def test_argument_validation_blocks_storage_runs_on_development_host(self):
        with mock.patch.object(SUITE.platform, "system", return_value="Linux"), \
             mock.patch.object(SUITE.os, "geteuid", return_value=0), \
             self.assertRaises(SystemExit) as error:
            SUITE.parse_args(["--binary", "/unrelated/lurker", "--commit", "0" * 40])
        self.assertEqual(error.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
