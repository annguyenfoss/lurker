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
ROOT = Path(__file__).resolve().parent.parent
HOST_PATCH = "native/openbsd/lkl/host.patch"
HOST_PATCH_SHA256 = "8d418903144152e49276e2b2b52071d31170bd7f0b29bdee926ed1bbbba63be6"
ELF_HEADER_URL = "https://raw.githubusercontent.com/bminor/glibc/glibc-2.42/elf/elf.h"
ELF_HEADER_SHA256 = "fd6e7f3e3c74ee9d64b9f92ce3cc31b34413ca2622a167320ea053ce9a68110f"
PATCH_TARGETS = {"tools/lkl/Makefile.autoconf", "tools/lkl/Makefile",
                 "tools/lkl/lib/posix-host.c", "tools/lkl/include/lkl.h",
                 "arch/lkl/include/uapi/asm/host_ops.h", "arch/lkl/kernel/setup.c"}
HELPER_SHA256 = {
    'openbsd-timer.h': '909eee1a37cb065ccb1e9ed9958b9633d9226ec1411f6fd860d3eba0655c23a5',
    'openbsd-memory.h': '78a87d48e189ec91f81f4e1ed8b996d26a23089c10b4435ea31ff5c8a4e2ddb5',
    'byteswap.h': 'f9259837ff0538b2f0de392d38bfd45ec3dc7f506d8b4b5c22a9aa37a9b18af0',
}


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


def verify_source(archive, source, overrides=None):
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
                if overrides and str(relative) in overrides:
                    expected = hashlib.sha256(overrides[str(relative)]).hexdigest()
                else:
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



def patched_inputs(archive, patch, expected):
    """Apply only reviewed existing-file contexts, exactly and without fuzz."""
    if digest(patch) != expected:
        raise RuntimeError("Host patch hash mismatch")
    chunks = []
    lines = Path(patch).read_bytes().splitlines(keepends=True)
    index = 0
    while index < len(lines):
        if not lines[index].startswith(b"--- a/") or index + 2 >= len(lines):
            raise RuntimeError("Expected host patch file header")
        name = lines[index][6:].strip().decode("ascii")
        if name not in PATCH_TARGETS or lines[index + 1] != ("+++ b/" + name + "\n").encode():
            raise RuntimeError("Unapproved host patch target")
        index += 2
        hunks = []
        while index < len(lines) and not lines[index].startswith(b"--- "):
            header = re.fullmatch(rb"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@[^\n]*\n", lines[index])
            if not header:
                raise RuntimeError("Expected host patch hunk")
            index += 1
            body = []
            while index < len(lines) and not lines[index].startswith((b"@@ ", b"--- ")):
                if lines[index][:1] not in (b" ", b"+", b"-"):
                    raise RuntimeError("Invalid host patch content")
                body.append(lines[index]); index += 1
            before = b"".join(line[1:] for line in body if line[:1] in (b" ", b"-"))
            after = b"".join(line[1:] for line in body if line[:1] in (b" ", b"+"))
            if len(before.splitlines()) != int(header.group(2) or 1) or len(after.splitlines()) != int(header.group(4) or 1):
                raise RuntimeError("Host patch hunk count mismatch")
            hunks.append((before, after))
        if not hunks: raise RuntimeError("Empty host patch file")
        chunks.append((name, hunks))
    wanted = {f"linux-{REVISION}/" + name: name for name, _ in chunks}
    originals = {}
    with tarfile.open(archive, "r|*") as tar:
        for member in tar:
            if member.name in wanted:
                if not member.isfile(): raise RuntimeError("Host patch input is not regular")
                originals[wanted[member.name]] = tar.extractfile(member).read()
    if set(originals) != set(wanted.values()):
        raise RuntimeError("Missing host patch source input")
    result = {}
    for name, hunks in chunks:
        text = result.get(name, originals[name])
        for before, after in hunks:
            if not before or text.count(before) != 1:
                raise RuntimeError("Host patch context mismatch or ambiguity")
            text = text.replace(before, after, 1)
        result[name] = text
    return result



