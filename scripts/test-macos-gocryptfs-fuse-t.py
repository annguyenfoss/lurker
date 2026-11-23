#!/usr/bin/env python3
"""Build or exercise the private, opt-in gocryptfs FUSE-T experiment on macOS."""
from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import platform
import secrets
import shutil
import sqlite3
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
PATCH = ROOT / "native/macos/gocryptfs-fuse-t.patch"
GOCRYPTFS = "756cd453d7afe69fd2a00a7592fd2d9869a6f756"
GO_FUSE = "2d50fff207d78c197869fa492eff3a79bc1d8408"
HELPER = Path("/Library/Application Support/fuse-t/bin/go-nfsv4-1.2.9")
HELPER_SHA256 = "348361a699b768d555ec41803e492075b9179691dbfac6b9020db5c20add5fa7"


def digest(path: Path) -> str:
    with path.open("rb") as data:
        return hashlib.file_digest(data, "sha256").hexdigest()


def capture(command: list[str], **options) -> str:
    return subprocess.check_output(command, text=True, timeout=15, **options).strip()


def checkout(url: str, revision: str, destination: Path) -> None:
    destination.mkdir()
    subprocess.run(["git", "init", "-q", str(destination)], check=True)
    subprocess.run(["git", "fetch", "--depth=1", url, revision], cwd=destination,
                   check=True, timeout=120)
    subprocess.run(["git", "checkout", "-q", "--detach", "FETCH_HEAD"], cwd=destination,
                   check=True, timeout=15)
    if capture(["git", "rev-parse", "HEAD"], cwd=destination) != revision:
        raise RuntimeError(f"source pin mismatch for {url}")


def build(output: Path, cache: Path, jobs: int) -> None:
    go = shutil.which("go") or "/opt/homebrew/bin/go"
    output.mkdir(mode=0o700, parents=True, exist_ok=True)
    cache.mkdir(mode=0o700, parents=True, exist_ok=True)
    if (output / "gocryptfs").exists():
        raise RuntimeError("candidate output already exists")
    work = Path(tempfile.mkdtemp(prefix="sources-", dir=output))
    source, fuse = work / "gocryptfs", work / "go-fuse"
    checkout("https://github.com/rfjakob/gocryptfs.git", GOCRYPTFS, source)
    checkout("https://github.com/hanwen/go-fuse.git", GO_FUSE, fuse)
    subprocess.run(["git", "apply", "--unidiff-zero", "--check", str(PATCH)], cwd=fuse, check=True)
    subprocess.run(["git", "apply", "--unidiff-zero", str(PATCH)], cwd=fuse, check=True)
    env = dict(os.environ, GOTOOLCHAIN="local", CGO_ENABLED="0", GOOS="darwin",
               GOARCH="arm64", GOMAXPROCS=str(jobs), GOCACHE=str(cache / "objects"),
               GOMODCACHE=str(cache / "modules"))
    subprocess.run([go, "test", "-p", str(jobs), "-mod=readonly", "-tags", "lurkerfuset",
                    "./fuse", "-run", "^TestFuseTRenameLayout$", "-v"],
                   cwd=fuse, env=env, check=True, timeout=180)
    subprocess.run([go, "mod", "edit", "-replace",
                    f"github.com/hanwen/go-fuse/v2={fuse}"], cwd=source, env=env, check=True)
    subprocess.run([go, "mod", "verify"], cwd=source, env=env, check=True, timeout=180)
    binary = output / "gocryptfs"
    subprocess.run([go, "build", "-p", str(jobs), "-mod=readonly", "-tags", "lurkerfuset",
                    "-trimpath", "-ldflags=-X main.GitVersion=v2.7.0", "-o", str(binary), "."],
                   cwd=source, env=env, check=True, timeout=600)
    record = {
        "gocryptfs_commit": GOCRYPTFS, "go_fuse_commit": GO_FUSE,
        "patch_sha256": digest(PATCH), "binary_sha256": digest(binary),
        "go_version": capture([go, "version"], env=env),
        "module_identities": capture([go, "version", "-m", str(binary)], env=env).splitlines(),
        "macho_dependencies": capture(["/usr/bin/otool", "-L", str(binary)]).splitlines(),
    }
    (output / "build.json").write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps(record), flush=True)


