#!/usr/bin/env python3
"""Wrap a verified full Linux staging archive in a native distribution package."""

import argparse
import hashlib
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
REQUIRED_ARTIFACTS = {"gocryptfs", "cryfs", "cryptomator"}
VERACRYPT_ARTIFACTS = {"veracrypt-create", "veracrypt-userspace"}
PRODUCTS = ("lurker", "lurker-desktop", "lurker-helper")
RPM_DISTROS = {"fedora", "opensuse-leap"}


def require_full_package(prefix, manifest):
    artifacts = set(manifest["bundled_artifacts"])
    if (set(manifest["selected_backends"]) != REQUIRED_BACKENDS
            or artifacts - VERACRYPT_ARTIFACTS != REQUIRED_ARTIFACTS
            or len(artifacts & VERACRYPT_ARTIFACTS) != 1):
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


def glibc_floor(manifest):
    versions = []
    for binary in manifest["native_dependencies"]:
        if binary["bundled"]:
            for dependency in binary["dependencies"]:
                for version in dependency["required_versions"]:
                    if re.fullmatch(r"GLIBC_[0-9]+(?:\.[0-9]+)+", version):
                        versions.append(tuple(map(int, version.removeprefix("GLIBC_").split("."))))
    if not versions:
        raise ValueError("Staged native dependency audit has no glibc version requirements")
    return ".".join(map(str, max(versions)))


