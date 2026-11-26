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


if __name__ == "__main__":
    unittest.main()
