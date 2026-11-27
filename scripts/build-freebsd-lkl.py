#!/usr/bin/env python3
"""Build an external FreeBSD filesystem prerequisite; never install or elevate.

This output is deliberately outside Lurker's private backend/package inventory.
FUSE remains the administrator-installed native host library and kernel module.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
REVISION = "86dd3afb590eccc1903611bdaa8bac87757eb80d"
SOURCE_SHA256 = "d63cccf64ab8938389b0054e3e68ee467100bfc98475b1864ab7dac542d51317"
PORT_REVISION = "46cf3d9696491c61d91ecd22db11c7f7c1e62c24"
HEADER_INSTALL_SHA256 = "7ac5daf12ca64e5e90ae458b851210f8f2248d9c3c7a7d7d87a191bb079f287b"
PORT_PATCHES = {
    "files/patch-tools_lkl_lib_virtio__blk.c": "38e2ea6c21ae0caba4840826917df94585bc561d313b1c9633b23888c0ffd03a",
    "files/patch-arch_lkl_Kconfig": "6fbd96666085b6658ee4ae6418fc627c324c417d573aabf45fcc6ff9ffb01e4f",
    "files/patch-arch_lkl_Makefile": "9643fc707c6242e38017023b0e1b48f143f2e7ac46348272c198cb1af5290516",
    "files/patch-arch_lkl_include_uapi_asm_host__ops.h": "3c817231e9b07d74f9d946a466b2d503813399b50e90105b772e6869347bbb67",
    "files/patch-tools_lkl_Makefile": "cfb6a6d1ea93e8f15ebcddb61aef659b28ad843e788fed484cc416edc3c503b6",
    "files/patch-tools_lkl_Makefile.autoconf": "d9c44960322240f9266e929ce4ad2288d38d1c8b0cf1e46308c46306499a398f",
    "files/patch-tools_lkl_Targets": "b70063fd07507813ea54984d7fc709f3fd7bb80445d3c85a083145e282d2222e",
}
# Filled only from the reviewed native patch, never from downloaded metadata.
HOST_PATCH_SHA256 = "4fd6e10bbd70b132c0c0742acd574eec458d2f16725406c65a4f738e32ec10f6"
HOST_PATCH = "native/freebsd/lkl/host-io.patch"


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def fetch(url, path, expected):
    path = Path(path)
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_suffix(path.suffix + ".partial")
        with urllib.request.urlopen(url, timeout=90) as response, temporary.open("wb") as out:
            shutil.copyfileobj(response, out)
        if digest(temporary) != expected:
            temporary.unlink()
            raise RuntimeError("Downloaded host-tool input failed SHA-256 verification")
        temporary.replace(path)
    if digest(path) != expected:
        raise RuntimeError("Cached host-tool input failed SHA-256 verification")
    return path


def patch_target(raw):
    target = raw.split(b"\t", 1)[0].decode("ascii").removesuffix(".orig")
    if target.startswith(("a/", "b/")):
        target = target[2:]
    parts = PurePosixPath(target)
    if parts.is_absolute() or ".." in parts.parts or str(parts) != target:
        raise RuntimeError("Unsafe host-tool patch target")
    return target


def apply_patch(source, patch):
    """Apply reviewed contexts exactly, without fuzzy patch offsets or shell use."""
    lines = Path(patch).read_bytes().splitlines(keepends=True)
    index = 0
    changed = []
    while index < len(lines):
        if not lines[index].startswith(b"--- ") or index + 2 >= len(lines):
            raise RuntimeError("Expected host-tool patch file header")
        target = patch_target(lines[index][4:].rstrip(b"\r\n"))
        if not lines[index + 1].startswith(b"+++ ") or target != patch_target(lines[index + 1][4:].rstrip(b"\r\n")):
            raise RuntimeError("Host-tool patch cannot change file paths")
        path = source / target
        if path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(source.resolve()):
            raise RuntimeError("Host-tool patch target is not a regular source file")
        original = path.read_bytes()
        index += 2
        hunks = 0
        while index < len(lines) and not lines[index].startswith(b"--- "):
            header = lines[index]
            if not re.fullmatch(rb"@@ -\d+(?:,\d+)? \+\d+(?:,\d+)? @@[^\n]*\n", header):
                raise RuntimeError("Expected host-tool patch hunk")
            index += 1
            body = []
            while index < len(lines) and not lines[index].startswith((b"@@ ", b"--- ")):
                if lines[index][:1] not in (b" ", b"+", b"-"):
                    raise RuntimeError("Unexpected host-tool patch content")
                body.append(lines[index])
                index += 1
            before = b"".join(line[1:] for line in body if line[:1] in (b" ", b"-"))
            after = b"".join(line[1:] for line in body if line[:1] in (b" ", b"+"))
            if not before or original.count(before) != 1:
                raise RuntimeError(f"Host-tool patch context does not uniquely match pinned source: {target}, hunk {hunks + 1}, matches {original.count(before)}")
            original = original.replace(before, after, 1)
            hunks += 1
        if not hunks:
            raise RuntimeError("Empty host-tool source patch")
        path.write_bytes(original)
        changed.append(target)
    return changed


def source_inventory(source):
    inventory = {}
    for path in sorted(source.rglob("*")):
        name = str(path.relative_to(source))
        if name == ".host-tool-inputs.json":
            continue
        if path.is_symlink():
            inventory[name] = {"link": os.readlink(path)}
        elif path.is_file():
            inventory[name] = {"sha256": digest(path), "mode": path.stat().st_mode & 0o777}
    return inventory


def prepare(work):
    patch = ROOT / HOST_PATCH
    if digest(patch) != HOST_PATCH_SHA256:
        raise RuntimeError("Local host-tool patch failed SHA-256 verification")
    downloads = work / "downloads"
    archive = fetch(f"https://codeload.github.com/lkl/linux/tar.gz/{REVISION}", downloads / "linux.tar.gz", SOURCE_SHA256)
    patches = []
    for name, expected in PORT_PATCHES.items():
        url = f"https://raw.githubusercontent.com/freebsd/freebsd-ports/{PORT_REVISION}/filesystems/lkl/{name}"
        patches.append(fetch(url, downloads / Path(name).name, expected))
    identity = {"source_revision": REVISION, "source_sha256": SOURCE_SHA256,
                "port_revision": PORT_REVISION, "port_patches": PORT_PATCHES,
                "host_patch_sha256": HOST_PATCH_SHA256,
                "port_shebang_python": str(Path(sys.executable).resolve())}
    key = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    source = work / ("source-" + key[:20])
    receipt = source / ".host-tool-inputs.json"
    if source.exists():
        prior = json.loads(receipt.read_text()) if receipt.is_file() else {}
        if prior.get("identity") != identity or prior.get("source_files") != source_inventory(source):
            raise RuntimeError("Incomplete or mismatched existing host-tool source; preserve it and choose a fresh work directory")
        return source, identity
    temporary = work / (source.name + ".extracting")
    temporary.mkdir()
    with tarfile.open(archive) as tar:
        tar.extractall(temporary, filter="data")
    children = list(temporary.iterdir())
    if len(children) != 1 or not children[0].is_dir():
        raise RuntimeError("Unexpected host-tool source archive layout")
    extracted = children[0]
    header_installer = extracted / "arch/lkl/scripts/headers_install.py"
    if digest(header_installer) != HEADER_INSTALL_SHA256:
        raise RuntimeError("Unexpected pinned header installer input")
    for entry in [*patches, patch]:
        apply_patch(extracted, entry)
    # Match the official port's SHEBANG_FILES transformation; do not require
    # a global unversioned `python` alias or change the script's body.
    body = header_installer.read_bytes()
    first, remainder = body.split(b"\n", 1)
    if first != b"#!/usr/bin/env python":
        raise RuntimeError("Unexpected header installer shebang")
    header_installer.write_bytes(b"#!" + os.fsencode(Path(sys.executable).resolve()) + b"\n" + remainder)
    extracted.rename(source)
    temporary.rmdir()
    receipt.write_text(json.dumps({"identity": identity, "source_files": source_inventory(source)}, indent=2) + "\n")
    return source, identity


HOST_ONLY_INPUTS = {
    "tools/lkl/lklfuse.c", "tools/lkl/lib/virtio_blk.c",
    "tools/lkl/lib/posix-host.c", "tools/lkl/include/lkl_host.h",
}


def kernel_source_changes(previous, current):
    if previous.keys() != current.keys():
        raise RuntimeError("Kernel reuse source inventory changed its file set")
    changed = sorted(name for name in previous if previous[name] != current[name])
    if set(changed) - HOST_ONLY_INPUTS:
        raise RuntimeError("Kernel reuse cannot include changed kernel/build inputs")
    return changed


def capture_kernel(output, destination, cc):
    """Audit a completed native build into a separately frozen input cache."""
    manifest_path = output / "host-tool.json"
    manifest = json.loads(manifest_path.read_text())
    source = Path(manifest["build_command"][-1]).parent.parent.parent
    original = json.loads((source / ".host-tool-inputs.json").read_text())
    if (manifest["source_revision"] != REVISION or manifest["source_sha256"] != SOURCE_SHA256
            or manifest["port_patches"] != PORT_PATCHES
            or digest(source / "tools/lkl/lklfuse") != manifest["binary_sha256"]
            or subprocess.check_output([str(cc), "--version"], text=True) != manifest["compiler"]):
        raise RuntimeError("Kernel cache requires the exact completed native factory build")
    for name, expected in original["source_files"].items():
        path = source / name
        actual = {"link": os.readlink(path)} if path.is_symlink() else {
            "sha256": digest(path), "mode": path.stat().st_mode & 0o777}
        if actual != expected:
            raise RuntimeError("Completed native source input changed: " + name)
    if digest(source / "lkl.o") != digest(source / "tools/lkl/lib/lkl.o"):
        raise RuntimeError("Installed kernel object differs from the native kernel output")
    if destination.exists():
        raise RuntimeError("Preserve the existing kernel cache; choose a fresh destination")
    destination.mkdir(parents=True)
    shutil.copyfile(source / "tools/lkl/lib/lkl.o", destination / "lkl.o")
    shutil.copytree(source / "tools/lkl/include/lkl", destination / "headers")
    compiled = {str(path.relative_to(destination)): digest(path)
                for path in destination.rglob("*") if path.is_file()}
    receipt = {"identity": original["identity"], "source_files": original["source_files"],
               "compiler": manifest["compiler"], "compiler_sha256": digest(cc),
               "configuration_sha256": digest(source / ".config"),
               "host_configuration": (source / "tools/lkl/Makefile.conf").read_text(),
               "base_manifest_sha256": digest(manifest_path),
               "base_binary_sha256": manifest["binary_sha256"], "compiled_files": compiled}
    (destination / "kernel-inputs.json").write_text(json.dumps(receipt, indent=2) + "\n")
    for path in destination.rglob("*"):
        path.chmod(0o555 if path.is_dir() else 0o444)
    destination.chmod(0o555)


def reuse_kernel(source, cache, cc):
    receipt_path = cache / "kernel-inputs.json"
    receipt = json.loads(receipt_path.read_text())
    current = json.loads((source / ".host-tool-inputs.json").read_text())
    for name in ["source_revision", "source_sha256", "port_revision", "port_patches", "port_shebang_python"]:
        if receipt["identity"][name] != current["identity"][name]:
            raise RuntimeError("Kernel cache source/build identity mismatch")
    changes = kernel_source_changes(receipt["source_files"], current["source_files"])
    if digest(cc) != receipt["compiler_sha256"] or subprocess.check_output([str(cc), "--version"], text=True) != receipt["compiler"]:
        raise RuntimeError("Kernel cache compiler changed")
    for name, expected in receipt["compiled_files"].items():
        path = cache / name
        if path.is_symlink() or not path.is_file() or digest(path) != expected:
            raise RuntimeError("Kernel cache artifact changed: " + name)
    # Native autoconfiguration and the official stat/sed aliases run first.
    environment = {"PATH": "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin", "LANG": "C", "LC_ALL": "C"}
    subprocess.run(["/usr/local/bin/gmake", "-j1", f"CC={cc}", f"HOSTCC={cc}", "bin/stat"],
                   cwd=source / "tools/lkl", env=environment, check=True)
    if (source / "tools/lkl/Makefile.conf").read_text() != receipt["host_configuration"]:
        raise RuntimeError("Native host-library build configuration changed")
    target = source / "tools/lkl/lib/lkl.o"
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(cache / "lkl.o", target)
    shutil.copytree(cache / "headers", source / "tools/lkl/include/lkl", dirs_exist_ok=True)
    # Only the kernel object is reused. GNU make builds every current host-library
    # and client object against the current source and cached kernel ABI headers.
    return {"cache_manifest_sha256": digest(receipt_path), "kernel_sha256": digest(target),
            "source_changes": changes, "base_manifest_sha256": receipt["base_manifest_sha256"],
            "configuration_sha256": receipt["configuration_sha256"],
            "compiled_inputs": receipt["compiled_files"]}


def host_fuse_metadata(binary):
    """Build provenance, not a lifetime byte pin across administrator updates."""
    alias = Path("/usr/local/lib/libfuse.so.2")
    provider = alias.resolve(strict=True)
    if provider.parent != alias.parent or not provider.is_file():
        raise RuntimeError("Host FUSE must resolve within its native library directory")
    for path in {alias, provider, *alias.parents, *provider.parents}:
        metadata = path.lstat()
        if metadata.st_uid != 0 or (not path.is_symlink() and metadata.st_mode & 0o022):
            raise RuntimeError("Host FUSE provider and ancestors must be administrator-controlled")
    environment = {"PATH": "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin", "LANG": "C", "LC_ALL": "C"}
    provider_dynamic = subprocess.check_output(["/usr/bin/readelf", "-d", str(provider)], text=True, env=environment)
    binary_dynamic = subprocess.check_output(["/usr/bin/readelf", "-d", str(binary)], text=True, env=environment)
    if not re.search(r"\(SONAME\).*\[libfuse\.so\.2\]", provider_dynamic):
        raise RuntimeError("Host FUSE provider has the wrong native SONAME")
    if not re.search(r"\(NEEDED\).*\[libfuse\.so\.2\]", binary_dynamic):
        raise RuntimeError("Built tool does not require the expected host FUSE ABI")
    loader = subprocess.check_output(["/usr/bin/ldd", str(binary)], text=True, env=environment)
    resolutions = re.findall(r"\blibfuse\.so\.2 => (/\S+)", loader)
    if len(resolutions) != 1 or Path(resolutions[0]).resolve(strict=True) != provider:
        raise RuntimeError("Actual native loader resolved a different host FUSE provider")
    return {"api": "fuse2", "library": "libfuse.so.2", "path": str(alias), "sha256": digest(provider)}


def restage(output, previous, cc):
    """Keep proven engine bytes only after rechecking every factory source input."""
    manifest_path = previous / "host-tool.json"
    manifest = json.loads(manifest_path.read_text())
    if (manifest.get("source_revision") != REVISION or manifest.get("source_sha256") != SOURCE_SHA256
            or manifest.get("port_revision") != PORT_REVISION or manifest.get("port_patches") != PORT_PATCHES
            or manifest.get("host_patch_sha256") != HOST_PATCH_SHA256
            or manifest.get("protocol") != "freebsd-lkl-host-io-1"
            or digest(previous / "lklfuse") != manifest.get("binary_sha256")
            or subprocess.check_output([str(cc), "--version"], text=True) != manifest.get("compiler")):
        raise RuntimeError("Restaging requires exact verified native factory inputs and binary")
    source = Path(manifest["build_command"][-1]).parent.parent.parent
    original = json.loads((source / ".host-tool-inputs.json").read_text())
    if any(manifest.get(key) != value for key, value in original["identity"].items()):
        raise RuntimeError("Factory source identity differs from its output manifest")
    for name, expected in original["source_files"].items():
        path = source / name
        actual = {"link": os.readlink(path)} if path.is_symlink() else {
            "sha256": digest(path), "mode": path.stat().st_mode & 0o777}
        if actual != expected:
            raise RuntimeError("Restaging found a changed factory source input: " + name)
    if (digest(source / "tools/lkl/lklfuse") != manifest["binary_sha256"]
            or digest(source / "COPYING") != digest(previous / "COPYING")):
        raise RuntimeError("Restaging binary or license differs from the verified source build")
    manifest["host_fuse"] = host_fuse_metadata(previous / "lklfuse")
    manifest["restaged_from_manifest_sha256"] = digest(manifest_path)
    if output.exists():
        raise RuntimeError("Preserve existing evidence; choose a fresh restaging output")
    output.mkdir(parents=True)
    for name in ["lklfuse", "COPYING"]:
        shutil.copyfile(previous / name, output / name)
        (output / name).chmod(0o755 if name == "lklfuse" else 0o644)
    (output / "host-tool.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (output / "host-tool.json").chmod(0o644)
    print(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--cc", default="/usr/local/bin/gcc14")
    parser.add_argument("--capture-kernel-from", type=Path)
    parser.add_argument("--kernel-cache", type=Path)
    parser.add_argument("--restage-from", type=Path)
    args = parser.parse_args()
    if platform.system() != "FreeBSD" or platform.machine() not in ("amd64", "x86_64"):
        parser.error("Build this external host tool natively on FreeBSD amd64")
    if os.geteuid() == 0:
        parser.error("Build as a normal user; administrator installation is a separate explicit step")
    work = args.work_dir.resolve()
    work.mkdir(parents=True, exist_ok=True)
    cc = Path(args.cc).resolve(strict=True)
    if args.capture_kernel_from is not None:
        if args.output is None:
            parser.error("--output names the fresh audited kernel-cache directory")
        capture_kernel(args.capture_kernel_from.resolve(), args.output.resolve(), cc)
        print(args.output.resolve())
        return
    if args.restage_from is not None:
        if args.output is None:
            parser.error("--output names a fresh restaging directory")
        restage(args.output.resolve(), args.restage_from.resolve(), cc)
        return
    source, identity = prepare(work)
    if args.prepare_only:
        print(source)
        return
    if args.output is None:
        parser.error("--output is required for a build")
    output = args.output.resolve()
    if output.exists():
        parser.error("Use a new output directory; existing host-tool evidence is preserved")
    if args.kernel_cache is not None:
        identity["kernel_reuse"] = reuse_kernel(source, args.kernel_cache.resolve(), cc)
    environment = {"PATH": "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin", "LANG": "C", "LC_ALL": "C"}
    command = ["/usr/local/bin/gmake", "-j1", "V=1", f"CC={cc}", f"HOSTCC={cc}", "LDFLAGS=-pie -Wl,-z,noexecstack", str(source / "tools/lkl/lklfuse")]
    with (work / (source.name + ".build.log")).open("ab") as log:
        subprocess.run(command, cwd=source / "tools/lkl", env=environment, stdout=log, stderr=subprocess.STDOUT, check=True)
    binary = source / "tools/lkl/lklfuse"
    version = subprocess.check_output([str(binary), "--freebsd-host-io-version"], text=True).strip()
    if version != "freebsd-lkl-host-io-1":
        raise RuntimeError("Built host tool lacks the qualified I/O protocol")
    identity["host_fuse"] = host_fuse_metadata(binary)
    output.mkdir(parents=True)
    shutil.copyfile(binary, output / "lklfuse")
    (output / "lklfuse").chmod(0o755)
    shutil.copyfile(source / "COPYING", output / "COPYING")
    (output / "COPYING").chmod(0o644)
    identity.update({"binary_sha256": digest(binary), "build_command": command,
                     "compiler": subprocess.check_output([str(cc), "--version"], text=True),
                     "uname": list(platform.uname()), "protocol": version,
                     "installation": "external administrator-installed host prerequisite; not a private Lurker backend"})
    (output / "host-tool.json").write_text(json.dumps(identity, indent=2) + "\n")
    (output / "host-tool.json").chmod(0o644)
    print(output)


if __name__ == "__main__":
    main()
