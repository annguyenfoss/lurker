"""Native backend provenance and Mach-O staging; no package installation or FUSE copy."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import urllib.request

VC_REVISION = "8172bc55f834485f67be271e02c1167981423621"
SOURCES = {
    "veracrypt": (
        f"https://codeload.github.com/veracrypt/VeraCrypt/tar.gz/{VC_REVISION}",
        "fc286de07d0ac88b48c65cade34bac932b30a93cd1719a22f9fef3b69feedfc4",
    ),
    "wxwidgets": (
        "https://github.com/wxWidgets/wxWidgets/releases/download/v3.2.8.1/wxWidgets-3.2.8.1.tar.bz2",
        "ad0cf6c18815dcf1a6a89ad3c3d21a306cd7b5d99a602f77372ef1d92cb7d756",
    ),
}


def run(*argv, capture=False, cwd=None, env=None):
    return subprocess.run(
        [str(arg) for arg in argv], check=True, cwd=cwd, env=env,
        text=True, stdout=subprocess.PIPE if capture else None,
    ).stdout


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def source(name, cache, offline=False, *, patch=None):
    url, digest = SOURCES[name]
    archive = cache / f"{name}-{digest[:16]}.archive"
    tag = f"{name}-{digest[:16]}" + (f"-patch-{sha256(patch)[:16]}" if patch else "")
    destination = cache / tag
    manifest = cache / f"{tag}-inputs.json"
    cache.mkdir(parents=True, exist_ok=True)
    if not archive.exists():
        if offline:
            raise RuntimeError(f"Missing pinned source archive: {archive}")
        temporary = archive.with_suffix(".download")
        try:
            with urllib.request.urlopen(url, timeout=120) as response, temporary.open("wb") as output:
                shutil.copyfileobj(response, output)
            if sha256(temporary) != digest:
                raise RuntimeError(f"Source checksum mismatch for {name}")
            temporary.replace(archive)
        finally:
            temporary.unlink(missing_ok=True)
    if sha256(archive) != digest:
        raise RuntimeError(f"Cached source checksum mismatch: {archive}")
    if not destination.exists():
        temporary = cache / f".{name}-extract-{os.getpid()}"
        temporary.mkdir()
        try:
            with tarfile.open(archive) as stream:
                stream.extractall(temporary, filter="data")
            entries = list(temporary.iterdir())
            if len(entries) != 1 or not entries[0].is_dir():
                raise RuntimeError(f"Unexpected source archive layout: {name}")
            entries[0].rename(destination)
            if patch is not None:
                run("patch", "-p1", "--forward", "--force", "--batch", "-i", Path(patch).resolve(), cwd=destination)
            write_json(manifest, {str(path.relative_to(destination)): sha256(path)
                                 for path in destination.rglob("*") if path.is_file()})
        finally:
            shutil.rmtree(temporary)
    if not manifest.is_file():
        if patch is not None:
            raise RuntimeError(f"Unverified patched source cache: {destination}; use a fresh --cache-dir")
        # Also verify caches created before input manifests were introduced;
        # derive expected bytes from the checked archive, never cached sources.
        expected_inputs = {}
        with tarfile.open(archive) as stream:
            for member in stream:
                if member.isfile():
                    relative = Path(member.name).parts[1:]
                    if not relative or ".." in relative:
                        raise RuntimeError(f"Invalid archive source path: {member.name}")
                    with stream.extractfile(member) as contents:
                        expected_inputs[str(Path(*relative))] = hashlib.file_digest(contents, "sha256").hexdigest()
        write_json(manifest, expected_inputs)
    for relative, expected in json.loads(manifest.read_text()).items():
        path = destination / relative
        if not path.is_file() or sha256(path) != expected:
            raise RuntimeError(f"Extracted source input changed: {path}; use a fresh --cache-dir")
    return destination


def dependencies(binary):
    return [line.strip().split(" (", 1)[0]
            for line in run("otool", "-L", binary, capture=True).splitlines()[1:]]


def is_system(path):
    return path.startswith(("/System/Library/", "/usr/lib/"))


def is_external_fuse(path):
    # Deliberately leave the separately installed runtime out of our bundle.
    # Do not allow arbitrary /usr/local or Homebrew libraries through this rule.
    return bool(re.fullmatch(r"/usr/local/lib/libfuse-t(?:-[0-9.]+)?\.dylib", path))


def minimum_version(binary):
    info = run("xcrun", "vtool", "-show-build", binary, capture=True)
    versions = re.findall(r"\bminos\s+([\d.]+)", info)
    if not versions:
        # Older deployment targets use LC_VERSION_MIN_MACOSX instead.
        info = run("otool", "-l", binary, capture=True)
        versions = re.findall(r"cmd LC_VERSION_MIN_MACOSX\s+cmdsize \d+\s+version ([\d.]+)", info)
    if not versions:
        raise RuntimeError(f"Missing deployment target in {binary}")
    return max(versions, key=lambda value: tuple(map(int, value.split("."))))


def architectures(binary):
    return set(run("lipo", "-archs", binary, capture=True).split())


def stage_macho(source_binary, destination, frameworks, architecture):
    """Copy a complete private dylib closure and rewrite it relative to each image.

    Reject unresolved rpaths and frameworks instead of leaking developer paths.
    Dependencies of external FUSE are the user's FUSE installation's contract.
    """
    destination.parent.mkdir(parents=True, exist_ok=True)
    frameworks.mkdir(parents=True, exist_ok=True)
    copied = {}
    original_by_name = {}
    externals = set()
    targets = []

    def copy_image(original, target):
        original = original.resolve(strict=True)
        if original in copied:
            return copied[original]
        if architecture not in architectures(original):
            raise RuntimeError(f"Missing {architecture} slice: {original}")
        if target.exists():
            raise RuntimeError(f"Refusing to overwrite staged code: {target}")
        shutil.copyfile(original, target)
        target.chmod(0o755)
        copied[original] = target
        targets.append(target)
        for dependency in dependencies(original):
            if dependency == f"@rpath/{original.name}" and original.suffix == ".dylib":
                continue
            if is_system(dependency):
                continue
            if dependency == "@rpath/libfuse-t.dylib":
                # FUSE-T's pkg-config emits an rpath install name. Pin this
                # external dependency to its administrator-installed location
                # before removing all developer/build rpaths below.
                external_fuse = "/usr/local/lib/libfuse-t.dylib"
                if not Path(external_fuse).is_file():
                    raise RuntimeError(f"Missing external FUSE-T runtime: {external_fuse}")
                run("install_name_tool", "-change", dependency, external_fuse, target)
                externals.add(external_fuse)
                continue
            if is_external_fuse(dependency):
                externals.add(dependency)
                continue
            if dependency.startswith("@loader_path/"):
                resolved = original.parent / dependency.removeprefix("@loader_path/")
            elif dependency.startswith("/"):
                resolved = Path(dependency)
            else:
                raise RuntimeError(f"Unresolved dependency {dependency} in {original}")
            if ".framework/" in str(resolved):
                raise RuntimeError(f"Private framework packaging requires an explicit recipe: {resolved}")
            resolved = resolved.resolve(strict=True)
            name = resolved.name
            if name in original_by_name and original_by_name[name] != resolved:
                raise RuntimeError(f"Dylib basename collision: {resolved}")
            original_by_name[name] = resolved
            # A dylib's own LC_ID_DYLIB appears in otool -L; rewrite its ID below.
            if resolved == original:
                continue
            private = copy_image(resolved, frameworks / name)
            relative = os.path.relpath(private, target.parent)
            run("install_name_tool", "-change", dependency, f"@loader_path/{relative}", target)
        if target.parent == frameworks:
            run("install_name_tool", "-id", f"@rpath/{target.name}", target)
        # Never preserve build-time rpaths. All private deps now use loader_path;
        # FUSE-T's absolute install name remains the sole external exception.
        load_commands = run("otool", "-l", target, capture=True)
        for rpath in re.findall(r"cmd LC_RPATH\s+cmdsize \d+\s+path (.*?) \(offset", load_commands):
            run("install_name_tool", "-delete_rpath", rpath, target)
        return target

    copy_image(Path(source_binary), Path(destination))
    for target in targets:
        for dependency in dependencies(target):
            if not (is_system(dependency) or is_external_fuse(dependency)
                    or dependency.startswith(("@loader_path/", "@rpath/"))):
                raise RuntimeError(f"Developer dependency survived staging: {dependency}")
    return targets, sorted(externals)


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
