#!/usr/bin/env python3
"""Build private Linux backends; never install, elevate, or change the host.

Upstream encryption and filesystem implementations stay in external processes.
Only the small, reviewed integration patches are part of the Lurker repository.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
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


def verify_source_cache(name, source, archive, patch):
    """Bind cached tracked inputs to the verified archive plus the exact patch.

    Build outputs absent from the archive are deliberately ignored; every
    upstream input, including entire patched files, is verified on each reuse.
    """
    source = source.resolve()
    inputs = {"repository": SOURCES[name][0], "revision": SOURCES[name][1],
              "archive_sha256": SOURCE_ARCHIVE_SHA256[name],
              "patch_sha256": hashlib.sha256(patch.read_bytes()).hexdigest()}
    binding = source / '.lurker-source-inputs.json'
    if binding.exists():
        previous = json.loads(binding.read_text())
        if any(previous.get(key) != value for key, value in inputs.items()):
            raise RuntimeError(f"Source cache inputs changed: {source}. Use a fresh source directory for the new revision/patch.")
    affected = set()
    for line in patch.read_text().splitlines():
        if line.startswith(('--- a/', '+++ b/')):
            relative = Path(line.split()[1][2:])
            if relative.is_absolute() or '..' in relative.parts:
                raise RuntimeError('Unsafe source patch path')
            affected.add(relative.as_posix())
    manifest, mismatches = {}, []
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
    with tempfile.TemporaryDirectory(prefix='lurker-source-verify-') as directory:
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
                    if (not actual.is_dir() or actual.is_symlink()) and len(mismatches) < 10: mismatches.append(key)
                elif member.issym():
                    compare(key, member.linkname, 'link')
                    if key in affected:
                        target = expected_root / relative; target.parent.mkdir(parents=True, exist_ok=True)
                        target.symlink_to(member.linkname)
                elif member.isfile():
                    stream = tar.extractfile(member)
                    if key in affected:
                        target = expected_root / relative; target.parent.mkdir(parents=True, exist_ok=True)
                        with target.open('wb') as output: shutil.copyfileobj(stream, output)
                    else:
                        compare(key, hashlib.file_digest(stream, 'sha256').hexdigest())
                else: raise RuntimeError('Unsupported pinned source archive member')
        applied = subprocess.run(['patch', '-p1', '--forward', '--force', '--batch', '--fuzz=0', '-i', str(patch.resolve())],
                                 cwd=expected_root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        if applied.returncode: raise RuntimeError('Cannot derive exact patched source inputs from the pinned archive: ' + applied.stdout.decode(errors='replace')[-2000:])
        for key in affected:
            expected = expected_root / key
            if expected.is_symlink(): compare(key, os.readlink(expected), 'link')
            elif expected.is_file(): compare(key, hashlib.sha256(expected.read_bytes()).hexdigest())
            else: compare(key, True, 'deleted')
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


def prepare(name, cache, offline):
    repository, revision = SOURCES[name]
    source = cache / name
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
    patch = ROOT / "native/linux-userspace/patches" / f"{name}.patch"
    digest = hashlib.sha256(patch.read_bytes()).hexdigest()
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
        run(["patch", "-p1", "--forward", "--force", "--batch", "-i", patch], cwd=source)
    verify_source_cache(name, source, archive, patch)
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=ROOT / "target/userspace/src")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/release/userspace")
    parser.add_argument("--jobs", type=int, default=min(8, os.cpu_count() or 2))
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--only-creator", action="store_true", help="Build only the private header/container creator; omit FUSE/LKL helpers")
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("These helpers are Linux-only.")
    if os.geteuid() == 0:
        parser.error("Build as your normal user, without sudo.")
    if args.jobs < 1:
        parser.error("--jobs must be positive.")
    required = ["gcc", "g++", "make", "wx-config", "pkg-config", "patch", "strip"]
    if not args.only_creator:
        required.extend(["flex", "bison", "bc", "perl"])
    for tool in required:
        if not shutil.which(tool):
            parser.error(f"Required build tool not found: {tool}")
    run(["pkg-config", "--exists", "fuse3"])
    names = ["veracrypt-create"] if args.only_creator else list(SOURCES)
    sources = {name: prepare(name, args.source_dir.resolve(), args.offline) for name in names}
    vc = sources["veracrypt-create"]
    flags = ["NOGUI=1", "WITHFUSE3=1"]
    if not shutil.which("yasm"):
        flags.append("NOASM=1")
    run(["make", "-C", vc / "src", f"-j{args.jobs}", *flags])
    if args.only_creator:
        publish_creator(vc, args.output_dir.resolve())
        return
    run(["make", "-C", sources["veracrypt"] / "src", f"-j{args.jobs}", *flags])
    lkl = sources["lkl"]
    # tools/lkl regenerates this from our patched defconfig plus host settings.
    # Never inherit filesystem selections from an earlier upstream/cache build.
    (lkl / ".config").unlink(missing_ok=True)
    run(["make", "-C", lkl / "tools/lkl", f"-j{args.jobs}", lkl / "tools/lkl/lklfuse"])
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    binaries = {
        "lurker-veracrypt": sources["veracrypt"] / "src/Main/veracrypt",
        "lurker-fs": lkl / "tools/lkl/lklfuse",
    }
    for name, binary in binaries.items():
        destination = output / name
        temporary = output / (name + ".tmp")
        shutil.copy2(binary, temporary)
        temporary.chmod(0o755)
        run(["strip", "--strip-unneeded", temporary])
        version = subprocess.check_output([temporary, "--lurker-userspace-version"], text=True).strip()
        if version != "lurker-userspace-3":
            raise RuntimeError(f"Invalid userspace helper: {binary}")
        temporary.replace(destination)
    publish_creator(vc, output)
    licenses = output / "licenses"
    licenses.mkdir(exist_ok=True)
    shutil.copy2(vc / "License.txt", licenses / "VeraCrypt.txt")
    shutil.copy2(lkl / "COPYING", licenses / "Linux.txt")
    shutil.copytree(lkl / "LICENSES", licenses / "Linux", dirs_exist_ok=True)
    (output / "source-archives.json").write_text(json.dumps(SOURCE_ARCHIVE_SHA256, indent=2) + "\n")
    (output / "sources.json").write_text(json.dumps(SOURCES, indent=2) + "\n")
    print(f"Userspace helpers built in {output}. Nothing was installed.")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
