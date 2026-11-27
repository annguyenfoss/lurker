#!/usr/bin/env python3
"""Capture an unprivileged native NetBSD LKL diagnostic build; never install.

This external filesystem prerequisite is separate from Lurker's backend bundle.
An artifact from this script is not permission to use it for privileged mounts.
"""
import argparse
import hashlib
import importlib.util
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
HOST_PATCH = "native/netbsd/lkl/host.patch"
HOST_PATCH_SHA256 = "8fd825c96c6fb0c16db834d3e406a367fcf5eaf1610696d1d39d8b392b333780"
TIMER_HEADER = "native/netbsd/lkl/netbsd-timer.h"
TIMER_HEADER_SHA256 = "8b302afe51ba85e2a83b9fd22df81257358b5731e725ace276a137a127355197"
REFUSE_PATCH = "native/netbsd/lkl/refuse-bridge.patch"
REFUSE_PATCH_SHA256 = "a41d4d06add1f3be118702e402ae1cdf00e7702145a34881cc3272a7cb445c41"
REFUSE_SOURCE_SHA256 = "2b42f71579ad9473b8eada3cc921dccad694b648b97fc7150d09cf819b34438a"
PUFFS_PROVIDER_SHA256 = "8705ccd3ff268666b0973905b44648ef014efcca10051355677f02b0f5a64099"
FLUSH_PATCH = "native/macos/lkl-darwin-flush.patch"
FLUSH_PATCH_SHA256 = "0dd2c89942ede5a2c373191601054dcd9b2e4e17c4e1b1c568d02a2bfc8fcf8b"
ELF_HEADER_URL = "https://raw.githubusercontent.com/bminor/glibc/glibc-2.42/elf/elf.h"
ELF_HEADER_SHA256 = "fd6e7f3e3c74ee9d64b9f92ce3cc31b34413ca2622a167320ea053ce9a68110f"
PATCH_TARGETS = {"tools/lkl/Makefile", "tools/lkl/Makefile.autoconf", "tools/lkl/Targets",
                 "tools/lkl/lib/posix-host.c", "tools/lkl/lib/virtio_blk.c", "tools/lkl/lklfuse.c",
                 "tools/lkl/include/lkl.h", "arch/lkl/kernel/setup.c",
                 "arch/lkl/include/uapi/asm/host_ops.h"}


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def fetch(path, url=None, expected=None):
    """Only the pinned archive is accepted, including on cache hits."""
    path = Path(path)
    url = SOURCE_URL if url is None else url
    expected = SOURCE_SHA256 if expected is None else expected
    if path.is_symlink():
        raise RuntimeError("Source cache must not be a symlink")
    if not path.exists():
        temporary = path.with_suffix(path.suffix + ".partial")
        with temporary.open("xb") as output:
            with urllib.request.urlopen(url, timeout=90) as response:
                shutil.copyfileobj(response, output)
        if digest(temporary) != expected:
            raise RuntimeError("Source download failed SHA-256 verification; partial retained")
        temporary.rename(path)
    if digest(path) != expected:
        raise RuntimeError("Source cache failed SHA-256 verification")
    return path


def verify_source(archive, source, overrides=None):
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
                if overrides and str(relative) in overrides:
                    expected = hashlib.sha256(overrides[str(relative)]).hexdigest()
                else:
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


def patched_inputs(archive, patches):
    """Render exact reviewed contexts against the pinned archive, without fuzz."""
    result = {}
    with tarfile.open(archive) as tar:
        for patch, expected in patches:
            if digest(patch) != expected:
                raise RuntimeError("Local host patch failed SHA-256 verification")
            lines = patch.read_bytes().splitlines(keepends=True)
            index = 0
            while index < len(lines):
                if not lines[index].startswith(b"--- a/") or index + 2 >= len(lines):
                    raise RuntimeError("Expected reviewed patch file header")
                name = lines[index][6:].strip().decode("ascii")
                if name not in PATCH_TARGETS or lines[index + 1] != ("+++ b/" + name + "\n").encode():
                    raise RuntimeError("Unapproved host patch target")
                original = result.get(name)
                if original is None:
                    original = tar.extractfile(f"linux-{REVISION}/{name}").read()
                index += 2
                while index < len(lines) and not lines[index].startswith(b"--- "):
                    if not re.fullmatch(rb"@@ -\d+(?:,\d+)? \+\d+(?:,\d+)? @@[^\n]*\n", lines[index]):
                        raise RuntimeError("Expected reviewed patch hunk")
                    index += 1
                    body = []
                    while index < len(lines) and not lines[index].startswith((b"@@ ", b"--- ")):
                        if lines[index][:1] not in (b" ", b"+", b"-"):
                            raise RuntimeError("Invalid reviewed patch line")
                        body.append(lines[index])
                        index += 1
                    before = b"".join(line[1:] for line in body if line[:1] in (b" ", b"-"))
                    after = b"".join(line[1:] for line in body if line[:1] in (b" ", b"+"))
                    if not before or original.count(before) != 1:
                        raise RuntimeError("Reviewed host patch context is not unique")
                    original = original.replace(before, after, 1)
                result[name] = original
    return result


