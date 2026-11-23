"""Native backend provenance and Mach-O staging; no package installation or FUSE copy."""
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import tarfile
import urllib.request

VC_REVISION = "7afc449f2bc69366e8b67b82ad66201325d318c3"
VERACRYPT_PATCHES = ("native/veracrypt/utf8-console.patch",
                     "native/veracrypt/macos-fuset-loopback.patch")
VERACRYPT_FUSE_T_MINIMUM = "1.2.9"
SOURCES = {
    "veracrypt": (
        f"https://codeload.github.com/veracrypt/VeraCrypt/tar.gz/{VC_REVISION}",
        "1bf27ae89671943fbd2ff0cc63e9c727951a774a5c6c3cb26e058bdea0ca39d1",
    ),
    "wxwidgets": (
        "https://github.com/wxWidgets/wxWidgets/releases/download/v3.2.8.1/wxWidgets-3.2.8.1.tar.bz2",
        "ad0cf6c18815dcf1a6a89ad3c3d21a306cd7b5d99a602f77372ef1d92cb7d756",
    ),
}

FOLDER_FUSE_MOUNT = {"runtime": "fuse-t", "transport": "fskit", "minimum_os": "26.0"}
GOCRYPTFS_FUSE_HELPER = Path("/Library/Application Support/fuse-t/bin/go-nfsv4-1.2.9")
GOCRYPTFS_FUSE_HELPER_SHA256 = "348361a699b768d555ec41803e492075b9179691dbfac6b9020db5c20add5fa7"
# Historical recipe metadata. Legacy install names are recognized only to reject them.
MACFUSE_REFERENCE = {"package": "fuse", "library": "/usr/local/lib/libfuse.2.dylib",
                     "info": "/Library/Filesystems/macfuse.fs/Contents/Info.plist"}
FUSE_RUNTIMES = {
    "fuse-t": {"package": "fuse-t", "library": "/usr/local/lib/libfuse-t.dylib",
               "flags": ["WITHFUSET=1", "VC_OSX_FUSET=1"]},
}


def run(*argv, capture=False, cwd=None, env=None):
    return subprocess.run(
        [str(arg) for arg in argv], check=True, cwd=cwd, env=env,
        text=True, stdout=subprocess.PIPE if capture else None,
    ).stdout


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def source(name, cache, offline=False, *, patches=(), variant=None):
    url, digest = SOURCES[name]
    archive = cache / f"{name}-{digest[:16]}.archive"
    tag = f"{name}-{digest[:16]}"
    if patches:
        tag += "-patch-" + "-".join(sha256(patch)[:16] for patch in patches)
    if variant is not None:
        if not re.fullmatch(r"[a-z0-9-]+", variant):
            raise ValueError("Unsafe native source cache variant")
        tag += "-" + variant
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
            for patch in patches:
                run("patch", "-p1", "--forward", "--force", "--batch", "-i", Path(patch).resolve(), cwd=destination)
            write_json(manifest, {str(path.relative_to(destination)): sha256(path)
                                 for path in destination.rglob("*") if path.is_file()})
        finally:
            shutil.rmtree(temporary)
    if not manifest.is_file():
        if patches:
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


def external_fuse_runtime(path):
    # Deliberately leave the separately installed runtime out of our bundle.
    # Do not allow arbitrary /usr/local or Homebrew libraries through this rule.
    if re.fullmatch(r"/usr/local/lib/libfuse-t(?:-[0-9.]+)?\.dylib", path):
        return "fuse-t"
    if (Path(path).name in {"libfuse.2.dylib", "libfuse.dylib", "libosxfuse.2.dylib"}
            or re.search(r"/(?:macfuse|osxfuse)\.fs/", path, re.IGNORECASE)):
        return "macfuse"
    return None


def is_external_fuse(path):
    return external_fuse_runtime(path) == "fuse-t"


def require_administrator_controlled(path):
    """The external runtime is system-installed code, never a private dev library."""
    path = Path(path).absolute()
    try:
        resolved = path.resolve(strict=True)
        for item in dict.fromkeys([path, *path.parents, resolved, *resolved.parents]):
            info = item.lstat()
            if info.st_uid != 0 or (not stat.S_ISLNK(info.st_mode) and info.st_mode & 0o022):
                raise RuntimeError(f"External FUSE path is not administrator-controlled: {item}")
        if not resolved.is_file():
            raise RuntimeError(f"External FUSE library is not a regular file: {path}")
    except OSError as error:
        raise RuntimeError(f"Missing/inaccessible external FUSE runtime: {path}: {error}") from error


