#!/usr/bin/env python3
"""Build private Linux crypto backends; never install, elevate, or change the host.

Upstream encryption and filesystem implementations stay in external processes.
Only the small, reviewed integration patches are part of the Lurker repository.
"""
import argparse
import hashlib
import importlib.util
import json
import lzma
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
SOURCES = {
    "veracrypt": ("veracrypt/VeraCrypt", "7afc449f2bc69366e8b67b82ad66201325d318c3"),
    "veracrypt-create": ("veracrypt/VeraCrypt", "7afc449f2bc69366e8b67b82ad66201325d318c3"),
    "lkl": ("lkl/linux", "d0f76a77e8c2ca68bd7d96190b8da09040eab072"),
}

# Checksums of GitHub codeload archives for the pinned commits, verified before
# extraction and before accepting an existing archive cache. Mutable generated
# build trees are not a substitute for these immutable source inputs.
SOURCE_ARCHIVE_SHA256 = {
    "veracrypt": "1bf27ae89671943fbd2ff0cc63e9c727951a774a5c6c3cb26e058bdea0ca39d1",
    "veracrypt-create": "1bf27ae89671943fbd2ff0cc63e9c727951a774a5c6c3cb26e058bdea0ca39d1",
    "lkl": "ef6e5b7cd66c023bc16193013fdacbc1285b9eb86df864b8c6cefe0ad4e0e168",
}

LKL_STABLE_PATCH = {
    "version": "6.12.118",
    "url": "https://cdn.kernel.org/pub/linux/kernel/v6.x/patch-6.12.118.xz",
    "sha256": "422a33351c5ac482c3b4a1f3897b17f9dc5670eb6938eb9eadd9cef84b948ee5",
}


def verify_archive(name, archive):
    with archive.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    if digest != SOURCE_ARCHIVE_SHA256[name]:
        raise RuntimeError(f"Pinned source archive checksum mismatch: {archive}")


def run(command, **kwargs):
    print("+", " ".join(map(str, command)), flush=True)
    return subprocess.run(list(map(str, command)), check=True, **kwargs)




# These are outputs of the pinned make recipes, not independently supplied source.
GENERATED_INPUTS = {
    "veracrypt": {"src/License.txt.h", "src/Common/Language.xml.h"},
    "veracrypt-create": {"src/License.txt.h", "src/Common/Language.xml.h"},
    "lkl": {".vmlinux.export.c", "init/utsversion-tmp.h", "kernel/bounds.s", "lib/crc32table.h",
            "tools/lkl/include/lkl_autoconf.h", "tools/lkl/tests/autoconf.sh", "tools/lkl/Makefile.conf",
            "scripts/kconfig/lexer.lex.c", "scripts/kconfig/parser.tab.h", "scripts/kconfig/parser.tab.c",
            "scripts/mod/devicetable-offsets.s", "scripts/mod/devicetable-offsets.h", "scripts/mod/elfconfig.h",
            "lib/raid6/int1.c", "lib/raid6/int2.c", "lib/raid6/int4.c", "lib/raid6/int8.c", "lib/raid6/tables.c",
            "arch/lkl/kernel/asm-offsets.s"},
}

def generated_input(name, relative):
    if relative in GENERATED_INPUTS.get(name, set()): return True
    return name == 'lkl' and relative.startswith(('tools/lkl/include/lkl/', 'include/generated/',
           'arch/lkl/include/generated/', '.checked-atomic-', '.tmp_vmlinux'))


