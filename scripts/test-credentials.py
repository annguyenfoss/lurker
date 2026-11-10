#!/usr/bin/env python3
"""Native CLI/helper credential checks on disposable LUKS headers; no root needed.

Build first: cargo build -p lurker-cli -p lurker-helper
Run: python scripts/test-credentials.py target/debug
"""
import argparse
import errno
import json
import os
from pathlib import Path
import pty
import select
import subprocess
import tempfile
import time


def command(args, data=b"", ok=True):
    result = subprocess.run(args, input=data, capture_output=True, timeout=180)
    assert (result.returncode == 0) == ok, (args, result.stdout, result.stderr)
    return result


def terminal(args, prompts, ok=True):
    pid, master = pty.fork()
    if pid == 0:
        os.execv(args[0], args)
    transcript = bytearray()
    pending = list(prompts)
    deadline = time.monotonic() + 180
    finished = False
    eof = False
    try:
        while time.monotonic() < deadline:
            if eof:
                # A closed PTY can precede the child's waitable exit status.
                time.sleep(0.01)
                ready = []
            else:
                ready, _, _ = select.select([master], [], [], 0.2)
            if ready:
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    chunk = b""
                if not chunk:
                    eof = True
                else:
                    transcript.extend(chunk)
                    if pending and pending[0][0].encode() in transcript:
                        _, answer = pending.pop(0)
                        os.write(master, answer.encode() + b"\n")
            child, status = os.waitpid(pid, os.WNOHANG)
            if child:
                finished = True
                assert os.waitstatus_to_exitcode(status) == (0 if ok else 1), transcript
                break
        if not finished:
            child, status = os.waitpid(pid, os.WNOHANG)
            if not child:
                raise AssertionError("CLI did not complete before its deadline")
            finished = True
            assert os.waitstatus_to_exitcode(status) == (0 if ok else 1), transcript
        assert not pending, ("CLI did not request all expected inputs", pending, transcript)
        for _, secret in prompts:
            if secret:
                assert secret.encode() not in transcript, "CLI echoed a password"
        return bytes(transcript)
    finally:
        if not finished:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        os.close(master)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binaries", nargs="?", default="target/debug")
    parser.add_argument("--prompt-only", action="store_true", help="Check fast paste, empty input and cancellation without enrolling credentials")
    options = parser.parse_args()
    binaries = Path(options.binaries).resolve()
    cli = str(binaries / "lurker")
    helper = str(binaries / "lurker-helper")
    old = "  CLI original 密碼  "
    new = "  CLI new password 密碼  "
    third = "  CLI third password 密碼  "
    with tempfile.TemporaryDirectory(prefix="lurker-credential-frontends-") as directory:
        root = Path(directory)
        image = root / "vault with spaces.img"
        with image.open("wb") as file:
            file.truncate(32 * 1024 * 1024)
        key = root / "key, with spaces.bin"
        second_key = root / "second key.bin"
        key.write_bytes(os.urandom(256))
        second_key.write_bytes(os.urandom(256))
        os.chmod(key, 0o600)
        os.chmod(second_key, 0o600)
        command(["cryptsetup", "luksFormat", "--type", "luks2", "--pbkdf", "pbkdf2", "--pbkdf-force-iterations", "1000", "--batch-mode", str(image), "-"], old.encode())
        before = image.read_bytes()
        # Paste immediately on prompt arrival, repeatedly, to expose echo races.
        for _ in range(20):
            terminal([cli, "add-key", str(image)], [("Existing password:", old), ("New password:", new), ("Confirm new password:", "mismatch")], ok=False)
        terminal([cli, "add-key", str(image)], [("Existing password:", "")], ok=False)
        terminal([cli, "add-key", str(image)], [("Existing password:", "\x03")], ok=False)
        assert image.read_bytes() == before
        if options.prompt_only:
            print("Fast-paste echo protection, empty input and cancellation: passed")
            return
        terminal([cli, "add-key", str(image), "--slot", "1", "--new-key-file", str(key)], [("Existing password:", old)])
        before = image.read_bytes()
        terminal([cli, "add-key", str(image), "--slot", "2", "--key-file", str(key)], [("New password:", new), ("Confirm new password:", "mismatch")], ok=False)
        assert image.read_bytes() == before
        terminal([cli, "add-key", str(image), "--slot", "2", "--key-file", str(key)], [("New password:", new), ("Confirm new password:", new)])
        terminal([cli, "add-key", str(image), "--slot", "3"], [("Existing password:", old), ("New password:", third), ("Confirm new password:", third)])
        for slot, password in [(2, new), (3, third)]:
            command(["cryptsetup", "open", "--test-passphrase", "--key-file", "-", "--key-slot", str(slot), str(image)], password.encode())
        listing = command([cli, "keyslots", str(image)]).stdout
        assert b"0, 1, 2, 3" in listing, listing
        command([cli, "remove-key", str(image), "--slot", "0", "--key-file", str(key)])

        def request(operation, **kwargs):
            payload = dict(action="keyslots", source=str(image), operation=operation, **kwargs)
            # This internal flag skips helper re-exec. File-header operations still
            # use the caller's real uid and require no administrator privileges.
            result = command([helper, "--as-root", "run"], json.dumps(payload).encode())
            response = json.loads(result.stdout)
            for secret in [old, new, third]:
                assert secret not in result.stdout.decode(), "Helper logged a password"
            return response

        capabilities = json.loads(command([helper, "capabilities"]).stdout)
        assert capabilities["keyfile_authentication"] and capabilities["luks_keyslots"]
        assert request("add", keyfile=str(key), new_keyfile=str(second_key))["ok"]
        assert "0, 1, 2, 3" in str(request("list"))
        assert request("remove", slot=0, passphrase=third)["ok"]
        before = image.read_bytes()
        assert not request("add", passphrase=old, keyfile=str(key), new_keyfile=str(second_key))["ok"]
        assert not request("add", keyfile=str(key), new_passphrase=new, new_keyfile=str(second_key))["ok"]
        assert image.read_bytes() == before
        result = command([cli, "add-key", str(image)], ok=False)
        assert b"terminal" in result.stderr
        assert image.read_bytes() == before
    print("CLI prompts, confirmation, password/keyfile slots and helper JSON: passed")


if __name__ == "__main__":
    main()