def fuse_runtime_info(runtime):
    if runtime not in FUSE_RUNTIMES:
        raise RuntimeError("macOS builds require FUSE-T")
    configuration = FUSE_RUNTIMES[runtime]
    package = configuration["package"]
    run("pkg-config", "--exists", package)
    api_version = run("pkg-config", "--modversion", package, capture=True).strip()
    library = configuration["library"]
    require_administrator_controlled(library)
    return {"implementation": runtime, "version": api_version, "api_version": api_version,
            "pkg_config": package, "library": library, "library_sha256": sha256(library)}


def manifest_fuse_runtime(provenance):
    runtime = provenance.get("fuse_runtime", {}).get("implementation")
    if runtime is None:
        # Existing developer FUSE-T manifests precede explicit runtime selection.
        choices = {external_fuse_runtime(path) for path in provenance.get("external_libraries", [])}
        if len(choices) != 1 or None in choices:
            raise RuntimeError("Backend manifest does not identify one supported FUSE runtime")
        runtime = choices.pop()
    if runtime not in FUSE_RUNTIMES:
        raise RuntimeError("macOS backend manifest must select FUSE-T")
    return runtime


def veracrypt_fuse_runtime(provenance):
    return manifest_fuse_runtime(provenance)


def require_veracrypt_fuse_t_version(version):
    if (not isinstance(version, str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version)
            or tuple(map(int, version.split("."))) < tuple(map(int, VERACRYPT_FUSE_T_MINIMUM.split(".")))):
        raise RuntimeError(f"macOS VeraCrypt requires FUSE-T {VERACRYPT_FUSE_T_MINIMUM} or later for loopback SMB.")


def verify_manifest_files(root, files):
    """Check digest-bound payload paths before following any private dependency."""
    root = Path(root).resolve(strict=True)
    for relative, expected in files.items():
        path = root / relative
        if (Path(relative).is_absolute() or ".." in Path(relative).parts
                or not path.resolve(strict=True).is_relative_to(root) or sha256(path) != expected):
            raise RuntimeError(f"Private backend provenance mismatch: {relative}")


def verify_veracrypt_manifest(root, provenance, project_root):
    runtime = veracrypt_fuse_runtime(provenance)
    require_veracrypt_fuse_t_version(provenance.get("fuse_runtime", {}).get("version"))
    expected = {name: {"url": spec[0], "sha256": spec[1]} for name, spec in SOURCES.items()}
    patches = {name: sha256(Path(project_root) / name) for name in VERACRYPT_PATCHES}
    if (provenance.get("veracrypt_revision") != VC_REVISION
            or provenance.get("source_archives") != expected
            or provenance.get("patches") != patches):
        raise RuntimeError("Private VeraCrypt manifest differs from the current reviewed source/patch pins")
    artifacts = provenance.get("artifacts", {})
    if "lurker-veracrypt" not in artifacts:
        raise RuntimeError("Private backend manifest does not cover lurker-veracrypt")
    verify_manifest_files(root, artifacts)
    return runtime


def source_constant(project_root, filename, name):
    for node in ast.parse((Path(project_root) / filename).read_text()).body:
        if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == name for t in node.targets):
            return ast.literal_eval(node.value)
    raise RuntimeError(f"Missing reviewed build constant: {name}")


