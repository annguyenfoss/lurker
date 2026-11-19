#!/usr/bin/env python3
"""Wrap a verified full Linux staging archive in a native distribution package."""

import argparse
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
import tarfile

from unix_package import STAGE, unpack_archive

SCRIPTS = Path(__file__).resolve().parent
PRIVATE = Path("usr/lib/lurker")
REQUIRED_BACKENDS = {
    "luks", "luks1", "veracrypt", "gocryptfs", "cryfs", "cryptomator",
}
REQUIRED_ARTIFACTS = {"veracrypt-userspace", "gocryptfs", "cryfs", "cryptomator"}
PRODUCTS = ("lurker", "lurker-desktop", "lurker-helper")


def require_full_package(prefix, manifest):
    if (set(manifest["selected_backends"]) != REQUIRED_BACKENDS
            or set(manifest["bundled_artifacts"]) != REQUIRED_ARTIFACTS):
        raise ValueError("A full Linux package with all locked routes is required")
    for product in PRODUCTS:
        STAGE.require_executable(prefix / "bin" / product)
    if not manifest["desktop_runtime"]:
        raise ValueError("Staged manifest must declare a desktop runtime")


def copy_layout(prefix, image):
    private = image / PRIVATE
    STAGE.copy_tree(prefix, private)
    for product in PRODUCTS[:2]:
        launcher = image / "usr/bin" / product
        launcher.parent.mkdir(parents=True, exist_ok=True)
        launcher.symlink_to("../lib/lurker/bin/" + product)
    extras = SCRIPTS.parent / "extras"
    assets = {
        "usr/share/applications/io.lurker.desktop.desktop":
            extras / "linux/io.lurker.desktop.desktop",
        "usr/share/pixmaps/lurker.png":
            extras / "assets/lurker.png",
    }
    for name, source in assets.items():
        target = image / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        target.chmod(0o644)


def debian_dependencies(manifest):
    versions = []
    for binary in manifest["native_dependencies"]:
        if binary["bundled"]:
            for dependency in binary["dependencies"]:
                for version in dependency["required_versions"]:
                    if re.fullmatch(r"GLIBC_[0-9]+(?:\.[0-9]+)+", version):
                        versions.append(tuple(map(int, version.removeprefix("GLIBC_").split("."))))
    if not versions:
        raise ValueError("Staged native dependency audit has no glibc version requirements")
    glibc = ".".join(map(str, max(versions)))
    return [
        f"libc6 (>= {glibc})", "cryptsetup-bin", "btrfs-progs", "util-linux", "mount",
        "fuse3", "libfuse2t64", "libfuse3-4", "libgcc-s1", "libgomp1", "libstdc++6",
        "fontconfig", "fonts-dejavu-core", "libx11-6", "libxcursor1", "libx11-xcb1",
        "libxi6", "libxcb1", "libxkbcommon0", "libxkbcommon-x11-0", "libwayland-client0",
        "sudo | pkexec",
    ]


def build_deb(image, output, version, manifest):
    arch = {"x86_64": "amd64", "aarch64": "arm64"}.get(platform.machine())
    if not arch:
        raise ValueError("Unsupported Debian package architecture")
    control = image / "DEBIAN/control"
    control.parent.mkdir()
    dependencies = ", ".join(debian_dependencies(manifest))
    control.write_text(
        f"Package: lurker\nVersion: {version}\nArchitecture: {arch}\n"
        "Section: utils\nPriority: optional\nMaintainer: Lurker contributors\n"
        f"Depends: {dependencies}\n"
        "Description: Create and mount encrypted volumes and directories\n"
        " Lurker provides a command-line tool and desktop application.\n"
        " Private encryption engines are included with the package.\n"
    )
    control.chmod(0o644)
    subprocess.run(
        ["dpkg-deb", "--root-owner-group", "--build", str(image), str(output)],
        check=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True, help="Exact stage-unix package.tar.gz")
    parser.add_argument("--sha256", required=True, help="Expected staged archive SHA-256")
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--version", required=True, help="Native package version")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        os.umask(0o022)
        if os.geteuid() == 0:
            raise ValueError("Build native packages as the login user")
        if platform.system() != "Linux" or platform.freedesktop_os_release()["ID"] not in {"debian", "ubuntu"}:
            raise ValueError("Build .deb packages on the intended Debian or Ubuntu guest")
        if not re.fullmatch(r"[0-9a-f]{64}", args.sha256):
            raise ValueError("An exact staged archive SHA-256 is required")
        if not re.fullmatch(r"[0-9a-f]{40}", args.source_commit):
            raise ValueError("An exact source commit is required")
        if not re.fullmatch(r"[0-9][0-9A-Za-z.+~\-]*", args.version):
            raise ValueError("Invalid Debian package version")
        output = args.output.absolute()
        if output.suffix != ".deb" or output.exists() or output.is_symlink():
            raise ValueError("Output must be a new .deb file")
        with tempfile.TemporaryDirectory(prefix=".lurker-deb-", dir=output.parent) as work:
            scratch = Path(work)
            prefix, manifest = unpack_archive(args.archive, args.sha256, args.source_commit, scratch)
            require_full_package(prefix, manifest)
            image = scratch / "root"
            copy_layout(prefix, image)
            artifact = scratch / output.name
            build_deb(image, artifact, args.version, manifest)
            digest = STAGE.digest(artifact)
            os.link(artifact, output)
        print(json.dumps({"status": "built", "source_commit": args.source_commit,
                          "archive_sha256": args.sha256, "package": str(output),
                          "package_sha256": digest}))
        return 0
    except (OSError, ValueError, RuntimeError, KeyError, tarfile.TarError, subprocess.SubprocessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