def require_fresh_adapter_source(source):
    generated = (".config", "tools/lkl/Makefile.conf", "tools/lkl/lib/lkl.o",
                 "tools/lkl/liblkl.a", "include/generated/compile.h")
    if any((source / name).exists() or (source / name).is_symlink() for name in generated):
        raise RuntimeError("Adapter needs a fresh uncompiled source tree; prior diagnostics retained")


def prepare(work, adapted=False):
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
    overrides = {}
    identity = None
    if adapted:
        overrides = patched_inputs(archive, ROOT / HOST_PATCH, HOST_PATCH_SHA256)
        identity = {"source_sha256": SOURCE_SHA256, "host_patch_sha256": HOST_PATCH_SHA256,
                    "helper_sha256": HELPER_SHA256, "elf_header_sha256": ELF_HEADER_SHA256}
        for name, expected in HELPER_SHA256.items():
            if digest(ROOT / "native/openbsd/lkl" / name) != expected:
                raise RuntimeError("Native helper hash mismatch")
        marker = source / ".openbsd-lkl-inputs.json"
        if marker.exists():
            if json.loads(marker.read_text()) != identity:
                raise RuntimeError("Different adapter identity; retain source and use a fresh directory")
        else:
            require_fresh_adapter_source(source)
            verify_source(archive, source)
            for name, contents in overrides.items():
                path = source / name
                temporary = path.with_name(path.name + ".openbsd-new")
                with temporary.open("xb") as output: output.write(contents)
                temporary.replace(path)
            for name in ("openbsd-timer.h", "openbsd-memory.h"):
                path = source / "tools/lkl/lib" / name
                with path.open("xb") as output:
                    output.write((ROOT / "native/openbsd/lkl" / name).read_bytes())
            marker.write_text(json.dumps(identity, indent=2) + "\n")
        for name in ("openbsd-timer.h", "openbsd-memory.h"):
            if digest(source / "tools/lkl/lib" / name) != HELPER_SHA256[name]:
                raise RuntimeError("Changed copied native host helper")
    return source, verify_source(archive, source, overrides), identity


def native_overlay(work):
    overlay = work / "openbsd-include"
    overlay.mkdir(exist_ok=True)
    source = ROOT / "native/openbsd/lkl/byteswap.h"
    if digest(source) != HELPER_SHA256[source.name]:
        raise RuntimeError("Native byteswap input hash mismatch")
    target = overlay / source.name
    if target.is_symlink() or (target.exists() and digest(target) != digest(source)):
        raise RuntimeError("Changed native byteswap overlay; retained")
    if not target.exists(): shutil.copyfile(source, target)
    header = work / "glibc-2.42-elf.h"
    if header.is_symlink(): raise RuntimeError("ELF input cache cannot be a symlink")
    if not header.exists():
        temporary = header.with_suffix(".partial")
        with temporary.open("xb") as output:
            with urllib.request.urlopen(ELF_HEADER_URL, timeout=60) as response:
                shutil.copyfileobj(response, output)
        if digest(temporary) != ELF_HEADER_SHA256:
            raise RuntimeError("ELF input hash mismatch; partial retained")
        temporary.rename(header)
    if digest(header) != ELF_HEADER_SHA256: raise RuntimeError("ELF input cache hash mismatch")
    target = overlay / "elf.h"
    if target.is_symlink() or (target.exists() and digest(target) != ELF_HEADER_SHA256):
        raise RuntimeError("Changed ELF include overlay; retained")
    if not target.exists(): shutil.copyfile(header, target)
    return overlay


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


