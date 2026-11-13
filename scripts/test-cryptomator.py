#!/usr/bin/env python3
"""Opt-in packaged-backend smoke test. Run on a dedicated Linux test guest only."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import tempfile
import time
from urllib.parse import quote


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--runtime-tests", action="store_true", required=True)
    args = parser.parse_args()
    if os.geteuid() == 0 or os.uname().sysname != "Linux":
        raise SystemExit("This smoke suite requires a normal user on a dedicated Linux test guest")
    if os.uname().nodename.split(".")[0] == "endeavour":
        raise SystemExit("Runtime encryption testing is disabled on endeavour")
    backend = str(args.backend.resolve(strict=True))
    root = Path(tempfile.mkdtemp(prefix="lurker-cryptomator-test-", dir="/tmp"))
    root.chmod(0o700)
    vault, mountpoint = root / "vault ü", root / "mount ü"
    vault.mkdir(mode=0o700)
    mountpoint.mkdir(mode=0o700)
    password = "  temporary 密碼  \n".encode()
    passed: list[str] = []
    process = None

    def mount(readonly=False):
        command = [backend, "mount", str(vault), str(mountpoint)] + (["--readonly"] if readonly else [])
        child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=open(root / "stdout.log", "ab"), stderr=open(root / "stderr.log", "ab"))
        child.stdin.write(password)
        child.stdin.close()
        for _ in range(200):
            if os.path.ismount(mountpoint):
                return child
            if child.poll() is not None:
                raise RuntimeError("Mount exited; inspect retained stderr.log")
            time.sleep(.1)
        # Never terminate a child that might still own a mount.
        if not os.path.ismount(mountpoint):
            child.terminate()
            child.wait(timeout=15)
        raise RuntimeError("Mount readiness timed out")

    def unmount(child):
        result = subprocess.run(["fusermount3", "-u", str(mountpoint)], capture_output=True, timeout=20)
        if result.returncode != 0 or os.path.ismount(mountpoint):
            raise RuntimeError("Graceful unmount failed; process and evidence retained")
        child.send_signal(signal.SIGTERM)
        child.wait(timeout=20)

    try:
        result = subprocess.run([backend, "init", str(vault)], input=password, capture_output=True, timeout=60)
        if result.returncode:
            raise RuntimeError("Initialization failed: " + result.stderr.decode(errors="replace")[:1000])
        passed.append("create")
        result = subprocess.run([backend, "init", str(vault)], input=password, capture_output=True, timeout=20)
        assert result.returncode != 0
        passed.append("existing-vault-protected")
        process = mount()
        passed.append("mount")
        target = mountpoint / "data ü.bin"
        with target.open("wb") as output:
            output.write(bytes(range(256)) * 1000)
            output.flush()
            os.fsync(output.fileno())
        target.rename(mountpoint / "renamed ü.bin")
        database = sqlite3.connect(mountpoint / "state.sqlite3")
        assert database.execute("PRAGMA journal_mode=WAL").fetchone()[0] == "wal"
        database.execute("CREATE TABLE state (value TEXT)")
        database.execute("INSERT INTO state VALUES ('persistent')")
        database.commit()
        database.close()
        passed.extend(["binary-write-fsync-rename", "sqlite-wal"])
        unmount(process)
        process = None
        passed.append("graceful-close")
        result = subprocess.run([backend, "mount", str(vault), str(mountpoint)], input=b"wrong\n", capture_output=True, timeout=20)
        assert result.returncode != 0 and not os.path.ismount(mountpoint)
        passed.append("incorrect-password")
        process = mount()
        database = sqlite3.connect(mountpoint / "state.sqlite3")
        assert database.execute("SELECT value FROM state").fetchone()[0] == "persistent"
        database.execute("INSERT INTO state VALUES ('reopened')")
        database.commit()
        database.close()
        passed.append("sqlite-wal-reopen")
        unmount(process)
        process = None
        process = mount(readonly=True)
        assert (mountpoint / "renamed ü.bin").read_bytes() == bytes(range(256)) * 1000
        # SQLite WAL normally creates shared-memory files on open. Immutable is
        # safe here because the previous connection closed and checkpointed and
        # this test owns the only mount, which is now read-only at both layers.
        database = sqlite3.connect(f"file:{quote(str(mountpoint / 'state.sqlite3'))}?mode=ro&immutable=1", uri=True)
        assert database.execute("SELECT value FROM state").fetchone()[0] == "persistent"
        database.close()
        passed.append("reopen-persistence")
        try:
            (mountpoint / "forbidden").write_text("no")
        except OSError:
            passed.append("readonly")
        else:
            raise AssertionError("Read-only mount accepted a write")
        unmount(process)
        process = None
        result = {"status": "pass", "passed": passed, "evidence": str(root)}
    except Exception as error:
        result = {"status": "fail", "passed": passed, "error": str(error), "evidence": str(root)}
    finally:
        if process is not None and not os.path.ismount(mountpoint):
            process.terminate()
            process.wait(timeout=15)
    (root / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    raise SystemExit(0 if result["status"] == "pass" else 1)


if __name__ == "__main__":
    main()
