#!/usr/bin/env python3
"""Build the separate native Darwin mkfs.xfs prerequisite, without executing it.

The release tar and v7.3.0 tag were signed by the pinned kernel.org maintainer
key; all 738 tracked files matched commit SOURCE_COMMIT byte for byte. Recheck
the signed tar and that complete source-content inventory before patching. The
build uses native SDK APIs, never Linux headers, syscall numbers or ABI shims.
Nothing is installed and no storage is formatted or mounted by this builder.
"""
from __future__ import annotations

import argparse
import base64
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tarfile
import time
import urllib.request

SOURCE_COMMIT = "e02d0a8efd81d454481641c9bd5680b5febb1261"
SOURCE_MANIFEST_SHA256 = "a9c6e3f0430e6f01ceca9a11df4f7713efda820e92fa8aac6af0806cb2a93b74"
SIGNER = "AE1B2A9562721A6FC4307C1F46A7EA18AC33E108"
SOURCES = {
    "xfsprogs.tar.gz": (
        "https://www.kernel.org/pub/linux/utils/fs/xfs/xfsprogs/xfsprogs-7.3.0.tar.gz",
        "715d4d91a1753d255080e7059db45273b1908f15324c87ca812d5d9ef08a44c0",
    ),
    "xfsprogs.tar.sign": (
        "https://www.kernel.org/pub/linux/utils/fs/xfs/xfsprogs/xfsprogs-7.3.0.tar.sign",
        "85ccf3ce59cec2023c3eecf31051011a8af6e8b25ef1f21b4f7fcd92236135d1",
    ),
    "maintainer-key.asc": (
        "https://kernel.googlesource.com/pub/scm/docs/kernel/pgpkeys/+/"
        "f3929937e22ba70c987bbf0dc89acc6f0ce9a53e/keys/46A7EA18AC33E108.asc?format=TEXT",
        "fa1561e87ce7dec01bb793aef29b14fb2b58edcc2ef492c498d8dd6fff9b5b5f",
    ),
    "inih.tar.gz": (
        "https://codeload.github.com/benhoyt/inih/tar.gz/refs/tags/r63",
        "5d7967de62c5bfdc4f9f5ab06ce44c9e498bae021c7dac5792c0696e76aa9963",
    ),
    "urcu.tar.bz2": (
        "https://lttng.org/files/urcu/userspace-rcu-0.15.7.tar.bz2",
        "2556b83adc0f9b3ac8024e613e17d014d04c4c49110604ce55fcb14eae32edd3",
    ),
}
FROG_OBJECTS = (
    "avl64", "bitmap", "convert", "crc32", "fsgeom", "fsproperties",
    "list_sort", "logging", "projects", "radix-tree", "randbytes", "util", "darwin",
)


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def fetch(name: str, cache: Path, offline: bool) -> Path:
    url, expected = SOURCES[name]
    path = cache / name
    if path.is_symlink():
        raise ValueError(f"Refusing a source-cache symlink: {path}")
    if not path.exists():
        if offline:
            raise ValueError(f"Missing verified offline input: {path}")
        data = urllib.request.urlopen(url, timeout=45).read()
        if name == "maintainer-key.asc":
            data = base64.b64decode(data.strip(), validate=True)
        if hashlib.sha256(data).hexdigest() != expected:
            raise ValueError(f"Downloaded source hash mismatch: {name}")
        path.write_bytes(data)
    if not path.is_file() or path.is_symlink() or digest(path) != expected:
        raise ValueError(f"Source hash mismatch: {path}")
    return path


def extract(archive: Path, output: Path) -> Path:
    """Extract one archive root, rejecting links and paths outside that root."""
    output.mkdir(mode=0o700)
    with tarfile.open(archive) as stream:
        members = stream.getmembers()
        roots = set()
        for member in members:
            path = Path(member.name)
            if path.is_absolute() or ".." in path.parts or not path.parts:
                raise ValueError(f"Unsafe source archive member: {member.name}")
            if not member.isfile() and not member.isdir():
                raise ValueError(f"Non-file source archive member: {member.name}")
            roots.add(path.parts[0])
        if len(roots) != 1:
            raise ValueError("Expected a single source archive root")
        stream.extractall(output, filter="data")
    return output / roots.pop()


def verify_source_tree(source: Path) -> dict:
    names = (source / ".gitcensus").read_text().splitlines()
    if len(names) != 738 or len(names) != len(set(names)):
        raise ValueError("Unexpected pinned Git source inventory")
    entries = []
    for name in sorted(names):
        path = Path(name)
        if path.is_absolute() or ".." in path.parts:
            raise ValueError("Unsafe pinned Git source path")
        item = source / path
        if not item.is_file() or item.is_symlink():
            raise ValueError(f"Missing pinned Git source: {name}")
        entries.append([name, digest(item)])
    value = hashlib.sha256(json.dumps(entries, separators=(",", ":")).encode()).hexdigest()
    if value != SOURCE_MANIFEST_SHA256:
        raise ValueError("Release contents differ from the authenticated pinned Git commit")
    return {"commit": SOURCE_COMMIT, "tracked_files": len(entries), "manifest_sha256": value}