def mount_line(point: Path) -> str | None:
    return next((line for line in capture(["/sbin/mount"]).splitlines()
                 if f" on {point} (" in line), None)


def matching_helpers(point: Path) -> list[str]:
    processes = capture(["/bin/ps", "-axww", "-o", "pid=,ppid=,command="])
    return [line.strip() for line in processes.splitlines()
            if str(HELPER) in line and line.endswith(f" {point}")]


def start_mount(binary, source, point, password, env, log, debug):
    args = [binary, "-fg", "-passfile", "/dev/stdin"]
    if debug:
        args.append("-fusedebug")
    process = subprocess.Popen(
        [*args, source, point],
        stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT, env=env,
    )
    assert process.stdin is not None
    try:
        process.stdin.write(password)
    finally:
        process.stdin.close()
    return process


def await_mount(process, point, transport):
    deadline = time.monotonic() + 28
    while time.monotonic() < deadline:
        line = mount_line(point)
        for owner in matching_helpers(point):
            pid, ppid, command = owner.split(maxsplit=2)
            if line and int(ppid) == process.pid and f"--backend {transport}" in command:
                return {"mount": line, "helper_pid": int(pid), "backend_pid": process.pid}
        if process.poll() is not None:
            raise RuntimeError(f"gocryptfs exited before the verified mount with status {process.returncode}")
        time.sleep(0.1)
    raise RuntimeError("timed out waiting for the FUSE-T mount and owned transport helper")


def close_mount(process, point):
    if mount_line(point):
        subprocess.run(["/sbin/umount", str(point)], check=True, timeout=15)
    process.wait(timeout=25)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if not mount_line(point) and not matching_helpers(point):
            return
        time.sleep(0.15)
    raise RuntimeError("the fixture mount or its helper survived ordinary close")


def write_and_sync(point):
    payload = bytes(range(256)) * 8192
    staging = point / "scratch"
    with staging.open("wb") as stream:
        stream.write(payload)
        stream.flush()
        os.fsync(stream.fileno())
        stream.truncate(len(payload) - 1)
        os.fsync(stream.fileno())
    expected = hashlib.sha256(payload[:-1]).hexdigest()
    if digest(staging) != expected:
        raise RuntimeError("written payload differs from the expected bytes")
    return expected


def write_wal(point, record):
    with closing(sqlite3.connect(point / "test.db", timeout=3)) as writer:
        mode = writer.execute("PRAGMA journal_mode=WAL").fetchone()[0]
        record["sqlite_journal_mode"] = mode
        if mode != "wal":
            raise RuntimeError(f"SQLite returned journal mode {mode!r}")
        writer.execute("PRAGMA synchronous=FULL")
        writer.execute("CREATE TABLE probe(value TEXT)")
        writer.execute("INSERT INTO probe VALUES ('committed')")
        writer.commit()
        with closing(sqlite3.connect(point / "test.db", timeout=3)) as reader:
            if reader.execute("SELECT value FROM probe").fetchone() != ("committed",):
                raise RuntimeError("second connection could not read the committed row")
        checkpoint = writer.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()
        record["sqlite_checkpoint"] = checkpoint
        if checkpoint[0] != 0:
            raise RuntimeError(f"SQLite checkpoint reported busy: {checkpoint!r}")


def read_wal(point):
    with closing(sqlite3.connect(point / "test.db", timeout=3)) as reader:
        if reader.execute("PRAGMA integrity_check").fetchone() != ("ok",):
            raise RuntimeError("SQLite integrity check failed after reopen")
        if reader.execute("SELECT value FROM probe").fetchone() != ("committed",):
            raise RuntimeError("SQLite committed row missing after reopen")