def verify_folder_manifest(root, backend, manifest, project_root, architecture):
    recipe = "scripts/build-folder-backends.py"
    url, revision, _ = source_constant(project_root, recipe, "PINS")[backend]
    patches = source_constant(project_root, recipe, "PATCHES").get(backend, [])
    patches += source_constant(project_root, recipe, "PLATFORM_PATCHES").get("Darwin", {}).get(backend, [])
    if backend == "gocryptfs":
        patches.append(source_constant(project_root, recipe, "DARWIN_GO_FUSE_PATCH"))
        revision_go_fuse = source_constant(project_root, recipe, "DARWIN_GO_FUSE")[1]
        if manifest.get("go_fuse_revision") != revision_go_fuse or manifest.get("external_libraries") != []:
            raise RuntimeError("Private gocryptfs must use the reviewed FUSE-T helper transport")
    expected = {relative: sha256(Path(project_root) / relative) for relative in patches}
    if manifest.get("source") != url or manifest.get("revision") != revision or manifest.get("patches", {}) != expected:
        raise RuntimeError(f"Private {backend} differs from reviewed source/patch pins")
    target = manifest.get("target", {})
    if target not in ({"GOOS": "darwin", "GOARCH": {"arm64": "arm64", "x86_64": "amd64"}[architecture]},
                      {"os": "Darwin", "machine": architecture}):
        raise RuntimeError(f"Private {backend} target differs from the native package")
    private = manifest.get("private_libraries", {})
    if any(not name.startswith(backend + "-libs/") or Path(name).suffix != ".dylib" for name in private):
        raise RuntimeError(f"Private {backend} has unexpected library paths")
    verify_manifest_files(root, {backend: manifest["sha256"], **private})
    notices = manifest.get("license_files")
    if not notices:
        raise RuntimeError(f"Private {backend} has no digest-bound license files")
    if any(Path(name).name == "libomp.dylib" for name in private) and "native/llvm-openmp/LICENSE.TXT" not in notices:
        raise RuntimeError(f"Private {backend} is missing the LLVM OpenMP notice")
    verify_manifest_files(Path(root) / "licenses" / backend, notices)
    if manifest.get("mount") != FOLDER_FUSE_MOUNT:
        raise RuntimeError(f"macOS {backend} requires the FUSE-T FSKit profile")
    if backend == "cryfs":
        manifest_fuse_runtime(manifest)


def verify_gocryptfs_fuse_helper():
    require_administrator_controlled(GOCRYPTFS_FUSE_HELPER)
    if sha256(GOCRYPTFS_FUSE_HELPER) != GOCRYPTFS_FUSE_HELPER_SHA256:
        raise RuntimeError("Install the qualified FUSE-T 1.2.9 helper for gocryptfs")
    run("codesign", "--verify", "--strict", GOCRYPTFS_FUSE_HELPER)


def verify_cryptomator_manifest(root, manifest, project_root, architecture):
    source = Path(project_root) / "native/folder/cryptomator"
    expected_mount = {"provider": "org.cryptomator.frontend.fuse.mount.FuseTMountProvider",
                      "runtime": "fuse-t", "transport": "fskit", "minimum_os": "26.0"}
    if manifest.get("mount") != expected_mount:
        raise RuntimeError("macOS Cryptomator requires the qualified FUSE-T FSKit profile")
    if (manifest.get("os") != "Darwin" or manifest.get("architecture") != architecture
            or manifest.get("sources") != json.loads((source / "dependencies.lock.json").read_text())):
        raise RuntimeError("Private Cryptomator differs from the reviewed target/dependency lock")
    for relative in ["pom.xml", "src/main/java/module-info.java", "src/main/java/org/lurker/cryptomator/Main.java"]:
        if manifest.get("files", {}).get("source/" + relative) != sha256(source / relative):
            raise RuntimeError(f"Private Cryptomator helper source differs: {relative}")
    verify_manifest_files(root, manifest["files"])


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


def stage_macho(source_binary, destination, frameworks, architecture, *, fuse_runtime=None):
    """Copy a complete private dylib closure and rewrite it relative to each image.

    Reject unresolved rpaths and frameworks instead of leaking developer paths.
    Dependencies of external FUSE are the user's FUSE installation's contract.
    """
    if fuse_runtime is not None and fuse_runtime not in FUSE_RUNTIMES:
        raise RuntimeError("macOS staging requires FUSE-T")
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
            # Rewrite only the reviewed external FUSE-T install names.
            external_fuse = dependency
            if dependency.startswith("@rpath/"):
                external_fuse = "/usr/local/lib/" + dependency.removeprefix("@rpath/")
            runtime = external_fuse_runtime(external_fuse)
            if runtime is not None:
                if runtime != "fuse-t":
                    raise RuntimeError(f"Unsupported external FUSE dependency: {dependency}")
                require_administrator_controlled(external_fuse)
                resolved_runtime = external_fuse_runtime(str(Path(external_fuse).resolve()))
                if resolved_runtime is not None and resolved_runtime != runtime:
                    raise RuntimeError(f"External FUSE alias resolves to a different runtime: {external_fuse}")
                if external_fuse != dependency:
                    run("install_name_tool", "-change", dependency, external_fuse, target)
                externals.add(external_fuse)
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
            if external_fuse_runtime(str(resolved)) == "macfuse":
                raise RuntimeError(f"Unsupported external FUSE dependency: {dependency}")
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
        # Approved absolute FUSE install names remain the external exceptions.
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
