#!/usr/bin/env python3
"""Mounted CryFS fault qualification on an existing tiny private Linux tmpfs.

Never mounts filesystems, changes quotas, fills ordinary host storage, forces an
unmount, repairs data, or kills a backend. Retains every fixture and JSON result.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import ctypes
import errno
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import shlex
import sqlite3
import subprocess
import time

SPEC = importlib.util.spec_from_file_location("folder_cli_test", Path(__file__).with_name("test-folder-backends.py"))
CLI_TEST = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CLI_TEST)


def fsync(path):
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path)
    parser.add_argument("backend", type=Path)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("--fault", choices=["create", "overwrite", "rename", "close"], default="overwrite")
    parser.add_argument("--expected-error", choices=["ENOSPC", "EDQUOT"], default="ENOSPC")
    parser.add_argument("--runtime-tests", action="store_true", required=True)
    args = parser.parse_args()
    if platform.system() != "Linux" or os.geteuid() == 0 or platform.node().split(".")[0] == "endeavour":
        parser.error("Use a dedicated Linux native host as its ordinary user")
    root = args.fixture.resolve()
    assert str(root).startswith("/tmp/lurker-folder-fault-")
    metadata = root.stat()
    assert metadata.st_uid == os.geteuid() and not metadata.st_mode & 0o077
    backing = root / "backing"
    # statfs.f_type is the first native long; reserve ample space for the rest.
    fields = (ctypes.c_long * 64)()
    libc = ctypes.CDLL(None, use_errno=True)
    assert libc.statfs(os.fsencode(backing), ctypes.byref(fields)) == 0
    capacity = os.statvfs(backing)
    assert fields[0] == 0x01021994 and capacity.f_blocks * capacity.f_frsize <= 16 * 1024 * 1024, "Only a finite tmpfs <=16 MiB may be filled"
    assert backing.stat().st_dev != root.stat().st_dev, "Backing filesystem must be separately mounted"
    backend_dir = root / "backend"
    backend_dir.mkdir(mode=0o700)
    wrapper = backend_dir / "cryfs"
    wrapper.write_text("#!/bin/sh\nexec " + shlex.quote(str(args.backend.resolve())) + ' "$@" --logfile ' + shlex.quote(str(root / "backend.log")) + "\n")
    wrapper.chmod(0o700)
    os.environ["LURKER_BACKEND_DIR"] = str(backend_dir)
    cli = str(args.cli.resolve())
    vault, mountpoint = backing / "vault", root / "mount"
    secret = "Lurker finite-storage disposable regression only"
    result = {"schema": 1, "backend": "cryfs", "backend_sha256": hashlib.sha256(args.backend.read_bytes()).hexdigest(), "fixture": str(root), "capacity_bytes": capacity.f_blocks * capacity.f_frsize, "repair": False, "fault": args.fault}
    expected_errno = getattr(errno, args.expected_error)
    result["expected_errno"] = expected_errno
    phase = "create"
    try:
        CLI_TEST.terminal([cli, "create", "--type", "cryfs", str(vault)], [secret, secret])
        phase = "mount"
        CLI_TEST.terminal([cli, "mount", "--type", "cryfs", str(vault), str(mountpoint)], [secret])
        phase = "commit-baseline"
        payload = b"committed-before-storage-fault\x00" * 256
        (mountpoint / "committed-file").write_bytes(payload)
        fsync(mountpoint / "committed-file")
        with closing(sqlite3.connect(mountpoint / "committed.sqlite")) as db:
            assert db.execute("PRAGMA journal_mode=WAL").fetchone()[0] == "wal"
            db.execute("PRAGMA synchronous=FULL")
            db.execute("CREATE TABLE records (id INTEGER PRIMARY KEY, payload BLOB)")
            db.execute("INSERT INTO records VALUES (1, ?)", (payload,))
            db.commit()
        (mountpoint / "scratch").write_bytes(payload)
        fsync(mountpoint / "scratch")
        fsync(mountpoint / "committed.sqlite")
        fsync(mountpoint)
        phase = "fill-finite-backing"
        filler = backing / "owned-space-filler"
        with filler.open("xb", buffering=0) as stream:
            try:
                while True:
                    stream.write(b"\x00" * 65536)
            except OSError as error:
                assert error.errno == expected_errno
                result["fill_errno"] = error.errno
                result["remaining_backing_bytes"] = os.statvfs(backing).f_bavail * os.statvfs(backing).f_frsize
        phase = "mounted-write-fault"
        try:
            if args.fault in ("rename", "close"):
                (mountpoint / "scratch").rename(mountpoint / "renamed-scratch")
                if args.fault == "rename":
                    fsync(mountpoint)
            else:
                target = mountpoint / ("uncommitted-file" if args.fault == "create" else "scratch")
                with target.open("wb" if args.fault == "create" else "r+b", buffering=0) as stream:
                    stream.write(b"uncommitted" * 1024)
                    os.fsync(stream.fileno())
            result["write_fault"] = "pending-directory-mutation-without-fsync" if args.fault == "close" else "false-success"
        except OSError as error:
            result["write_errno"] = error.errno
        if args.fault == "close":
            phase = "close-while-full"
            full_close = subprocess.run([cli, "unmount", "--type", "cryfs", str(vault)], capture_output=True, text=True, timeout=45)
            result["full_close_exit"] = full_close.returncode
            result["full_close_diagnostic"] = (full_close.stdout + full_close.stderr).replace(secret, "<redacted>")[-4096:]
        # Restore only this fixture's reserved space, before a graceful retry.
        filler.unlink()
        phase = "close-after-restoring-space"
        close = subprocess.run([cli, "unmount", "--type", "cryfs", str(vault)], capture_output=True, text=True, timeout=45)
        result["close_exit"] = close.returncode
        result["close_diagnostic"] = (close.stdout + close.stderr).replace(secret, "<redacted>")[-4096:]
        phase = "reopen-committed-baseline"
        CLI_TEST.terminal([cli, "mount", "--type", "cryfs", str(vault), str(mountpoint)], [secret])
        assert (mountpoint / "committed-file").read_bytes() == payload, "Previously committed file changed"
        with closing(sqlite3.connect((mountpoint / "committed.sqlite").as_uri() + "?mode=rw", uri=True)) as db:
            assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
            assert db.execute("SELECT payload FROM records WHERE id=1").fetchone()[0] == payload
        result["committed_baseline_preserved"] = True
        phase = "final-close"
        CLI_TEST.terminal([cli, "unmount", "--type", "cryfs", str(vault)])
        if args.fault == "close":
            assert result["full_close_exit"] != 0, "Closing with an unflushed directory while full must not report success"
            assert "backend close failed" in result["full_close_diagnostic"], "Close failure must be explicit"
            archives = []
            registry = Path(f"/tmp/lurker-folders-{os.geteuid()}")
            for directory in registry.glob("failed-session-*"):
                record = directory / "session.json"
                if record.is_file() and json.loads(record.read_text())["source"] == str(vault):
                    status = json.loads((directory / "backend-result.json").read_text())
                    assert status["signal"] is not None or status["code"] != 0
                    archives.append(str(directory))
            assert len(archives) == 1, "Unclean close must retain the real backend exit status"
            result["failure_evidence"] = archives[0]
            result["outcome"] = "unclean-exit-reported-and-previous-commits-preserved"
        else:
            assert result.get("write_errno") == expected_errno, "Mounted write must report the real full-storage errno"
            assert result["close_exit"] == 0, "Backend did not recover a clean close after restoring space"
            result["outcome"] = "storage-error-returned-and-clean-close-recovered"
        result["status"] = "passed"
    except Exception as error:
        result.update(status="failed", failed_stage=phase, error=str(error).replace(secret, "<redacted>")[:4096])
    result["log"] = str(root / "backend.log")
    (root / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)
    return result["status"] != "passed"


if __name__ == "__main__":
    raise SystemExit(main())