def prepare(work, adapted=False):
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
    overrides = {}
    identity = None
    if adapted:
        identity = {"source_sha256": SOURCE_SHA256, "host_patch_sha256": HOST_PATCH_SHA256,
                    "flush_patch_sha256": FLUSH_PATCH_SHA256,
                    "timer_header_sha256": TIMER_HEADER_SHA256}
        header = ROOT / TIMER_HEADER
        if digest(header) != TIMER_HEADER_SHA256:
            raise RuntimeError("Local native timer header failed SHA-256 verification")
        overrides = patched_inputs(archive, [(ROOT / HOST_PATCH, HOST_PATCH_SHA256),
                                            (ROOT / FLUSH_PATCH, FLUSH_PATCH_SHA256)])
        marker = source / ".netbsd-lkl-inputs.json"
        if marker.exists():
            if json.loads(marker.read_text()) != identity:
                raise RuntimeError("Different adapter cache; preserve it and choose a fresh work directory")
            install_timer_header(source, header, allow_create=False)
        else:
            verify_source(archive, source)
            for name, contents in overrides.items():
                path = source / name
                temporary = path.with_name(path.name + ".netbsd-new")
                with temporary.open("xb") as output:
                    output.write(contents)
                temporary.replace(path)
            install_timer_header(source, header, allow_create=True)
            marker.write_text(json.dumps(identity, indent=2) + "\n")
    inputs = verify_source(archive, source, overrides)
    return source, inputs, identity


def install_timer_header(source, header, allow_create):
    target = source / "tools/lkl/lib/netbsd-timer.h"
    if target.exists() or target.is_symlink():
        if target.is_symlink() or not target.is_file() or digest(target) != TIMER_HEADER_SHA256:
            raise RuntimeError("Modified cached native timer input")
    elif allow_create:
        contents = header.read_bytes()
        if hashlib.sha256(contents).hexdigest() != TIMER_HEADER_SHA256:
            raise RuntimeError("Native timer input changed before copying")
        with target.open("xb") as output:
            output.write(contents)
    else:
        raise RuntimeError("Missing cached native timer input")


def native_overlay(work):
    overlay = work / "netbsd-include"
    fuse = overlay / "fuse3"
    fuse.mkdir(parents=True, exist_ok=True)
    for name in ("fuse.h", "fuse_opt.h", "fuse_lowlevel.h"):
        path = fuse / name
        target = "/usr/include/" + name
        if path.exists() or path.is_symlink():
            if not path.is_symlink() or os.readlink(path) != target:
                raise RuntimeError("Unexpected native ReFUSE overlay")
        else:
            path.symlink_to(target)
    (overlay / "byteswap.h").write_text("#include <sys/bswap.h>\n#define bswap_16 bswap16\n#define bswap_32 bswap32\n#define bswap_64 bswap64\n")
    (overlay / "endian.h").write_text("#include <sys/endian.h>\n")
    # modpost parses several target architectures even for a native x86 build.
    # This unmodified portable ELF file-format header has only a stdint.h
    # dependency; it supplies no libc implementation or runtime ABI.
    header = fetch(work / "glibc-2.42-elf.h", ELF_HEADER_URL, ELF_HEADER_SHA256)
    shutil.copyfile(header, overlay / "elf.h")
    return overlay


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
            # Do not poll/wait here: the unreaped session leader reserves its
            # PID/PGID while descendants receive a brief termination grace.
            # A leader can exit before a compiler child that ignores TERM.
            time.sleep(0.2)
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait(timeout=10)
            return {"pid": child.pid, "returncode": child.returncode, "timed_out": True}


