#!/usr/bin/env python3
"""Build a pinned private macOS VeraCrypt backend; the selected FUSE stays external."""
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import sys

from macos_backend import FUSE_RUNTIMES, SOURCES, VC_REVISION, fuse_runtime_info, run, sha256, source, stage_macho, write_json

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache-dir", type=Path, default=Path.home() / ".cache/lurker/macos-backends")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/release/macos-backends")
    parser.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 2))
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--fuse-runtime", choices=["fuse-t"], default="fuse-t",
                        help="VeraCrypt requires separately installed FUSE-T")
    args = parser.parse_args()
    if sys.platform != "darwin" or os.geteuid() == 0:
        parser.error("Build on macOS as the login user, without sudo.")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    for tool in ["make", "clang", "xcrun", "pkg-config", "otool", "lipo", "install_name_tool", "codesign", "patch"]:
        if not shutil.which(tool):
            parser.error(f"Missing build prerequisite: {tool}")
    runtime = fuse_runtime_info(args.fuse_runtime)
    cache = args.cache_dir.expanduser().absolute()
    cache.mkdir(mode=0o700, parents=True, exist_ok=True)
    if cache.is_symlink() or cache.stat().st_uid != os.geteuid() or cache.stat().st_mode & 0o022:
        parser.error("--cache-dir must be a real, user-owned directory without group/world write access")
    cache = cache.resolve()
    patch = ROOT / "native/veracrypt/utf8-console.patch"
    vc = source("veracrypt", cache, args.offline, patch=patch, variant=args.fuse_runtime)
    wx = source("wxwidgets", cache, args.offline)
    # Use the supported native Cocoa build with a static wxWidgets dependency;
    # Lurker calls only its text interface. Console-only and reduced GUI builds
    # have incomplete wx feature guards on this Mac toolchain.
    wx_flags = ["WXSTATIC=FULL", f"WX_ROOT={wx}", f"WX_BUILD_DIR={cache / 'wx-build'}",
                "LOCAL_DEVELOPMENT_BUILD=true", "WX_CONFIGURE_EXTRA_FLAGS=--with-regex=builtin"]
    flags = wx_flags + FUSE_RUNTIMES[args.fuse_runtime]["flags"]
    specification = {"schema": 1, "sources": SOURCES, "architecture": platform.machine(),
                     "patches": {"native/veracrypt/utf8-console.patch": sha256(patch)},
                     "flags": flags, "fuse_runtime": runtime,
                     "sdk": run("xcrun", "--show-sdk-version", capture=True).strip(),
                     "compiler": run("clang", "--version", capture=True).splitlines()[0]}
    stamp = vc / ".lurker-native-build-spec.json"
    serialized = json.dumps(specification, sort_keys=True)
    if stamp.exists() and stamp.read_text() != serialized:
        raise RuntimeError("Native build inputs changed; use a new --cache-dir to avoid stale objects.")
    stamp.write_text(serialized)
    wx_specification = {key: specification[key] for key in ["architecture", "sdk", "compiler"]}
    wx_specification["flags"] = wx_flags
    wx_specification.update(wxwidgets=SOURCES["wxwidgets"], veracrypt_build_revision=VC_REVISION)
    wx_serialized = json.dumps(wx_specification, sort_keys=True)
    wx_stamp = cache / "wx-built.json"
    if wx_stamp.exists() and wx_stamp.read_text() != wx_serialized:
        raise RuntimeError("wxWidgets build inputs changed; use a new --cache-dir.")
    if not wx_stamp.exists():
        run("make", f"-j{args.jobs}", *flags, "wxbuild", cwd=vc / "src")
        wx_stamp.write_text(wx_serialized)
    run("make", f"-j{args.jobs}", *flags, cwd=vc / "src")
    output = args.output_dir.resolve()
    # Replace only our generated staging directory after the complete build.
    temporary = output.with_name(f".{output.name}-{os.getpid()}")
    temporary.mkdir(parents=True)
    try:
        targets, external = stage_macho(vc / "src/Main/VeraCrypt", temporary / "lurker-veracrypt",
                                       temporary / "Frameworks", platform.machine(), fuse_runtime=args.fuse_runtime)
        licenses = temporary / "licenses"
        licenses.mkdir()
        shutil.copyfile(vc / "License.txt", licenses / "VeraCrypt-LICENSE.txt")
        shutil.copyfile(wx / "docs/licence.txt", licenses / "wxWidgets-LICENSE.txt")
        # Include the notices shipped with wx's optional static dependencies;
        # a static link does not remove their attribution obligations.
        notice_names = {"license", "license.txt", "licence", "licence.txt", "copying", "copying.txt",
                        "copyright", "copyright.txt", "readme", "readme.txt"}
        for path in (wx / "src").rglob("*"):
            if path.is_file() and path.name.lower() in notice_names:
                relative = path.relative_to(wx)
                target = licenses / "wxWidgets" / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, target)
        for target in sorted(targets, key=lambda path: path.suffix != ".dylib"):
            # install_name_tool invalidates the compiler's arm64 signature.
            # Development artifacts must be executable before DMG re-signing.
            run("codesign", "--force", "--sign", "-", target)
        run(temporary / "lurker-veracrypt", "-t", "--test")
        write_json(temporary / "provenance.json", {
            **specification, "veracrypt_revision": VC_REVISION,
            "source_archives": {name: {"url": spec[0], "sha256": spec[1]} for name, spec in SOURCES.items()},
            "external_libraries": external,
            "artifacts": {str(path.relative_to(temporary)): sha256(path) for path in targets},
        })
        if output.exists():
            if output.is_symlink() or not (output / "provenance.json").is_file():
                raise RuntimeError(f"Refusing to replace an unrecognized directory: {output}")
            shutil.rmtree(output)
        temporary.rename(output)
    finally:
        if temporary.exists():
            shutil.rmtree(temporary)
    print(json.dumps({"status": "built", "backend": str(output / "lurker-veracrypt"), "fuse_runtime": runtime}))


if __name__ == "__main__":
    main()
