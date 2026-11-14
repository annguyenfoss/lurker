import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("keyfile_conformance", Path(__file__).parents[1] / "test-veracrypt-keyfiles.py")
suite_module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(suite_module)


class KeyfileConformanceTests(unittest.TestCase):
    def suite(self, root):
        options = suite_module.conformance.parse_args([
            "--binary", sys.executable, "--backend", "veracrypt", "--root", str(root), "--cipher", "aes"])
        return suite_module.KeyfileSuite(options)

    def test_binary_keys_are_private_and_never_become_password_input(self):
        with tempfile.TemporaryDirectory() as root, patch.object(suite_module.conformance.platform, "system", return_value="Darwin"):
            suite = self.suite(root)
            self.assertEqual(suite.keyfile.stat().st_mode & 0o777, 0o600)
            self.assertEqual(suite.keyfile.read_bytes()[:2], b"\0\xff")
            self.assertNotEqual(suite.keyfile.read_bytes(), suite.wrong_keyfile.read_bytes())
            suite.result["cases"].append({"name": "create"})
            with patch.object(suite_module.conformance, "terminal", return_value={"exit_code": 0, "prompts": 0, "timed_out": False}) as terminal:
                suite.cli(suite.create_args(), password=suite.password)
            arguments = terminal.call_args.args
            self.assertIsNone(arguments[1])
            self.assertEqual(arguments[0][-2:], ["--key-file", str(suite.keyfile)])
            self.assertNotIn(suite.password, arguments[0])

    def test_wrong_credential_selects_the_wrong_key_without_sending_a_password(self):
        with tempfile.TemporaryDirectory() as root, patch.object(suite_module.conformance.platform, "system", return_value="Darwin"):
            suite = self.suite(root)
            suite.result["cases"].append({"name": "wrong-keyfile"})
            with patch.object(suite_module.conformance, "terminal", return_value={"exit_code": 1, "prompts": 0, "timed_out": False}) as terminal:
                suite.incorrect()
            self.assertIsNone(terminal.call_args.args[1])
            self.assertEqual(terminal.call_args.args[0][-2:], ["--key-file", str(suite.wrong_keyfile)])


if __name__ == "__main__":
    unittest.main()