def probe(output: Path, transport: str, results: Path, debug: bool) -> bool:
    if digest(HELPER) != HELPER_SHA256:
        raise RuntimeError("installed FUSE-T helper differs from the pinned 1.2.9 release")
    subprocess.run(["/usr/bin/codesign", "--verify", "--strict", str(HELPER)], check=True,
                   timeout=15)
    results.mkdir(mode=0o700, parents=True, exist_ok=True)
    binary = output / "gocryptfs"
    built = json.loads((output / "build.json").read_text())
    if digest(binary) != built["binary_sha256"] or digest(PATCH) != built["patch_sha256"]:
        raise RuntimeError("candidate identity differs from its build record")
    env = dict(os.environ, LURKER_EXPERIMENT_FUSE_T_TRANSPORT=transport)
    root = Path(tempfile.mkdtemp(prefix="lurker-gocryptfs-fuset-", dir="/private/tmp"))
    source, point = root / "encrypted", root / "view"
    source.mkdir()
    point.mkdir()
    password = (secrets.token_urlsafe(24) + "\n").encode()
    record = {"status": "failed", "transport": transport, "fixture": str(root),
              "binary_sha256": built["binary_sha256"], "helper_sha256": HELPER_SHA256,
              "sqlite_version": sqlite3.sqlite_version,
              "boot": capture(["/usr/sbin/sysctl", "-n", "kern.boottime"]),
              "mounts": [], "checks": [], "failures": []}

    process = None
    stage = "init"
    try:
        with (results / f"{transport}-mount.log").open("xb") as log:
            subprocess.run([binary, "-init", "-scryptn", "16", "-passfile", "/dev/stdin", source],
                           input=password, check=True, stdout=log, stderr=subprocess.STDOUT,
                           env=env, timeout=30)
            for step in ("write", "reopen"):
                stage = f"{step}-mount"
                process = start_mount(binary, source, point, password, env, log, debug)
                record["mounts"].append(await_mount(process, point, transport))
                stage = f"{step}-payload"
                if step == "write":
                    expected = write_and_sync(point)
                    record["payload_sha256"] = expected
                    record["checks"].append("write-fsync-truncate")
                    try:
                        (point / "scratch").rename(point / "payload")
                        record["checks"].append("rename")
                    except OSError as err:
                        record["failures"].append({"stage": "rename", "error": str(err)})
                    payload_name = "payload" if (point / "payload").is_file() else "scratch"
                    try:
                        write_wal(point, record)
                        record["checks"].append("sqlite-wal-full-cross-connection-checkpoint")
                    except (OSError, RuntimeError, sqlite3.Error) as err:
                        record["failures"].append({"stage": "sqlite-wal", "error": str(err)})
                else:
                    if digest(point / payload_name) != expected:
                        raise RuntimeError("payload hash changed after reopen")
                    record["checks"].append("durable-payload-reopen")
                    if "sqlite-wal-full-cross-connection-checkpoint" in record["checks"]:
                        read_wal(point)
                        record["checks"].append("durable-sqlite-reopen")
                stage = f"{step}-close"
                close_mount(process, point)
                process = None
            if not record["failures"]:
                record["status"] = "passed"
    except (OSError, RuntimeError, sqlite3.Error, subprocess.SubprocessError) as err:
        record.update(error=f"{type(err).__name__}: {err}", stage=stage)
    finally:
        try:
            if process is not None:
                close_mount(process, point)
            if mount_line(point) or matching_helpers(point):
                raise RuntimeError("active mount or helper remains on fixture")
            record["cleanup"] = "verified"
            if record["status"] == "passed":
                shutil.rmtree(root)
        except (OSError, RuntimeError, subprocess.SubprocessError) as err:
            record.update(status="failed", cleanup=f"{type(err).__name__}: {err}")
        record["boot_unchanged"] = record["boot"] == capture(["/usr/sbin/sysctl", "-n", "kern.boottime"])
        (results / f"{transport}.json").write_text(json.dumps(record, indent=2) + "\n")
        print(json.dumps(record), flush=True)
    return record["status"] == "passed" and record["boot_unchanged"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    builder = commands.add_parser("build")
    builder.add_argument("--cache", type=Path, required=True)
    builder.add_argument("--jobs", type=int, default=2)
    builder.add_argument("--output", type=Path, required=True)
    tester = commands.add_parser("probe")
    tester.add_argument("--output", type=Path, required=True)
    tester.add_argument("--transport", choices=["fskit", "smb", "nfs"], required=True)
    tester.add_argument("--results", type=Path)
    tester.add_argument("--debug", action="store_true")
    options = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine() != "arm64" or os.geteuid() == 0:
        parser.error("Run the experiment as the macOS arm64 login user")
    if options.command == "build":
        if options.jobs < 1:
            parser.error("--jobs must be positive")
        build(options.output.resolve(), options.cache.resolve(), options.jobs)
        return 0
    results = options.results or options.output
    return 0 if probe(options.output.resolve(), options.transport, results.resolve(), options.debug) else 1


if __name__ == "__main__":
    raise SystemExit(main())
