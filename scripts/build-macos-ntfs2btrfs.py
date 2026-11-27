#!/usr/bin/env python3
"""Build a separate Darwin fresh-file formatter candidate, without installation.

This script never formats, mounts, opens a device, elevates, or adds a tool to an
application bundle. The guarded converter is an unqualified host prerequisite.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import signal
import stat
import subprocess
import sys
import tarfile
import time

ROOT = Path(__file__).resolve().parents[1]
COMMIT = "a13f5713f6a49522e4fac896c1982884fb784799"
ARCHIVE_SHA256 = "344809bb4bebe5f27e13082b6dd9bdf7833af429de25936b44e4c3972c6aeab3"
SOURCE_URL = f"https://codeload.github.com/maharmstone/ntfs2btrfs/tar.gz/{COMMIT}"
PATCH = ROOT / "native/macos/ntfs2btrfs-darwin.patch"
PATCH_SHA256 = "6bb7fd24cf83a3f15374e1d38efc0a01336d3d202060418b75637a2344ac1fd9"
PATCH_FILES = {
    "CMakeLists.txt", "src/crc32c.cpp", "src/rollback.cpp", "src/ntfs.cpp",
    "src/ntfs2.cpp", "src/ntfs2btrfs.cpp", "src/darwin_crc_check.cpp",
}


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def require_digest(path: Path, expected: str) -> None:
    if not path.is_file() or digest(path) != expected:
        raise ValueError(f"Pinned SHA-256 mismatch: {path}")


def safe_members(archive: tarfile.TarFile) -> list[tuple[tarfile.TarInfo, str]]:
    """Validate the entire archive before extracting any of its source bytes."""
    result = []
    seen = set()
    prefix = "ntfs2btrfs-" + COMMIT
    for item in archive.getmembers():
        name = item.name.rstrip("/")
        parts = PurePosixPath(name).parts
        if (not parts or parts[0] != prefix or PurePosixPath(name).is_absolute()
                or "/".join(parts) != name or any(p in (".", "..") for p in parts)
                or not (item.isfile() or item.isdir())):
            raise ValueError(f"Unsafe source archive member: {item.name}")
        relative = "/".join(parts[1:])
        if relative in seen or (not relative and not item.isdir()):
            raise ValueError(f"Duplicate/invalid archive member: {item.name}")
        seen.add(relative)
        result.append((item, relative))
    return result


def extract_source(archive_path: Path, destination: Path) -> None:
    if destination.exists():
        raise ValueError("Source destination must be new")
    with tarfile.open(archive_path, "r:gz") as archive:
        members = safe_members(archive)
        destination.mkdir(mode=0o700)
        for member, relative in members:
            if not relative:
                continue
            target = destination / relative
            if member.isdir():
                target.mkdir(mode=0o700, parents=True, exist_ok=True)
            else:
                target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                with archive.extractfile(member) as source, target.open("xb") as output:
                    while block := source.read(1024 * 1024):
                        output.write(block)
                target.chmod(0o600)


def validate_patch_paths(numstat: str) -> None:
    names = [line.split("\t")[-1] for line in numstat.splitlines()]
    if len(names) != len(set(names)) or set(names) != PATCH_FILES:
        raise ValueError("Darwin patch must change only its reviewed converter files")


def prepare_output(path: Path) -> None:
    parent = path.parent.resolve(strict=True)
    info = parent.stat()
    if info.st_uid != os.geteuid() or stat.S_IMODE(info.st_mode) & 0o022:
        raise ValueError("Output parent must be a protected login-user-owned directory")
    if any(part.lower().endswith(".app") for part in (*parent.parts, path.name)):
        raise ValueError("Host formatter output cannot be an application bundle")
    path.mkdir(mode=0o700, exist_ok=False)



def check_policy_refusals(binary: Path, output: Path, run) -> list[dict]:
    """Exercise only invalid profiles against an absent file in private output."""
    sentinel = output / "private-never-created-policy-sentinel.ntfs"
    if sentinel.exists() or sentinel.is_symlink():
        raise RuntimeError("Expected a nonexistent private policy sentinel")
    refused = []
    for index, arguments in enumerate((
        ["--compress=none"],
        ["--fresh-format=1:2:2147483648", "--no-datasum", "--compress=none"],
        ["--fresh-format=1:2:2147483648", "--rollback", "--compress=none"],
        ["--fresh-format=1:2:2147483648", "--hash=sha256", "--compress=none"],
    )):
        text = run([str(binary), *arguments, str(sentinel)],
                   f"profile-refusal-{index}", accepted=(1,))
        if "Darwin candidate requires --fresh-format=" not in text:
            raise RuntimeError("Invalid profile did not fail at the pre-open policy guard")
        refused.append({"arguments": arguments, "sentinel": str(sentinel),
                        "outcome": "rejected-before-input-open"})
    return refused


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-archive", type=Path, required=True)
    parser.add_argument("--llvm-root", type=Path, required=True)
    parser.add_argument("--cmake", type=Path, required=True)
    parser.add_argument("--ninja", type=Path, required=True)
    parser.add_argument("--pkg-config", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        parser.error("Native Darwin arm64 required; Intel/Rosetta and cross-builds are unqualified")
    if not os.getuid() or os.getuid() != os.geteuid() or os.getgid() != os.getegid():
        parser.error("Build only as the unelevated login user")
    archive = args.source_archive.resolve(strict=True)
    require_digest(archive, ARCHIVE_SHA256)
    require_digest(PATCH, PATCH_SHA256)
    llvm = args.llvm_root.resolve(strict=True)
    tools = {"clang": llvm / "bin/clang", "clang++": llvm / "bin/clang++",
             "clang-scan-deps": llvm / "bin/clang-scan-deps",
             "cmake": args.cmake.resolve(strict=True), "ninja": args.ninja.resolve(strict=True),
             "pkg-config": args.pkg_config.resolve(strict=True)}
    for name, path in tools.items():
        if not path.is_file() or not os.access(path, os.X_OK):
            parser.error(f"Missing executable {name}: {path}")
    output = args.output_dir.absolute()
    prepare_output(output)
    logs = output / "logs"
    logs.mkdir(mode=0o700)
    source = output / "source"
    build = output / "build"
    manifest = {
        "schema": 1, "scope": "Separate uninstalled Darwin fresh regular-file formatter candidate; no app bundle, elevation, format, device or mount operations",
        "qualification": "Build-only; native fresh-format and durability gates remain unproven",
        "source_commit": COMMIT, "source_url": SOURCE_URL,
        "builder": {"path": str(Path(__file__).resolve()), "sha256": digest(Path(__file__).resolve())},
        "python": {"executable": sys.executable, "version": sys.version},
        "source_archive": {"path": str(archive), "sha256": digest(archive)},
        "patch": {"path": str(PATCH), "sha256": digest(PATCH)},
        "host": {"system": platform.system(), "machine": platform.machine(), "uname": list(os.uname()),
                 "uid": os.getuid(), "euid": os.geteuid(), "gid": os.getgid()},
        "tools": {name: {"path": str(path), "sha256": digest(path)} for name, path in tools.items()},
        "started": time.time(), "commands": [], "status": "running",
        "licenses": "Preserve pinned source/header notices: overall GPL2-or-later, CRC LGPL3-or-later, BSD2 xxHash, public-domain SHA256 and selected BLAKE2 license. Distribution closure is a separate gate.",
    }
    receipt = output / "build-manifest.json"
    env = os.environ.copy()
    # Avoid ambient compiler flags changing this explicitly selected candidate.
    removed = ("CC", "CXX", "CFLAGS", "CPPFLAGS", "CXXFLAGS", "LDFLAGS", "CPATH",
               "CPLUS_INCLUDE_PATH", "C_INCLUDE_PATH", "LIBRARY_PATH", "SDKROOT",
               "MACOSX_DEPLOYMENT_TARGET", "CMAKE_TOOLCHAIN_FILE", "CMAKE_PREFIX_PATH",
               "DYLD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH", "DYLD_FRAMEWORK_PATH",
               "DYLD_FALLBACK_FRAMEWORK_PATH", "DYLD_INSERT_LIBRARIES", "LD_PRELOAD")
    manifest["removed_ambient_keys"] = [key for key in removed if key in env]
    for key in removed:
        env.pop(key, None)
    env["CMAKE_BUILD_PARALLEL_LEVEL"] = "1"
    env["PATH"] = str(llvm / "bin") + ":" + env.get("PATH", "/usr/bin:/bin")

    def save() -> None:
        temporary = receipt.with_suffix(".tmp")
        temporary.write_text(json.dumps(manifest, indent=2) + "\n")
        temporary.replace(receipt)

    def run(command: list[str], label: str, *, cwd: Path = output,
            accepted: tuple[int, ...] = (0,), timeout: int = 900) -> str:
        path = logs / (label + ".log")
        started = time.time()
        try:
            with path.open("w") as stream:
                with subprocess.Popen(command, cwd=cwd, env=env, stdout=stream,
                                      stderr=subprocess.STDOUT, start_new_session=True) as process:
                    try:
                        code = process.wait(timeout=timeout)
                    except subprocess.TimeoutExpired:
                        try:
                            os.killpg(process.pid, signal.SIGTERM)
                        except ProcessLookupError:
                            pass
                        try:
                            process.wait(timeout=15)
                        except subprocess.TimeoutExpired:
                            try:
                                os.killpg(process.pid, signal.SIGKILL)
                            except ProcessLookupError:
                                pass
                            process.wait()
                        code = "timeout"
        except OSError as error:
            with path.open("a") as stream:
                stream.write(f"Process error: {error}\n")
            code = "launch-failed"
        manifest["commands"].append({"argv": command, "cwd": str(cwd), "exit_code": code,
                                      "log": str(path), "log_sha256": digest(path),
                                      "started": started, "finished": time.time()})
        save()
        if code not in accepted:
            raise RuntimeError(f"{label} failed ({code}); retained log: {path}")
        return path.read_text()

    try:
        save()
        for name, tool in tools.items():
            manifest["tools"][name]["version"] = run([str(tool), "--version"], "tool-" + name)
        manifest["macos_version"] = run(["/usr/bin/sw_vers", "-productVersion"], "macos-version").strip()
        triple = run([str(tools["clang++"]), "-dumpmachine"], "compiler-target").strip()
        if not triple.startswith(("arm64-apple-", "aarch64-apple-")):
            raise RuntimeError("Expected a native Apple arm64 compiler target")
        manifest["compiler_target"] = triple
        sdk = run(["/usr/bin/xcrun", "--show-sdk-path"], "sdk").strip()
        manifest["sdk"] = {"path": sdk, "version": run(["/usr/bin/xcrun", "--show-sdk-version"], "sdk-version").strip()}
        probe = output / "standard-library-probe.cpp"
        probe.write_text('#include <format>\n#include <print>\nint main() { std::println("{}", std::format("native C++{}", 23)); }\n')
        probe_binary = output / "standard-library-probe"
        run([str(tools["clang++"]), "-std=c++23", "-stdlib=libc++", "-v", "-H", "-isysroot", sdk,
             str(probe), "-o", str(probe_binary)], "standard-library-compile")
        probe_file = run(["/usr/bin/file", str(probe_binary)], "standard-library-file")
        if "Mach-O 64-bit executable arm64" not in probe_file:
            raise RuntimeError("Standard-library probe must be native arm64, without translation")
        manifest["standard_library_probe"] = run([str(probe_binary)], "standard-library-run")
        manifest["standard_library_linkage"] = run(["/usr/bin/otool", "-L", str(probe_binary)], "standard-library-linkage")
        extract_source(archive, source)
        manifest["original_source_files"] = {str(p.relative_to(source)): digest(p) for p in sorted(source.rglob("*")) if p.is_file()}
        numstat = run(["/usr/bin/git", "apply", "--numstat", str(PATCH)], "patch-files", cwd=source)
        validate_patch_paths(numstat)
        run(["/usr/bin/git", "apply", "--check", str(PATCH)], "patch-check", cwd=source)
        run(["/usr/bin/git", "apply", str(PATCH)], "patch-apply", cwd=source)
        manifest["patched_source_files"] = {str(p.relative_to(source)): digest(p) for p in sorted(source.rglob("*")) if p.is_file()}
        run([str(tools["cmake"]), "-S", str(source), "-B", str(build), "-G", "Ninja",
             "-DCMAKE_BUILD_TYPE=Release", "-DCMAKE_C_COMPILER=" + str(tools["clang"]),
             "-DCMAKE_CXX_COMPILER=" + str(tools["clang++"]),
             "-DCMAKE_CXX_COMPILER_CLANG_SCAN_DEPS=" + str(tools["clang-scan-deps"]),
             "-DCMAKE_CXX_FLAGS=-stdlib=libc++", "-DCMAKE_OSX_SYSROOT=" + sdk,
             "-DCMAKE_OSX_ARCHITECTURES=" + platform.machine(),
             "-DCMAKE_MAKE_PROGRAM=" + str(tools["ninja"]),
             "-DPKG_CONFIG_EXECUTABLE=" + str(tools["pkg-config"]),
             "-DWITH_ZLIB=OFF", "-DWITH_LZO=OFF", "-DWITH_ZSTD=OFF"], "configure")
        run([str(tools["cmake"]), "--build", str(build), "--parallel", "1"], "compile")
        binary = build / "ntfs2btrfs"
        manifest["binary"] = {"path": str(binary), "sha256": digest(binary)}
        manifest["file"] = run(["/usr/bin/file", str(binary)], "binary-file")
        if "Mach-O 64-bit executable " + platform.machine() not in manifest["file"]:
            raise RuntimeError("Expected a native Mach-O executable")
        manifest["linkage"] = run(["/usr/bin/otool", "-L", str(binary)], "binary-linkage")
        version = run([str(binary), "--version"], "binary-version", accepted=(1,))
        if version.strip() != "ntfs2btrfs 20260810":
            raise RuntimeError("Unexpected upstream converter version output")
        manifest["crc32c"] = run([str(build / "ntfs2btrfs-crc32c-check")], "crc32c-check")
        # These invalid profiles must fail before any input is opened. They do
        # not create a file, format NTFS/Btrfs, or exercise a storage path.
        manifest["profile_refusals"] = check_policy_refusals(binary, output, run)
        manifest["status"] = "passed-build-only"
    except Exception as error:
        manifest["status"] = "failed-build-only"
        manifest["error"] = repr(error)
        raise
    finally:
        manifest["finished"] = time.time()
        save()
    print(json.dumps({"status": manifest["status"], "manifest": str(receipt), "binary": manifest.get("binary")}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
