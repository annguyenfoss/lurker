"""Conformance harness invariants; no real encryption, mount or device operation."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("backend_conformance", ROOT / "scripts/test-backends.py")
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)


class HarnessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lurker-harness-unit-", dir="/tmp")
        self.root = Path(self.temp.name)
        self.binary = self.root / "fake-cli"
        self.binary.write_text("#!/usr/bin/env python3\nimport sys\nsys.exit(0)\n")
        self.binary.chmod(0o700)

    def tearDown(self):
        self.temp.cleanup()

    def options(self, *extras):
        with mock.patch.object(harness.os, "geteuid", return_value=1000):
            return harness.parse_args(["--binary", str(self.binary), "--backend", "gocryptfs", "--root", str(self.root), *extras])

    def test_explicit_artifact_and_backend_are_mandatory(self):
        with self.assertRaises(SystemExit):
            harness.parse_args([])
        with self.assertRaises(SystemExit):
            harness.parse_args(["--binary", "/missing/cli", "--backend", "geli"])
        with self.assertRaises(SystemExit):
            self.options("--cipher", "aes")
        with self.assertRaises(SystemExit):
            self.options("--skip-readonly", "  ")

    def test_allocates_private_child_and_never_reuses_supplied_parent(self):
        sentinel = self.root / "user-data"
        sentinel.write_text("preserve")
        suite = harness.Suite(self.options())
        self.assertEqual(suite.root.parent, self.root)
        self.assertNotEqual(suite.root, self.root)
        self.assertEqual(suite.root.stat().st_mode & 0o777, 0o700)
        self.assertFalse(suite.source.exists())
        self.assertEqual(sentinel.read_text(), "preserve")
        record = json.loads((suite.root / "result.json").read_text())
        self.assertNotIn(suite.password, json.dumps(record))
        self.assertEqual(record["binary_sha256"], harness.file_hash(self.binary))

    def test_failed_stage_stops_and_retains_evidence(self):
        suite = harness.Suite(self.options())
        with mock.patch.object(suite, "create", side_effect=RuntimeError("backend unavailable")), mock.patch.object(suite, "mount") as mount:
            result = suite.run()
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["counts"], {"passed": 0, "failed": 1, "skipped": 0})
        self.assertTrue(result["retained"])
        mount.assert_not_called()
        self.assertTrue((suite.root / "result.json").exists())

    def test_successful_cli_without_mount_is_not_a_pass(self):
        suite = harness.Suite(self.options())
        suite.result["cases"].append({"name": "unlock", "status": "running"})
        with mock.patch.object(suite, "cli"), mock.patch.object(harness.os.path, "ismount", return_value=False):
            with self.assertRaisesRegex(AssertionError, "without a mount"):
                suite.mount()

    def test_rejected_create_must_preserve_inode_and_cgd_metadata(self):
        suite = harness.Suite(self.options())
        suite.source.write_bytes(b"existing-image")
        params = suite.source.with_name(suite.source.name + ".cgdparams")
        params.write_bytes(b"necessary-recovery-metadata")
        before = harness.snapshot(suite.source)
        params.write_bytes(b"changed")
        self.assertNotEqual(before, harness.snapshot(suite.source))
        def replacement(*_args, **_kwargs):
            suite.source.unlink()
            suite.source.write_bytes(b"changed")
        with mock.patch.object(suite, "cli", side_effect=replacement):
            with self.assertRaisesRegex(AssertionError, "modified"):
                suite.no_overwrite()

    def test_native_prompt_is_answered_without_echo_or_logged_secret(self):
        self.binary.write_text("#!/usr/bin/env python3\nimport getpass, sys\na = getpass.getpass('Passphrase: ')\nb = getpass.getpass('Confirm passphrase: ')\nsys.exit(0 if a == b and a.startswith('  ') and a.endswith('  ') else 1)\n")
        secret = "  密碼 secret  "
        log = self.root / "terminal.log"
        result = harness.terminal([str(self.binary)], secret, 5, log)
        self.assertEqual(result["exit_code"], 0)
        self.assertEqual(result["prompts"], 2)
        self.assertNotIn(secret, log.read_text())

    def test_echoed_credential_fails_and_is_redacted(self):
        self.binary.write_text("#!/usr/bin/env python3\nimport getpass\nprint(getpass.getpass('Password: '))\n")
        secret = "must-never-be-logged"
        log = self.root / "echo.log"
        with self.assertRaisesRegex(RuntimeError, "exposed a credential"):
            harness.terminal([str(self.binary)], secret, 5, log)
        self.assertNotIn(secret, log.read_text())
        self.assertIn("[REDACTED]", log.read_text())

    def test_unexpected_sudo_prompt_never_receives_vault_secret(self):
        self.binary.write_text("#!/usr/bin/env python3\nimport getpass\ngetpass.getpass('[sudo] password for user: ')\n")
        log = self.root / "sudo.log"
        with self.assertRaisesRegex(RuntimeError, "Unexpected authorization"):
            harness.terminal([str(self.binary)], "vault-secret", 5, log)
        self.assertNotIn("vault-secret", log.read_text())

    def test_timeout_is_recorded_and_only_owned_child_is_stopped(self):
        self.binary.write_text("#!/usr/bin/env python3\nimport time\ntime.sleep(10)\n")
        log = self.root / "timeout.log"
        result = harness.terminal([str(self.binary)], None, 0.1, log)
        self.assertTrue(result["timed_out"])
        self.assertNotEqual(result["exit_code"], 0)

    def test_readonly_negative_mode_requires_actual_request_and_rejection(self):
        suite = harness.Suite(self.options("--expect-readonly-unsupported"))
        suite.result["cases"].append({"name": "readonly", "status": "running"})
        with mock.patch.object(harness, "terminal", return_value={"exit_code": 1, "prompts": 0, "timed_out": False}) as terminal, \
             mock.patch.object(harness.os.path, "ismount", return_value=False):
            suite.readonly_unsupported()
        self.assertIn("--readonly", terminal.call_args.args[0])
        self.assertEqual(suite.result["readonly_expectation"], "unsupported")
        self.assertEqual(suite.result["cases"][-1]["commands"][0]["exit_code"], 1)

    def test_readonly_negative_mode_fails_on_successful_mount_command(self):
        suite = harness.Suite(self.options("--expect-readonly-unsupported"))
        suite.result["cases"].append({"name": "readonly", "status": "running"})
        with mock.patch.object(harness, "terminal", return_value={"exit_code": 0, "prompts": 0, "timed_out": False}), \
             mock.patch.object(harness.os.path, "ismount", return_value=False):
            with self.assertRaisesRegex(AssertionError, "Expected failure"):
                suite.readonly_unsupported()

    def test_readonly_negative_mode_rejects_attached_resource_even_on_error(self):
        suite = harness.Suite(self.options("--expect-readonly-unsupported"))
        suite.result["cases"].append({"name": "readonly", "status": "running"})
        with mock.patch.object(harness, "terminal", return_value={"exit_code": 1, "prompts": 0, "timed_out": False}), \
             mock.patch.object(harness.os.path, "ismount", return_value=True):
            with self.assertRaisesRegex(AssertionError, "left storage attached"):
                suite.readonly_unsupported()

    def test_readonly_skip_and_negative_mode_are_mutually_exclusive(self):
        with self.assertRaises(SystemExit):
            self.options("--skip-readonly", "native limitation", "--expect-readonly-unsupported")

    def test_explicit_skip_is_retained_and_not_counted_as_passed(self):
        suite = harness.Suite(self.options())
        suite.case("readonly", skip="native API does not support read-only attach")
        case = suite.result["cases"][-1]
        self.assertEqual(case["status"], "skipped")
        self.assertIn("native API", case["reason"])


if __name__ == "__main__":
    unittest.main()
