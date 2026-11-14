#!/usr/bin/env python3
"""Opt-in macOS Cryptomator backend crash test, not supervised CLI qualification.

The JVM is our unreaped direct child; Popen retains its PID lifetime until wait.
Only that verified child receives SIGKILL. Uses private /tmp fixtures, ordinary
unmount only, and retains ciphertext and concise evidence even on failure.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import ctypes
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import resource
import secrets
import select
import shutil
import signal
import sqlite3
import subprocess
import sys
import tempfile
import time

SPEC = importlib.util.spec_from_file_location("folder_crash_workload", Path(__file__).with_name("test-folder-crash.py"))
WORKLOAD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WORKLOAD)


def mounted(path):
    return any(f" on {path} (" in line for line in subprocess.check_output(["/sbin/mount"], text=True).splitlines())


def ack(path, status):
    if not path.exists():
        return False
    with path.open("rb") as stream:
        content = stream.read(131073)
    assert len(content) <= 131072, "Backend output exceeded bounded protocol capture"
    for line in content.splitlines(keepends=True):
        if not line.endswith(b"\n"):
            continue
        try:
            if json.loads(line) == {"protocol": 1, "status": status}:
                return True
        except ValueError:
            pass
    return False


def child_identity(child):
    assert child.poll() is None, "Owned backend already exited; no signal authorized"
    class BSDInfo(ctypes.Structure):
        _fields_ = [("prefix", ctypes.c_uint32 * 12), ("comm", ctypes.c_char * 16),
                    ("name", ctypes.c_char * 32), ("suffix", ctypes.c_uint32 * 6),
                    ("seconds", ctypes.c_uint64), ("microseconds", ctypes.c_uint64)]
    lib = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
    lib.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_int]
    lib.proc_pidinfo.restype = ctypes.c_int
    info = BSDInfo()
    assert lib.proc_pidinfo(child.pid, 3, 0, ctypes.byref(info), ctypes.sizeof(info)) == ctypes.sizeof(info)
    assert info.prefix[3] == child.pid and info.prefix[4] == os.getpid(), "Not our direct child"
    assert [info.prefix[i] for i in (5, 7, 9)] == [os.geteuid()] * 3, "Unexpected backend credentials"
    return {"pid": child.pid, "birth": f"{info.seconds}:{info.microseconds}", "uid": os.geteuid()}


def check(backend, root):
    vault, point = root / "vault", root / "mount"
    vault.mkdir(mode=0o700)
    point.mkdir(mode=0o700)
    password = ("crash-" + secrets.token_urlsafe(32) + "\n").encode()
    child = worker = None
    sequence = 0
    output = None
    result = {"backend": "cryptomator", "transport": "fuse-t/fskit", "fixture": str(root),
              "qualification": "native backend only; not supervised CLI recovery",
              "in_flight_writes": "No durability assertion; test only acknowledges FULL/WAL plus fsync"}
    phase = "initialize"

    def start():
        nonlocal child, sequence, output
        assert not mounted(point), "Previous test mount remains active"
        point.mkdir(mode=0o700, exist_ok=True)
        sequence += 1
        output = root / f"backend-{sequence}.out"
        with output.open("wb") as stdout, (root / f"backend-{sequence}.err").open("wb") as stderr:
            child = subprocess.Popen([str(backend), "mount", str(vault), str(point)],
                                     stdin=subprocess.PIPE, stdout=stdout, stderr=stderr)
        child.stdin.write(password)
        child.stdin.close()
        until = time.monotonic() + 30
        while time.monotonic() < until:
            assert child.poll() is None, "Native backend failed before readiness"
            if mounted(point) and ack(output, "mounted"):
                return
            time.sleep(.05)
        raise AssertionError("Backend readiness timed out")

    def detach():
        if mounted(point):
            completed = subprocess.run(["/sbin/umount", str(point)], capture_output=True, timeout=20)
            result.setdefault("ordinary_unmount_attempts", []).append({
                "exit": completed.returncode, "stderr": completed.stderr.decode(errors="replace")[:1024]})
            until = time.monotonic() + 5
            while mounted(point) and time.monotonic() < until:
                time.sleep(.1)
            assert not mounted(point), "Ordinary unmount failed; preserve fixture"

    try:
        init = subprocess.run([str(backend), "init", str(vault)], input=password, capture_output=True, timeout=45)
        assert init.returncode == 0, "Native initialization failed"
        phase = "mount"
        start()
        identity = child_identity(child)
        phase = "committed-workload"
        payload = secrets.token_bytes(8192)
        result["committed_sha256"] = hashlib.sha256(payload).hexdigest()
        (point / "atomic.tmp").write_bytes(payload)
        WORKLOAD.fsync_path(point / "atomic.tmp")
        (point / "atomic.tmp").rename(point / "durable data")
        WORKLOAD.fsync_path(point)
        worker = subprocess.Popen([sys.executable, __file__, "--sqlite-worker", str(point)],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        assert select.select([worker.stdout], [], [], 45)[0], "Blocked WAL worker did not become ready"
        result["before_kill_sizes"] = json.loads(worker.stdout.readline(8192))
        result["committed_workload"] = "FULL/WAL commit plus explicit file and directory fsync; writer remains open"
        phase = "verified-child-kill"
        assert child_identity(child) == identity
        child.kill()  # Our unreaped direct child: its PID cannot be reused here.
        assert child.wait(timeout=20) == -signal.SIGKILL
        result["killed"] = dict(identity, signal="SIGKILL", direct_unreaped_child=True)
        child = None
        worker.stdin.write(b"x")  # os._exit without touching dead-FUSE SHM mappings.
        worker.stdin.flush()
        worker.stdin.close()
        assert worker.wait(timeout=15) == 0
        worker = None
        phase = "ordinary-recovery-detach"
        # FUSE-T's FSKit helper tears its mount down asynchronously after the
        # serving JVM exits. Observe a bounded grace period before native close.
        teardown = time.monotonic()
        while mounted(point) and time.monotonic() - teardown < 10:
            time.sleep(.1)
        result["post_crash_teardown_wait_seconds"] = round(time.monotonic() - teardown, 3)
        result["automatic_detach_after_daemon_exit"] = not mounted(point)
        # Only automatic detach creates an unannounced fall-through concern.
        if result["automatic_detach_after_daemon_exit"]:
            try:
                with (point / "fallthrough-probe").open("xb") as stream:
                    stream.write(b"synthetic crash probe")
                result["underlying_writable_without_explicit_unmount"] = True
                (point / "fallthrough-probe").unlink()
            except (PermissionError, FileNotFoundError):
                result["underlying_writable_without_explicit_unmount"] = False
        detach()
        shutil.copytree(vault, root / "ciphertext-before-reopen")
        phase = "durable-reopen"
        start()
        assert (point / "durable data").read_bytes() == payload, "Durably fsynced file changed"
        with closing(sqlite3.connect(point / "profile.sqlite")) as connection:
            assert connection.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
            assert connection.execute("SELECT value FROM committed WHERE id=1").fetchone()[0] == payload
        phase = "graceful-close"
        detach()
        child.terminate()
        assert child.wait(timeout=20) in (0, 143) and ack(output, "closed"), "No graceful close acknowledgement"
        child = None
        result.update(status="passed", wal="committed WAL replay and integrity verified", repair=False)
    except Exception as error:
        result.update(status="failed", phase=phase, error=str(error))
    finally:
        if worker is not None:
            try:
                worker.stdin.write(b"x")
                worker.stdin.flush()
                worker.stdin.close()
                worker.wait(timeout=15)
            except (OSError, subprocess.TimeoutExpired):
                result["application_worker_retained"] = worker.pid
        try:
            detach()
            if child is not None:
                child.terminate()
                child.wait(timeout=20)
        except (AssertionError, OSError, subprocess.TimeoutExpired):
            result["cleanup_incomplete"] = True
            if child is not None:
                result["backend_retained"] = child.pid
        result["mount_present"] = mounted(point)
    return result


def main():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    if len(sys.argv) == 3 and sys.argv[1] == "--sqlite-worker":
        assert platform.system() == "Darwin" and os.geteuid() != 0 and platform.node().split(".")[0] != "endeavour"
        point = Path(sys.argv[2]).resolve()
        assert point.parent.parent == Path("/tmp").resolve() and point.parent.name.startswith("lurker-cryptomator-child-crash-")
        assert point.parent.stat().st_uid == os.geteuid()
        WORKLOAD.sqlite_worker(point)
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--backend", required=True, type=Path)
    parser.add_argument("--runtime-tests", required=True, action="store_true")
    args = parser.parse_args()
    if platform.system() != "Darwin" or os.geteuid() == 0 or platform.node().split(".")[0] == "endeavour":
        parser.error("Use a normal user on the dedicated Mac; no runtime tests on endeavour")
    backend = args.backend.resolve(strict=True)
    manifest = json.loads((backend.parent / "manifest.json").read_text())
    assert manifest["mount"] == {"provider": "org.cryptomator.frontend.fuse.mount.FuseTMountProvider",
                                 "runtime": "fuse-t", "transport": "fskit", "minimum_os": "26.0"}
    for relative, digest in manifest["files"].items():
        assert hashlib.sha256((backend.parent / relative).read_bytes()).hexdigest() == digest, "Backend image changed"
    root = Path(tempfile.mkdtemp(prefix="lurker-cryptomator-child-crash-", dir="/tmp")).resolve()
    started = time.monotonic()
    result = check(backend, root)
    result.update(runner_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  manifest_sha256=hashlib.sha256((backend.parent / "manifest.json").read_bytes()).hexdigest(),
                  duration_seconds=round(time.monotonic() - started, 3), os=platform.platform())
    (root / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    raise SystemExit(0 if result["status"] == "passed" else 1)


if __name__ == "__main__":
    main()