def verify_source_cache(name, source, archive, patches):
    """Bind cached tracked inputs to the archive and ordered patches.

    Build outputs absent from the archive are deliberately ignored; every
    upstream input, including entire patched files, is verified on each reuse.
    """
    source = source.resolve()
    inputs = {"repository": SOURCES[name][0], "revision": SOURCES[name][1],
              "archive_sha256": SOURCE_ARCHIVE_SHA256[name],
              "patch_sha256": hashlib.sha256(patches[-1].read_bytes()).hexdigest()}
    if name == "lkl":
        inputs["stable_patch"] = LKL_STABLE_PATCH
    binding = source / '.lurker-source-inputs.json'
    if binding.exists():
        previous = json.loads(binding.read_text())
        if any(previous.get(key) != value for key, value in inputs.items()):
            raise RuntimeError(f"Source cache inputs changed: {source}. Use a fresh source directory for the new revision/patch.")
    affected = set()
    for patch in patches:
        with patch.open(encoding="utf-8", errors="surrogateescape") as contents:
            for line in contents:
                if line.startswith('diff --git '):
                    names = shlex.split(line)[2:]
                    if len(names) != 2 or not names[0].startswith('a/') or not names[1].startswith('b/'):
                        raise RuntimeError('Invalid git patch paths')
                elif line.startswith(('--- a/', '+++ b/')):
                    names = [line[4:].split('\t', 1)[0].rstrip('\n')]
                else:
                    continue
                for name_in_patch in names:
                    relative = Path(name_in_patch[2:])
                    if not relative.parts or relative.is_absolute() or '..' in relative.parts:
                        raise RuntimeError('Unsafe source patch path')
                    affected.add(relative.as_posix())
    manifest, mismatches, missing_directories = {}, [], []
    def compare(relative, expected, kind='file'):
        actual = source / relative
        if kind == 'link':
            expected = os.path.normpath(expected)
            valid = actual.is_symlink() and os.path.normpath(os.readlink(actual)) == expected
        elif kind == 'deleted':
            valid = not actual.exists() and not actual.is_symlink()
        else:
            valid = actual.is_file() and not actual.is_symlink()
            if valid:
                with actual.open('rb') as stream:
                    valid = hashlib.file_digest(stream, 'sha256').hexdigest() == expected
        manifest[relative] = {kind: expected}
        if not valid and len(mismatches) < 10: mismatches.append(relative)
    with tempfile.TemporaryDirectory(prefix='lurker-source-verify-', dir=archive.parent) as directory:
        expected_root = Path(directory)
        top = None
        with tarfile.open(archive, 'r|gz') as tar:
            for member in tar:
                parts = Path(member.name).parts
                if not parts or Path(member.name).is_absolute() or '..' in parts:
                    raise RuntimeError('Unsafe pinned archive path')
                if top is None: top = parts[0]
                if parts[0] != top: raise RuntimeError('Pinned archive has multiple source roots')
                if len(parts) == 1: continue
                relative = Path(*parts[1:]); key = relative.as_posix()
                if member.isdir():
                    actual = source / relative
                    if not actual.exists() and not actual.is_symlink():
                        missing_directories.append(key)
                    elif (not actual.is_dir() or actual.is_symlink()) and len(mismatches) < 10:
                        mismatches.append(key)
                elif member.issym():
                    if key in affected:
                        target = expected_root / relative; target.parent.mkdir(parents=True, exist_ok=True)
                        target.symlink_to(member.linkname)
                    else:
                        compare(key, member.linkname, 'link')
                elif member.isfile():
                    stream = tar.extractfile(member)
                    if key in affected:
                        target = expected_root / relative; target.parent.mkdir(parents=True, exist_ok=True)
                        with target.open('wb') as output: shutil.copyfileobj(stream, output)
                    else:
                        compare(key, hashlib.file_digest(stream, 'sha256').hexdigest())
                else: raise RuntimeError('Unsupported pinned source archive member')
        for patch in patches:
            applied = subprocess.run(['patch', '-p1', '--forward', '--force', '--batch', '--fuzz=0',
                                      '--no-backup-if-mismatch', '-i', str(patch.resolve())],
                                     cwd=expected_root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            if applied.returncode:
                raise RuntimeError('Cannot derive exact patched source inputs from the pinned archive: '
                                   + applied.stdout.decode(errors='replace')[-2000:])
        for key in affected:
            expected = expected_root / key
            if expected.is_symlink(): compare(key, os.readlink(expected), 'link')
            elif expected.is_file(): compare(key, hashlib.sha256(expected.read_bytes()).hexdigest())
            else: compare(key, True, 'deleted')
    # GNU patch removes an empty parent after deleting its last file.
    for directory in missing_directories:
        members = [entry for key, entry in manifest.items() if key.startswith(directory + '/')]
        if (not members or any('deleted' not in entry for entry in members)) and len(mismatches) < 10:
            mismatches.append(directory)
    source_suffixes = {'.c', '.cc', '.cpp', '.cxx', '.h', '.hpp', '.hxx', '.S', '.s', '.asm', '.inc', '.py', '.sh', '.pl', '.in'}
    for actual in source.rglob('*'):
        if not (actual.is_file() or actual.is_symlink()): continue
        relative = actual.relative_to(source).as_posix()
        if relative in manifest or generated_input(name, relative): continue
        if (actual.suffix in source_suffixes or actual.name.startswith(('Makefile', 'Kconfig')) or actual.name == 'Kbuild') and len(mismatches) < 10:
            mismatches.append(relative + ' (unexpected source input)')
    if mismatches:
        raise RuntimeError('Source cache does not match pinned source inputs: ' + ', '.join(mismatches))
    inputs['source_files_sha256'] = hashlib.sha256(json.dumps(manifest, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    inputs['verified_source_files'] = len(manifest)
    temporary = binding.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(inputs, indent=2, sort_keys=True) + '\n')
    temporary.replace(binding)
    return inputs


def unpack_lkl_stable_patch(cache, offline, destination):
    pin = LKL_STABLE_PATCH
    archive = cache / f"patch-{pin['version']}.xz"
    if not archive.is_file():
        if offline:
            raise RuntimeError(f"Missing source archive {archive}; run once without --offline.")
        cache.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=cache, prefix=archive.name + ".") as directory:
            temporary = Path(directory) / archive.name
            with urllib.request.urlopen(pin["url"], timeout=120) as response, temporary.open("wb") as output:
                shutil.copyfileobj(response, output)
            with temporary.open("rb") as downloaded:
                if hashlib.file_digest(downloaded, "sha256").hexdigest() != pin["sha256"]:
                    raise RuntimeError(f"Pinned LKL stable patch checksum mismatch: {archive}")
            temporary.replace(archive)
    with archive.open("rb") as compressed:
        if hashlib.file_digest(compressed, "sha256").hexdigest() != pin["sha256"]:
            raise RuntimeError(f"Pinned LKL stable patch checksum mismatch: {archive}")
        compressed.seek(0)
        with lzma.open(compressed, "rb") as source, destination.open("wb") as output:
            shutil.copyfileobj(source, output)
    return destination


def prepare(name, cache, offline):
    patch = ROOT / "native/linux-userspace/patches" / f"{name}.patch"
    if name == "lkl":
        if not offline:
            cache.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="lurker-lkl-stable-", dir=cache if cache.is_dir() else None) as directory:
            stable = unpack_lkl_stable_patch(cache, offline, Path(directory) / "stable.patch")
            return prepare_source(name, cache, offline, (stable, patch))
    return prepare_source(name, cache, offline, (patch,))


