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
PINS = {
    "veracrypt": {
        "url": "https://codeload.github.com/veracrypt/VeraCrypt/tar.gz/8172bc55f834485f67be271e02c1167981423621",
        "revision": "8172bc55f834485f67be271e02c1167981423621",
        "sha256": "fc286de07d0ac88b48c65cade34bac932b30a93cd1719a22f9fef3b69feedfc4",
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
    source = cache / name
    stamp = cache / f"{name}.inputs.json"
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
    for relative, expected in recorded["inputs"].items():
        path = source / relative
        if path.is_symlink() or not path.is_file() or digest(path) != expected:
            raise RuntimeError(f"Modified source input {name}/{relative}; use an empty --source-dir")
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
        "patches": [], "build_options": build_options,
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
