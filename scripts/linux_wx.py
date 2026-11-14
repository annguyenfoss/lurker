"""Pinned build-only wxBase dependency for private Linux VeraCrypt helpers.

Source, compiler/configuration and the installed headers/static archives are
checked before reuse. All compilation is out of tree; no host installation,
privilege elevation, encryption operation or mount is performed.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request

VERSION = "3.2.8.1"
URL = "https://github.com/wxWidgets/wxWidgets/releases/download/v3.2.8.1/wxWidgets-3.2.8.1.tar.bz2"
SHA256 = "ad0cf6c18815dcf1a6a89ad3c3d21a306cd7b5d99a602f77372ef1d92cb7d756"
FLAGS = ("--disable-shared", "--disable-gui", "--disable-secretstore", "--disable-debug",
         "--disable-protocol", "--disable-protocols", "--disable-url", "--disable-ipc", "--disable-sockets",
         "--without-libcurl", "--disable-fs_inet", "--without-expat", "--with-regex=builtin", "--with-zlib=sys")
LICENSES = ("docs/licence.txt", "docs/lgpl.txt", "docs/gpl.txt", "3rdparty/pcre/LICENCE")
ENV_FLAGS = ("CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS")


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    temporary.replace(path)


def inventory(root):
    """Inventory every installed file/link; never follow an escaping symlink."""
    root = Path(root).resolve(strict=True)
    entries = {}
    for directory, directories, files in os.walk(root, followlinks=False):
        for name in sorted(directories + files):
            path = Path(directory) / name
            relative = str(path.relative_to(root))
            if path.is_symlink():
                resolved = path.resolve(strict=True)
                if not resolved.is_relative_to(root):
                    raise RuntimeError("Installed wx symlink escapes its prefix: " + relative)
                entries[relative] = {"symlink": os.readlink(path)}
            elif path.is_file():
                entries[relative] = {"sha256": digest(path)}
            elif not path.is_dir():
                raise RuntimeError("Unexpected special file in wx installation: " + relative)
    return entries


def verify_source(archive, source):
    """Derive expected bytes from the immutable archive, not a mutable stamp.

    Generated objects/install files live outside this tree. An extra source file
    is rejected too, so old revision leftovers cannot enter future builds.
    """
    if digest(archive) != SHA256:
        raise RuntimeError("Pinned wxWidgets source archive checksum mismatch")
    source = Path(source).resolve(strict=True)
    expected, directories = {}, set()
    with tarfile.open(archive, "r|*") as stream:
        top = None
        for entry in stream:
            parts = Path(entry.name).parts
            if not parts or Path(entry.name).is_absolute() or ".." in parts:
                raise RuntimeError("Unsafe wx source archive path")
            top = top or parts[0]
            if parts[0] != top:
                raise RuntimeError("Unexpected wx source archive root")
            if len(parts) == 1:
                continue
            relative = Path(*parts[1:]); path = source / relative
            for parent in relative.parents:
                directories.add(str(parent))
            if entry.isdir():
                directories.add(str(relative)); continue
            if entry.isfile():
                contents = stream.extractfile(entry)
                expected[str(relative)] = {"sha256": hashlib.file_digest(contents, "sha256").hexdigest(),
                                           "executable": entry.mode & 0o111}
            elif entry.issym():
                expected[str(relative)] = {"symlink": entry.linkname}
            else:
                raise RuntimeError("Unsupported special file in wx source archive")
    for directory in directories - {"."}:
        path = source / directory
        if path.is_symlink() or not path.is_dir():
            raise RuntimeError("wx source directory changed: " + directory)
    for relative, wanted in expected.items():
        path = source / relative
        if "symlink" in wanted:
            valid = path.is_symlink() and os.readlink(path) == wanted["symlink"] and path.resolve().is_relative_to(source)
        else:
            valid = (not path.is_symlink() and path.is_file() and digest(path) == wanted["sha256"]
                     and path.stat().st_mode & 0o111 == wanted["executable"])
        if not valid:
            raise RuntimeError("wx source differs from verified archive: " + relative)
    actual = {str(path.relative_to(source)) for path in source.rglob("*") if path.is_symlink() or not path.is_dir()}
    if actual != set(expected):
        raise RuntimeError("wx source contains unarchived/missing files")
    return hashlib.sha256(json.dumps(expected, sort_keys=True).encode()).hexdigest()


def compiler_info():
    result = {}
    for variable, fallback in (("CC", "gcc"), ("CXX", "g++")):
        command = shlex.split(os.environ.get(variable, fallback))
        if len(command) != 1:
            raise RuntimeError(variable + " must select one compiler executable; use the corresponding FLAGS for options")
        executable = shutil.which(command[0])
        if not executable:
            raise RuntimeError("Missing wx build compiler: " + command[0])
        executable = Path(executable).resolve(strict=True)
        version = subprocess.check_output([executable, "--version"], text=True, env={**os.environ, "LC_ALL": "C"}).splitlines()[0]
        result[variable] = {"path": str(executable), "version": version, "sha256": digest(executable)}
    return result


def validate_receipt(metadata, inputs, prefix):
    if metadata.get("schema") != 1 or metadata.get("inputs") != inputs:
        raise RuntimeError("wx source/configuration/compiler changed; use a fresh wx cache")
    if metadata.get("files") != inventory(prefix):
        raise RuntimeError("Cached wx installation headers/static libraries/configuration changed")


def source_tree(root, offline):
    cache = root / "sources"; cache.mkdir(exist_ok=True)
    archive = cache / ("wxwidgets-" + SHA256[:16] + ".archive")
    source = cache / ("wxwidgets-" + SHA256[:16])
    if not archive.exists():
        if offline:
            raise RuntimeError("Pinned wxWidgets archive missing in offline mode")
        temporary = archive.with_suffix(".download")
        try:
            with urllib.request.urlopen(URL, timeout=120) as response, temporary.open("xb") as target:
                shutil.copyfileobj(response, target)
            if digest(temporary) != SHA256:
                raise RuntimeError("Downloaded wxWidgets source checksum mismatch")
            temporary.replace(archive)
        finally:
            temporary.unlink(missing_ok=True)
    if digest(archive) != SHA256:
        raise RuntimeError("Pinned wxWidgets source archive checksum mismatch")
    if not source.exists():
        with tempfile.TemporaryDirectory(prefix=".wx-extract-", dir=cache) as directory:
            with tarfile.open(archive) as stream:
                stream.extractall(directory, filter="data")
            entries = list(Path(directory).iterdir())
            if len(entries) != 1 or not entries[0].is_dir():
                raise RuntimeError("Unexpected wx archive layout")
            entries[0].rename(source)
    if source.is_symlink() or not source.is_dir():
        raise RuntimeError("wx source cache must be a real directory")
    return archive, source, verify_source(archive, source)


def adopt_experiment(root, inputs, prefix):
    """Adopt the earlier build only after validating its existing evidence.

    The old experiment recorded the compiler version, not its binary hash. That
    limitation is explicit; the new receipt binds current compiler bytes for all
    subsequent reuse and does not relabel them as historical measurements.
    """
    specification = root / "minimal-build-spec.json"
    artifacts = root / "minimal-artifacts.json"
    if not specification.is_file() or not artifacts.is_file():
        raise RuntimeError("Existing wx installation lacks verified build evidence; use a fresh cache")
    old, old_files = json.loads(specification.read_text()), json.loads(artifacts.read_text())
    for record in (old, old_files):
        if record.get("source") != [URL, SHA256] or record.get("source_dir") != inputs["source_dir"] or record.get("flags") != inputs["flags"]:
            raise RuntimeError("Legacy wx source/configuration evidence does not match this build")
    if old.get("compiler") != inputs["compilers"]["CXX"]["version"] or any(inputs["environment"].values()):
        raise RuntimeError("Legacy wx compiler/flags cannot be verified")
    makefile = (root / "minimal-build/Makefile").read_text()
    for variable in ("CC", "CXX"):
        line = next((line for line in makefile.splitlines() if line.startswith(variable + " = ")), "")
        words = shlex.split(line.partition(" = ")[2])
        executable = shutil.which(words[0]) if words else None
        if not executable or str(Path(executable).resolve()) != inputs["compilers"][variable]["path"]:
            raise RuntimeError("Legacy wx configured compiler differs from recorded/current toolchain")
    actual = inventory(prefix)
    regular = {name: value["sha256"] for name, value in actual.items() if "sha256" in value}
    if old_files.get("files") != regular:
        raise RuntimeError("Legacy wx installed artifacts differ from verified inventory")
    links = {name: value for name, value in actual.items() if "symlink" in value}
    expected = str(prefix / "lib/wx/config/base-unicode-static-3.2")
    if links != {"bin/wx-config": {"symlink": expected}}:
        raise RuntimeError("Legacy wx installation has unexpected/unverified symlinks")
    return {"method": "verified-experiment-adoption", "specification_sha256": digest(specification),
            "artifact_inventory_sha256": digest(artifacts), "recorded_compiler_version": old["compiler"],
            "historical_compiler_binary_hash_recorded": False}


def build(cache_dir, jobs=8, offline=False):
    if sys.platform != "linux":
        raise RuntimeError("This private wxBase profile is Linux-only")
    if os.geteuid() == 0:
        raise RuntimeError("Build private wxBase as the ordinary user, without sudo")
    if jobs < 1:
        raise ValueError("wx build jobs must be positive")
    root = Path(cache_dir).expanduser().absolute()
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    if root.is_symlink() or root.stat().st_uid != os.geteuid() or root.stat().st_mode & 0o022:
        raise RuntimeError("wx cache must be a real user-owned directory without group/world write access")
    root = root.resolve()
    archive, source, tree_hash = source_tree(root, offline)
    prefix, directory = root / "install-minimal", root / "minimal-build"
    flags = ["--prefix=" + str(prefix), *FLAGS]
    compilers = compiler_info()
    inputs = {"version": VERSION, "url": URL, "archive_sha256": SHA256, "source_tree_sha256": tree_hash,
              "source_dir": str(source), "flags": flags, "compilers": compilers,
              "environment": {name: os.environ.get(name, "") for name in ENV_FLAGS}}
    receipt = root / "linux-wx-build.json"
    if receipt.exists():
        metadata = json.loads(receipt.read_text()); validate_receipt(metadata, inputs, prefix)
        return prefix / "bin/wx-config", metadata
    if prefix.exists():
        provenance = adopt_experiment(root, inputs, prefix)
    else:
        if directory.exists():
            raise RuntimeError("Incomplete wx build exists without a receipt; preserve it and use a fresh cache")
        directory.mkdir()
        environment = {**os.environ, **{name: value["path"] for name, value in compilers.items()}}
        with (root / "configure.log").open("wb") as log:
            subprocess.run([str(source / "configure"), *flags], cwd=directory, env=environment,
                           stdout=log, stderr=subprocess.STDOUT, check=True)
        with (root / "build.log").open("wb") as log:
            subprocess.run(["make", "-j" + str(jobs)], cwd=directory, env=environment, stdout=log, stderr=subprocess.STDOUT, check=True)
            subprocess.run(["make", "install"], cwd=directory, env=environment, stdout=log, stderr=subprocess.STDOUT, check=True)
        provenance = {"method": "pinned-source-out-of-tree-build", "historical_compiler_binary_hash_recorded": True}
    files = inventory(prefix)
    required = ("lib/libwx_baseu-3.2.a", "lib/libwxregexu-3.2.a", "lib/wx/include/base-unicode-static-3.2/wx/setup.h")
    if any(name not in files or "sha256" not in files[name] for name in required):
        raise RuntimeError("Private static wxBase installation is incomplete")
    config = prefix / "bin/wx-config"
    libs = subprocess.check_output([config, "--libs", "base"], text=True).strip()
    if str(prefix / "lib/libwx_baseu-3.2.a") not in libs or any(name in libs for name in ("-lcurl", "-lexpat", "-lsecret", "-lglib", "-lgio")):
        raise RuntimeError("wxBase is not static/minimal as required")
    metadata = {"schema": 1, "inputs": inputs, "source_dir": str(source), "archive": str(archive),
                "wx_config": str(config), "config": flags, "files": files, "provenance": provenance,
                "libs_base": libs, "license_paths": [str(source / name) for name in LICENSES],
                "licenses": {name: digest(source / name) for name in LICENSES},
                "static_libraries": {name: files[name]["sha256"] for name in required if name.endswith(".a")}}
    write_json(receipt, metadata)
    return config, metadata
