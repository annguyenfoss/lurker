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

    def test_root_firefox_requires_a_login_identity_and_native_backend(self):
        args = ["--binary", str(self.binary), "--firefox", str(self.binary), "--backend", "luks"]
        account = mock.Mock(pw_uid=1300, pw_gid=1400)
        with mock.patch.object(harness.os, "geteuid", return_value=0), \
             mock.patch.object(harness.platform, "system", return_value="Linux"), \
             mock.patch.object(harness.pwd, "getpwuid", return_value=account) as lookup:
            for denied in ({}, {"SUDO_UID": "0"}, {"SUDO_UID": "+1300"}):
                with mock.patch.dict(os.environ, denied, clear=True), self.assertRaises(SystemExit):
                    harness.parse_args(args)
            with mock.patch.dict(os.environ, {"SUDO_UID": "1300"}, clear=True):
                self.assertIs(harness.parse_args(args).firefox_account, account)
                lookup.assert_called_once_with(1300)
                with self.assertRaises(SystemExit):
                    harness.parse_args([*args[:-1], "gocryptfs"])

    def test_root_firefox_routes_native_bsd_backends_to_the_doas_user(self):
        args = ["--binary", str(self.binary), "--firefox", str(self.binary), "--backend"]
        account = mock.Mock(pw_uid=1300, pw_gid=1400)
        routes = {"FreeBSD": "geli", "OpenBSD": "softraid", "NetBSD": "cgd", "DragonFly": "luks1"}
        with mock.patch.object(harness.os, "geteuid", return_value=0), \
             mock.patch.object(harness.pwd, "getpwnam", return_value=account), \
             mock.patch.dict(os.environ, {"DOAS_USER": "fixture"}, clear=True):
            for system, native in routes.items():
                with self.subTest(system=system), mock.patch.object(harness.platform, "system", return_value=system):
                    self.assertIs(harness.parse_args([*args, native]).firefox_account, account)
                    with self.assertRaises(SystemExit):
                        harness.parse_args([*args, "luks"])

    def test_explicit_artifact_and_backend_are_mandatory(self):
        with self.assertRaises(SystemExit):
            harness.parse_args([])
        with self.assertRaises(SystemExit):
            harness.parse_args(["--binary", "/missing/cli", "--backend", "geli"])
        with self.assertRaises(SystemExit):
            self.options("--cipher", "aes")
        with self.assertRaises(SystemExit):
            self.options("--skip-readonly", "  ")

    def test_full_storage_requires_a_bounded_file_container(self):
        with self.assertRaises(SystemExit):
            self.options("--full-storage")
        args = ["--binary", str(self.binary), "--backend", "luks", "--full-storage"]
        with mock.patch.object(harness.os, "geteuid", return_value=0), \
             mock.patch.object(harness.platform, "system", return_value="Linux"):
            for size in ("nan", "2", "0", "-0.125"):
                with self.subTest(size=size), self.assertRaises(SystemExit):
                    harness.parse_args([*args, "--size-gb", size])
            self.assertTrue(harness.parse_args([*args, "--size-gb", "0.125"]).full_storage)

    def test_full_storage_never_fills_the_fixture_parent_filesystem(self):
        suite = harness.Suite(self.options())
        with mock.patch.object(harness.os.path, "ismount", return_value=True), \
             mock.patch.object(harness, "space_bytes", return_value={"total": harness.MIB}), \
             mock.patch.object(harness, "fill_to_enospc") as fill:
            with self.assertRaisesRegex(AssertionError, "separate, bounded owned mount"):
                suite.exhaust_storage()
        fill.assert_not_called()
        self.assertEqual(list(suite.mountpoint.iterdir()), [])

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
        self.binary.write_text("""#!/usr/bin/env python3
import getpass
import sys

a = getpass.getpass('Enter passphrase for /tmp/my vault:disk: ')
b = getpass.getpass('Confirm passphrase: ')
sys.exit(0 if a == b and a.startswith('  ') and a.endswith('  ') else 1)
""")
        secret = "  密碼 secret  "
        log = self.root / "terminal.log"
        result = harness.terminal([str(self.binary)], secret, 5, log)
        self.assertEqual(result["exit_code"], 0)
        self.assertEqual(result["prompts"], 2)
        self.assertNotIn(secret, log.read_text())

    def test_diagnostic_with_password_in_a_path_is_not_a_prompt(self):
        diagnostic = "Error: mount /tmp/veracrypt-aes-password/mount: fixture unavailable"
        self.binary.write_text(f"#!/usr/bin/env python3\nimport sys\nprint({diagnostic!r})\nsys.exit(1)\n")
        log = self.root / "password-in-path.log"
        result = harness.terminal([str(self.binary)], None, 5, log)
        self.assertEqual(result["exit_code"], 1)
        self.assertEqual(result["prompts"], 0)
        self.assertIn(diagnostic, log.read_text())

    def test_prompt_before_delayed_noecho_and_flush_preserves_both_inputs(self):
        self.binary.write_text("""#!/usr/bin/env python3
import sys, termios, time
fd = sys.stdin.fileno()
original = termios.tcgetattr(fd)
answers = []
for prompt in ['Passphrase: ', 'Verify passphrase: ']:
    termios.tcsetattr(fd, termios.TCSANOW, original)
    sys.stdout.write(prompt)
    sys.stdout.flush()
    time.sleep(0.08)
    hidden = termios.tcgetattr(fd)
    hidden[3] &= ~termios.ECHO
    termios.tcsetattr(fd, termios.TCSAFLUSH, hidden)
    answers.append(sys.stdin.readline().rstrip('\\n'))
termios.tcsetattr(fd, termios.TCSANOW, original)
sys.exit(0 if answers == ['  密碼 delayed secret  '] * 2 else 1)
""")
        secret = "  密碼 delayed secret  "
        log = self.root / "delayed-echo.log"
        result = harness.terminal([str(self.binary)], secret, 3, log)
        self.assertEqual(result["exit_code"], 0)
        self.assertEqual(result["prompts"], 2)
        self.assertFalse(result["timed_out"])
        self.assertNotIn(secret, log.read_text())

    def test_prompt_that_never_disables_echo_fails_without_sending_secret(self):
        self.binary.write_text("#!/usr/bin/env python3\nimport sys, time\nsys.stdout.write('Password: '); sys.stdout.flush()\ntime.sleep(10)\n")
        log = self.root / "never-noecho.log"
        with mock.patch.object(harness, "ECHO_WAIT_TIMEOUT", 0.1):
            with self.assertRaisesRegex(RuntimeError, "never disabled terminal echo"):
                harness.terminal([str(self.binary)], "must-not-be-sent", 2, log)
        self.assertNotIn("must-not-be-sent", log.read_text())

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

    def test_alias_duplicate_preserves_cgd_parameters_and_checks_real_mount(self):
        suite = harness.Suite(self.options())
        suite.source.write_bytes(b"vault")
        params = suite.source.with_name(suite.source.name + ".cgdparams")
        params.write_bytes(b"required parameters")
        with mock.patch.object(suite, "cli") as cli, \
             mock.patch.object(suite, "verify"), \
             mock.patch.object(harness.os.path, "ismount", return_value=False):
            suite.aliased_duplicate()
        alias = suite.root / "source alias"
        self.assertEqual(alias.stat().st_ino, suite.source.stat().st_ino)
        self.assertEqual(alias.with_name(alias.name + ".cgdparams").stat().st_ino, params.stat().st_ino)
        self.assertIn(str(alias), cli.call_args.args[0])
        self.assertFalse(cli.call_args.kwargs["expected"])

    def test_alias_duplicate_never_treats_attached_storage_as_a_pass(self):
        suite = harness.Suite(self.options())
        suite.source.mkdir()
        with mock.patch.object(suite, "cli"), \
             mock.patch.object(harness.os.path, "ismount", return_value=True):
            with self.assertRaisesRegex(AssertionError, "bypassed duplicate"):
                suite.aliased_duplicate()

    def test_auto_mode_keeps_create_explicit_and_routes_mounts_through_auto(self):
        suite = harness.Suite(self.options("--auto-detect"))
        create = suite.create_args()
        self.assertEqual(create[create.index("--type") + 1], "gocryptfs")
        for action in ["mount", "unmount"]:
            args = suite.args(action, "/owned/path")
            self.assertEqual(args[args.index("--type") + 1], "auto")
        self.assertEqual(suite.result["detection_mode"], "auto")

    def test_default_mode_still_requests_the_selected_format_explicitly(self):
        suite = harness.Suite(self.options())
        for action in ["create", "mount", "unmount"]:
            args = suite.args(action, "/owned/path")
            self.assertEqual(args[args.index("--type") + 1], "gocryptfs")
        self.assertEqual(suite.result["detection_mode"], "explicit")

    def test_profile_verification_closes_sqlite_before_unmount(self):
        suite = harness.Suite(self.options())
        (suite.mountpoint / "profile 密碼.bin").write_bytes(suite.payload)
        (suite.mountpoint / "atomic.data").write_bytes(b"initial")
        connection = mock.Mock()
        connection.execute.return_value.fetchone.side_effect = [("ok",), (suite.payload,)]
        with mock.patch.object(harness.sqlite3, "connect", return_value=connection):
            suite.verify()
        connection.close.assert_called_once_with()

    def test_failed_profile_verification_also_closes_sqlite(self):
        suite = harness.Suite(self.options())
        (suite.mountpoint / "profile 密碼.bin").write_bytes(suite.payload)
        connection = mock.Mock()
        connection.execute.return_value.fetchone.return_value = ("corrupt",)
        with mock.patch.object(harness.sqlite3, "connect", return_value=connection):
            with self.assertRaisesRegex(AssertionError, "integrity"):
                suite.verify()
        connection.close.assert_called_once_with()

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
