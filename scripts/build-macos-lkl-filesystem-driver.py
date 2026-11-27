#!/usr/bin/env python3
"""Build the separate native macFUSE3 LKL driver; never install or elevate it.

The static Darwin LKL archive and its matching exported headers are explicit,
verified inputs. Build them from the pinned source with lkl-darwin-flush.patch
as described in native/macos/lkl-macfuse.md. This script does not change them.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import stat
import subprocess
import sys
import time

LKL_COMMIT = "c4d0895ead9badb52f7bbffe3d529c61c8fc9485"
KERNEL_EXPORTS = {
    "_lkl_cleanup", "_lkl_cpu_change_owner", "_lkl_cpu_get", "_lkl_cpu_init",
    "_lkl_cpu_put", "_lkl_cpu_shutdown", "_lkl_cpu_try_run_irq", "_lkl_cpu_wait_shutdown",
    "_lkl_get_free_irq", "_lkl_init", "_lkl_is_running", "_lkl_ops", "_lkl_put_irq",
    "_lkl_start_kernel", "_lkl_sys_halt", "_lkl_syscall", "_lkl_trigger_irq",
    "___vmalloc_noprof", "_blake2s_compress_generic", "_init_thread_info", "_jiffies_64",
}
MACFUSE_LIBRARY = Path("/usr/local/lib/libfuse3.4.dylib")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def checked_host_dependency(path: Path) -> None:
    """Check the actual native library and every resolved path ancestor."""
    for item in (path.resolve(strict=True), *path.resolve(strict=True).parents):
        info = item.stat()
        if info.st_uid != 0 or stat.S_IMODE(info.st_mode) & 0o022:
            raise ValueError(f"Expected an administrator-owned, protected dependency: {item}")


def kernel_exports(symbols: str) -> set[str]:
    """Require the combined kernel object, not the native host archive members."""
    found = False
    result = set()
    for line in symbols.splitlines():
        if line.endswith(":"):
            found = line.endswith("(vmlinux_macho.o):")
        elif found and line.split():
            result.add(line.split()[-1])
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lkl-source", type=Path, required=True)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--library-sha256", required=True)
    parser.add_argument("--include-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        parser.error("This qualified build entry point requires native Darwin arm64")
    if not os.getuid() or os.geteuid() != os.getuid() or os.getegid() != os.getgid():
        parser.error("Build as the login user; this script has no privileged installation mode")
    source = args.lkl_source.resolve(strict=True)
    library = args.library.resolve(strict=True)
    include = args.include_dir.resolve(strict=True)
    if sha256(library) != args.library_sha256:
        parser.error("The LKL archive differs from the explicitly reviewed input hash")
    if not (include / "lkl.h").is_file() or not (include / "lkl_host.h").is_file():
        parser.error("Expected matching exported LKL headers")
    revision = subprocess.check_output(
        ["/usr/bin/git", "-C", str(source), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != LKL_COMMIT:
        parser.error(f"Expected pinned companion LKL {LKL_COMMIT}")
    checked_host_dependency(MACFUSE_LIBRARY)
    root = Path(__file__).resolve().parents[1]
    patch = root / "native/macos/lklfuse-darwin.patch"
    flush_patch = root / "native/macos/lkl-darwin-flush.patch"
    symbols_patch = root / "native/macos/lkl-darwin-symbols.patch"
    output = args.output_dir.resolve()
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    work = output / "source"
    work.mkdir(mode=0o700)
    logs = output / "logs"
    logs.mkdir(mode=0o700)
    manifest = {
        "schema": 1,
        "scope": "Separate host filesystem driver only; no install, elevation, app bundle or core policy change",
        "started": time.time(),
        "uid": os.getuid(),
        "gid": os.getgid(),
        "lkl_source": str(source),
        "lkl_commit": revision,
        "archive": {"path": str(library), "sha256": sha256(library)},
        "patches": {str(p): sha256(p) for p in (patch, flush_patch, symbols_patch)},
        "macfuse_headers_sha256": {
            str(p): sha256(p) for p in sorted(Path("/usr/local/include/fuse3").glob("*.h"))
        },
        "headers_sha256": {
            str(p.relative_to(include)): sha256(p) for p in sorted(include.rglob("*.h"))
        },
        "commands": [],
        "status": "running",
    }
    receipt = output / "build-manifest.json"

    def save() -> None:
        temporary = receipt.with_suffix(".tmp")
        temporary.write_text(json.dumps(manifest, indent=2) + "\n")
        temporary.replace(receipt)

    def run(command: list[str], label: str, *, cwd: Path = work) -> str:
        path = logs / f"{label}.log"
        started = time.time()
        with path.open("w") as stream:
            result = subprocess.run(
                command, cwd=cwd, stdout=stream, stderr=subprocess.STDOUT,
                timeout=180, check=False,
            )
        manifest["commands"].append({
            "argv": command, "cwd": str(cwd), "exit": result.returncode,
            "log": str(path), "log_sha256": sha256(path),
            "started": started, "finished": time.time(),
        })
        save()
        if result.returncode:
            raise RuntimeError(f"{label} failed; see {path}")
        return path.read_text()

    try:
        save()
        manifest["kernel_symbol_closure"] = {}
        for arch in ("arm64", "x86_64"):
            symbols = run(["/usr/bin/nm", "-gU", "-arch", arch, str(library)],
                          "archive-symbols-" + arch)
            exports = kernel_exports(symbols)
            if exports != KERNEL_EXPORTS:
                raise ValueError(f"Kernel/native host symbol boundary differs for {arch}: "
                                 f"{sorted(exports ^ KERNEL_EXPORTS)}")
            manifest["kernel_symbol_closure"][arch] = sorted(exports)
        numstat = run(["/usr/bin/git", "apply", "--numstat", str(patch)], "patch-files")
        files = [line.split("\t")[-1] for line in numstat.splitlines()]
        if files != ["tools/lkl/lklfuse-darwin.c"]:
            raise ValueError("The adapter patch must add only tools/lkl/lklfuse-darwin.c")
        run(["/usr/bin/git", "apply", "--check", str(patch)], "patch-check")
        run(["/usr/bin/git", "apply", str(patch)], "patch-apply")
        driver_source = work / "tools/lkl/lklfuse-darwin.c"
        manifest["driver_source_sha256"] = sha256(driver_source)
        binary = output / "lklfuse-macfuse"
        run([
            "/usr/bin/xcrun", "clang", "-arch", "arm64", "-mmacosx-version-min=15.4",
            "-O2", "-Wall", "-Wextra", "-Wno-unused-parameter",
            "-Werror=implicit-function-declaration", "-Werror=incompatible-function-pointer-types",
            "-DLKL_HOST_CONFIG_POSIX", "-I" + str(include), "-I/usr/local/include",
            str(driver_source), str(library), str(MACFUSE_LIBRARY), "-o", str(binary),
        ], "compile")
        manifest["binary"] = {"path": str(binary), "sha256": sha256(binary)}
        manifest["file"] = run(["/usr/bin/file", str(binary)], "file")
        manifest["linkage"] = run(["/usr/bin/otool", "-L", str(binary)], "linkage")
        manifest["undefined_symbols"] = run(["/usr/bin/nm", "-u", str(binary)], "symbols")
        manifest["version"] = run([str(binary), "--version"], "version")
        manifest["macfuse_library"] = {
            "path": str(MACFUSE_LIBRARY), "sha256": sha256(MACFUSE_LIBRARY),
            "linkage": run(["/usr/bin/otool", "-L", str(MACFUSE_LIBRARY)], "macfuse-linkage"),
        }
        manifest["toolchain"] = run(["/usr/bin/xcrun", "clang", "--version"], "clang")
        manifest["sdk"] = run(["/usr/bin/xcrun", "--show-sdk-path"], "sdk")
        if "Mach-O 64-bit executable arm64" not in manifest["file"]:
            raise ValueError("Expected a native arm64 Mach-O executable")
        if str(MACFUSE_LIBRARY) not in manifest["linkage"] or "fuse-t" in manifest["linkage"]:
            raise ValueError("Expected explicit native macFUSE3 dynamic linkage")
        manifest["status"] = "passed-build-only"
    except Exception as error:
        manifest["status"] = "failed"
        manifest["error"] = repr(error)
        raise
    finally:
        manifest["finished"] = time.time()
        save()
    print(json.dumps({"status": manifest["status"], "manifest": str(receipt),
                      "binary": manifest["binary"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