def debian_dependencies(manifest):
    return [
        f"libc6 (>= {glibc_floor(manifest)})", "cryptsetup-bin", "btrfs-progs", "util-linux", "mount",
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


def build_rpm(image, output, version, manifest, distro):
    native_dependencies = ["cryptsetup", "util-linux", "fuse", "fuse3", "fontconfig", "sudo"]
    if distro == "fedora":
        native_dependencies += [
            "btrfs-progs", "fuse-libs", "fuse3-libs", "libgcc", "libgomp", "libstdc++",
            "dejavu-sans-fonts", "libX11", "libX11-xcb", "libXcursor", "libXi", "libxcb",
            "libxkbcommon", "libxkbcommon-x11", "libwayland-client",
        ]
    else:
        native_dependencies += [
            "btrfsprogs", "libfuse2", "libgcc_s1", "libgomp1", "libstdc++6",
            "dejavu-fonts", "libX11-6", "libX11-xcb1", "libXcursor1", "libXi6", "libxcb1",
            "libxkbcommon0", "libxkbcommon-x11-0", "libwayland-client0", "libfuse3-3",
        ]
    requirements = "\n".join("Requires: " + dep for dep in native_dependencies)
    work = image.parent
    spec = work / "lurker.spec"
    spec.write_text(f"""Name: lurker
Version: {version}
Release: 1
Summary: Create and mount encrypted volumes and directories
License: GPL-3.0-only
AutoReqProv: no
Requires: glibc >= {glibc_floor(manifest)}
{requirements}

%description
Lurker provides a command-line tool and desktop application with private
encryption engines.

%install
mkdir -p %{{buildroot}}
cp -a %{{_sourcedir}}/root/usr %{{buildroot}}/

%files
%defattr(-,root,root,-)
/usr/bin/lurker
/usr/bin/lurker-desktop
/usr/lib/lurker
/usr/share/applications/io.lurker.desktop.desktop
/usr/share/pixmaps/lurker.png
""")
    subprocess.run([
        "rpmbuild", "--define", f"_topdir {work}", "--define", f"_sourcedir {work}",
        "--define", "__os_install_post %{nil}", "--define", "debug_package %{nil}",
        "--define", "_build_id_links none", "-bb", str(spec),
    ], check=True)
    [artifact] = (work / "RPMS").glob("*/lurker-*.rpm")
    artifact.rename(output)


def build_arch(image, output, version, manifest):
    work = image.parent
    (work / "PKGBUILD").write_text(f"""pkgname=lurker
pkgver={version}
pkgrel=1
pkgdesc='Create and mount encrypted volumes and directories'
arch=('{platform.machine()}')
license=('GPL-3.0-only')
depends=('glibc>={glibc_floor(manifest)}' 'cryptsetup' 'btrfs-progs' 'util-linux'
         'fuse2' 'fuse3' 'gcc-libs' 'fontconfig' 'ttf-dejavu' 'libx11' 'libxcursor'
         'libxi' 'libxcb' 'libxkbcommon' 'libxkbcommon-x11' 'wayland' 'sudo')
options=('!strip' '!debug' '!zipman' '!purge' 'docs' 'libtool' 'staticlibs' 'emptydirs' '!lto')

package() {{
    cp -a "$startdir/root/usr" "$pkgdir/"
}}
""")
    env = os.environ | {"PKGDEST": str(work), "BUILDDIR": str(work)}
    subprocess.run(["makepkg", "--nodeps", "--noconfirm"], cwd=work, env=env, check=True)
    [artifact] = work.glob("lurker-*.pkg.tar.zst")
    artifact.rename(output)


def build_gentoo(image, output, version, manifest, commit):
    arch = {"x86_64": "amd64", "aarch64": "arm64"}.get(platform.machine())
    if not arch:
        raise ValueError("Unsupported Gentoo package architecture")
    work = image.parent
    repo = work / "repository"
    package = repo / "app-crypt/lurker"
    package.mkdir(parents=True)
    (repo / "metadata").mkdir()
    (repo / "profiles").mkdir()
    (repo / "profiles/repo_name").write_text("lurker-local\n")
    (repo / "profiles/categories").write_text("app-crypt\n")
    (repo / "metadata/layout.conf").write_text("masters = gentoo\nthin-manifests = true\n")
    distfiles = work / "distfiles"
    distfiles.mkdir()
    payload = distfiles / f"lurker-install-{version}-{commit[:12]}.tar.gz"
    with tarfile.open(payload, "w:gz") as archive:
        archive.add(image, arcname="lurker-install")
    checksums = {}
    for algorithm in ("blake2b", "sha512"):
        with payload.open("rb") as stream:
            checksums[algorithm.upper()] = hashlib.file_digest(stream, algorithm).hexdigest()
    (package / "Manifest").write_text(
        f"DIST {payload.name} {payload.stat().st_size} "
        + " ".join(f"{name} {digest}" for name, digest in checksums.items()) + "\n"
    )
    (package / f"lurker-{version}.ebuild").write_text(f"""EAPI=8

DESCRIPTION="Create and mount encrypted volumes and directories"
SRC_URI="{payload.name}"
S="${{WORKDIR}}/lurker-install"
LICENSE="GPL-3"
SLOT="0"
KEYWORDS="~{arch}"
RESTRICT="fetch mirror strip"
RDEPEND="
    >=sys-libs/glibc-{glibc_floor(manifest)}
    sys-fs/cryptsetup
    sys-fs/btrfs-progs
    sys-apps/util-linux
    sys-fs/fuse:0
    sys-fs/fuse:3
    app-admin/sudo
    media-libs/fontconfig
    media-fonts/dejavu
    x11-libs/libX11
    x11-libs/libXcursor
    x11-libs/libXi
    x11-libs/libxcb
    x11-libs/libxkbcommon[X,wayland]
    dev-libs/wayland
"

src_install() {{
    cp -a usr "${{ED}}" || die "copying verified Lurker payload failed"
}}
""")
    with tarfile.open(output, "w:gz") as bundle:
        bundle.add(repo, arcname="repository")
        bundle.add(distfiles, arcname="distfiles")


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
        if platform.system() != "Linux":
            raise ValueError("Build Linux native packages on the intended distribution guest")
        distro = platform.freedesktop_os_release()["ID"]
        if distro in {"debian", "ubuntu"}:
            suffix, builder, version_pattern = ".deb", build_deb, r"[0-9][0-9A-Za-z.+~\-]*"
        elif distro in RPM_DISTROS:
            suffix, builder, version_pattern = ".rpm", build_rpm, r"[0-9][0-9A-Za-z.+~^]*"
        elif distro == "arch":
            suffix, builder, version_pattern = ".pkg.tar.zst", build_arch, r"[0-9][0-9A-Za-z.+_]*"
        elif distro == "gentoo":
            suffix, builder, version_pattern = ".gentoo.tar.gz", build_gentoo, r"[0-9]+(?:\.[0-9]+)+"
        else:
            raise ValueError("No native package builder for this distribution: " + distro)
        if not re.fullmatch(r"[0-9a-f]{64}", args.sha256):
            raise ValueError("An exact staged archive SHA-256 is required")
        if not re.fullmatch(r"[0-9a-f]{40}", args.source_commit):
            raise ValueError("An exact source commit is required")
        if not re.fullmatch(version_pattern, args.version):
            raise ValueError("Invalid native package version")
        output = args.output.absolute()
        if not output.name.endswith(suffix) or output.exists() or output.is_symlink():
            raise ValueError("Output must be a new " + suffix + " file")
        with tempfile.TemporaryDirectory(prefix=".lurker-native-", dir=output.parent) as work:
            scratch = Path(work)
            prefix, manifest = unpack_archive(args.archive, args.sha256, args.source_commit, scratch)
            require_full_package(prefix, manifest)
            image = scratch / "root"
            copy_layout(prefix, image)
            artifact = scratch / output.name
            if distro in RPM_DISTROS:
                builder(image, artifact, args.version, manifest, distro)
            elif distro == "gentoo":
                builder(image, artifact, args.version, manifest, args.source_commit)
            else:
                builder(image, artifact, args.version, manifest)
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
