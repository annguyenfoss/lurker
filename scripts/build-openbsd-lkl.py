#!/usr/bin/env python3
"""Capture a native OpenBSD LKL compilation diagnostic; never install or mount.

The output is an unqualified external host-tool candidate, outside Lurker's
private backends. Even a successful build does not authorize filesystem use.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
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
    path = Path(path)
    if path.is_symlink():
        raise RuntimeError("Source archive cache cannot be a symlink")
    if not path.exists():
        temporary = path.with_suffix(path.suffix + ".partial")
        with temporary.open("xb") as output:
            with urllib.request.urlopen(SOURCE_URL, timeout=60) as response:
                shutil.copyfileobj(response, output)
        if digest(temporary) != SOURCE_SHA256:
            raise RuntimeError("Source download hash mismatch; partial retained")
        temporary.rename(path)
    if not path.is_file() or digest(path) != SOURCE_SHA256:
        raise RuntimeError("Source archive cache hash mismatch; retained")
    return path


def verify_source(archive, source):
    """Verify every original input before reusing any generated objects."""
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
            if member.isfile() or member.islnk():
                if path.is_symlink() or not path.is_file():
                    raise RuntimeError(f"Missing regular upstream input: {relative}")
                with tar.extractfile(member) as stream:
                    expected = hashlib.file_digest(stream, "sha256").hexdigest()
                if digest(path) != expected:
                    raise RuntimeError(f"Modified upstream input: {relative}")
                checked += 1
            elif member.issym():
                if not path.is_symlink() or os.readlink(path) != os.path.normpath(member.linkname):
                    raise RuntimeError(f"Modified upstream symlink: {relative}")
            elif member.isdir():
                if path.is_symlink() or not path.is_dir():
                    raise RuntimeError(f"Modified upstream directory: {relative}")
            else:
                raise RuntimeError("Unsupported source archive entry")
    return checked


def prepare(work):
    archive = fetch(work / "linux-current.tar.gz")
    source = work / f"linux-{REVISION}"
    if source.is_symlink():
        raise RuntimeError("Source directory cannot be a symlink")
    if not source.exists():
        temporary = work / "source.extracting"
        temporary.mkdir()
        with tarfile.open(archive) as tar:
            tar.extractall(temporary, filter="data")
        extracted = temporary / source.name
        if list(temporary.iterdir()) != [extracted]:
            raise RuntimeError("Unexpected extracted source layout")
        extracted.rename(source)
        temporary.rmdir()
    return source, verify_source(archive, source)


def run_build(command, cwd, environment, log, timeout, termination_grace=2):
    """Own a new session and terminate all its descendants on a deadline."""
    with Path(log).open("xb") as output:
        child = subprocess.Popen(command, cwd=cwd, env=environment,
                                 stdout=output, stderr=subprocess.STDOUT,
                                 start_new_session=True)
        try:
            return {"pid": child.pid, "returncode": child.wait(timeout=timeout),
                    "timed_out": False}
        except subprocess.TimeoutExpired:
            # Do not reap the leader before the final group signal. Its zombie
            # retains the PID, so another process group cannot reuse this ID.
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            time.sleep(termination_grace)
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired as error:
                raise RuntimeError(f"Owned build group {child.pid} remains after cancellation; retain and inspect") from error
            return {"pid": child.pid, "returncode": child.returncode,
                    "timed_out": True}



def install_alias(directory, name, target):
    path = directory / name
    if path.exists() or path.is_symlink():
        if not path.is_symlink() or os.readlink(path) != target:
            raise RuntimeError("Unexpected private build-tool alias; retained")
    else:
        path.symlink_to(target)


def gnu_toolchain(work):
    """Use official native tools privately; derive format from an actual object."""
    tools = {"gcc": "/usr/local/bin/egcc", "cc": "/usr/local/bin/egcc",
             "ar": "/usr/local/bin/gar", "as": "/usr/local/bin/gas",
             "nm": "/usr/local/bin/gnm", "objcopy": "/usr/local/bin/gobjcopy",
             "objdump": "/usr/local/bin/gobjdump", "sed": "/usr/local/bin/gsed",
             "stat": "/usr/local/bin/gstat", "sha1sum": "/usr/local/bin/gsha1sum",
             "python": str(Path(sys.executable).resolve()),
             "python3": str(Path(sys.executable).resolve())}
    directory = work / "native-gnu-bin"
    directory.mkdir(exist_ok=True)
    for name, target in tools.items():
        if not Path(target).is_file() or not os.access(target, os.X_OK):
            raise RuntimeError("Missing native GNU build prerequisite: " + target)
        install_alias(directory, name, target)
    probe = work / "native-format-probe.c"
    contents = "int lurker_lkl_native_format_probe;\n"
    if probe.exists() and probe.read_text() != contents:
        raise RuntimeError("Changed native format probe; retained")
    probe.write_text(contents)
    obj = work / "native-format-probe.o"
    subprocess.run([tools["cc"], "-c", str(probe), "-o", str(obj)], check=True,
                   capture_output=True, timeout=30)
    output = subprocess.check_output([tools["objdump"], "-f", str(obj)],
                                     text=True, timeout=30)
    match = re.search(r"file format (\S+)", output)
    if not match or match.group(1) not in ("elf64-x86-64", "elf64-x86-64-openbsd"):
        raise RuntimeError("Unexpected actual native object format")
    identity = {"tool_paths": tools, "tool_sha256": {name: digest(target) for name, target in tools.items()},
                "format_probe_source_sha256": digest(probe),
                "format_probe_object_sha256": digest(obj), "objdump": output,
                "compiler_version": subprocess.check_output([tools["cc"], "--version"], text=True),
                "format": match.group(1)}
    arguments = ["CC=" + tools["cc"], "HOSTCC=" + tools["cc"],
                 "LD=/usr/bin/ld", "AR=" + tools["ar"], "AS=" + tools["as"],
                 "NM=" + tools["nm"], "OBJCOPY=" + tools["objcopy"],
                 "OBJDUMP=" + tools["objdump"], "LD_FMT=" + match.group(1)]
    return directory, arguments, identity


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=600)
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--gnu-tools", action="store_true", help="Use a private native GNU toolchain overlay; sources stay unmodified")
    parser.add_argument("--target", choices=("library", "boot"), default="library")
    args = parser.parse_args(argv)
    if platform.system() != "OpenBSD" or platform.machine() not in ("amd64", "x86_64"):
        parser.error("Run natively on OpenBSD amd64")
    if os.geteuid() == 0:
        parser.error("Build as an ordinary user; no install or mount is performed")
    if not 1 <= args.timeout <= 7200:
        parser.error("Use a timeout between 1 and 7200 seconds")
    if args.receipt.exists() or args.receipt.is_symlink():
        parser.error("Use a new receipt; existing diagnostics are preserved")
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    if shutil.disk_usage(work).free < 6 * 1024**3:
        parser.error("Provide at least 6 GiB free in this native build work directory")
    receipt = {"revision": REVISION, "source_sha256": SOURCE_SHA256,
               "phase": "unmodified-native-baseline", "uid": os.getuid(),
               "uname": list(platform.uname()), "started": time.time(),
               "script_sha256": digest(__file__), "state": "preparing",
               "target": args.target, "runtime_qualification": False, "gnu_tools": args.gnu_tools}
    def save():
        args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    save()
    try:
        source, count = prepare(work)
        receipt.update(source=str(source), verified_upstream_files=count)
        print(json.dumps({"state": "source-verified", "files": count}), flush=True)
        if args.prepare_only:
            receipt["state"] = "prepared"
        else:
            environment = {k: v for k, v in os.environ.items()
                           if k not in ("LD_PRELOAD", "LD_LIBRARY_PATH", "CFLAGS", "LDFLAGS", "KOPT", "CROSS_COMPILE", "LLVM", "ARCH",
                                        "CC", "HOSTCC", "HOSTCFLAGS", "HOSTLDFLAGS", "LD", "AR", "MAKEFLAGS",
                                        "KCFLAGS", "KCPPFLAGS", "CPATH", "C_INCLUDE_PATH", "LIBRARY_PATH")}
            environment.update(PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
                               LC_ALL="C", PYTHONDONTWRITEBYTECODE="1")
            compiler_arguments = ["CC=/usr/bin/cc", "HOSTCC=/usr/bin/cc"]
            if args.gnu_tools:
                directory, compiler_arguments, identity = gnu_toolchain(work)
                environment["PATH"] = str(directory) + ":" + environment["PATH"]
                receipt["native_toolchain"] = identity
            target = source / ("tools/lkl/liblkl.a" if args.target == "library" else "tools/lkl/tests/boot")
            command = ["/usr/local/bin/gmake", "-j1", "V=1", "kasan=no",
                       *compiler_arguments, str(target)]
            log = args.receipt.with_suffix(".log")
            receipt.update(command=command, log=str(log), state="building")
            save()
            receipt.update(run_build(command, source / "tools/lkl", environment, log, args.timeout))
            generated = [source / name for name in
                         (".config", "tools/lkl/Makefile.conf", "tools/lkl/include/lkl_autoconf.h")]
            receipt["generated_inputs"] = {str(path.relative_to(source)): digest(path)
                                            for path in generated if path.is_file()}
            receipt["state"] = "terminal"
    except BaseException as error:
        receipt.update(state="error", error=repr(error))
        raise
    finally:
        receipt["finished"] = time.time()
        save()
    print(args.receipt, flush=True)
    return int(receipt.get("returncode", 0) != 0)


if __name__ == "__main__":
    raise SystemExit(main())
