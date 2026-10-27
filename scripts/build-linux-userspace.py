#!/usr/bin/env python3
"""Build the file-only Linux helpers; never install, elevate, or change the host.

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
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
SOURCES = {
    "veracrypt": ("veracrypt/VeraCrypt", "8172bc55f834485f67be271e02c1167981423621"),
    "lkl": ("lkl/linux", "d0f76a77e8c2ca68bd7d96190b8da09040eab072"),
}


def run(command, **kwargs):
    print("+", " ".join(map(str, command)), flush=True)
    return subprocess.run(list(map(str, command)), check=True, **kwargs)


def prepare(name, cache, offline):
    repository, revision = SOURCES[name]
    source = cache / name
    if not source.exists():
        if offline:
            raise RuntimeError(f"Missing source {source}; run once without --offline.")
        cache.mkdir(parents=True, exist_ok=True)
        archive = cache / f"{name}-{revision}.tar.gz"
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
    stamp.write_text(digest + "\n")
    return source.resolve()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=ROOT / "target/userspace/src")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "target/release/userspace")
    parser.add_argument("--jobs", type=int, default=min(8, os.cpu_count() or 2))
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("These helpers are Linux-only.")
    if os.geteuid() == 0:
        parser.error("Build as your normal user, without sudo.")
    if args.jobs < 1:
        parser.error("--jobs must be positive.")
    for tool in ["gcc", "g++", "make", "wx-config", "pkg-config", "patch", "flex", "bison", "bc", "perl", "strip"]:
        if not shutil.which(tool):
            parser.error(f"Required build tool not found: {tool}")
    run(["pkg-config", "--exists", "fuse3"])
    sources = {name: prepare(name, args.source_dir.resolve(), args.offline) for name in SOURCES}
    vc = sources["veracrypt"]
    flags = ["NOGUI=1", "WITHFUSE3=1"]
    if not shutil.which("yasm"):
        flags.append("NOASM=1")
    run(["make", "-C", vc / "src", f"-j{args.jobs}", *flags])
    lkl = sources["lkl"]
    # tools/lkl regenerates this from our patched defconfig plus host settings.
    # Never inherit filesystem selections from an earlier upstream/cache build.
    (lkl / ".config").unlink(missing_ok=True)
    run(["make", "-C", lkl / "tools/lkl", f"-j{args.jobs}", lkl / "tools/lkl/lklfuse"])
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    binaries = {
        "lurker-veracrypt": vc / "src/Main/veracrypt",
        "lurker-fs": lkl / "tools/lkl/lklfuse",
    }
    for name, binary in binaries.items():
        destination = output / name
        temporary = output / (name + ".tmp")
        shutil.copy2(binary, temporary)
        temporary.chmod(0o755)
        run(["strip", "--strip-unneeded", temporary])
        version = subprocess.check_output([temporary, "--lurker-userspace-version"], text=True).strip()
        if version != "lurker-userspace-2":
            raise RuntimeError(f"Invalid userspace helper: {binary}")
        temporary.replace(destination)
    licenses = output / "licenses"
    licenses.mkdir(exist_ok=True)
    shutil.copy2(vc / "License.txt", licenses / "VeraCrypt.txt")
    shutil.copy2(lkl / "COPYING", licenses / "Linux.txt")
    shutil.copytree(lkl / "LICENSES", licenses / "Linux", dirs_exist_ok=True)
    (output / "sources.json").write_text(json.dumps(SOURCES, indent=2) + "\n")
    print(f"Userspace helpers built in {output}. Nothing was installed.")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
