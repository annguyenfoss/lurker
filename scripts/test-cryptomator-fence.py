#!/usr/bin/env python3
"""Opt-in macOS CLI fence recovery using only private disposable /tmp fixtures.

A test-only backend wrapper owns the unreaped JVM child it crashes. No signal
targets a PID recovered from a session record. Production backend bytes remain
unchanged; this test complements normal packaged CLI lifecycle qualification.
"""
import argparse
from contextlib import closing
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import secrets
import shlex
import signal
import sqlite3
import subprocess
import sys
import tempfile
import time


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def wait_for(predicate, message, timeout=30):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        if predicate():
            return
        time.sleep(.05)
    raise AssertionError(message)


def backend_child(backend, control, arguments):
    if not arguments or arguments[0] != "mount":
        os.execv(backend, [backend, *arguments])
    # Inherit the credential/output pipes; never copy credentials to a file.
    child = subprocess.Popen([backend, *arguments])
    stopping = False

    def stop(_signal, _frame):
        nonlocal stopping
        stopping = True

    signal.signal(signal.SIGTERM, stop)
    while child.poll() is None:
        if (Path(control) / "crash-request").exists():
            identity = load("fence_child_identity", "test-cryptomator-child-crash.py").child_identity(child)
            # Only our still-unreaped direct child can receive this signal.
            child.kill()
            status = child.wait(timeout=15)
            (Path(control) / "crashed-child.json").write_text(json.dumps({**identity, "exit": status}))
            return 1
        if stopping:
            child.terminate()
            return 0 if child.wait(timeout=20) == 0 else 1
        time.sleep(.025)
    return 0 if child.returncode == 0 else 1


def run(args):
    assert platform.system() == "Darwin" and os.geteuid() == args.expected_uid != 0
    assert len(args.commit) == 40 and all(c in "0123456789abcdef" for c in args.commit)
    binary, backend = args.binary.resolve(strict=True), args.backend.resolve(strict=True)
    assert hashlib.sha256(binary.read_bytes()).hexdigest() == args.binary_sha256
    root = Path(tempfile.mkdtemp(prefix="lurker-cryptomator-fence-", dir="/private/tmp"))
    root.chmod(0o700)
    result = {"status": "failed", "source_commit": args.commit, "fixture": str(root),
              "binary_sha256": args.binary_sha256,
              "backend_sha256": hashlib.sha256(backend.read_bytes()).hexdigest(),
              "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), "checks": []}
    base = load("fence_cli", "test-backends.py")
    wrapper_dir = root / "backend"
    wrapper_dir.mkdir(mode=0o700)
    wrapper = wrapper_dir / "lurker-cryptomator"
    command = [sys.executable, str(Path(__file__).resolve()), "--backend-child", str(backend), str(root)]
    wrapper.write_text("#!/bin/sh\nexec " + shlex.join(command) + ' "$@"\n')
    wrapper.chmod(0o700)
    os.environ["LURKER_BACKEND_DIR"] = str(wrapper_dir)
    source, point = root / "vault", root / "mount"
    point.mkdir(mode=0o750)
    original_mode = point.stat().st_mode & 0o7777
    password = "  fence 密碼 " + secrets.token_urlsafe(24) + "  "
    sequence = 0

    def cli(arguments, expected=True):
        nonlocal sequence
        sequence += 1
        outcome = base.terminal([str(binary), "--color=never", *map(str, arguments)],
                                password, 90, root / f"cli-{sequence}.log")
        assert not outcome["timed_out"] and (outcome["exit_code"] == 0) == expected, str(outcome)

    def denied(path):
        assert path.stat().st_mode & 0o7777 == 0
        try:
            (path / "forbidden plaintext").write_bytes(b"owned test")
        except PermissionError:
            return
        raise AssertionError("Plaintext fallthrough was writable")

    connection = None
    phase = "initialize-and-mount"
    try:
        cli(["create", "--type", "cryptomator", source])
        cli(["mount", "--type", "cryptomator", source, point])
        assert os.path.ismount(point)
        phase = "committed-workload"
        payload = secrets.token_bytes(8192)
        with (point / "durable record").open("wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        connection = sqlite3.connect(point / "profile.sqlite")
        assert connection.execute("PRAGMA journal_mode=WAL").fetchone()[0] == "wal"
        connection.execute("PRAGMA synchronous=FULL")
        connection.execute("PRAGMA wal_autocheckpoint=0")
        connection.execute("CREATE TABLE committed (value BLOB)")
        connection.execute("INSERT INTO committed VALUES (?)", (payload,))
        connection.commit()
        crash = load("fence_fsync", "test-folder-crash.py")
        for path in (point / "profile.sqlite", point / "profile.sqlite-wal", point):
            crash.fsync_path(path)
        phase = "owned-jvm-crash"
        (root / "crash-request").write_text("crash owned child\n")
        wait_for(lambda: (root / "crashed-child.json").exists(), "Owned child crash not confirmed")
        assert json.loads((root / "crashed-child.json").read_text())["exit"] == -signal.SIGKILL
        wait_for(lambda: not os.path.ismount(point), "FSKit did not detach after JVM crash")
        try:
            connection.close()
        except sqlite3.Error:
            pass  # The live connection lost its filesystem with the JVM.
        connection = None
        denied(point)
        result["checks"].append("crash-detach-denies-plaintext-writes")
        phase = "replacement-refusal"
        moved = root / "original-underlay"
        point.rename(moved)
        point.mkdir(mode=0o700)
        cli(["unmount", "--type", "cryptomator", source], expected=False)
        denied(moved)
        assert point.stat().st_mode & 0o7777 == 0o700
        point.rmdir()
        moved.rename(point)
        result["checks"].append("replacement-refused-with-both-inodes-unchanged")
        phase = "explicit-crash-recovery"
        cli(["unmount", "--type", "cryptomator", source], expected=False)
        assert point.stat().st_mode & 0o7777 == original_mode
        result["checks"].append("explicit-recovery-restores-original-mode-and-reports-crash")
        (root / "crash-request").unlink()
        phase = "reopen-and-integrity"
        cli(["mount", "--type", "cryptomator", source, point])
        assert (point / "durable record").read_bytes() == payload
        with closing(sqlite3.connect(point / "profile.sqlite")) as recovered:
            assert recovered.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
            assert recovered.execute("SELECT value FROM committed").fetchone()[0] == payload
        result["checks"].append("committed-file-and-sqlite-wal-recover-with-integrity")
        phase = "normal-close"
        cli(["unmount", "--type", "cryptomator", source])
        assert not os.path.ismount(point) and point.stat().st_mode & 0o7777 == original_mode
        result["checks"].append("normal-close-restores-original-mode")
        result["status"] = "passed"
    except Exception as error:
        result.update(failed_stage=phase, error=str(error).replace(password, "<redacted>")[:2048])
    finally:
        if connection is not None:
            try:
                connection.close()
            except sqlite3.Error:
                pass
        base.atomic_json(root / "result.json", result)
    print(json.dumps({"status": result["status"], "checks": len(result["checks"]),
                      "failed_stage": result.get("failed_stage"), "result": str(root / "result.json")}))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--backend-child":
        raise SystemExit(backend_child(sys.argv[2], sys.argv[3], sys.argv[4:]))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--expected-uid", type=int, required=True)
    raise SystemExit(run(parser.parse_args()))