def prepare_source(name, cache, offline, patches):
    repository, revision = SOURCES[name]
    source = cache / (f"lkl-{LKL_STABLE_PATCH['version']}" if name == "lkl" else name)
    archive = cache / f"{name}-{revision}.tar.gz"
    if archive.exists():
        verify_archive(name, archive)
    elif source.exists() and not (source / ".git").exists():
        raise RuntimeError(f"Missing pinned source archive {archive}. Restore the archive or remove generated sources and download again.")
    if not source.exists():
        if offline and not archive.exists():
            raise RuntimeError(f"Missing source archive {archive}; run once without --offline.")
        cache.mkdir(parents=True, exist_ok=True)
        if not archive.exists():
            url = f"https://codeload.github.com/{repository}/tar.gz/{revision}"
            print(f"Downloading pinned source: {url}", flush=True)
            temporary = archive.with_suffix(".download")
            try:
                with urllib.request.urlopen(url, timeout=120) as response, temporary.open("wb") as out:
                    shutil.copyfileobj(response, out)
                temporary.replace(archive)
            finally:
                temporary.unlink(missing_ok=True)
        verify_archive(name, archive)
        with tarfile.open(archive) as tar:
            top = tar.getmembers()[0].name.split("/")[0]
            tar.extractall(cache, filter="data")
        (cache / top).rename(source)
    if (source / ".git").exists():
        actual = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source, text=True).strip()
        if actual != revision:
            raise RuntimeError(f"{source} is {actual}; expected pinned revision {revision}.")
    patch = patches[-1]
    digest = hashlib.sha256(patch.read_bytes()).hexdigest()
    if name == "lkl":
        digest = hashlib.sha256((LKL_STABLE_PATCH["sha256"] + ":" + digest).encode()).hexdigest()
    stamp = source / ".lurker-patch-sha256"
    if stamp.exists() and stamp.read_text().strip() != digest:
        raise RuntimeError(f"Patch changed: remove the generated source directory {source} and rebuild.")
    # --force disables patch's automatic direction guessing: a pristine source
    # must fail a reverse check. Verify even stamped caches before every build.
    reversed_patch = subprocess.run(
        ["patch", "-p1", "--reverse", "--dry-run", "--force", "--batch", "-i", str(patch)],
        cwd=source, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    if reversed_patch.returncode:
        if stamp.exists():
            raise RuntimeError(f"Cached sources do not match their patch: {source}. Restore pristine sources and remove the stamp.")
        for source_patch in patches:
            run(["patch", "-p1", "--forward", "--force", "--batch", "--fuzz=0", "--no-backup-if-mismatch",
                 "-i", source_patch], cwd=source)
    verify_source_cache(name, source, archive, patches)
    stamp.write_text(digest + "\n")
    return source.resolve()


def publish_creator(source, output):
    output.mkdir(parents=True, exist_ok=True)
    temporary = output / "lurker-veracrypt-create.tmp"
    shutil.copy2(source / "src/Main/veracrypt", temporary)
    temporary.chmod(0o755)
    run(["strip", "--strip-unneeded", temporary])
    version = subprocess.check_output([temporary, "--lurker-creator-version"], text=True).strip()
    if version != "lurker-creator-1":
        raise RuntimeError("Invalid private VeraCrypt creator")
    temporary.replace(output / "lurker-veracrypt-create")
    licenses = output / "licenses"
    licenses.mkdir(exist_ok=True)
    shutil.copy2(source / "License.txt", licenses / "VeraCrypt.txt")
    (output / "creator-source.json").write_text(json.dumps({
        "source": SOURCES["veracrypt-create"],
        "source_archive_sha256": SOURCE_ARCHIVE_SHA256["veracrypt-create"],
        "patch_sha256": hashlib.sha256((ROOT / "native/linux-userspace/patches/veracrypt-create.patch").read_bytes()).hexdigest(),
        "binary_sha256": hashlib.sha256((output / "lurker-veracrypt-create").read_bytes()).hexdigest(),
    }, indent=2) + "\n")



def build_module(name):
    location = Path(__file__).with_name(name + '.py')
    spec = importlib.util.spec_from_file_location(name, location)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def build_cpp(source, flags, wx_metadata, jobs):
    wanted = {"flags": flags, "wx_inputs": wx_metadata["inputs"], "wx_static_libraries": wx_metadata["static_libraries"]}
    binding = source / '.lurker-build-inputs.json'
    previous = json.loads(binding.read_text()) if binding.exists() else None
    if previous != wanted and any((source / 'src').rglob('*.o')):
        # Only these small VC trees are cleaned. Never discard the unchanged
        # multi-gigabyte LKL object cache because a wx/toolchain input changed.
        run(['make', '-C', source / 'src', 'clean', *flags])
    run(['make', '-C', source / 'src', f'-j{jobs}', *flags])
    temporary = binding.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(wanted, indent=2, sort_keys=True) + '\n')
    temporary.replace(binding)


