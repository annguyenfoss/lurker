#!/usr/bin/env python3
"""Native CLI conformance on disposable /tmp vaults; never invoke on endeavour.

Requires a built Lurker CLI and its private backend directory. Prints one JSON
result per backend; keeps failed vaults for investigation, without forced detach.
"""
from __future__ import annotations

import argparse
import errno
import hashlib
import json
import os
from pathlib import Path
import pty
import platform
import select
import shutil
import sqlite3
import subprocess
import tempfile
import time


def terminal(args, secrets=(), ok=True, timeout=150):
    pid, master = pty.fork()
    if pid == 0:
        os.execv(args[0], args)
    transcript = bytearray()
    pending = list(secrets)
    handled = 0
    finished = False
    deadline = time.monotonic() + timeout
    try:
        while time.monotonic() < deadline:
            if select.select([master], [], [], 0.1)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    chunk = b""
                transcript.extend(chunk)
                # Each prompt has its own marker; do not match an earlier prompt.
                for marker in (b"Passphrase:", b"Confirm passphrase:"):
                    index = transcript.find(marker, handled)
                    if index >= 0 and pending:
                        handled = index + len(marker)
                        os.write(master, pending.pop(0).encode() + b"\n")
                        break
            child, status = os.waitpid(pid, os.WNOHANG)
            if child:
                finished = True
                code = os.waitstatus_to_exitcode(status)
                diagnostic = transcript.decode(errors="replace")
                for secret in secrets:
                    diagnostic = diagnostic.replace(secret, "<redacted>")
                assert (code == 0) == ok, f"{args[1]} exit={code}: {diagnostic[-2000:]}"
                assert not pending, "Expected credential prompts did not execute"
                for secret in secrets:
                    assert secret.encode() not in transcript, "Secret echoed to terminal"
                return bytes(transcript)
        raise AssertionError(f"{args[1]} timed out")
    finally:
        if not finished:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        os.close(master)


def check_backend(cli: str, kind: str, root: Path) -> int:
    vault, mountpoint = root / "cipher directory 密碼", root / "open directory 密碼"
    alias = root / "alias"
    secret = "  folder test 密碼  "
    terminal([cli, "create", "--type", kind, str(vault)], [secret, secret])
    terminal([cli, "create", "--type", kind, str(vault)], ok=False)
    terminal([cli, "mount", "--type", kind, str(vault), str(mountpoint)], ["wrong credential"], ok=False)
    terminal([cli, "mount", "--type", kind, str(vault), str(mountpoint)], [secret])
    alias.symlink_to(vault, target_is_directory=True)
    try:
        terminal([cli, "mount", "--type", kind, str(alias), str(root / "duplicate")], [secret], ok=False)
        payload = b"durable folder integration content\x00\xff\n"
        target = mountpoint / "profile data 密碼"
        with target.open("wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        database = sqlite3.connect(mountpoint / "firefox-like.sqlite")
        assert database.execute("PRAGMA journal_mode=WAL").fetchone()[0] == "wal"
        database.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)")
        database.execute("INSERT INTO test VALUES (1, 'durable WAL')")
        database.commit()
        database.close()
        temporary = mountpoint / "atomic.tmp"
        temporary.write_bytes(b"atomic rename")
        temporary.rename(mountpoint / "atomic")
        with target.open("rb"):
            terminal([cli, "unmount", str(mountpoint)], ok=False)
        terminal([cli, "unmount", str(mountpoint)])
        terminal([cli, "mount", str(vault), str(mountpoint)], [secret])
        assert target.read_bytes() == payload
        database = sqlite3.connect(mountpoint / "firefox-like.sqlite")
        assert database.execute("SELECT value FROM test").fetchone()[0] == "durable WAL"
        database.close()
        with target.open("ab") as stream:
            stream.write(b"appended after reopen")
            stream.flush()
            os.fsync(stream.fileno())
        terminal([cli, "unmount", str(vault)])
    except BaseException:
        # Best-effort ordinary unmount; preserve evidence if it is busy or fails.
        subprocess.run([cli, "unmount", str(mountpoint)], capture_output=True, timeout=30)
        raise
    return 12


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path)
    parser.add_argument("backends", nargs="+", choices=["gocryptfs", "cryfs", "cryptomator"])
    parser.add_argument("--source-commit", help="Verified deployment commit, supplied by the remote coordinator")
    args = parser.parse_args()
    cli = str(args.cli.resolve())
    identity = {"cli_sha256": hashlib.sha256(Path(cli).read_bytes()).hexdigest(),
                "source_commit": args.source_commit, "os": platform.system(),
                "release": platform.release(), "architecture": platform.machine()}
    if os.geteuid() == 0:
        parser.error("Run as the login user without sudo")
    failed = False
    for kind in args.backends:
        root = Path(tempfile.mkdtemp(prefix=f"lurker-folder-{kind}-", dir="/tmp"))
        started = time.monotonic()
        try:
            count = check_backend(cli, kind, root)
            shutil.rmtree(root)
            result = {"backend": kind, "status": "passed", "checks": count}
        except Exception as error:
            failed = True
            result = {"backend": kind, "status": "failed", "error": str(error)[:2048], "retained": str(root)}
        result["duration_seconds"] = round(time.monotonic() - started, 2)
        result.update(identity)
        print(json.dumps(result), flush=True)
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
