#!/usr/bin/env python3
"""Capture an unprivileged native NetBSD LKL diagnostic build; never install.

This external filesystem prerequisite is separate from Lurker's backend bundle.
An artifact from this script is not permission to use it for privileged mounts.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import tarfile
import time
import urllib.request

REVISION = "d0f76a77e8c2ca68bd7d96190b8da09040eab072"
SOURCE_SHA256 = "ef6e5b7cd66c023bc16193013fdacbc1285b9eb86df864b8c6cefe0ad4e0e168"
SOURCE_URL = f"https://codeload.github.com/lkl/linux/tar.gz/{REVISION}"


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def fetch(path):
    """Only the pinned archive is accepted, including on cache hits."""
    path = Path(path)
    if path.is_symlink():
        raise RuntimeError("Source cache must not be a symlink")
    if not path.exists():
        temporary = path.with_suffix(path.suffix + ".partial")
        with temporary.open("xb") as output:
            with urllib.request.urlopen(SOURCE_URL, timeout=90) as response:
                shutil.copyfileobj(response, output)
        if digest(temporary) != SOURCE_SHA256:
            raise RuntimeError("Source download failed SHA-256 verification; partial retained")
        temporary.rename(path)
    if digest(path) != SOURCE_SHA256:
        raise RuntimeError("Source cache failed SHA-256 verification")
    return path


def verify_source(archive, source):
    """Permit resuming baseline objects only when every upstream input matches."""
    prefix = f"linux-{REVISION}/"
    checked = 0
    with tarfile.open(archive) as tar:
        for member in tar:
            if member.isdir() and member.name == prefix.rstrip("/"):
                continue
            if not member.name.startswith(prefix):
                raise RuntimeError("Unexpected source archive root")
            relative = Path(member.name[len(prefix):])
            if relative.is_absolute() or ".." in relative.parts:
                raise RuntimeError("Unsafe source archive path")
            path = source / relative
            if member.isfile():
                if path.is_symlink() or not path.is_file():
                    raise RuntimeError(f"Missing regular upstream input: {relative}")
                with tar.extractfile(member) as stream:
                    expected = hashlib.file_digest(stream, "sha256").hexdigest()
                if digest(path) != expected:
                    raise RuntimeError(f"Modified upstream input: {relative}")
                checked += 1
            elif member.issym() and (not path.is_symlink() or os.readlink(path) != os.path.normpath(member.linkname)):
                raise RuntimeError(f"Modified upstream symlink: {relative}")
            elif member.isdir() and (path.is_symlink() or not path.is_dir()):
                raise RuntimeError(f"Modified upstream directory: {relative}")
    return checked


def prepare(work):
    archive = fetch(work / "linux-current.tar.gz")
    source = work / f"linux-{REVISION}"
    if source.is_symlink():
        raise RuntimeError("Source directory must not be a symlink")
    if not source.exists():
        temporary = work / "source.extracting"
        temporary.mkdir()
        with tarfile.open(archive) as tar:
            tar.extractall(temporary, filter="data")
        extracted = temporary / source.name
        if list(temporary.iterdir()) != [extracted]:
            raise RuntimeError("Unexpected source archive layout")
        extracted.rename(source)
        temporary.rmdir()
    inputs = verify_source(archive, source)
    return source, inputs


def run_build(command, cwd, environment, log, timeout):
    """Bound the whole owned make process group, including compiler children."""
    with Path(log).open("xb") as output:
        child = subprocess.Popen(command, cwd=cwd, env=environment,
                                 stdout=output, stderr=subprocess.STDOUT,
                                 start_new_session=True)
        try:
            return {"pid": child.pid, "returncode": child.wait(timeout=timeout),
                    "timed_out": False}
        except subprocess.TimeoutExpired:
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            return {"pid": child.pid, "returncode": child.returncode, "timed_out": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=600)
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    if platform.system() != "NetBSD" or platform.machine() not in ("amd64", "x86_64"):
        parser.error("Run natively on NetBSD amd64")
    if os.geteuid() == 0:
        parser.error("Build as an ordinary user; no installation is performed")
    if not 1 <= args.timeout <= 7200:
        parser.error("Use a timeout between 1 and 7200 seconds")
    if args.receipt.exists():
        parser.error("Use a new receipt path; existing diagnostics are retained")
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    receipt = {"revision": REVISION, "source_sha256": SOURCE_SHA256,
               "phase": "unmodified-baseline", "uid": os.getuid(),
               "uname": list(platform.uname()), "started": time.time(),
               "script_sha256": digest(__file__), "state": "preparing"}
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    try:
        source, count = prepare(work)
        receipt.update(source=str(source), verified_upstream_files=count)
        if args.prepare_only:
            receipt["state"] = "prepared"
        else:
            environment = os.environ.copy()
            environment.update(PATH="/usr/pkg/bin:/usr/bin:/bin:/usr/sbin:/sbin", LC_ALL="C")
            command = ["gmake", "-j1", "V=1", str(source / "tools/lkl/lklfuse")]
            log = args.receipt.with_suffix(".log")
            receipt.update(command=command, log=str(log), state="building")
            args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
            receipt.update(run_build(command, source / "tools/lkl", environment, log, args.timeout))
            receipt["state"] = "terminal"
    except BaseException as error:
        receipt.update(state="error", error=repr(error))
        raise
    finally:
        receipt["finished"] = time.time()
        args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    print(args.receipt)
    return int(receipt.get("returncode", 0) != 0)


if __name__ == "__main__":
    raise SystemExit(main())
