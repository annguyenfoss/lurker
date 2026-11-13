#!/usr/bin/env python3
"""Opt-in Linux daemon SIGKILL recovery tests on private /tmp vaults.

Kills only the verified foreground backend recorded for this disposable vault.
Uses normal CLI unmount, never force/lazy detach or filesystem repair. Retains
failed fixtures and reports durability failures as failures, including CryFS.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import secrets
import select
import resource
import sys
import signal
import sqlite3
import shutil
import subprocess
import tempfile
import time

SPEC = importlib.util.spec_from_file_location("folder_cli_test", Path(__file__).with_name("test-folder-backends.py"))
CLI_TEST = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CLI_TEST)


def proc_identity(pid):
    text = Path(f"/proc/{pid}/stat").read_text()
    fields = text.rsplit(") ", 1)[1].split()
    status = Path(f"/proc/{pid}/status").read_text()
    uids = next(line for line in status.splitlines() if line.startswith("Uid:")).split()[1:]
    return {"birth": fields[19], "state": fields[0], "uids": [int(value) for value in uids]}


def owned_session(vault, mountpoint):
    directory = Path(f"/tmp/lurker-folders-{os.geteuid()}")
    state = directory.stat()
    assert state.st_uid == os.geteuid() and not state.st_mode & 0o077
    matches = []
    for path in directory.glob("session-*/session.json"):
        record = json.loads(path.read_text())
        if record["source"] == str(vault) and record["mountpoint"] == str(mountpoint):
            matches.append((path, record))
    assert len(matches) == 1, "Vault must have exactly one owned session"
    path, record = matches[0]
    metadata = vault.stat()
    assert record["source_identity"] == {"device": metadata.st_dev, "inode": metadata.st_ino}
    assert record["mount_identity"] and record["process"], "Unconfirmed session cannot authorize a kill"
    mounts = Path("/proc/self/mountinfo").read_text().splitlines()
    assert record["mount_identity"] in mounts, "Recorded kernel mount identity changed"
    return path, record


def kill_recorded_daemon(record):
    process = record["process"]
    pid = process["pid"]
    assert pid not in (0, 1, os.getpid(), os.getppid())
    current = proc_identity(pid)
    assert current["birth"] == process["birth"] and current["uids"] == [os.geteuid()] * 4
    # pidfd binds the signal to the opened process rather than a reusable PID.
    descriptor = os.pidfd_open(pid)
    try:
        current = proc_identity(pid)
        assert current["birth"] == process["birth"] and current["uids"] == [os.geteuid()] * 4
        signal.pidfd_send_signal(descriptor, signal.SIGKILL)
        assert select.select([descriptor], [], [], 15)[0], "Owned backend did not exit"
    finally:
        os.close(descriptor)
    return {"pid": pid, "birth": process["birth"], "uid": os.geteuid(), "signal": "SIGKILL", "pidfd": True}


def fsync_path(path):
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def sqlite_worker(mountpoint):
    # The only application process with a live SHM mapping. It must not execute
    # a SQLite close/checkpoint after the filesystem server is killed.
    payload = (mountpoint / "durable data").read_bytes()
    connection = sqlite3.connect(mountpoint / "profile.sqlite")
    assert connection.execute("PRAGMA journal_mode=WAL").fetchone()[0] == "wal"
    connection.execute("PRAGMA synchronous=FULL")
    assert connection.execute("PRAGMA synchronous").fetchone()[0] == 2
    connection.execute("CREATE TABLE committed (id INTEGER PRIMARY KEY, value BLOB)")
    connection.execute("INSERT INTO committed VALUES (1, ?)", (payload,))
    connection.commit()
    for name in ("durable data", "profile.sqlite", "profile.sqlite-wal"):
        fsync_path(mountpoint / name)
    fsync_path(mountpoint)
    sizes = {name: (mountpoint / name).stat().st_size
             for name in ("durable data", "profile.sqlite", "profile.sqlite-wal", "profile.sqlite-shm")}
    print(json.dumps(sizes), flush=True)
    sys.stdin.buffer.read(1)
    os._exit(0)


def check(cli, backend, root):
    phase = "create"
    result = {"backend": backend, "retained": str(root)}
    secret = "crash-test-" + secrets.token_urlsafe(32)
    vault, mountpoint = root / "encrypted vault", root / "mounted vault"
    worker = None
    try:
        CLI_TEST.terminal([cli, "create", "--type", backend, str(vault)], [secret, secret])
        phase = "mount"
        CLI_TEST.terminal([cli, "mount", "--type", backend, str(vault), str(mountpoint)], [secret])
        phase = "committed-workload"
        payload = secrets.token_bytes(8192)
        result["committed_sha256"] = hashlib.sha256(payload).hexdigest()
        (mountpoint / "atomic.tmp").write_bytes(payload)
        fsync_path(mountpoint / "atomic.tmp")
        (mountpoint / "atomic.tmp").rename(mountpoint / "durable data")
        fsync_path(mountpoint)
        worker = subprocess.Popen([sys.executable, __file__, "--sqlite-worker", str(mountpoint)],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        assert select.select([worker.stdout], [], [], 45)[0], "SQLite worker did not finish its committed workload"
        readiness = worker.stdout.readline(8192)
        assert readiness, "SQLite worker exited before confirming fsync"
        result["before_kill_sizes"] = json.loads(readiness)
        result["fsync_complete"] = True
        phase = "verified-daemon-kill"
        _, record = owned_session(vault, mountpoint)
        assert record["backend"] == backend
        result["killed"] = kill_recorded_daemon(record)
        # A live SQLite SHM mapping can SIGBUS after its FUSE server dies.
        # Tell the blocked application worker to exit without SQLite cleanup;
        # its writable descriptors stayed open until after the daemon kill.
        worker.stdin.write(b"x")
        worker.stdin.flush()
        worker.stdin.close()
        assert worker.wait(timeout=15) == 0, "SQLite worker failed while dropping dead-FUSE mappings"
        worker = None
        try:
            mountpoint.stat()
        except OSError as error:
            result["dead_mount_stat_errno"] = error.errno
        phase = "normal-unmount"
        # Source remains inspectable after a daemon dies. Mountpoint lookup may
        # itself be ENOTCONN; record both paths without bypassing CLI checks.
        first = subprocess.run([cli, "unmount", str(mountpoint)], capture_output=True, text=True, timeout=45)
        if first.returncode:
            result["mountpoint_unmount_error"] = first.stderr[-1500:]
            second = subprocess.run([cli, "unmount", str(vault)], capture_output=True, text=True, timeout=45)
            if second.returncode:
                raise AssertionError("Source unmount failed: " + second.stderr[-1500:])
        phase = "reopen-durability"
        CLI_TEST.terminal([cli, "mount", "--type", backend, str(vault), str(mountpoint)], [secret])
        # Preserve the raw test database before SQLite's normal WAL recovery
        # can change it. This is disposable fixture data, never credentials.
        evidence = root / "evidence"
        evidence.mkdir(mode=0o700)
        result["reopened_entries"] = {path.name: path.stat().st_size for path in mountpoint.iterdir()}
        for name in ("profile.sqlite", "profile.sqlite-wal", "profile.sqlite-shm"):
            if (mountpoint / name).is_file():
                shutil.copyfile(mountpoint / name, evidence / name)
        assert (mountpoint / "durable data").read_bytes() == payload, "Committed file changed after daemon crash"
        assert (mountpoint / "profile.sqlite").stat().st_size, "Committed SQLite database is empty after daemon crash"
        with closing(sqlite3.connect((mountpoint / "profile.sqlite").as_uri() + "?mode=rw", uri=True)) as connection:
            assert connection.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
            assert connection.execute("SELECT value FROM committed WHERE id=1").fetchone()[0] == payload
        phase = "final-close"
        CLI_TEST.terminal([cli, "unmount", str(mountpoint)])
        result.update(status="passed", checks=10, wal="FULL/uncheckpointed-before-kill", repair=False)
        shutil.rmtree(root)
        result.pop("retained", None)
    except Exception as error:
        result.update(status="failed", failed_stage=phase, error=str(error).replace(secret, "<redacted>")[:2048])
    finally:
        if worker is not None and worker.poll() is None:
            # No signal or SQLite finalizer is needed, even when the test aborts.
            worker.stdin.write(b"x")
            worker.stdin.flush()
            worker.stdin.close()
            worker.wait(timeout=15)
        if result.get("status") != "passed":
            # Explicit backend prevents an absent folder session from falling
            # through to a privileged auto-detection route during cleanup.
            try:
                cleanup = subprocess.run([cli, "unmount", "--type", backend, str(vault)],
                                         capture_output=True, text=True, timeout=45)
                result["ordinary_cleanup_exit"] = cleanup.returncode
                if cleanup.returncode:
                    result["ordinary_cleanup_error"] = cleanup.stderr.replace(secret, "<redacted>")[-1000:]
            except subprocess.TimeoutExpired:
                result["ordinary_cleanup_error"] = "Normal cleanup timed out; fixture retained"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path)
    parser.add_argument("backends", nargs="+", choices=["gocryptfs", "cryfs", "cryptomator"])
    parser.add_argument("--runtime-tests", action="store_true", required=True)
    parser.add_argument("--source-commit")
    args = parser.parse_args()
    if platform.system() != "Linux" or os.geteuid() == 0 or platform.node().split(".")[0] == "endeavour":
        parser.error("Use a dedicated Linux test host as its ordinary user")
    if not hasattr(os, "pidfd_open") or not hasattr(signal, "pidfd_send_signal"):
        parser.error("Verified process signalling requires Linux pidfd support")
    cli = str(args.cli.resolve())
    failed = False
    for backend in args.backends:
        root = Path(tempfile.mkdtemp(prefix=f"lurker-folder-crash-{backend}-", dir="/tmp"))
        start = time.monotonic()
        result = check(cli, backend, root)
        result.update(source_commit=args.source_commit, cli_sha256=hashlib.sha256(Path(cli).read_bytes()).hexdigest(),
                      duration_seconds=round(time.monotonic() - start, 2), os=platform.system(), kernel=platform.release())
        print(json.dumps(result), flush=True)
        failed |= result["status"] != "passed"
    return int(failed)


if __name__ == "__main__":
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    if len(sys.argv) == 3 and sys.argv[1] == "--sqlite-worker":
        mountpoint = Path(sys.argv[2])
        if platform.system() != "Linux" or platform.node().split(".")[0] == "endeavour" or not str(mountpoint).startswith("/tmp/lurker-folder-crash-"):
            raise SystemExit("Invalid disposable SQLite worker target")
        sqlite_worker(mountpoint)
    raise SystemExit(main())
