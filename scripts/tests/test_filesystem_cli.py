"""Regress terminal password handling with actual sudo control-sequence shapes."""
import importlib.util
from pathlib import Path
import sys
import tempfile
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


if __name__ == "__main__":
    unittest.main()