def finalize_private_libraries(output, helper_names, wx_metadata, cache, offline):
    module = build_module('linux_private_elf')
    manifest = module.bundle(output, helper_names, cache, offline)
    notices = []
    directory = output / 'licenses/wxwidgets'
    directory.mkdir(parents=True, exist_ok=True)
    for path in wx_metadata['license_paths']:
        source = Path(path)
        target = directory / source.relative_to(wx_metadata['source_dir'])
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target); target.chmod(0o644)
        relative = str(target.relative_to(output)); notices.append(relative)
        manifest['files'][relative] = {'sha256': hashlib.sha256(target.read_bytes()).hexdigest()}
    provenance = directory / 'provenance.json'
    provenance.write_text(json.dumps({'schema': 1, 'inputs': wx_metadata['inputs'],
        'static_libraries': wx_metadata['static_libraries'], 'license_files': notices}, indent=2, sort_keys=True) + '\n')
    provenance.chmod(0o644)
    relative = str(provenance.relative_to(output))
    manifest['files'][relative] = {'sha256': hashlib.sha256(provenance.read_bytes()).hexdigest()}
    manifest['static_libraries'] = [{'name': 'wxWidgets', 'provenance': relative, 'license_files': notices}]
    (output / 'private-libraries.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    creator = output / 'creator-source.json'
    information = json.loads(creator.read_text())
    information['binary_sha256'] = hashlib.sha256((output / 'lurker-veracrypt-create').read_bytes()).hexdigest()
    information['private_libraries_sha256'] = hashlib.sha256((output / 'private-libraries.json').read_bytes()).hexdigest()
    creator.write_text(json.dumps(information, indent=2, sort_keys=True) + '\n')
    return manifest


def publish_crypto_manifest(output):
    """Describe only independent crypto inputs, never a bundled filesystem driver."""
    if any((output / name).exists() for name in ("lurker-fs", "lklfuse")):
        raise RuntimeError("Private crypto artifact must not contain a filesystem driver")
    manifest = {"schema": 1, "protocol": "lurker-userspace-4",
        "contract": "private-crypto-external-filesystem",
        "source": SOURCES["veracrypt"],
        "source_archive_sha256": SOURCE_ARCHIVE_SHA256["veracrypt"],
        "patch_sha256": hashlib.sha256((ROOT / "native/linux-userspace/patches/veracrypt.patch").read_bytes()).hexdigest(),
        "binary_sha256": hashlib.sha256((output / "lurker-veracrypt").read_bytes()).hexdigest(),
        "private_libraries_sha256": hashlib.sha256((output / "private-libraries.json").read_bytes()).hexdigest()}
    (output / "userspace-crypto-source.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=ROOT / "target/userspace/src")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/release/userspace")
    parser.add_argument("--wx-cache-dir", type=Path, default=ROOT / "target/private-wx-linux")
    parser.add_argument("--tool-cache-dir", type=Path, default=ROOT / "target/private-elf-cache")
    parser.add_argument("--jobs", type=int, default=min(8, os.cpu_count() or 2))
    parser.add_argument("--offline", action="store_true")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--only-creator", action="store_true", help="Build only the private header/container creator")
    mode.add_argument("--crypto-only", action="store_true", help="Build private crypto and creator only (the default); filesystem drivers are installed separately")
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("These helpers are Linux-only.")
    if os.geteuid() == 0:
        parser.error("Build as your normal user, without sudo.")
    if args.jobs < 1:
        parser.error("--jobs must be positive.")
    required = ["gcc", "g++", "make", "pkg-config", "patch", "strip", "readelf"]
    for tool in required:
        if not shutil.which(tool):
            parser.error(f"Required build tool not found: {tool}")
    run(["pkg-config", "--exists", "fuse3"])
    output = args.output_dir.resolve()
    if any((output / name).exists() for name in ("lurker-fs", "lklfuse")):
        parser.error("Output contains a filesystem driver; use a fresh crypto-only output directory")
    names = ["veracrypt-create"] if args.only_creator else ["veracrypt-create", "veracrypt"]
    sources = {name: prepare(name, args.source_dir.resolve(), args.offline) for name in names}
    vc = sources["veracrypt-create"]
    wx_config, wx_metadata = build_module("linux_wx").build(args.wx_cache_dir, args.jobs, args.offline)
    flags = ["NOGUI=1", "WITHFUSE3=1", "WX_CONFIG=" + str(wx_config)]
    if not shutil.which("yasm"):
        flags.append("NOASM=1")
    build_cpp(vc, flags, wx_metadata, args.jobs)
    if args.only_creator:
        publish_creator(vc, args.output_dir.resolve())
        finalize_private_libraries(args.output_dir.resolve(), ["lurker-veracrypt-create"], wx_metadata, args.tool_cache_dir, args.offline)
        return
    build_cpp(sources["veracrypt"], flags, wx_metadata, args.jobs)
    output.mkdir(parents=True, exist_ok=True)
    binary = sources["veracrypt"] / "src/Main/veracrypt"
    temporary = output / "lurker-veracrypt.tmp"
    shutil.copy2(binary, temporary)
    temporary.chmod(0o755)
    run(["strip", "--strip-unneeded", temporary])
    version = subprocess.check_output([temporary, "--lurker-userspace-version"], text=True, timeout=5).strip()
    if version != "lurker-userspace-4":
        raise RuntimeError("Invalid private crypto helper protocol")
    temporary.replace(output / "lurker-veracrypt")
    publish_creator(vc, output)
    finalize_private_libraries(output, ["lurker-veracrypt", "lurker-veracrypt-create"], wx_metadata, args.tool_cache_dir, args.offline)
    publish_crypto_manifest(output)
    print(f"Private crypto helpers built in {output}. Install the host filesystem prerequisite separately.")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
