"""Build-only private ELF dependency closure; never installs or elevates.

The host supplies its ABI (glibc/loader and C++ runtime). Every other DT_NEEDED
object is copied under its exact SONAME, with private origin-relative lookup.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request

PATCHELF_VERSION = "0.18.0"
PATCHELF_URL = "https://github.com/NixOS/patchelf/releases/download/0.18.0/patchelf-0.18.0.tar.bz2"
PATCHELF_SHA256 = "1952b2a782ba576279c211ee942e341748fdb44997f704dd53def46cd055470b"
SYSTEM = re.compile(r"^(?:ld(?:-linux[^/]*|64)?\.so(?:\..*)?|lib(?:c|m|pthread|dl|rt|resolv|util|anl|nss_[^/]+|BrokenLocale)\.so(?:\..*)?)$")
BASELINE = re.compile(r"^(?:ld(?:-linux[^/]*|64)?\.so(?:\..*)?|lib(?:c|m|pthread|dl|rt|resolv|util|anl|nss_[^/]+|BrokenLocale|stdc\+\+|gcc_s)\.so(?:\..*)?)$")
LIBRARY_DIRS = ("/usr/lib", "/usr/lib64", "/lib", "/lib64", "/usr/lib/x86_64-linux-gnu", "/lib/x86_64-linux-gnu", "/usr/lib/aarch64-linux-gnu", "/lib/aarch64-linux-gnu")
SAFE_NAME = re.compile(r"^[A-Za-z0-9_+.:@-]+$")


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def output(command):
    return subprocess.check_output(list(map(str, command)), text=True, env={**os.environ, "LC_ALL": "C"})


def elf_info(path):
    path = Path(path)
    with path.open("rb") as stream:
        if stream.read(4) != b"\x7fELF":
            raise RuntimeError("Private dependency is not ELF: " + str(path))
    tool = shutil.which("readelf") or shutil.which("llvm-readelf")
    if not tool:
        raise RuntimeError("readelf/llvm-readelf is required for private libraries")
    text = output([tool, "-d", path])
    header = output([tool, "-h", path])
    return {"needed": re.findall(r"\(NEEDED\).*?\[([^]]+)\]", text),
            "soname": re.findall(r"\(SONAME\).*?\[([^]]+)\]", text),
            "search": re.findall(r"\((?:RPATH|RUNPATH)\).*?\[([^]]*)\]", text),
            "machine": re.search(r"Machine:\s*(.*)", header).group(1).strip()}


def validate_search(info, expected, path):
    if info["search"] not in ([], [expected]):
        raise RuntimeError("Unsafe/non-private ELF search path: " + str(path))


def system_library(name, machine):
    if not SAFE_NAME.fullmatch(name) or "/" in name:
        raise RuntimeError("Invalid dependency name: " + name)
    for directory in LIBRARY_DIRS:
        candidate = Path(directory) / name
        if candidate.exists():
            candidate = candidate.resolve(strict=True)
            info = elf_info(candidate)
            if info["machine"] == machine:
                if info["soname"] != [name]:
                    raise RuntimeError("Dependency SONAME differs from its requested ABI: " + name)
                return candidate
    raise RuntimeError("Cannot resolve native dependency for target machine: " + name)


def private_closure(helpers):
    """Resolve by native ELF metadata only; never execute ldd or payload code."""
    queue = [(Path(path), False) for path in helpers]
    resolved = {}
    machine = None
    while queue:
        path, library = queue.pop(0)
        info = elf_info(path)
        machine = machine or info["machine"]
        if info["machine"] != machine:
            raise RuntimeError("Mixed ELF architectures in private closure")
        validate_search(info, "$ORIGIN" if library else "$ORIGIN/lib", path)
        for name in info["needed"]:
            if BASELINE.fullmatch(name):
                continue
            if SYSTEM.fullmatch(name):
                raise RuntimeError("Host glibc components must never be bundled: " + name)
            if name not in resolved:
                dependency = system_library(name, machine)
                resolved[name] = dependency
                queue.append((dependency, True))
    return resolved


def patcher(cache_dir, offline=False):
    """Use a pinned private build tool, without modifying the host installation."""
    cache = Path(cache_dir).resolve() / "patchelf-0.18.0"
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / "source.tar.bz2"
    if not archive.exists():
        if offline:
            raise RuntimeError("Pinned private patchelf archive missing in offline mode")
        temporary = cache / "source.download"
        try:
            with urllib.request.urlopen(PATCHELF_URL, timeout=120) as source, temporary.open("xb") as target:
                shutil.copyfileobj(source, target)
            temporary.replace(archive)
        finally:
            temporary.unlink(missing_ok=True)
    if digest(archive) != PATCHELF_SHA256:
        raise RuntimeError("Pinned patchelf source checksum mismatch")
    executable = cache / "source/src/patchelf"
    receipt = cache / "build.json"
    if executable.exists():
        if not receipt.exists():
            raise RuntimeError("Private patchelf cache has no verified build receipt")
        previous = json.loads(receipt.read_text())
        if previous.get("archive_sha256") != PATCHELF_SHA256 or previous.get("binary_sha256") != digest(executable):
            raise RuntimeError("Private patchelf cached build differs from its receipt")
    else:
        source = cache / "source"
        if source.exists():
            raise RuntimeError("Incomplete private patchelf build; preserve evidence and use a fresh cache")
        with tarfile.open(archive) as tree:
            tree.extractall(cache, filter="data")
        (cache / ("patchelf-" + PATCHELF_VERSION)).rename(source)
        log = cache / "build.log"
        with log.open("wb") as stream:
            for command in [[source / "configure", "--disable-dependency-tracking"], ["make", "-j2"]]:
                subprocess.run(list(map(str, command)), cwd=source, stdout=stream, stderr=subprocess.STDOUT, check=True)
        receipt.write_text(json.dumps({"archive_url": PATCHELF_URL, "archive_sha256": PATCHELF_SHA256,
                                       "binary_sha256": digest(executable)}, indent=2) + "\n")
    if output([executable, "--version"]).strip() != "patchelf " + PATCHELF_VERSION:
        raise RuntimeError("Unexpected private patchelf version")
    return executable, json.loads(receipt.read_text())


def package_info(path):
    path = Path(path).resolve(strict=True)
    if shutil.which("pacman"):
        name = output(["pacman", "-Qqo", path]).strip()
        text = output(["pacman", "-Qi", name])
        fields = dict(re.findall(r"^([^:\n]+?)\s*:\s*(.*)$", text, re.M))
        return {"manager": "pacman", "name": name, "version": fields.get("Version"),
                "licenses": fields.get("Licenses", ""), "url": fields.get("URL", "")}
    if shutil.which("dpkg-query"):
        text = output(["dpkg-query", "-S", path]).splitlines()[0]
        name = text.split(": /", 1)[0]
        return {"manager": "dpkg", "name": name, "version": output(["dpkg-query", "-W", "-f=${Version}", name]).strip(),
                "licenses": "See the packaged copyright notice", "url": "https://sources.debian.org/"}
    if shutil.which("rpm"):
        fields = output(["rpm", "-qf", "--qf", "%{NAME}\n%{VERSION}-%{RELEASE}\n%{LICENSE}\n%{URL}\n", path]).splitlines()
        return dict(zip(("name", "version", "licenses", "url"), fields), manager="rpm")
    database = Path("/var/db/pkg")
    if database.is_dir():
        for contents in database.glob("*/*/CONTENTS"):
            if not any(line.startswith("obj " + str(path) + " ") for line in contents.read_text().splitlines()):
                continue
            directory = contents.parent
            def field(name):
                source = directory / name
                return source.read_text().strip() if source.is_file() else ""
            name = field("PN") or re.sub(r"-[0-9].*$", "", directory.name)
            return {"manager": "portage", "name": name, "version": field("PF") or directory.name,
                    "doc_name": directory.name, "licenses": field("LICENSE"), "url": field("HOMEPAGE"),
                    "category": directory.parent.name}
    raise RuntimeError("Cannot identify library producer package/notices: " + str(path))


def library_notices(package, root):
    """Copy actual installed notices, plus unmodified common SPDX license text.

    Common texts are recorded by their real source path; they are not attributed
    as package-authored copyright notices. Debian's copyright files already carry
    the package-specific notices and complete license terms.
    """
    name = package["name"]
    if not SAFE_NAME.fullmatch(name):
        raise RuntimeError("Invalid producer package name")
    candidates = []
    for directory in [Path("/usr/share/licenses") / name, Path("/usr/share/doc") / name,
                      Path("/usr/share/doc") / name.split(":")[0],
                      Path("/usr/share/doc") / package.get("doc_name", name)]:
        if directory.is_dir():
            for path in directory.rglob("*"):
                if path.is_file() and (directory.parts[-2] == "licenses" or re.search(r"(?:copyright|licen[cs]e|copying|authors|notice)", path.name, re.I)):
                    candidates.append(path.resolve())
    for identifier in re.findall(r"[A-Za-z0-9][A-Za-z0-9.+-]*", package.get("licenses", "")):
        for directory in [Path("/usr/share/licenses/spdx"), Path("/usr/share/licenses/common"), Path("/usr/share/licenses/qt6-base"), Path("/var/db/repos/gentoo/licenses")]:
            for suffix in (".txt", ""):
                path = directory / (identifier + suffix)
                if path.is_file(): candidates.append(path.resolve())
    candidates = sorted(set(candidates))
    if not candidates:
        raise RuntimeError("Library license/copyright notices are missing for producer package " + name)
    destination = Path(root) / "licenses/private-libraries" / name
    destination.mkdir(parents=True, exist_ok=True)
    inventory = []
    for index, source in enumerate(candidates):
        if source.stat().st_size > 2 * 1024 * 1024:
            raise RuntimeError("Unexpectedly large license file: " + str(source))
        target = destination / (f"{index:02}-" + source.name)
        shutil.copyfile(source, target); target.chmod(0o644)
        inventory.append({"file": str(target.relative_to(root)), "source_path": str(source), "sha256": digest(target)})
    notice = destination / "producer.json"
    notice.write_text(json.dumps({"package": package, "notice_sources": inventory}, indent=2) + "\n")
    notice.chmod(0o644)
    return [entry["file"] for entry in inventory] + [str(notice.relative_to(root))]


def bundle(output_dir, helper_names, cache_dir, offline=False):
    root = Path(output_dir).resolve(strict=True)
    if not helper_names: raise RuntimeError("At least one helper is required for private closure")
    helpers = [root / name for name in helper_names]
    for path in helpers:
        if not SAFE_NAME.fullmatch(path.name) or path.parent != root or path.is_symlink() or not path.is_file():
            raise RuntimeError("Unsafe/missing helper input: " + str(path))
    libraries = private_closure(helpers)
    tool, tool_provenance = patcher(cache_dir, offline)
    directory = root / "lib"
    if directory.is_symlink(): raise RuntimeError("Private library directory must not be a symlink")
    directory.mkdir(exist_ok=True)
    if set(path.name for path in directory.iterdir()) - set(libraries):
        raise RuntimeError("Private library directory contains unowned/stale files")
    records, files, package_notices = [], {}, {}
    for name, source in sorted(libraries.items()):
        if SYSTEM.fullmatch(name): raise RuntimeError("Forbidden glibc/loader library: " + name)
        target = directory / name
        if target.is_symlink(): raise RuntimeError("Private libraries must be regular SONAME files")
        source_sha256 = digest(source)
        shutil.copyfile(source, target); target.chmod(0o755)
        subprocess.run([tool, "--set-rpath", "$ORIGIN", target], check=True)
        info = elf_info(target)
        if info["soname"] != [name] or info["search"] != ["$ORIGIN"]:
            raise RuntimeError("Private library relocation failed: " + name)
        package = package_info(source)
        key = (package["name"], package["version"])
        if key not in package_notices: package_notices[key] = library_notices(package, root)
        notices = package_notices[key]
        records.append({"name": name, "sha256": digest(target), "soname": name, "source_path": str(source),
                        "source_sha256": source_sha256, "producer_package": package, "license_files": notices,
                        "needed": info["needed"]})
        files["lib/" + name] = {"sha256": digest(target)}
        for notice in notices: files[notice] = {"sha256": digest(root / notice)}
    helper_records = {}
    for path in helpers:
        before = digest(path)
        subprocess.run([tool, "--set-rpath", "$ORIGIN/lib", path], check=True)
        info = elf_info(path)
        if info["search"] != ["$ORIGIN/lib"]: raise RuntimeError("Helper relocation failed: " + path.name)
        helper_records[path.name] = {"source_sha256": before, "sha256": digest(path), "needed": info["needed"]}
    manifest = {"schema": 1, "libraries": records, "files": files, "helpers": helper_records,
                "patcher": tool_provenance, "platform_libraries": "host glibc/loader and C++ runtime; never copied"}
    temporary = root / "private-libraries.json.tmp"
    temporary.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n"); temporary.chmod(0o644)
    temporary.replace(root / "private-libraries.json")
    return manifest
