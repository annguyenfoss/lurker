#!/usr/bin/env python3
"""Build pinned private directory backends without installing them on the host.

Build tools are prerequisites; no package-manager or privilege changes are made.
Verbose logs remain under target/folder-build. Runtime FUSE stays external.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PINS = {
    "gocryptfs": ("https://github.com/rfjakob/gocryptfs.git", "756cd453d7afe69fd2a00a7592fd2d9869a6f756", "v2.7.0"),
    "cryfs": ("https://github.com/cryfs/cryfs.git", "5d0cc36785d93e12250696a3ffc928042f2e8d61", "1.0.3"),
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run(args: list[str], cwd: Path, log, env=None) -> None:
    log.write(("$ " + " ".join(args) + "\n").encode())
    log.flush()
    subprocess.run(args, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)


def capture(args: list[str], cwd: Path) -> str:
    return subprocess.check_output(args, cwd=cwd, text=True, stderr=subprocess.STDOUT).strip()


def copy_notice(source: Path, destination: Path) -> None:
    """Module caches use readonly licenses; replace old copies atomically."""
    descriptor, name = tempfile.mkstemp(prefix=".notice-", dir=destination.parent)
    os.close(descriptor)
    temporary = Path(name)
    try:
        shutil.copyfile(source, temporary)
        temporary.chmod(0o644)
        temporary.replace(destination)
    finally:
        temporary.unlink(missing_ok=True)


def go_module_notices(src: Path, destination: Path) -> list[dict]:
    """Keep exact transitive identities and original license/notice files."""
    text = capture(["go", "list", "-m", "-json", "all"], src)
    decoder = json.JSONDecoder()
    modules = []
    while text.strip():
        module, offset = decoder.raw_decode(text.lstrip())
        text = text.lstrip()[offset:]
        modules.append(module)
        directory = module.get("Dir")
        if not directory or module.get("Main"):
            continue
        notices = destination / module["Path"] / module.get("Version", "unknown")
        for path in Path(directory).iterdir():
            if path.is_file() and path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "AUTHORS")):
                notices.mkdir(parents=True, exist_ok=True)
                copy_notice(path, notices / path.name)
    return [{key: value for key, value in module.items() if key in {"Path", "Version", "Sum", "GoModSum"}}
            for module in modules]


def go_runtime_notices(src: Path, destination: Path) -> dict[str, str]:
    goroot = Path(capture(["go", "env", "GOROOT"], src))
    # Binary distro packages may relocate the top-level Go license.
    candidates = [goroot, goroot.parent.parent / "share/licenses/go",
                  Path("/usr/share/doc/golang-go")]
    if os.environ.get("LURKER_GO_LICENSE_DIR"):
        candidates.insert(0, Path(os.environ["LURKER_GO_LICENSE_DIR"]))
    destination.mkdir(parents=True, exist_ok=True)
    for directory in candidates:
        license = next((directory / name for name in ("LICENSE", "LICENSE.txt", "copyright")
                        if (directory / name).is_file()), None)
        if license:
            copy_notice(license, destination / "LICENSE")
            for name in ("PATENTS", "NOTICE", "COPYRIGHT"):
                if (directory / name).is_file():
                    copy_notice(directory / name, destination / name)
            break
    else:
        raise RuntimeError("Go runtime license missing; set LURKER_GO_LICENSE_DIR to the toolchain's license directory")
    # The standard library carries separately licensed vendored code. Retaining
    # all source-tree notices is conservative and avoids dropping a notice when
    # a Go update changes its internal dependency closure.
    for path in (goroot / "src").rglob("*"):
        if path.is_file() and path.name.upper().startswith(("LICENSE", "PATENTS", "NOTICE", "COPYING")):
            target = destination / path.relative_to(goroot)
            target.parent.mkdir(parents=True, exist_ok=True)
            copy_notice(path, target)
    return {str(path.relative_to(destination)): sha256(path)
            for path in sorted(destination.rglob("*")) if path.is_file()}


def source(name: str, cache: Path, log) -> Path:
    url, revision, version = PINS[name]
    path = cache / f"{name}-{revision}"
    if not path.exists():
        path.mkdir()
        run(["git", "init", "-q"], path, log)
        run(["git", "remote", "add", "origin", url], path, log)
        run(["git", "fetch", "--depth=1", "origin", revision], path, log)
        run(["git", "checkout", "--detach", "FETCH_HEAD"], path, log)
        run(["git", "submodule", "update", "--init", "--recursive"], path, log)
    # CryFS embeds a tag-derived version and refuses malformed untagged builds.
    run(["git", "fetch", "--depth=1", "origin", "tag", version], path, log)
    if capture(["git", "rev-parse", f"{version}^{{commit}}"], path) != revision:
        raise RuntimeError(f"{name} release tag does not match its pinned revision")
    if capture(["git", "rev-parse", "HEAD"], path) != revision:
        raise RuntimeError(f"Unexpected {name} source revision: {path}")
    if capture(["git", "status", "--porcelain", "--untracked-files=no"], path):
        raise RuntimeError(f"Modified {name} source cache: {path}")
    status = capture(["git", "submodule", "status", "--recursive"], path)
    if any(line.startswith(("+", "-", "U")) for line in status.splitlines()):
        raise RuntimeError(f"Unverified {name} submodule state")
    return path


def build(name: str, cache: Path, output: Path, jobs: int) -> dict:
    url, revision, version = PINS[name]
    logpath = cache / f"{name}.log"
    with logpath.open("wb") as log:
        src = source(name, cache, log)
        env = os.environ.copy()
        env["CMAKE_BUILD_PARALLEL_LEVEL"] = str(jobs)
        if name == "gocryptfs":
            if not shutil.which("go"):
                raise RuntimeError("Go 1.26+ is required as a build tool")
            toolchain = capture(["go", "version"], src)
            env.update(CGO_ENABLED="0", GOMAXPROCS=str(jobs))
            # Direct build avoids upstream build.bash installing into GOPATH/bin.
            run(["go", "mod", "verify"], src, log, env)
            run(["go", "build", "-mod=readonly", "-trimpath", "-ldflags="
                 f"-X main.GitVersion={version} -X main.BuildDate=2026-11-11",
                 "-o", str(output / name), "."], src, log, env)
            target_os, target_arch = (capture(["go", "env", key], src) for key in ("GOOS", "GOARCH"))
            host_os = {"Linux": "linux", "Darwin": "darwin", "FreeBSD": "freebsd"}.get(platform.system())
            host_arch = {"x86_64": "amd64", "aarch64": "arm64", "arm64": "arm64"}.get(platform.machine())
            if (target_os, target_arch) == (host_os, host_arch):
                run([str(output / name), "-version"], src, log, env)
            else:
                # Cross-compiled pure-Go artifacts still require native tests.
                run(["go", "version", "-m", str(output / name)], src, log, env)
            license_paths = [src / "LICENSE"]
        else:
            if not shutil.which("uv"):
                raise RuntimeError("uv and native CMake/compiler/FUSE development files are build prerequisites")
            # Isolated build-only Conan; never installs a Python package globally.
            env["CONAN_HOME"] = str(cache / "conan")
            conan = ["uv", "tool", "run", "--from", "conan==2.32.0", "conan"]
            run(conan + ["profile", "detect", "--exist-ok"], src, log, env)
            lock = ROOT / "native/folder/cryfs/conan.lock"
            run(conan + ["build", ".", "-s", "build_type=Release", "--build=missing",
                         "-o", "&:build_tests=False", "-o", "&:update_checks=False",
                         "--lockfile", str(lock)], src, log, env)
            binary = src / "build/Release/src/cryfs-cli/cryfs"
            shutil.copy2(binary, output / name)
            run([str(output / name), "--version"], src, log, env)
            toolchain = capture(["cmake", "--version"], src).splitlines()[0]
            license_paths = [src / "LICENSE"]
        license_dir = output / "licenses" / name
        license_dir.mkdir(parents=True, exist_ok=True)
        for path in license_paths:
            if path.is_file():
                shutil.copy2(path, license_dir / path.name)
        # Record Go dependency pins or CryFS Conan recipe/submodule identities.
        dependency_files = [p for p in (src / "go.mod", src / "go.sum", src / "conanfile.py") if p.is_file()]
        for path in dependency_files:
            shutil.copy2(path, license_dir / path.name)
        dependencies = None
        target = {"os": platform.system(), "machine": platform.machine()}
        if name == "gocryptfs":
            dependencies = go_module_notices(src, license_dir / "modules")
            target = {key: capture(["go", "env", key], src) for key in ("GOOS", "GOARCH")}
            (license_dir / "modules.json").write_text(json.dumps(dependencies, indent=2) + "\n")
            runtime_notices = go_runtime_notices(src, license_dir / "go-runtime")
            (license_dir / "go-runtime.json").write_text(json.dumps(runtime_notices, indent=2) + "\n")
        else:
            shutil.copy2(ROOT / "native/folder/cryfs/conan.lock", license_dir / "conan.lock")
            dependencies = json.loads((license_dir / "conan.lock").read_text())
            # Conan packages preserve dependency licenses separately from their
            # compiled archives. Retain those notices for the static linkage.
            for path in (cache / "conan/p").glob("**/p/licenses"):
                dest = license_dir / "dependencies" / path.parent.parent.name
                shutil.copytree(path, dest, dirs_exist_ok=True)
        native_dependencies = ""
        if platform.system() == "Linux":
            probe = subprocess.run(["ldd", str(output / name)], capture_output=True, text=True)
            native_dependencies = probe.stdout + probe.stderr
        elif platform.system() == "Darwin":
            native_dependencies = capture(["otool", "-L", str(output / name)], src)
        return {"schema": 1, "backend": name, "source": url, "revision": revision,
                "version": version, "build_platform": platform.platform(), "target": target,
                "toolchain": toolchain, "sha256": sha256(output / name),
                "dependencies": {p.name: sha256(p) for p in dependency_files},
                "dependency_identities": dependencies,
                "native_dependencies": native_dependencies, "log": str(logpath)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("backends", nargs="+", choices=sorted(PINS))
    parser.add_argument("--cache", type=Path, default=ROOT / "target/folder-build")
    parser.add_argument("--output", type=Path, default=ROOT / "target/release/backends")
    parser.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 1))
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    cache, output = args.cache.resolve(), args.output.resolve()
    cache.mkdir(parents=True, exist_ok=True)
    output.mkdir(parents=True, exist_ok=True)
    for name in args.backends:
        try:
            manifest = build(name, cache, output, args.jobs)
            (output / f"{name}.json").write_text(json.dumps(manifest, indent=2) + "\n")
            print(json.dumps({"backend": name, "status": "built", "artifact": str(output / name), "sha256": manifest["sha256"]}))
        except (OSError, subprocess.CalledProcessError, RuntimeError) as error:
            print(json.dumps({"backend": name, "status": "failed", "error": str(error), "log": str(cache / f"{name}.log")}), file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
