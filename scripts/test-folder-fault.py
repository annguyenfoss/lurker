#!/usr/bin/env python3
"""Mounted directory-backend storage qualification on a private Linux tmpfs.

Never mounts filesystems, changes quotas, fills ordinary host storage, forces an
unmount, repairs data, or kills a backend. Retains every fixture and JSON result.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import ctypes
import errno
import importlib.util
import json
import os
from pathlib import Path
import platform
import shlex
import sqlite3
import subprocess

SPEC = importlib.util.spec_from_file_location("folder_cli_test", Path(__file__).with_name("test-folder-backends.py"))
CLI_TEST = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CLI_TEST)
SPEC = importlib.util.spec_from_file_location("linux_package", Path(__file__).with_name("test-linux-package.py"))
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)

BACKENDS = {
    "cryfs": "cryfs",
    "gocryptfs": "gocryptfs",
    "cryptomator": "cryptomator/lurker-cryptomator",
}


def fsync(path):
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def fixture_backing(path):
    root = path.resolve(strict=True)
    if path.is_symlink() or root.parent != Path("/tmp") or not root.name.startswith("lurker-folder-fault-"):
        raise ValueError("Use a private /tmp/lurker-folder-fault-* fixture")
    backing = root / "backing"
    for directory in (root, backing):
        if not directory.is_dir() or directory.is_symlink():
            raise ValueError("Use real fixture and backing directories")
        info = directory.stat()
        if info.st_uid != os.geteuid() or info.st_mode & 0o077:
            raise ValueError("The fixture and backing must belong to the login user and be private")
    mountpoint = root / "mount"
    if mountpoint.is_symlink() or (mountpoint.exists() and (
            not mountpoint.is_dir() or os.path.ismount(mountpoint) or any(mountpoint.iterdir()))):
        raise ValueError("The fixture mountpoint must be absent or an empty unmounted directory")
    report = root / "result.json"
    if (not os.path.ismount(backing) or backing.stat().st_dev == root.stat().st_dev
            or any(backing.iterdir()) or report.exists() or report.is_symlink()):
        raise ValueError("Use a new fixture with a separate, empty backing mount")
    # statfs.f_type is the first native long; reserve ample space for the rest.
    fields = (ctypes.c_long * 64)()
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.statfs(os.fsencode(backing), ctypes.byref(fields)) != 0:
        raise OSError(ctypes.get_errno(), "Could not inspect backing filesystem")
    capacity = os.statvfs(backing)
    size = capacity.f_blocks * capacity.f_frsize
    if fields[0] != 0x01021994 or not 0 < size <= 16 * 1024 * 1024:
        raise ValueError("Only a finite tmpfs of at most 16 MiB may be filled")
    return root, backing, size


def write_synced(path, payload):
    with path.open("xb") as stream:
        stream.write(payload)
        stream.flush()
        os.fsync(stream.fileno())


def check_prefix(path, expected):
    try:
        with path.open("rb") as stream:
            return {"matches": stream.read(len(expected)) == expected}
    except OSError as error:
        return {"matches": False, "errno": error.errno}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path)
    parser.add_argument("backend", type=Path, nargs="?", help="Legacy development CryFS executable override")
    parser.add_argument("fixture", type=Path)
    parser.add_argument("--type", choices=BACKENDS, default="cryfs")
    parser.add_argument("--commit", help="Exact source commit of the installed package")
    parser.add_argument("--fault", choices=["create", "overwrite", "append", "rename", "close"])
    parser.add_argument("--leave-one-block", action="store_true", help="Leave one backing block for an append to exhaust")
    parser.add_argument("--expected-error", choices=["ENOSPC", "EDQUOT"], default="ENOSPC")
    parser.add_argument("--runtime-tests", action="store_true", required=True)
    args = parser.parse_args()
    if platform.system() != "Linux" or os.geteuid() == 0 or platform.node().split(".")[0] == "endeavour":
        parser.error("Use a dedicated Linux native host as its ordinary user")
    kind = args.type
    args.fault = args.fault or ("overwrite" if args.backend else "create")
    append = args.fault == "append"
    if kind != "cryfs" and (args.backend or args.fault == "close"):
        parser.error("The development override and close-while-full scenario require CryFS")
    if args.leave_one_block and (not append or args.expected_error != "ENOSPC"):
        parser.error("--leave-one-block requires the append scenario with ENOSPC")
    try:
        root, backing, size = fixture_backing(args.fixture)
        cli_path = args.cli.resolve(strict=True)
        for name in list(os.environ):
            if name.startswith(("LURKER_", "LD_")) or name in ("JAVA_HOME", "CLASSPATH"):
                os.environ.pop(name)
        if args.backend:
            backend = args.backend.resolve(strict=True)
            backend_dir = root / "backend"
            backend_dir.mkdir(mode=0o700)
            wrapper = backend_dir / "cryfs"
            wrapper.write_text("#!/bin/sh\nexec " + shlex.quote(str(backend)) + ' "$@" --logfile '
                               + shlex.quote(str(root / "backend.log")) + "\n")
            wrapper.chmod(0o700)
            os.environ["LURKER_BACKEND_DIR"] = str(backend_dir)
            backend_hash = PACKAGE.digest(backend)
        else:
            if not args.commit or cli_path.name != "lurker" or cli_path.parent.name != "bin":
                raise ValueError("The packaged route needs an installed CLI and its exact --commit")
            prefix = cli_path.parent.parent
            manifest = PACKAGE.verified_package(prefix, args.commit)
            if kind not in manifest["selected_backends"]:
                raise ValueError("Backend absent from verified package: " + kind)
            engine = "libexec/lurker/backends/" + BACKENDS[kind]
            backend_hash = manifest["files"][engine]["sha256"]
    except (KeyError, OSError, ValueError) as error:
        parser.error(str(error))
    cli = str(cli_path)
    vault, mountpoint = backing / "vault", root / "mount"
    secret = "Lurker finite-storage disposable regression only"
    result = {"schema": 1, "backend": kind, "backend_sha256": backend_hash, "cli_sha256": PACKAGE.digest(cli_path),
              "source_commit": args.commit, "installed_package": args.backend is None,
              "fixture": str(root), "capacity_bytes": size, "repair": False, "fault": args.fault}
    expected_errno = getattr(errno, args.expected_error)
    result["expected_errno"] = expected_errno
    phase = "create"
    try:
        CLI_TEST.terminal([cli, "create", "--type", kind, str(vault)], [secret, secret])
        phase = "mount"
        CLI_TEST.terminal([cli, "mount", "--type", kind, str(vault), str(mountpoint)], [secret])
        phase = "commit-baseline"
        payload = b"committed-before-storage-fault\x00" * 256
        write_synced(mountpoint / "committed-file", payload)
        with closing(sqlite3.connect(mountpoint / "committed.sqlite")) as db:
            assert db.execute("PRAGMA journal_mode=WAL").fetchone()[0] == "wal"
            db.execute("PRAGMA synchronous=FULL")
            db.execute("CREATE TABLE records (id INTEGER PRIMARY KEY, payload BLOB)")
            db.execute("INSERT INTO records VALUES (1, ?)", (payload,))
            db.commit()
        scratch = mountpoint / "scratch"
        scratch_payload = payload[:2048] if append else payload
        write_synced(scratch, scratch_payload)
        fsync(mountpoint / "committed.sqlite")
        fsync(mountpoint)
        phase = "fill-finite-backing"
        filler = backing / "owned-space-filler"
        with filler.open("xb", buffering=0) as stream:
            if args.leave_one_block:
                space = os.statvfs(backing)
                remaining = (space.f_bavail - 1) * space.f_frsize
                assert 0 < remaining <= size
                while remaining:
                    written = stream.write(b"\x00" * min(remaining, 65536))
                    assert written > 0
                    remaining -= written
            else:
                try:
                    while stream.tell() <= size:
                        stream.write(b"\x00" * 65536)
                    raise RuntimeError("Bounded backing writes did not reach a storage limit")
                except OSError as error:
                    assert error.errno == expected_errno
                    result["fill_errno"] = error.errno
        space = os.statvfs(backing)
        result["remaining_backing_bytes"] = space.f_bavail * space.f_frsize
        if args.leave_one_block:
            assert space.f_bavail == 1, "The append must start with exactly one backing block free"
        phase = "mounted-write-fault"
        try:
            if args.fault in ("rename", "close"):
                (mountpoint / "scratch").rename(mountpoint / "renamed-scratch")
                if args.fault == "rename":
                    fsync(mountpoint)
            else:
                target = mountpoint / "uncommitted-file" if args.fault == "create" else scratch
                mode = {"create": "xb", "append": "ab", "overwrite": "r+b"}[args.fault]
                operation = "open"
                with target.open(mode, buffering=0) as stream:
                    operation = "write"
                    pending = b"uncommitted" * 1024
                    result["write_requested_bytes"] = len(pending)
                    result["write_accepted_bytes"] = 0
                    while pending:
                        written = stream.write(pending)
                        assert written > 0, "Mounted write made no progress"
                        result["write_accepted_bytes"] += written
                        pending = pending[written:]
                    operation = "fsync"
                    os.fsync(stream.fileno())
                    result["fsync_acknowledged"] = True
                    operation = "close"
            result["write_fault"] = "pending-directory-mutation-without-fsync" if args.fault == "close" else "false-success"
        except OSError as error:
            result["write_errno"] = error.errno
            if args.fault in ("create", "append", "overwrite"):
                result["write_error_operation"] = operation
        if append:
            result["append_prefix_while_full"] = check_prefix(scratch, scratch_payload)
        if append or args.fault == "close":
            phase = "close-while-full"
            full_close = subprocess.run([cli, "unmount", "--type", kind, str(vault)], capture_output=True, text=True, timeout=45)
            result["full_close_exit"] = full_close.returncode
            result["full_close_diagnostic"] = (full_close.stdout + full_close.stderr).replace(secret, "<redacted>")[-4096:]
        # Restore only this fixture's reserved space, before a graceful retry.
        filler.unlink()
        if append and not os.path.ismount(mountpoint):
            phase = "reopen-after-full-close"
            CLI_TEST.terminal([cli, "mount", "--type", kind, str(vault), str(mountpoint)], [secret])
        resumed = b"committed-after-releasing-space\x00" * 256
        if args.backend is None and args.fault != "close":
            phase = "commit-after-restoring-space"
            write_synced(mountpoint / "resumed-file", resumed)
            with closing(sqlite3.connect((mountpoint / "committed.sqlite").as_uri() + "?mode=rw", uri=True)) as db:
                db.execute("PRAGMA synchronous=FULL")
                db.execute("INSERT INTO records VALUES (2, ?)", (resumed,))
                db.commit()
            fsync(mountpoint / "committed.sqlite")
            fsync(mountpoint)
        phase = "close-after-restoring-space"
        close = subprocess.run([cli, "unmount", "--type", kind, str(vault)], capture_output=True, text=True, timeout=45)
        result["close_exit"] = close.returncode
        result["close_diagnostic"] = (close.stdout + close.stderr).replace(secret, "<redacted>")[-4096:]
        phase = "reopen-committed-baseline"
        CLI_TEST.terminal([cli, "mount", "--type", kind, str(vault), str(mountpoint)], [secret])
        if append:
            result["append_prefix_after_reopen"] = check_prefix(scratch, scratch_payload)
        assert (mountpoint / "committed-file").read_bytes() == payload, "Previously committed file changed"
        with closing(sqlite3.connect((mountpoint / "committed.sqlite").as_uri() + "?mode=rw", uri=True)) as db:
            assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
            assert db.execute("SELECT payload FROM records WHERE id=1").fetchone()[0] == payload
            if args.backend is None and args.fault != "close":
                assert (mountpoint / "resumed-file").read_bytes() == resumed
                assert db.execute("SELECT payload FROM records WHERE id=2").fetchone()[0] == resumed
                result["resumed_commits_preserved"] = True
        result["committed_baseline_preserved"] = True
        phase = "final-close"
        CLI_TEST.terminal([cli, "unmount", "--type", kind, str(vault)])
        phase = "verify-full-storage-result"
        failed_close = args.fault == "close" or (append and result["full_close_exit"] != 0)
        if failed_close:
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
        if args.fault != "close":
            if append:
                assert result["append_prefix_after_reopen"]["matches"], "Committed prefix unavailable or changed after reopen"
            assert result.get("write_errno") == expected_errno, "Mounted write must report the real full-storage errno"
            assert result["close_exit"] == 0, "Backend did not recover a clean close after restoring space"
            if not failed_close:
                result["outcome"] = "storage-error-returned-and-clean-close-recovered"
        result["status"] = "passed"
    except Exception as error:
        result.update(status="failed", failed_stage=phase, error=str(error).replace(secret, "<redacted>")[:4096])
    if args.backend:
        result["log"] = str(root / "backend.log")
    with (root / "result.json").open("x") as report:
        json.dump(result, report, indent=2)
        report.write("\n")
    print(json.dumps(result), flush=True)
    return result["status"] != "passed"


if __name__ == "__main__":
    raise SystemExit(main())