def gnu_toolchain(work, bc=None):
    """Use official native tools privately; derive format from an actual object."""
    tools = {"gcc": "/usr/local/bin/egcc", "cc": "/usr/local/bin/egcc",
             "ar": "/usr/local/bin/gar", "as": "/usr/local/bin/gas",
             "nm": "/usr/local/bin/gnm", "objcopy": "/usr/local/bin/gobjcopy",
             "objdump": "/usr/local/bin/gobjdump", "sed": "/usr/local/bin/gsed",
             "stat": "/usr/local/bin/gstat", "sha1sum": "/usr/local/bin/gsha1sum",
             "python": str(Path(sys.executable).resolve()),
             "python3": str(Path(sys.executable).resolve())}
    if bc is not None:
        path = Path(bc)
        if not path.is_absolute() or not path.is_file() or not os.access(path, os.X_OK):
            raise RuntimeError("Use an absolute executable native compatible bc path")
        tools["bc"] = str(path.resolve(strict=True))
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
    if bc is not None:
        identity["bc_version"] = subprocess.check_output([tools["bc"], "--version"], text=True, timeout=30)
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
    parser.add_argument("--bc", type=Path, help="Compatible native bc build tool; base OpenBSD bc is not compatible")
    parser.add_argument("--adapted", action="store_true", help="Apply reviewed OpenBSD host-only inputs; no mount approval")
    parser.add_argument("--gnu-tools", action="store_true", help="Use a private native GNU toolchain overlay; sources stay unmodified")
    parser.add_argument("--target", choices=("library", "boot"), default="library")
    args = parser.parse_args(argv)
    if platform.system() != "OpenBSD" or platform.machine() not in ("amd64", "x86_64"):
        parser.error("Run natively on OpenBSD amd64")
    if os.geteuid() == 0:
        parser.error("Build as an ordinary user; no install or mount is performed")
    if not 1 <= args.timeout <= 7200:
        parser.error("Use a timeout between 1 and 7200 seconds")
    if args.gnu_tools and not args.bc:
        parser.error("Provide --bc for the native compatible GNU bc prerequisite")
    if args.bc and not args.gnu_tools:
        parser.error("--bc is used with --gnu-tools")
    if args.adapted and not args.gnu_tools:
        parser.error("The reviewed adapter requires --gnu-tools")
    if args.receipt.exists() or args.receipt.is_symlink():
        parser.error("Use a new receipt; existing diagnostics are preserved")
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    if shutil.disk_usage(work).free < 6 * 1024**3:
        parser.error("Provide at least 6 GiB free in this native build work directory")
    receipt = {"revision": REVISION, "source_sha256": SOURCE_SHA256,
               "phase": "reviewed-native-host-adapter" if args.adapted else "unmodified-native-baseline", "uid": os.getuid(),
               "uname": list(platform.uname()), "started": time.time(),
               "script_sha256": digest(__file__), "state": "preparing",
               "target": args.target, "runtime_qualification": False, "gnu_tools": args.gnu_tools}
    def save():
        args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    save()
    try:
        source, count, identity = prepare(work, args.adapted)
        receipt.update(source=str(source), verified_upstream_files=count, adapter_identity=identity)
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
                directory, compiler_arguments, identity = gnu_toolchain(work, args.bc)
                environment["PATH"] = str(directory) + ":" + environment["PATH"]
                receipt["native_toolchain"] = identity
            if args.adapted:
                overlay = native_overlay(work)
                environment["CFLAGS"] = "-I" + str(overlay) + " -include endian.h"
                environment["HOSTCFLAGS"] = "-O2 -I" + str(overlay)
                receipt["native_overlay"] = {str(p): digest(p) for p in overlay.iterdir()}
                receipt["native_sdk"] = {str(p): digest(p) for p in
                                         map(Path, ["/usr/include/endian.h", "/usr/include/pthread_np.h",
                                                    "/usr/include/sys/mman.h", "/usr/include/sys/signal.h",
                                                    "/usr/include/sys/siginfo.h"])}
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
            if (source / ".config").is_file():
                receipt["filesystem_configuration"] = [line for line in (source / ".config").read_text().splitlines()
                                                        if re.match(r"(?:# )?CONFIG_(?:BTRFS_FS|XFS_FS|MMU|KASAN|NET)(?:=| )", line)]
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