def build_refuse_bridge(cache, output, timeout):
    """Build a separately named, uninstalled library for this diagnostic only."""
    if output.exists() or output.is_symlink():
        raise RuntimeError("Use a new isolated ReFUSE output path")
    parent = output.parent
    metadata = parent.stat()
    if parent.resolve() != parent or metadata.st_uid != os.geteuid() or metadata.st_mode & 0o022:
        raise RuntimeError("ReFUSE output parent must be real, caller-owned and private")
    patch = ROOT / REFUSE_PATCH
    if patch.is_symlink() or digest(patch) != REFUSE_PATCH_SHA256:
        raise RuntimeError("Diagnostic ReFUSE patch checksum differs")
    # Reuse the independently pinned native source acquisition/access patch.
    # This does not run that builder's installation or replace its output.
    spec = importlib.util.spec_from_file_location("native_fuse_inputs", ROOT / "scripts/build-netbsd-fuse-prerequisite.py")
    base = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(base)
    if base.PATCH.is_symlink() or digest(base.PATCH) != base.PATCH_SHA256:
        raise RuntimeError("Native access prerequisite patch checksum differs")
    if digest("/usr/lib/libpuffs.so.2.0") != PUFFS_PROVIDER_SHA256:
        raise RuntimeError("This diagnostic requires the exact qualified native puffs provider")
    cache.mkdir(parents=True, exist_ok=True)
    original = base.acquire(cache)
    output.mkdir(mode=0o700)
    source = output / "source"
    shutil.copytree(original, source)
    for name, expected in base.SOURCE_SHA256.items():
        if digest(source / name) != expected:
            raise RuntimeError("Copied native ReFUSE input differs: " + name)
    commands = []
    deadline = time.monotonic() + timeout
    environment = {key: value for key, value in os.environ.items() if not key.startswith("LD_")}
    environment.update(PATH="/usr/pkg/bin:/usr/bin:/bin", LC_ALL="C")

    def run(command):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError("Isolated ReFUSE build deadline reached")
        result = run_build(command, source, environment,
                           output / ("command-%02d.log" % len(commands)), remaining)
        commands.append({"command": command, **result})
        if result["returncode"]:
            raise RuntimeError("Isolated ReFUSE command failed; preserve its logs")

    for current in (base.PATCH, patch):
        run(["/usr/bin/patch", "--batch", "--forward", "--fuzz=0", "-p1", "-i", str(current)])
        if current == base.PATCH:
            for name, expected in base.PATCHED_SHA256.items():
                if digest(source / name) != expected:
                    raise RuntimeError("Native access prerequisite output differs: " + name)
    root = source / "lib/librefuse"
    if digest(root / "refuse.c") != REFUSE_SOURCE_SHA256:
        raise RuntimeError("Diagnostic ReFUSE output differs")
    objects = output / "objects"
    objects.mkdir()
    compiled = []
    for name in base.LIBRARIES[2][2]:
        target = objects / (name.replace("/", "-") + ".o")
        run(["/usr/bin/cc", "-O2", "-g", "-fPIC", "-D_KERNTYPES", "-D_REENTRANT",
             "-I" + str(root), "-I" + str(source / "lib/libpuffs"),
             "-c", str(root / name), "-o", str(target)])
        compiled.append(str(target))
    library = output / "librefuse-lkl.so.2"
    run(["/usr/bin/cc", "-shared", "-Wl,-soname,librefuse-lkl.so.2", "-o", str(library),
         *compiled, "-lpuffs", "-lpthread", "-lutil"])
    (output / "librefuse-lkl.so").symlink_to(library.name)
    probe = ("import ctypes;lib=ctypes.CDLL(" + repr(str(library)) + ");"
             "fn=lib.refuse_lkl_bridge_version;fn.restype=ctypes.c_uint;assert fn()==1")
    run([sys.executable, "-c", probe])
    manifest = {"kind": "uninstalled-netbsd-lkl-refuse-diagnostic", "uid": os.getuid(),
                "base_revision": base.REVISION, "base_patch_sha256": base.PATCH_SHA256,
                "bridge_patch_sha256": REFUSE_PATCH_SHA256, "bridge_source_sha256": REFUSE_SOURCE_SHA256,
                "library_path": str(library), "library_sha256": digest(library),
                "puffs_provider_sha256": PUFFS_PROVIDER_SHA256, "commands": commands,
                "dependencies": subprocess.check_output(["/usr/bin/ldd", str(library)], text=True, env=environment),
                "elf": subprocess.check_output(["/usr/bin/readelf", "-h", "-d", str(library)], text=True, env=environment)}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=600)
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--adapted", action="store_true", help="Apply diagnostic-only native host inputs; no runtime qualification")
    parser.add_argument("--target", choices=("lklfuse", "boot"), default="lklfuse")
    parser.add_argument("--refuse-output", type=Path, help="Build only the isolated diagnostic ReFUSE bridge; never install")
    parser.add_argument("--refuse-cache", type=Path, help="Pinned native source cache for --refuse-output")
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
               "phase": "native-adapter-diagnostic" if args.adapted else "unmodified-baseline", "uid": os.getuid(),
               "uname": list(platform.uname()), "started": time.time(),
               "script_sha256": digest(__file__), "state": "preparing"}
    receipt["target"] = args.target
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    try:
        if args.refuse_output:
            receipt["phase"] = "isolated-refuse-diagnostic"
            receipt["bridge"] = build_refuse_bridge(
                (args.refuse_cache or work / "refuse-cache").absolute(),
                args.refuse_output.absolute(), args.timeout)
            receipt["state"] = "terminal"
            receipt["returncode"] = 0
            return 0
        source, count, identity = prepare(work, args.adapted)
        receipt.update(source=str(source), verified_upstream_files=count, adapter_identity=identity)
        if args.prepare_only:
            receipt["state"] = "prepared"
        else:
            environment = os.environ.copy()
            environment.update(PATH="/usr/pkg/bin:/usr/bin:/bin:/usr/sbin:/sbin", LC_ALL="C")
            if args.adapted:
                overlay = native_overlay(work)
                native_bin = work / "netbsd-bin"
                native_bin.mkdir(exist_ok=True)
                interpreter = str(Path(sys.executable).resolve(strict=True))
                for name, target in {"python3": interpreter, "sed": "/usr/pkg/bin/gsed",
                                     "stat": "/usr/pkg/bin/gstat", "sha1sum": "/usr/pkg/bin/gsha1sum"}.items():
                    alias = native_bin / name
                    if alias.exists() or alias.is_symlink():
                        if not alias.is_symlink() or os.readlink(alias) != target:
                            raise RuntimeError("Unexpected private native build alias")
                    else:
                        alias.symlink_to(target)
                environment["PATH"] = str(native_bin) + ":" + environment["PATH"]
                receipt["python_interpreter"] = {"path": interpreter, "sha256": digest(interpreter)}
                receipt["private_tools"] = {path.name: {"path": os.readlink(path), "sha256": digest(path)}
                                            for path in native_bin.iterdir()}
                # lkl.h hides __unused while importing its UAPI structures;
                # preload native endian inlines while the native macro exists.
                environment["CFLAGS"] = f"-I{overlay} -D_NETBSD_SOURCE -include sys/endian.h"
                environment["HOSTCFLAGS"] = f"-O2 -I{overlay}"
                receipt["overlay_inputs"] = {str(path.relative_to(overlay)): digest(path)
                                              for path in overlay.rglob("*") if path.is_file()}
                headers = [Path("/usr/include") / name for name in
                           ("sys/bswap.h", "sys/endian.h", "sys/mman.h", "sys/sysctl.h", "sys/proc.h", "sys/siginfo.h", "signal.h")]
                headers.extend(Path("/usr/include/refuse").glob("*.h"))
                receipt["native_sdk_inputs"] = {str(path): digest(path) for path in headers}
                receipt["toolchain"] = {name: subprocess.check_output(command, text=True)
                                        for name, command in
                                        (("compiler", ["/usr/bin/gcc", "--version"]),
                                         ("target", ["/usr/bin/gcc", "-dumpmachine"]),
                                         ("linker", ["/usr/bin/ld", "--version"]))}
                # The unmodified build creates this empty generated placeholder.
                stat_alias = source / "tools/lkl/bin/stat"
                if stat_alias.is_file() and not stat_alias.is_symlink() and stat_alias.stat().st_size == 0:
                    stat_alias.unlink()
            relative = "tools/lkl/tests/boot" if args.target == "boot" else "tools/lkl/lklfuse"
            command = ["gmake", "-j1", "V=1", str(source / relative)]
            log = args.receipt.with_suffix(".log")
            receipt.update(command=command, log=str(log), state="building")
            args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
            receipt.update(run_build(command, source / "tools/lkl", environment, log, args.timeout))
            generated = [source / name for name in
                         (".config", "tools/lkl/Makefile.conf", "tools/lkl/include/lkl_autoconf.h",
                          "arch/lkl/include/generated/uapi/asm/syscall_defs.h")]
            receipt["generated_inputs"] = {str(path.relative_to(source)): digest(path)
                                            for path in generated if path.is_file()}
            if (source / ".config").is_file():
                receipt["filesystem_configuration"] = [line for line in (source / ".config").read_text().splitlines()
                                                        if re.match(r"(?:# )?CONFIG_(?:BTRFS_FS|XFS_FS|MMU|KASAN|UFS_FS|UFS_FS_WRITE|NET)(?:=| )", line)]
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
