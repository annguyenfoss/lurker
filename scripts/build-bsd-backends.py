#!/usr/bin/env python3
"""Build and stage private BSD VeraCrypt; never install packages or elevate.

The output is an input to Lurker packaging, not an installed privileged helper.
Build natively as a normal user on FreeBSD >=14 or OpenBSD >=7.8 (amd64).
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
PATCH_SHA256 = "b532ade3b7944a414162efe5d2b21a137698a0a05bbe3b6cdd146a861a4ace9d"
COMMON_PATCH_SHA256 = "3d945f1ed832ffa61d7fb1ab817b36d5b15de858cd864a65ea181567cf6986e0"
OPENBSD_VND_PATCH_SHA256 = "4173de985131b068615bf4a7f16d75fd300a02245007b35af353faa6e999abd0"
FREEBSD_MD_PATCH_SHA256 = "9e2793dfa9671819b81719d14e18c12c99c9d621924bab08d5f9028677eba708"
PATCH_TARGET = "src/Main/UserInterface.cpp"
PATCH_TARGETS = (PATCH_TARGET, "src/Core/Unix/CoreUnix.cpp", "src/Driver/Fuse/FuseService.cpp")
PINS = {
    "veracrypt": {
        "url": "https://codeload.github.com/veracrypt/VeraCrypt/tar.gz/7afc449f2bc69366e8b67b82ad66201325d318c3",
        "revision": "7afc449f2bc69366e8b67b82ad66201325d318c3",
        "sha256": "1bf27ae89671943fbd2ff0cc63e9c727951a774a5c6c3cb26e058bdea0ca39d1",
    },
    "wxwidgets": {
        "url": "https://codeload.github.com/wxWidgets/wxWidgets/tar.gz/refs/tags/v3.2.8.1",
        "revision": "v3.2.8.1",
        "sha256": "65654b00e8d6901f53cee31b0175720620a4153f32bd95980d03aa4424a26978",
    },
    "libiconv": {
        "url": "https://ftp.gnu.org/pub/gnu/libiconv/libiconv-1.18.tar.gz",
        "revision": "1.18",
        "sha256": "3b08f5f4f9b4eb82f151a7040bfd6fe6c6fb922efe4b1659c66ea933276965e8",
    },
}


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def patched_inputs(archive):
    originals = {}
    with tarfile.open(archive) as tar:
        top = tar.getmembers()[0].name.split("/")[0]
        for target in PATCH_TARGETS:
            originals[target] = tar.extractfile(top + "/" + target).read()
    for entry in patch_inventory():
        patch = ROOT / entry["path"]
        if digest(patch) != entry["sha256"]:
            raise RuntimeError("BSD private backend patch failed SHA-256 verification")
        lines = patch.read_bytes().splitlines(keepends=True)
        index = 0
        seen = set()
        while index < len(lines):
            if not lines[index].startswith(b"--- a/"):
                raise RuntimeError("Expected reviewed private backend patch file header")
            target = lines[index][6:].strip().decode("ascii")
            if target not in PATCH_TARGETS or target in seen or index + 2 >= len(lines) or lines[index + 1].rstrip() != b"+++ b/" + target.encode():
                raise RuntimeError("Unexpected private backend patch target")
            seen.add(target)
            index += 2
            hunks = []
            while index < len(lines) and not lines[index].startswith(b"--- a/"):
                line = lines[index]
                if line.startswith(b"@@ "):
                    hunks.append([])
                elif line.startswith((b" ", b"+", b"-")) and hunks:
                    hunks[-1].append(line)
                else:
                    raise RuntimeError("Expected reviewed private backend patch hunks")
                index += 1
            if not hunks or any(not hunk for hunk in hunks):
                raise RuntimeError("Empty private backend patch hunks")
            original = originals[target]
            for hunk in hunks:
                before = b"".join(line[1:] for line in hunk if line[:1] in (b" ", b"-"))
                after = b"".join(line[1:] for line in hunk if line[:1] in (b" ", b"+"))
                if original.count(before) != 1:
                    raise RuntimeError("Reviewed BSD private backend patch does not match pinned upstream")
                original = original.replace(before, after, 1)
            originals[target] = original
    return originals


def patch_inventory():
    patches = [{"path": "native/veracrypt/utf8-console.patch", "sha256": COMMON_PATCH_SHA256},
               {"path": "native/bsd-veracrypt/auxiliary-path.patch", "sha256": PATCH_SHA256}]
    system = platform.system()
    if system == "FreeBSD":
        patches.append({"path": "native/bsd-veracrypt/freebsd-md-sync.patch",
                        "sha256": FREEBSD_MD_PATCH_SHA256})
    elif system == "OpenBSD":
        patches.append({"path": "native/bsd-veracrypt/openbsd-vnd-io.patch",
                        "sha256": OPENBSD_VND_PATCH_SHA256})
    return patches


def prepare(name, cache, offline=False):
    """Verify downloaded bytes and original source inputs on every cache reuse."""
    pin = PINS[name]
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / f"{name}-{pin['revision']}.tar.gz"
    if not archive.exists():
        if offline:
            raise RuntimeError(f"Missing pinned archive: {archive}")
        temporary = archive.with_suffix(".download")
        try:
            with urllib.request.urlopen(pin["url"], timeout=120) as response, temporary.open("wb") as out:
                shutil.copyfileobj(response, out)
            if digest(temporary) != pin["sha256"]:
                raise RuntimeError(f"Downloaded {name} archive failed SHA-256 verification")
            temporary.replace(archive)
        finally:
            temporary.unlink(missing_ok=True)
    if digest(archive) != pin["sha256"]:
        raise RuntimeError(f"Cached {name} archive failed SHA-256 verification")
    # Preserve old qualified sources and their wx output across engine updates.
    # wx and libiconv keep their existing pinned cache layout.
    cache_name = f"{name}-{pin['revision'][:16]}" if name == "veracrypt" else name
    source = cache / cache_name
    stamp = cache / f"{cache_name}.inputs.json"
    if not source.exists():
        with tempfile.TemporaryDirectory(prefix=f".{name}-", dir=cache) as tmp:
            temporary = Path(tmp)
            with tarfile.open(archive) as tar:
                members = tar.getmembers()
                roots = {Path(m.name).parts[0] for m in members}
                if len(roots) != 1 or any(m.issym() or m.islnk() for m in members):
                    raise RuntimeError(f"Unexpected {name} source archive layout")
                tar.extractall(temporary, filter="data")
            extracted = temporary / roots.pop()
            inputs = {str(p.relative_to(extracted)): digest(p) for p in sorted(extracted.rglob("*")) if p.is_file()}
            extracted.rename(source)
            stamp.write_text(json.dumps({"archive": pin["sha256"], "inputs": inputs}, sort_keys=True) + "\n")
    if not stamp.is_file():
        raise RuntimeError(f"Unverified source cache {source}; use an empty --source-dir")
    recorded = json.loads(stamp.read_text())
    if recorded.get("archive") != pin["sha256"]:
        raise RuntimeError(f"Stale source cache {source}; use an empty --source-dir")
    patched = patched_inputs(archive) if name == "veracrypt" else {}
    patched_hashes = {target: hashlib.sha256(data).hexdigest() for target, data in patched.items()}
    for relative, expected in recorded["inputs"].items():
        path = source / relative
        permitted = {expected}
        if relative in patched_hashes:
            permitted.add(patched_hashes[relative])
        if path.is_symlink() or not path.is_file() or digest(path) not in permitted:
            raise RuntimeError(f"Modified source input {name}/{relative}; use an empty --source-dir")
    for target, data in patched.items():
        if digest(source / target) != patched_hashes[target]:
            (source / target).write_bytes(data)
    return source.resolve()


def command(argv, logs, name, cwd=None, timeout=3600):
    """Keep build output in a named log; stdout stays concise."""
    logs.mkdir(parents=True, exist_ok=True)
    log = logs / f"{name}.log"
    with log.open("wb") as output:
        process = subprocess.run(list(map(str, argv)), cwd=cwd, stdout=output, stderr=subprocess.STDOUT, timeout=timeout)
    if process.returncode:
        excerpt = log.read_text(errors="replace").splitlines()[-12:]
        raise RuntimeError(f"{name} failed ({process.returncode}); log={log}\n" + "\n".join(excerpt))
    return log


def linked_libraries(binary):
    text = subprocess.check_output(["ldd", str(binary)], text=True, stderr=subprocess.STDOUT)
    if "not found" in text:
        raise RuntimeError("Built backend has unresolved shared-library dependencies")
    libraries = set()
    for line in text.splitlines():
        # FreeBSD `libx => /path (address)` and OpenBSD's address/type table.
        for path in re.findall(r"(/[^\s()]+\.so(?:\.[0-9]+)*)", line):
            libraries.add(Path(path))
    if not libraries:
        raise RuntimeError("Could not audit the built backend's dynamic dependencies")
    return sorted(libraries)


def wx_specification(source, wx):
    return {"source": str(source), "wx_source": str(wx), "wx_pin": PINS["wxwidgets"],
            "vc_pin": PINS["veracrypt"], "configure": ["NOGUI=1", "WXSTATIC=1", "--disable-tests"],
            "system": platform.system(), "release": platform.release(),
            "compiler": subprocess.check_output(["c++", "--version"], text=True)}


def wx_cached(stamp, specification):
    if not stamp.is_file():
        return False
    try:
        recorded = json.loads(stamp.read_text())
        return (recorded["specification"] == specification and bool(recorded["files"])
                and all(Path(p).is_file() and digest(p) == value for p, value in recorded["files"].items()))
    except (KeyError, ValueError, OSError):
        return False


def record_wx(stamp, specification, source):
    build = source / "src/wxrelease"
    files = [build / "wx-config", *build.glob("lib/*.a"), *build.glob("lib/wx/include/*/wx/setup.h")]
    if len(files) < 3 or not all(p.is_file() for p in files):
        raise RuntimeError("Static wx build is incomplete")
    stamp.write_text(json.dumps({"specification": specification, "files": {str(p): digest(p) for p in files}}, sort_keys=True) + "\n")


def compatible_wx_build(stamp, source, wx):
    """Reuse only exact wx artifacts built by an identical upstream recipe.

    wx-config embeds its build prefix; keep that directory in place and pass
    WX_BUILD_DIR instead of relocating or rewriting the verified static build.
    """
    if not stamp.is_file():
        return None
    try:
        recorded = json.loads(stamp.read_text())
        previous = recorded["specification"]
        current = wx_specification(source, wx)
        if any(previous.get(key) != value for key, value in current.items() if key not in {"source", "vc_pin"}):
            return None
        if not wx_cached(stamp, previous):
            return None
        old_pin = previous["vc_pin"]
        archive = stamp.parent / f"veracrypt-{old_pin['revision']}.tar.gz"
        if not archive.is_file() or digest(archive) != old_pin["sha256"]:
            return None
        with tarfile.open(archive) as tar:
            top = tar.getmembers()[0].name.split("/")[0]
            with tar.extractfile(top + "/src/Makefile") as stream:
                recipe = hashlib.file_digest(stream, "sha256").hexdigest()
        if recipe != digest(source / "src/Makefile"):
            return None
        directory = Path(previous["source"]) / "src/wxrelease"
        config = directory / "wx-config"
        expected = {str(config), *map(str, directory.glob("lib/*.a")),
                    *map(str, directory.glob("lib/wx/include/*/wx/setup.h"))}
        if len(expected) < 3 or set(recorded["files"]) != expected:
            return None
        return directory
    except (KeyError, ValueError, OSError, tarfile.TarError):
        return None


def stage(binary, output, source, wx, target, build_options, iconv_source=None):
    output.mkdir(parents=True, exist_ok=True)
    (output / "bsd-veracrypt-source.json").unlink(missing_ok=True)
    destination = output / "lurker-veracrypt-bsd"
    temporary = output / ".lurker-veracrypt-bsd.tmp"
    shutil.copy2(binary, temporary)
    temporary.chmod(0o755)
    subprocess.run(["strip", str(temporary)], check=True)
    dependencies = linked_libraries(temporary)
    private = []
    for library in dependencies:
        if str(library).startswith(("/lib/", "/usr/lib/", "/usr/libexec/")):
            continue
        if target != "FreeBSD" or library.parent not in (Path("/usr/local/lib"), output / "lib"):
            raise RuntimeError(f"Unqualified runtime library path: {library}")
        if library.name not in ("libfuse.so.2", "libiconv.so.2"):
            raise RuntimeError(f"Unqualified private library/license inventory: {library.name}")
        libdir = output / "lib"
        libdir.mkdir(exist_ok=True)
        copied = libdir / library.name
        # A repeated stage already resolves ORIGIN to the previous private copy.
        # Refresh from the known installed build dependency rather than trusting
        # that prior generated artifact as its own provenance.
        shutil.copy2((Path("/usr/local/lib") / library.name).resolve(), copied)
        private.append({"name": copied.name, "sha256": digest(copied)})
    if private:
        dynamic = subprocess.check_output(["readelf", "-d", str(temporary)], text=True)
        if "$ORIGIN/lib" not in dynamic:
            raise RuntimeError("FreeBSD private libraries require the qualified $ORIGIN/lib runpath")
        resolved = {p.name: p.resolve() for p in linked_libraries(temporary)}
        for item in private:
            if resolved.get(item["name"]) != (output / "lib" / item["name"]).resolve():
                raise RuntimeError(f"Private runtime library resolves outside the staged package: {item['name']}")
    version = subprocess.check_output([str(temporary), "--text", "--version"], text=True).strip()
    if "VeraCrypt" not in version:
        raise RuntimeError("Built backend failed its version check")
    temporary.replace(destination)
    licenses = output / "licenses"
    licenses.mkdir(exist_ok=True)
    shutil.copy2(source / "License.txt", licenses / "VeraCrypt.txt")
    shutil.copy2(wx / "docs/licence.txt", licenses / "wxWidgets.txt")
    if iconv_source is not None:
        shutil.copy2(iconv_source / "COPYING.LIB", licenses / "libiconv-LGPL-2.1.txt")
    # Private copies of OS packages retain their upstream notices. Base system
    # libraries are external; their installed OS license inventory remains host-owned.
    for component in ["libiconv", "fusefs-libs"]:
        if any(component.startswith("libiconv") and p["name"].startswith("libiconv") or component == "fusefs-libs" and p["name"].startswith("libfuse") for p in private):
            candidates = list(Path("/usr/local/share/licenses").glob(component + "-*"))
            if not candidates:
                raise RuntimeError(f"Missing installed package notices for {component}")
            shutil.copytree(candidates[0], licenses / component, dirs_exist_ok=True)
    manifest = {
        "schema": 1, "backend": "veracrypt", "platform": target,
        "os_release": platform.release(), "architecture": platform.machine(),
        "sources": {name: pin for name, pin in PINS.items() if name != "libiconv" or iconv_source is not None},
        "patches": patch_inventory(), "build_options": build_options,
        "version": version, "binary": destination.name, "binary_sha256": digest(destination),
        "private_libraries": private,
        "system_libraries": [p.name for p in dependencies if str(p).startswith(("/lib/", "/usr/lib/", "/usr/libexec/"))],
        "fuse_version": subprocess.check_output(["pkg-config", "--modversion", "fuse"], text=True).strip(),
        "runtime": {"external_fuse_kernel": True, "separate_veracrypt": False, "privilege": "root", "source_kind": "regular_file"},
    }
    members = [destination, *[output / "lib" / item["name"] for item in private],
               licenses / "VeraCrypt.txt", licenses / "wxWidgets.txt"]
    if iconv_source is not None:
        members.append(licenses / "libiconv-LGPL-2.1.txt")
    for name in ("libiconv", "fusefs-libs"):
        owned = licenses / name
        if owned.is_dir():
            members.extend(p for p in owned.rglob("*") if p.is_file())
    manifest["files"] = {str(p.relative_to(output)): digest(p) for p in sorted(members)}
    (output / "bsd-veracrypt-source.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=ROOT / "target/bsd-backends/src")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/release/backends")
    parser.add_argument("--jobs", type=int, default=min(2, os.cpu_count() or 1))
    parser.add_argument("--offline", action="store_true")
    options = parser.parse_args()
    target = platform.system()
    if target not in ("FreeBSD", "OpenBSD") or platform.machine() not in ("amd64", "x86_64"):
        parser.error("Build natively on FreeBSD/OpenBSD amd64; other targets are outside the approved matrix")
    release = tuple(int(x) for x in platform.release().split("-", 1)[0].split(".")[:2])
    if release < ((14,) if target == "FreeBSD" else (7, 8)):
        parser.error("Pinned VeraCrypt requires FreeBSD 14+ or OpenBSD 7.8+")
    if os.geteuid() == 0:
        parser.error("Build as a normal user; this script never installs or elevates")
    if options.jobs < 1:
        parser.error("--jobs must be positive")
    for tool in ["gmake", "cc", "c++", "yasm", "pkg-config", "strip", "ldd", "readelf"]:
        if not shutil.which(tool):
            parser.error(f"Missing native build prerequisite: {tool}")
    subprocess.run(["pkg-config", "--exists", "fuse", "libpcsclite"], check=True)
    source = prepare("veracrypt", options.source_dir.resolve(), options.offline)
    wx = prepare("wxwidgets", options.source_dir.resolve(), options.offline)
    iconv = prepare("libiconv", options.source_dir.resolve(), options.offline) if target == "FreeBSD" else None
    logs = options.output_dir.resolve() / "build-logs"
    flags = ["NOGUI=1", "WXSTATIC=1"]
    if target == "FreeBSD":
        # The top-level immediate variable and exported submake each consume
        # one $$ layer; the recipe shell then preserves quoted ORIGIN.
        # Base wx is static; bundle libiconv/libfuse as private shared objects.
        flags.append("TC_EXTRA_LFLAGS=-Wl,-rpath,'$$$$ORIGIN/lib'")
    wx_stamp = options.source_dir.resolve() / "wx-build.json"
    wx_spec = wx_specification(source, wx)
    if not wx_cached(wx_stamp, wx_spec):
        compatible = compatible_wx_build(wx_stamp, source, wx)
        if compatible is not None:
            flags.append(f"WX_BUILD_DIR={compatible}")
        else:
            command(["gmake", f"-j{options.jobs}", *flags, f"WX_ROOT={wx}", "WX_CONFIGURE_EXTRA_FLAGS=--disable-tests", "wxbuild"], logs, "wxwidgets", cwd=source / "src")
            record_wx(wx_stamp, wx_spec, source)
    # Changed link flags must not reuse an executable linked by a prior recipe.
    (source / "src/Main/veracrypt").unlink(missing_ok=True)
    command(["gmake", f"-j{options.jobs}", *flags], logs, "veracrypt", cwd=source / "src")
    manifest = stage(source / "src/Main/veracrypt", options.output_dir.resolve(), source, wx, target, flags, iconv)
    print(json.dumps({"status": "built", "backend": manifest["binary"], "manifest": str(options.output_dir.resolve() / "bsd-veracrypt-source.json")}, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError, subprocess.TimeoutExpired, OSError, ValueError) as error:
        sys.exit(str(error))
