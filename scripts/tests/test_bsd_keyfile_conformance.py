import importlib.util
import os
from pathlib import Path
import stat
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("bsd_keyfiles", Path(__file__).parents[1] / "test-bsd-keyfiles.py")
HARNESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HARNESS)


class BSDKeyfileSafetyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lurker-keyfile-selftest-")
        self.addCleanup(self.temporary.cleanup)
        self.parent = Path(self.temporary.name)

    def suite(self, backend="geli"):
        return HARNESS.BSDKeyfileSuite(SimpleNamespace(
            binary=Path(sys.executable).resolve(), backend=backend, root=str(self.parent),
            cipher="aes" if backend == "geli" else None, auto_detect=True, unprivileged=False,
            timeout=3, commit="a" * 40, expect_readonly_unsupported=False,
            skip_readonly=None, size_gb="0.125"))

    def arguments(self, *extra):
        return ["--binary", sys.executable, "--backend", "geli", "--commit", "a" * 40,
                "--root", str(self.parent), *extra]

    def test_fixture_is_new_private_child_and_keys_include_binary_delimiters(self):
        sentinel = self.parent / "user-data"
        sentinel.write_bytes(b"preserve")
        suite = self.suite()
        self.assertNotEqual(suite.root, self.parent)
        self.assertEqual(suite.root.parent, self.parent)
        self.assertEqual(suite.root.stat().st_mode & 0o777, 0o700)
        for key in [suite.keyfile, suite.wrong_keyfile]:
            self.assertEqual(key.stat().st_mode & 0o777, 0o600)
            self.assertEqual(key.read_bytes()[:4], b"\0\xff\n\r")
            self.assertEqual(len(key.read_bytes()), 256)
        self.assertNotEqual(suite.keyfile.read_bytes(), suite.wrong_keyfile.read_bytes())
        self.assertFalse(suite.source.exists())
        self.assertEqual(sentinel.read_bytes(), b"preserve")

    def test_keyfile_cli_never_supplies_password_or_accepts_password_prompt(self):
        suite = self.suite()
        suite.result["cases"].append({"name": "create"})
        outcome = {"exit_code": 0, "prompts": 0, "timed_out": False}
        with patch.object(HARNESS.BASE, "terminal", return_value=outcome) as terminal:
            suite.cli(suite.create_args(), password=suite.password)
            self.assertIsNone(terminal.call_args.args[1])
            self.assertEqual(terminal.call_args.args[0][-2:], ["--key-file", str(suite.keyfile)])
            self.assertNotIn(suite.password, terminal.call_args.args[0])
        with patch.object(HARNESS.BASE, "terminal", return_value={**outcome, "prompts": 1}):
            with self.assertRaisesRegex(AssertionError, "unexpectedly prompted"):
                suite.cli(suite.create_args())

    def test_wrong_key_is_distinct_and_rejection_must_preserve_source(self):
        suite = self.suite()
        suite.source.write_bytes(b"existing encrypted source")
        suite.result["cases"].append({"name": "wrong-keyfile"})
        outcome = {"exit_code": 1, "prompts": 0, "timed_out": False}
        with patch.object(suite, "footprint", return_value={}), patch.object(HARNESS.BASE, "terminal", return_value=outcome) as terminal:
            suite.incorrect()
            self.assertIsNone(terminal.call_args.args[1])
            self.assertEqual(terminal.call_args.args[0][-2:], ["--key-file", str(suite.wrong_keyfile)])
        def corrupt(*_args):
            suite.source.write_bytes(b"mutated source")
            return outcome
        with patch.object(suite, "footprint", return_value={}), patch.object(HARNESS.BASE, "terminal", side_effect=corrupt):
            with self.assertRaisesRegex(AssertionError, "changed encrypted bytes"):
                suite.incorrect()

    def test_rejected_cli_resource_leak_is_failure(self):
        suite = self.suite()
        suite.result["cases"].append({"name": "wrong-keyfile"})
        with patch.object(suite, "footprint", side_effect=[{"md": ""}, {"md": "md42"}]), patch.object(
                HARNESS.BASE, "terminal", return_value={"exit_code": 1, "prompts": 0, "timed_out": False}):
            with self.assertRaisesRegex(AssertionError, "changed native resources"):
                suite.cli(suite.args("mount", suite.source, suite.mountpoint), expected=False, password=suite.wrong)

    def test_publication_permissions_and_ownership_are_required(self):
        suite = self.suite()
        suite.source.write_bytes(b"image")
        suite.source.chmod(0o600)
        suite.private_owned(suite.source, 0o600)
        suite.source.chmod(0o644)
        with self.assertRaisesRegex(AssertionError, "private storage"):
            suite.private_owned(suite.source, 0o600)

    def test_binary_and_ancestors_require_root_control_except_sticky_tmp(self):
        binary = Path("/tmp/owned-runtime/bin/lurker")
        def trusted(path, **_kwargs):
            return SimpleNamespace(st_uid=0, st_mode=(stat.S_IFREG | 0o755) if path == binary
                                   else (stat.S_IFDIR | (0o1777 if path == Path("/tmp") else 0o755)))
        with patch.object(Path, "stat", trusted), patch.object(Path, "is_file", return_value=True), patch.object(HARNESS.os, "access", return_value=True):
            HARNESS.private_binary(binary)
        def untrusted(path, **kwargs):
            value = trusted(path, **kwargs)
            if path == binary.parent:
                value.st_uid = 1300
            return value
        with patch.object(Path, "stat", untrusted), patch.object(Path, "is_file", return_value=True), patch.object(HARNESS.os, "access", return_value=True):
            with self.assertRaisesRegex(ValueError, "administrator-controlled"):
                HARNESS.private_binary(binary)
        with patch.object(Path, "stat", return_value=SimpleNamespace(st_uid=0, st_mode=stat.S_IFREG | 0o777)):
            with self.assertRaisesRegex(ValueError, "administrator-controlled"):
                HARNESS.private_binary(binary)

    def test_no_overwrite_must_preserve_existing_encrypted_source(self):
        suite = self.suite()
        suite.source.write_bytes(b"existing encrypted source")
        suite.result["cases"].append({"name": "no-overwrite"})
        def corrupt(*_args):
            suite.source.write_bytes(b"changed")
            return {"exit_code": 1, "prompts": 0, "timed_out": False}
        with patch.object(suite, "footprint", return_value={}), patch.object(HARNESS.BASE, "terminal", side_effect=corrupt):
            with self.assertRaisesRegex(AssertionError, "modified the existing source"):
                suite.no_overwrite()

    def test_environment_overrides_and_synthetic_owners_are_removed(self):
        overrides = {"LURKER_BACKEND_DIR": "/unsafe", "LURKER_TEST_PLATFORM": "Linux",
                     "LD_PRELOAD": "/unsafe.so", "SUDO_UID": "1300", "DOAS_USER": "user"}
        with patch.dict(os.environ, overrides):
            HARNESS.clear_overrides()
            for name in overrides:
                self.assertNotIn(name, os.environ)

    def test_native_only_parser_rejects_other_platform_and_nonroot(self):
        for system, uid in [("Linux", 0), ("FreeBSD", 1300)]:
            with patch.object(HARNESS.platform, "system", return_value=system), patch.object(HARNESS.os, "geteuid", return_value=uid):
                with self.assertRaises(SystemExit):
                    HARNESS.parse_args(self.arguments())

    def test_parser_requires_exact_sha_and_private_tmp_parent(self):
        with patch.object(HARNESS.platform, "system", return_value="FreeBSD"), patch.object(
                HARNESS.os, "geteuid", return_value=0), patch.object(HARNESS, "private_binary"):
            for extra in [["--commit", "main"], ["--root", "/"], ["--timeout", "nan"], ["--size-gb", "NaN"], ["--size-gb", "100"]]:
                with self.assertRaises(SystemExit):
                    HARNESS.parse_args(self.arguments(*extra))
            options = HARNESS.parse_args(self.arguments())
        self.assertTrue(options.auto_detect)
        self.assertFalse(options.expect_readonly_unsupported)
        self.assertIsNone(options.skip_readonly)

    def test_failure_retains_evidence_and_does_not_try_future_storage_steps(self):
        suite = self.suite("pefs")
        with patch.object(suite, "create", side_effect=RuntimeError("missing module")), patch.object(suite, "mount") as mount:
            result = suite.run()
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["counts"], {"passed": 0, "failed": 1, "skipped": 0})
        self.assertTrue(result["retained"])
        self.assertTrue((suite.root / "result.json").exists())
        mount.assert_not_called()


if __name__ == "__main__":
    unittest.main()