def libxfs_sources(source: Path) -> list[str]:
    text = (source / "libxfs/Makefile").read_text()
    match = re.search(r"^CFILES = (.*?)(?=\n\n)", text, re.M | re.S)
    if not match:
        raise ValueError("Missing upstream libxfs source list")
    names = match.group(1).replace("\\\n", " ").split()
    if any(not re.fullmatch(r"[a-z0-9_]+\.c", name) for name in names):
        raise ValueError("Unexpected libxfs source-list syntax")
    # This object consumes Linux block-zone reports. The native candidate
    # rejects zoned layouts before opening storage; no formatter caller uses it.
    return ["libxfs/" + name for name in names if name != "xfs_zones.c"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--cc", default="clang")
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine() not in {"arm64", "x86_64"}:
        parser.error("Build only on native Darwin arm64 or x86_64")
    if not os.getuid() or os.getuid() != os.geteuid() or os.getgid() != os.getegid():
        parser.error("Build only as the unelevated login user")
    output = args.output_dir.expanduser().absolute()
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    cache = args.cache_dir.expanduser().resolve()
    cache.mkdir(mode=0o700, parents=True, exist_ok=True)
    logs = output / "logs"
    logs.mkdir(mode=0o700)
    record = {"schema": 1, "scope": "separate host formatter build only; no installation or execution",
              "status": "running", "started": time.time(), "uid": os.getuid(), "euid": os.geteuid(),
              "architecture": platform.machine(), "source_commit": SOURCE_COMMIT,
              "commands": [], "jobs": 1}
    receipt = output / "build-manifest.json"

    def save():
        temporary = receipt.with_suffix(".tmp")
        temporary.write_text(json.dumps(record, indent=2) + "\n")
        temporary.replace(receipt)

    environment = {k: v for k, v in os.environ.items()
                   if k not in {"CFLAGS", "CPPFLAGS", "CXXFLAGS", "LDFLAGS", "DYLD_LIBRARY_PATH",
                                "DYLD_INSERT_LIBRARIES", "CPATH", "LIBRARY_PATH", "SDKROOT"}}

    def run(argv, *, cwd=None, data=None, capture=False):
        name = f"{len(record['commands']):03d}.log"
        command = {"argv": [str(a) for a in argv], "cwd": str(cwd or output), "log": name}
        record["commands"].append(command)
        save()
        result = subprocess.run(command["argv"], cwd=cwd or output, env=environment,
                                input=data, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        (logs / name).write_bytes(result.stdout)
        command.update(exit_code=result.returncode, log_sha256=digest(logs / name))
        save()
        if result.returncode:
            raise RuntimeError(f"Command failed; retained {logs / name}: {command['argv']}")
        return result.stdout.decode() if capture else None

    try:
        files = {name: fetch(name, cache, args.offline) for name in SOURCES}
        record["sources"] = {name: {"url": SOURCES[name][0], "sha256": digest(path)}
                             for name, path in files.items()}
        gnupg = output / "gnupg"
        gnupg.mkdir(mode=0o700)
        keyring = output / "maintainer-key.gpg"
        run(["gpg", "--no-options", "--batch", "--homedir", gnupg,
             "--output", keyring, "--dearmor", files["maintainer-key.asc"]])
        signature = run(["gpgv", "--homedir", gnupg, "--status-fd=1", "--keyring", keyring,
                         files["xfsprogs.tar.sign"], "-"],
                        data=gzip.decompress(files["xfsprogs.tar.gz"].read_bytes()), capture=True)
        if f"[GNUPG:] VALIDSIG {SIGNER} " not in signature:
            raise ValueError("Release signature does not match the pinned maintainer")
        record["signer"] = SIGNER
        source = extract(files["xfsprogs.tar.gz"], output / "source")
        record["verified_source"] = verify_source_tree(source)
        patch = Path(__file__).resolve().parents[1] / "native/macos/xfsprogs-darwin.patch"
        record["patch"] = {"path": str(patch), "sha256": digest(patch)}
        run(["git", "apply", "--check", patch], cwd=source)
        run(["git", "apply", patch], cwd=source)
        cc = shutil.which(args.cc)
        if not cc:
            raise ValueError(f"Native compiler not found: {args.cc}")
        record["compiler"] = run([cc, "--version"], capture=True)
        environment["CC"] = cc
        sdk = run(["xcrun", "--show-sdk-path"], capture=True).strip()
        record["sdk"] = {"path": sdk, "version": run(["xcrun", "--show-sdk-version"], capture=True).strip()}
        urcu = extract(files["urcu.tar.bz2"], output / "urcu")
        run(["./configure", "--disable-shared", "--enable-static", "--disable-maintainer-mode"], cwd=urcu)
        run(["make", "-C", "src", "-j1", "liburcu.la", "liburcu-common.la"], cwd=urcu)
        urcu_include = urcu / "include"
        urcu_libs = [urcu / "src/.libs/liburcu.a", urcu / "src/.libs/liburcu-common.a"]
        record["urcu"] = {"include": str(urcu_include), "libraries": {
            str(path): digest(path) for path in urcu_libs}}
        ini = extract(files["inih.tar.gz"], output / "inih")
        headers = source / "include/xfs"
        headers.mkdir()
        for directory in [source / "include", source / "libxfs"]:
            for header in directory.glob("*.h"):
                target = headers / header.name
                if not target.exists():
                    target.symlink_to(header)
        flags = ["-std=gnu11", "-O2", "-funsigned-char", "-fno-strict-aliasing",
                 "-Werror=implicit-function-declaration", "-Wno-address-of-packed-member",
                 "-DLURKER_DARWIN_MKFS", "-D_LGPL_SOURCE", "-DHAVE_LIBURCU_ATOMIC64",
                 '-DVERSION="7.3.0"', '-DPACKAGE="xfsprogs"', '-DLOCALEDIR=""',
                 "-isysroot", sdk, "-I" + str(source / "include"),
                 "-I" + str(source / "libxfs/stubs"), "-I" + str(source / "libxfs"),
                 "-I" + str(source), "-I" + str(urcu_include), "-I" + str(ini)]
        objects = output / "objects"
        objects.mkdir()
        generator = output / "gen_crc32table"
        run([cc, "-isysroot", sdk, source / "libfrog/gen_crc32table.c", "-o", generator])
        table = run([generator], capture=True)
        (source / "libfrog/crc32table.h").write_text(table)
        units = libxfs_sources(source) + ["libfrog/" + n + ".c" for n in FROG_OBJECTS]
        units += ["mkfs/proto.c", "mkfs/xfs_mkfs.c"]
        compiled = []
        archives = {"libxfs": [], "libfrog": []}
        for unit in units:
            obj = objects / (unit.replace("/", "_") + ".o")
            # libxfs/Makefile explicitly builds with DEBUG=-DNDEBUG upstream.
            local_flags = ["-DNDEBUG"] if unit.startswith("libxfs/") else []
            run([cc, *flags, *local_flags, "-MD", "-MF", obj.with_suffix(".d"),
                 "-c", source / unit, "-o", obj])
            group = unit.split("/")[0]
            if group in archives:
                archives[group].append(obj)
            else:
                compiled.append(obj)
        forbidden = []
        for dependency in objects.glob("*.d"):
            text = dependency.read_text()
            if re.search(r"/(linux|asm)/", text) or str(source / "include/linux.h") in text:
                forbidden.append(str(dependency))
        record["host_header_audit"] = {"linux_host_header_dependencies": forbidden}
        if forbidden:
            raise ValueError("Darwin formatter compilation included Linux host headers")
        for name, members in archives.items():
            run(["ar", "rcs", output / (name + ".a"), *members])
        ini_object = objects / "ini.o"
        run([cc, "-O2", "-isysroot", sdk, "-c", ini / "ini.c", "-o", ini_object])
        binary = output / "mkfs.xfs"
        run([cc, "-isysroot", sdk, *compiled, ini_object, output / "libxfs.a",
             output / "libfrog.a", output / "libxfs.a", *urcu_libs,
             "-framework", "DiskArbitration", "-framework", "CoreFoundation", "-o", binary])
        record["native_linkage"] = run(["otool", "-L", binary], capture=True)
        record["native_architectures"] = run(["lipo", "-archs", binary], capture=True).strip()
        if record["native_architectures"] != platform.machine():
            raise ValueError("Formatter architecture differs from this native build host")
        notices = output / "licenses"
        notices.mkdir()
        for name in ["GPL-2.0", "LGPL-2.1"]:
            shutil.copy2(source / "LICENSES" / name, notices / ("xfsprogs-" + name))
        shutil.copy2(ini / "LICENSE.txt", notices / "inih-LICENSE.txt")
        urcu_source = extract(files["urcu.tar.bz2"], output / "urcu-notices")
        shutil.copytree(urcu_source / "LICENSES", notices / "userspace-rcu")
        shutil.copy2(urcu_source / "LICENSE.md", notices / "userspace-rcu-LICENSE.md")
        record.update(status="built-not-executed", finished=time.time(),
                      binary={"path": str(binary), "sha256": digest(binary)},
                      source_units={unit: digest(source / unit) for unit in units},
                      licenses={str(p.relative_to(notices)): digest(p) for p in notices.rglob("*") if p.is_file()})
        save()
        print(json.dumps({"status": record["status"], "receipt": str(receipt), "binary": record["binary"]}))
        return 0
    except Exception as error:
        record.update(status="failed", error=str(error), finished=time.time())
        save()
        raise


if __name__ == "__main__":
    raise SystemExit(main())
