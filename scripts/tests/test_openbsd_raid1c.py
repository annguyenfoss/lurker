"""Pure safety checks for the opt-in CLI suite; no native storage operations."""
import importlib.util
from pathlib import Path
import unittest
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

    def test_argument_validation_blocks_storage_runs_on_development_host(self):
        with mock.patch.object(SUITE.platform, "system", return_value="Linux"), \
             mock.patch.object(SUITE.os, "geteuid", return_value=0), \
             self.assertRaises(SystemExit) as error:
            SUITE.parse_args(["--binary", "/unrelated/lurker", "--commit", "0" * 40])
        self.assertEqual(error.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
