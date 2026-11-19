#!/usr/bin/env python3
"""Wrap an exact stage-unix archive in the current BSD's native package format."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import tarfile
import tempfile

from unix_package import STAGE, unpack_archive


PREFIXES = {"FreeBSD": "/usr/local", "DragonFly": "/usr/local",
            "NetBSD": "/usr/pkg", "OpenBSD": "/usr/local"}
DESKTOP_LIBRARIES = ("X11", "X11-xcb", "Xcursor", "Xi", "xcb", "xkbcommon", "xkbcommon-x11")
BASE_X11 = {"OpenBSD": Path("/usr/X11R6/lib"), "NetBSD": Path("/usr/X11R7/lib")}
PKG = "/usr/local/sbin/pkg"
PRODUCTS = {"bin/lurker", "bin/lurker-helper", "bin/lurker-desktop"}


def command(*args, cwd=None):
    result = subprocess.run(args, cwd=cwd, capture_output=True, text=True, timeout=180, check=False)
    if result.returncode:
        detail = (result.stderr or result.stdout)[-2000:].strip()
        raise ValueError(f"{Path(args[0]).name} failed ({result.returncode}): {detail}")
    return result.stdout.strip()


def staged_libraries(manifest):
    dependencies = {entry["path"]: entry
                    for binary in manifest["native_dependencies"]
                    for entry in binary["dependencies"] if not entry["bundled"]}
    for path, entry in dependencies.items():
        if not Path(path).is_absolute() or STAGE.digest(path) != entry["sha256"]:
            raise ValueError("native library differs from the staging host: " + path)
    return {path: entry["name"] for path, entry in dependencies.items()}


def find_display_libraries(system):
    candidates = [Path(PREFIXES[system]) / "lib"]
    if system in BASE_X11:
        candidates.insert(0, BASE_X11[system])
    result = {}
    for library in DESKTOP_LIBRARIES:
        paths = [path for root in candidates for path in sorted(root.glob(f"lib{library}.so.*"))
                 if path.is_file() and path.stat().st_size]
        if not paths:
            raise ValueError(f"desktop package needs the native lib{library} runtime on the build guest")
        path = paths[0].resolve()
        result[str(path)] = path.name
    return result


def dependency_paths(system, manifest, desktop):
    libraries = staged_libraries(manifest)
    tools = set(manifest["host_tools"].values())
    if desktop:
        libraries.update(find_display_libraries(system))
        # Native desktop profiles require an administrator policy for the
        # helper. Use sudo as the declared native package dependency.
        sudo = Path(PREFIXES[system]) / "bin/sudo"
        if not sudo.is_file() or not os.access(sudo, os.X_OK):
            raise ValueError("BSD desktop package requires the native sudo package")
        tools.add(str(sudo))
    if "pefs" in manifest["selected_backends"]:
        module = "/boot/modules/pefs.ko"
        if not Path(module).is_file():
            raise ValueError("FreeBSD PEFS packaging requires the native pefs-kmod dependency")
        tools.add(module)
    local_prefix = PREFIXES[system] + "/"
    external = sorted(path for path in tools | libraries.keys()
                      if path.startswith((local_prefix, "/boot/modules/")))
    return libraries, external


def pkg_dependencies(paths):
    dependencies = {}
    for path in paths:
        owner = command(PKG, "which", "-q", path)
        name, version, origin = command(PKG, "query", "%n %v %o", owner).split()
        dependencies[name] = {"origin": origin, "version": version}
    return dependencies


def pkg_package(system, prefix, work, files, version, description, paths):
    dependencies = pkg_dependencies(paths)
    osreldate = command("/sbin/sysctl", "-n", "kern.osreldate")
    if "pefs-kmod" in dependencies:
        annotations = dict(line.split(maxsplit=1) for line in
                           command(PKG, "query", "%At %Av", "pefs-kmod").splitlines())
        if annotations.get("FreeBSD_version") != osreldate:
            raise ValueError("installed pefs-kmod was built for a different FreeBSD kernel version")
        if command(PKG, "which", "-q", "/usr/local/sbin/pefs") != command(PKG, "which", "-q", "/boot/modules/pefs.ko"):
            raise ValueError("PEFS utility and module must come from the same native package")
    install = PREFIXES[system]
    root = work / "destdir"
    destination = root / install.lstrip("/")
    destination.parent.mkdir(parents=True)
    prefix.rename(destination)
    meta = work / "meta"
    meta.mkdir()
    abi = command(PKG, "config", "ABI")
    metadata = {"name": "lurker", "version": version, "origin": "security/lurker",
                "prefix": install, "abi": abi, "comment": "Encrypted storage manager",
                "maintainer": "Lurker maintainers", "desc": description, "categories": ["security"],
                "licenses": ["GPLv3"], "deps": dependencies,
                "annotations": {system + "_version": osreldate}}
    (meta / "+MANIFEST").write_text(json.dumps(metadata))
    plist = work / "plist"
    plist.write_text("@owner root\n@group wheel\n" + "\n".join(files) + "\n")
    command(PKG, "create", "-f", "txz", "-m", str(meta), "-p", str(plist),
            "-r", str(root), "-o", str(work))
    return abi, dependencies


def netbsd_package(prefix, work, files, version, description, paths):
    dependencies = sorted({command("/usr/sbin/pkg_info", "-Fe", path) for path in paths})
    build_info = work / "build-info"
    architecture = command("/usr/bin/uname", "-p")
    tools_version = command("/usr/sbin/pkg_create", "-V")
    build_info.write_text(f"OPSYS=NetBSD\nOS_VERSION={platform.release()}\nMACHINE_ARCH={architecture}\n"
                          f"PKGTOOLS_VERSION={tools_version}\n"
                          "PKGPATH=security/lurker\nCATEGORIES=security\nLICENSE=gnu-gpl-v3\n")
    plist = work / "plist"
    plist.write_text("@owner root\n@group wheel\n" + "\n".join(files) + "\n")
    args = ["/usr/sbin/pkg_create", "-c", "-Encrypted storage manager", "-d", "-" + description,
            "-B", str(build_info), "-f", str(plist), "-p", str(prefix), "-I", PREFIXES["NetBSD"],
            "-u", "root", "-g", "wheel"]
    if dependencies:
        args.extend(["-P", " ".join(dependencies)])
    command(*args, str(work / f"lurker-{version}.tgz"))
    return platform.release() + "/" + architecture, dependencies


def openbsd_package(prefix, work, files, version, description, libraries, paths):
    dependencies = {}
    for path in paths:
        owner = command("/usr/sbin/pkg_info", "-E", path).splitlines()[0].split()[-1]
        pkgpath = command("/usr/sbin/pkg_info", "-q", "-P", owner).splitlines()[-1]
        name = re.fullmatch(r"(.+)-[0-9][A-Za-z0-9._+-]*", owner)
        if not name or not re.fullmatch(r"[A-Za-z0-9/_.,+-]+", pkgpath):
            raise ValueError(f"could not resolve OpenBSD package identity for {path}")
        dependencies[pkgpath] = (name[1] + "-*", owner)
    wantlibs = set()
    for name in libraries.values():
        match = re.fullmatch(r"lib(.+)\.so\.([0-9]+\.[0-9]+)", name)
        if match:
            wantlibs.add(".".join(match.groups()))
    install = PREFIXES["OpenBSD"]
    root = work / "destdir"
    destination = root / install.lstrip("/")
    destination.parent.mkdir(parents=True)
    prefix.rename(destination)
    plist = work / "plist"
    plist.write_text("@owner root\n@group wheel\n" + "\n".join(files) + "\n")
    ports = work / "ports"
    ports.mkdir()
    args = ["/usr/sbin/pkg_create", "-B", str(root), "-p", install, "-D", "FULLPKGPATH=security/lurker",
            "-D", "PORTSDIR=" + str(ports), "-D", "COMMENT=encrypted storage manager",
            "-A", platform.machine(), "-d", "-" + description, "-f", str(plist)]
    for path, (pattern, owner) in sorted(dependencies.items()):
        args.extend(["-P", f"{path}:{pattern}:{owner}"])
    for lib in sorted(wantlibs):
        args.extend(["-W", lib])
    command(*args, f"lurker-{version}.tgz", cwd=work)
    return platform.release() + "/" + platform.machine(), dependencies


def verify_package(package, install, manifest, manifest_digest, use_pkg):
    expected = {**manifest["files"], "share/lurker/manifest.json":
                {"sha256": manifest_digest, "executable": False}}
    with tarfile.open(package, mode="r:*") as archive:
        for member in archive:
            if member.name in {"+COMPACT_MANIFEST", "+MANIFEST", "+CONTENTS", "+COMMENT", "+DESC", "+BUILD_INFO"}:
                continue
            name = member.name.removeprefix(install + "/") if use_pkg else member.name
            entry = expected.pop(name, None)
            # pkg keeps the staging user's numeric tar IDs and supplies the
            # requested root/wheel names to its installer.
            root_owned = (member.uname == "root" and member.gname == "wheel" if use_pkg
                          else member.uid == 0 and member.gid == 0)
            if entry is None or not root_owned:
                raise ValueError("unexpected native package entry or owner: " + member.name)
            if "symlink" in entry:
                if not member.issym() or member.linkname != entry["symlink"]:
                    raise ValueError("native package changed staged symlink: " + name)
            elif not member.isfile() or member.mode & 0o7022 or (
                    bool(member.mode & 0o111) != entry["executable"] or
                    hashlib.file_digest(archive.extractfile(member), "sha256").hexdigest() != entry["sha256"]):
                raise ValueError("native package changed staged file or permissions: " + name)
    if expected:
        raise ValueError("native package omitted staged payload: " + ", ".join(sorted(expected)))


def payload_files(manifest, system):
    files = manifest["files"]
    selected = manifest["selected_backends"]
    if not selected or any(system not in STAGE.ALLOW.get(backend, set()) for backend in selected):
        raise ValueError("staged backend selection is outside the native BSD matrix")
    for path, entry in files.items():
        if not re.fullmatch(r"[A-Za-z0-9_./+-]+", path):
            raise ValueError("payload name incompatible with BSD packing-list syntax")
        if path not in PRODUCTS and not path.startswith(("libexec/lurker/", "share/lurker/")):
            raise ValueError("payload path is outside the installed Lurker layout: " + path)
        if path in PRODUCTS and (not entry.get("executable") or "sha256" not in entry):
            raise ValueError("staged Lurker product must be an executable regular file: " + path)
    if not {"bin/lurker", "bin/lurker-helper"} <= files.keys():
        raise ValueError("staged Lurker CLI or helper is missing")
    return sorted([*files, "share/lurker/manifest.json"])


def build(args):
    system = platform.system()
    if system not in PREFIXES:
        raise ValueError("build native BSD packages on their target BSD")
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", args.version):
        raise ValueError("package version must have three numeric components")
    if not re.fullmatch(r"[0-9a-f]{40}", args.commit):
        raise ValueError("specify the full staged product commit")
    use_pkg = system in {"FreeBSD", "DragonFly"}
    suffix = ".pkg" if use_pkg else ".tgz"
    filename = f"lurker-{args.version}{suffix}"
    output = args.output.absolute()
    if output.name != filename:
        raise ValueError("output filename must be " + filename)
    parent = STAGE.safe_root(output.parent)
    info = parent.stat()
    if info.st_uid != os.geteuid() or info.st_mode & 0o022:
        raise ValueError("output parent must be owned by the packager and have no group/world write")
    if output.exists() or output.is_symlink():
        raise ValueError("output package already exists")
    with tempfile.TemporaryDirectory(prefix=".lurker-native-", dir=parent) as scratch:
        work = Path(scratch)
        prefix, manifest = unpack_archive(args.archive, args.sha256, args.commit, work)
        manifest_digest = STAGE.digest(prefix / "share/lurker/manifest.json")
        files = payload_files(manifest, system)
        desktop = "bin/lurker-desktop" in files
        libraries, paths = dependency_paths(system, manifest, desktop)
        products = "CLI and desktop" if desktop else "CLI"
        backends = ", ".join(manifest["selected_backends"])
        description = f"Lurker {products} for {system}. Backends: {backends}. Source: {args.commit}."
        if use_pkg:
            abi, dependencies = pkg_package(system, prefix, work, files, args.version, description, paths)
        elif system == "NetBSD":
            abi, dependencies = netbsd_package(prefix, work, files, args.version, description, paths)
        else:
            abi, dependencies = openbsd_package(prefix, work, files, args.version, description, libraries, paths)
        package = work / filename
        verify_package(package, PREFIXES[system], manifest, manifest_digest, use_pkg)
        digest = STAGE.digest(package)
        os.link(package, output)
    return {"status": "packaged", "source_commit": args.commit, "staged_archive_sha256": args.sha256,
            "package": str(output), "package_sha256": digest, "os": system,
            "os_release": platform.release(), "abi": abi, "desktop": desktop,
            "staged_manifest_sha256": manifest_digest, "payload_count": len(files),
            "selected_backends": manifest["selected_backends"], "package_dependencies": dependencies}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True, help="Verified stage-unix package.tar.gz")
    parser.add_argument("--sha256", required=True, help="Archive SHA-256 from the staging receipt")
    parser.add_argument("--commit", required=True, help="Exact staged product source commit")
    parser.add_argument("--version", required=True, help="Native package version, for example 0.1.0")
    parser.add_argument("--output", type=Path, required=True, help="New lurker-VERSION.pkg or lurker-VERSION.tgz path")
    try:
        print(json.dumps(build(parser.parse_args()), sort_keys=True))
    except (OSError, KeyError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"BSD package build failed: {error}\n")


if __name__ == "__main__":
    main()
