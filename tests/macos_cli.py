#!/usr/bin/env python3
"""Native CLI password-prompt smoke test. Build lurker-cli first; macOS only."""
import os
from pathlib import Path
import pty
import select
import signal
import subprocess
import sys
import tempfile
import termios
import time

binary = Path(__file__).resolve().parents[1] / "target/debug/lurker"
password = b"CLI test password with spaces"


def invoke(arguments, prompts=(), succeeds=True):
    pid, master = pty.fork()
    if pid == 0:
        os.execv(str(binary), [str(binary), *map(str, arguments)])
    output = b""
    next_prompt = 0
    deadline = time.monotonic() + 60
    try:
        while time.monotonic() < deadline:
            if not select.select([master], [], [], .1)[0]:
                continue
            try:
                data = os.read(master, 65536)
            except OSError:
                break
            if not data:
                break
            output += data
            if next_prompt < len(prompts):
                prompt, reply = prompts[next_prompt]
                if prompt in output:
                    # rpassword prints the prompt just before switching tty modes.
                    # Wait for echo to be disabled as a terminal user would.
                    while termios.tcgetattr(master)[3] & termios.ECHO:
                        if time.monotonic() >= deadline:
                            raise AssertionError("password input did not disable echo")
                        time.sleep(.005)
                    os.write(master, reply + b"\n")
                    next_prompt += 1
        done, status = os.waitpid(pid, os.WNOHANG)
        while not done and time.monotonic() < deadline:
            time.sleep(.01)
            done, status = os.waitpid(pid, os.WNOHANG)
        if not done:
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
            raise AssertionError("CLI command did not finish: " + output.decode(errors="replace"))
        assert os.waitstatus_to_exitcode(status) == (0 if succeeds else 1), output.decode(errors="replace")
        assert next_prompt == len(prompts), output
        assert password not in output, "password was echoed"
        return output
    finally:
        os.close(master)


def main():
    if sys.platform != "darwin":
        raise SystemExit("macOS only")
    with tempfile.TemporaryDirectory(prefix="lurker-cli-test-") as directory:
        root = Path(directory)
        source = root / "vault with spaces.dmg"
        mount = root / "new mount directory"
        prompts = [(b"Passphrase: ", password), (b"Confirm passphrase: ", password)]
        invoke(["create", source, "0.03125"], prompts)
        try:
            invoke(["mount", source, mount], [(b"Passphrase: ", password)])
            (mount / "saved.txt").write_text("CLI persistence")
            invoke(["unmount", source])
            invoke(["mount", "--type", "apple", source, mount], [(b"Passphrase: ", password)])
            assert (mount / "saved.txt").read_text() == "CLI persistence"
            invoke(["unmount", mount])
            invoke(["create", root / "mismatch.dmg", "0.03125"], [(b"Passphrase: ", password), (b"Confirm passphrase: ", b"different")], succeeds=False)
            assert not (root / "mismatch.dmg").exists()
            invoke(["create", "--type", "luks", root / "unsupported.img", "1"], succeeds=False)
            invoke(["create", "--cipher", "serpent", root / "unsupported.dmg", "1"], succeeds=False)
        finally:
            subprocess.run([str(binary), "unmount", str(source)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print("Native macOS CLI prompts, create, mount, persistence, unmount, and errors passed.")


if __name__ == "__main__":
    main()
