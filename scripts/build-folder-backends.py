#!/usr/bin/env python3
"""Build pinned private directory backends without installing them on the host.

Build tools are prerequisites; no package-manager or privilege changes are made.
Verbose logs remain under target/folder-build. Runtime FUSE stays external.
"""
from __future__ import annotations

import argparse
from contextlib import ExitStack
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PINS = {
    "gocryptfs": ("https://github.com/rfjakob/gocryptfs.git", "756cd453d7afe69fd2a00a7592fd2d9869a6f756", "v2.7.0"),
    "cryfs": ("https://github.com/cryfs/cryfs.git", "5d0cc36785d93e12250696a3ffc928042f2e8d61", "1.0.3"),
}
PATCHES = {"cryfs": ["native/folder/cryfs/fsyncdir.patch"]}
PLATFORM_PATCHES = {
    "Linux": {"cryfs": ["native/folder/cryfs/linux-localstate.patch",
                        "native/folder/cryfs/linux-exclusive-publication.patch"]},
    "Darwin": {"cryfs": ["native/macos/cryfs-fuse-t.patch"]},
    "FreeBSD": {"gocryptfs": ["native/folder/gocryptfs/freebsd-futimes.patch"]},
}
DARWIN_GO_FUSE = ("https://github.com/hanwen/go-fuse.git", "2d50fff207d78c197869fa492eff3a79bc1d8408")
DARWIN_GO_FUSE_PATCH = "native/macos/gocryptfs-fuse-t.patch"
FREEBSD_LICENSE_ROOT = Path("/usr/local/share/licenses")


def selected_patches(name: str, system: str | None = None) -> list[str]:
    return PATCHES.get(name, []) + PLATFORM_PATCHES.get(system or platform.system(), {}).get(name, [])


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


def capture(args: list[str], cwd: Path, env=None) -> str:
    return subprocess.check_output(args, cwd=cwd, env=env, text=True, stderr=subprocess.STDOUT).strip()


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


def go_module_notices(src: Path, destination: Path, module_flags=()) -> list[dict]:
    """Keep exact transitive identities and original license/notice files."""
    text = capture(["go", "list", "-m", "-json", *module_flags, "all"], src)
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
            if path.is_file() and path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "AUTHORS", "PATENTS")):
                notices.mkdir(parents=True, exist_ok=True)
                copy_notice(path, notices / path.name)
    return [{key: value for key, value in module.items() if key in {"Path", "Version", "Sum", "GoModSum"}}
            for module in modules]


def go_runtime_notices(src: Path, destination: Path) -> dict[str, str]:
    goroot = Path(capture(["go", "env", "GOROOT"], src))
    # Binary distro packages may relocate the top-level Go license.
    candidates = [goroot, goroot.parent, goroot.parent.parent / "share/licenses/go",
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


def source_notices(src: Path, destination: Path) -> None:
    # Inspect tracked paths rather than build output; CryFS vendors Crypto++
    # under its own source pin, outside the Conan dependency graph.
    names = capture(["git", "ls-files", "-z"], src).split("\0")
    for name in filter(None, names):
        relative = Path(name)
        if relative.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "PATENTS")):
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            copy_notice(src / relative, target)


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
    if capture(["git", "status", "--porcelain", "--untracked-files=all"], path):
        raise RuntimeError(f"Modified {name} source cache: {path}")
    status = capture(["git", "submodule", "status", "--recursive"], path)
    if any(line.startswith(("+", "-", "U")) for line in status.splitlines()):
        raise RuntimeError(f"Unverified {name} submodule state")
    return path


def patched_source(name: str, pristine: Path, cache: Path, log, *, system: str | None = None) -> tuple[Path, dict[str, str]]:
    """Verify the entire effective diff against snapshotted reviewed patches.

    Build in a separate worktree, retaining the original pristine checkout.
    Never reset an existing cache that differs from the expected source.
    """
    paths = selected_patches(name, system)
    if not paths:
        return pristine, {}
    with tempfile.TemporaryDirectory(prefix="patch-input-", dir=cache) as temporary:
        inputs = Path(temporary)
        snapshots, identities = [], {}
        for number, relative in enumerate(paths):
            content = (ROOT / relative).read_bytes()
            identities[relative] = hashlib.sha256(content).hexdigest()
            path = inputs / f"{number}.patch"
            path.write_bytes(content)
            snapshots.append(str(path))
        key = hashlib.sha256(json.dumps(identities, sort_keys=True).encode()).hexdigest()
        worktree = cache / f"{name}-{PINS[name][1]}-patched-{key[:20]}"
        index_env = dict(os.environ, GIT_INDEX_FILE=str(inputs / "expected.index"))
        run(["git", "read-tree", "HEAD"], pristine, log, index_env)
        for snapshot in snapshots:
            run(["git", "apply", "--cached", "--whitespace=nowarn", snapshot], pristine, log, index_env)
        flags = ["--binary", "--no-ext-diff", "--no-renames"]
        expected = capture(["git", "diff", *flags, "--cached", "HEAD"], pristine, index_env)
        if not worktree.exists():
            run(["git", "worktree", "add", "--detach", str(worktree), PINS[name][1]], pristine, log)
            for snapshot in snapshots:
                run(["git", "apply", "--index", "--whitespace=nowarn", snapshot], worktree, log)
        if capture(["git", "rev-parse", "HEAD"], worktree) != PINS[name][1]:
            raise RuntimeError(f"Unexpected patched {name} source revision: {worktree}")
        actual = capture(["git", "diff", *flags, "HEAD"], worktree)
        unknown = capture(["git", "ls-files", "--others", "--exclude-standard"], worktree)
        if actual != expected or unknown:
            raise RuntimeError(f"Modified or incomplete patched {name} cache; preserve for inspection: {worktree}")
        return worktree, identities


def darwin_go_fuse(src: Path, workspace: Path, log, env) -> tuple[list[str], dict[str, str]]:
    url, revision = DARWIN_GO_FUSE
    fuse = workspace / "go-fuse"
    fuse.mkdir()
    run(["git", "init", "-q"], fuse, log)
    run(["git", "fetch", "--depth=1", url, revision], fuse, log)
    run(["git", "checkout", "--detach", "FETCH_HEAD"], fuse, log)
    if capture(["git", "rev-parse", "HEAD"], fuse) != revision:
        raise RuntimeError("Unexpected Darwin Go-FUSE source revision")
    patch = workspace / "go-fuse.patch"
    patch.write_bytes((ROOT / DARWIN_GO_FUSE_PATCH).read_bytes())
    run(["git", "apply", "--unidiff-zero", str(patch)], fuse, log)
    for suffix in ("mod", "sum"):
        shutil.copyfile(src / f"go.{suffix}", workspace / f"darwin.{suffix}")
    flags = ["-modfile=" + str(workspace / "darwin.mod")]
    run(["go", "mod", "edit", *flags, "-replace", f"github.com/hanwen/go-fuse/v2={fuse}"], src, log, env)
    return flags, {DARWIN_GO_FUSE_PATCH: sha256(patch)}


def stage_macos_cryfs(binary: Path, output: Path) -> dict:
    """Keep compiler libraries private and use the installed FUSE-T runtime."""
    from macos_backend import FOLDER_FUSE_MOUNT, fuse_runtime_info, sha256 as native_digest, stage_macho
    destination, libraries = output / "cryfs", output / "cryfs-libs"
    if destination.exists() or libraries.exists():
        raise RuntimeError("macOS CryFS output already exists; choose a fresh output directory")
    with tempfile.TemporaryDirectory(prefix=".cryfs-stage-", dir=output) as temporary:
        root = Path(temporary)
        targets, external = stage_macho(binary, root / "cryfs", root / "cryfs-libs",
                                       platform.machine(), fuse_runtime="fuse-t")
        for target in reversed(targets):
            subprocess.run(["codesign", "--force", "--sign", "-", str(target)], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        # Publish the manifest only after every library and executable is ready.
        (root / "cryfs-libs").rename(libraries)
        (root / "cryfs").rename(destination)
    return {"fuse_runtime": fuse_runtime_info("fuse-t"), "mount": FOLDER_FUSE_MOUNT,
            "external_libraries": external,
            "private_libraries": {str(path.relative_to(output)): native_digest(path)
                                  for path in sorted(libraries.iterdir()) if path.is_file()}}


def macos_private_notices(binary: Path, libraries: dict[str, str], destination: Path) -> None:
    """Ship LLVM's license with the private OpenMP runtime, fail on new closures."""
    from macos_backend import dependencies
    originals = {Path(path).name: Path(path) for path in dependencies(binary) if path.startswith("/")}
    for relative in libraries:
        name = Path(relative).name
        if name != "libomp.dylib" or name not in originals:
            raise RuntimeError("No reviewed private dylib notice recipe: " + name)
        prefix = originals[name].resolve(strict=True).parent.parent
        license = prefix / "LICENSE.TXT"
        if not license.is_file():
            raise RuntimeError("LLVM OpenMP runtime license missing: " + str(license))
        target = destination / "llvm-openmp"
        target.mkdir(parents=True, exist_ok=True)
        copy_notice(license, target / license.name)


def conan_boost_include(conan: list[str], options: list[str], src: Path,
                        cache: Path, log, env: dict) -> tuple[Path, str]:
    """Resolve the host Boost headers from the same locked graph as the build."""
    with tempfile.TemporaryDirectory(prefix=".cryfs-conan-install-", dir=cache) as directory:
        graph = Path(directory) / "graph.json"
        run(conan + ["install", ".", "--format=json", "--out-file", str(graph),
                     "--output-folder", directory] + options, src, log, env)
        try:
            nodes = json.loads(graph.read_text())["graph"]["nodes"].values()
        except (KeyError, ValueError) as error:
            raise RuntimeError("Cannot read the locked CryFS Conan package graph") from error
    candidates = [node for node in nodes if node.get("context") == "host"
                  and (node.get("ref") or "").startswith("boost/")]
    if len(candidates) != 1 or not candidates[0].get("package_folder"):
        raise RuntimeError("Cannot identify the locked CryFS host Boost package in Conan's graph")
    boost = candidates[0]
    include = Path(boost["package_folder"]).resolve() / "include"
    if not (include / "boost/version.hpp").is_file():
        raise RuntimeError("Resolved CryFS Boost package has no headers: " + str(include))
    version = boost["ref"].split("#", 1)[0].split("/", 1)[1]
    return include, f"/usr/src/boost-{version}/include"


def freebsd_runtime_packages(binary: Path, src: Path, licenses: Path) -> tuple[str, list[dict]]:
    """Retain the actual official-package closure and its notices, not Conan identities."""
    linked = capture(["ldd", str(binary)], src)
    if "not found" in linked:
        raise RuntimeError("FreeBSD CryFS has unresolved native runtime libraries")
    packages = {}
    for line in linked.splitlines():
        fields = line.split()
        if len(fields) < 3 or fields[1] != "=>" or not fields[2].startswith("/usr/local/"):
            continue
        library = fields[2]
        owner = capture(["pkg", "which", "-q", library], src)
        if owner in packages:
            packages[owner]["libraries"].append(library)
            continue
        name, version, origin = capture(["pkg", "query", "%n %v %o", owner], src).split()
        license_names = capture(["pkg", "query", "%L", owner], src).splitlines()
        notice = FREEBSD_LICENSE_ROOT / owner
        if not notice.is_dir() or not any(p.is_file() for p in notice.rglob("*")):
            raise RuntimeError("FreeBSD runtime package license missing: " + owner)
        shutil.copytree(notice, licenses / owner, dirs_exist_ok=True)
        packages[owner] = {"name": name, "version": version, "origin": origin,
                           "licenses": license_names, "libraries": [library]}
    if not packages:
        raise RuntimeError("FreeBSD CryFS runtime package closure is empty")
    return linked, [packages[key] for key in sorted(packages)]


def build_freebsd_cryfs(src: Path, jobs: int, log, env) -> tuple[Path, dict]:
    """Use FreeBSD's supported native build packages and FUSE ABI 2."""
    for tool in ("cmake", "ninja", "c++", "pkg-config", "pkg", "ldd"):
        if not shutil.which(tool):
            raise RuntimeError("Missing FreeBSD CryFS build prerequisite: " + tool)
    run(["pkg-config", "--exists", "fuse"], src, log, env)
    build = src / "build/FreeBSD"
    configure = ["-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release", "-DBUILD_TESTING=OFF",
                 "-DCRYFS_UPDATE_CHECKS=OFF", "-DDISABLE_OPENMP=ON", "-DCMAKE_SKIP_RPATH=TRUE",
                 "-DCMAKE_CXX_FLAGS=-DBOOST_STACKTRACE_GNU_SOURCE_NOT_REQUIRED"]
    run(["cmake", "-S", str(src), "-B", str(build), *configure], src, log, env)
    run(["cmake", "--build", str(build), "--parallel", str(jobs)], src, log, env)
    return build / "src/cryfs-cli/cryfs", {
        "native_build": {"provider": "FreeBSD packages", "configure": configure,
                         "compiler": capture(["c++", "--version"], src).splitlines()[0],
                         "fuse_abi": capture(["pkg-config", "--modversion", "fuse"], src),
                         "packages": capture(["pkg", "query", "%n %v %o", "cmake-core", "ninja",
                                              "boost-libs", "range-v3", "spdlog", "fusefs-libs", "curl"], src).splitlines()},
    }


def build(name: str, cache: Path, output: Path, jobs: int) -> dict:
    url, revision, version = PINS[name]
    logpath = cache / f"{name}.log"
    with logpath.open("wb") as log, ExitStack() as cleanup:
        pristine = source(name, cache, log)
        build_system = platform.system()
        source_system = build_system
        if name == "gocryptfs":
            if not shutil.which("go"):
                raise RuntimeError("Go 1.26+ is required as a build tool")
            target_os, target_arch = capture(["go", "env", "GOOS", "GOARCH"], pristine).splitlines()
            source_system = {"linux": "Linux", "darwin": "Darwin", "freebsd": "FreeBSD"}.get(target_os, target_os)
        src, patches = patched_source(name, pristine, cache, log, system=source_system)
        env = os.environ.copy()
        native_manifest = {}
        env["CMAKE_BUILD_PARALLEL_LEVEL"] = str(jobs)
        if name == "gocryptfs":
            toolchain = capture(["go", "version"], src)
            env.update(CGO_ENABLED="0", GOMAXPROCS=str(jobs))
            module_flags, tags = [], []
            ldflags = f"-X main.GitVersion={version} -X main.BuildDate=2026-11-11"
            if source_system == "Darwin":
                from macos_backend import FOLDER_FUSE_MOUNT
                workspace = Path(cleanup.enter_context(tempfile.TemporaryDirectory(prefix="darwin-go-fuse-", dir=cache)))
                module_flags, patches = darwin_go_fuse(src, workspace, log, env)
                tags = ["-tags", "lurkerfuset"]
                ldflags += " -X github.com/hanwen/go-fuse/v2/fuse.fuseTTransport=fskit"
                native_manifest = {"mount": FOLDER_FUSE_MOUNT, "go_fuse_revision": DARWIN_GO_FUSE[1],
                                   "external_libraries": []}
            # Direct build avoids upstream build.bash installing into GOPATH/bin.
            run(["go", "mod", "verify", *module_flags], src, log, env)
            run(["go", "build", "-p", str(jobs), "-mod=readonly", *module_flags, *tags, "-trimpath", "-ldflags=" + ldflags,
                 "-o", str(output / name), "."], src, log, env)
            host_arch = {"x86_64": "amd64", "amd64": "amd64", "aarch64": "arm64", "arm64": "arm64"}.get(platform.machine())
            if (source_system, target_arch) == (build_system, host_arch):
                run([str(output / name), "-version"], src, log, env)
            else:
                # Cross-compiled pure-Go artifacts still require native tests.
                run(["go", "version", "-m", str(output / name)], src, log, env)
            license_paths = [src / "LICENSE"]
        elif build_system == "FreeBSD":
            binary, native_manifest = build_freebsd_cryfs(src, jobs, log, env)
            shutil.copy2(binary, output / name)
            run([str(output / name), "--version"], src, log, env)
            toolchain = capture(["cmake", "--version"], src).splitlines()[0]
            license_paths = [src / "LICENSE.txt"]
        else:
            if not shutil.which("uv"):
                raise RuntimeError("uv and native CMake/compiler/FUSE development files are build prerequisites")
            # Isolated build-only Conan; never installs a Python package globally.
            env["CONAN_HOME"] = str(cache / "conan")
            conan = ["uv", "tool", "run", "--from", "conan==2.32.0", "conan"]
            run(conan + ["profile", "detect", "--exist-ok"], src, log, env)
            lock = ROOT / "native/folder/cryfs/conan.lock"
            build_options = ["-s", "build_type=Release", "--build=missing",
                             "-c", f"tools.build:jobs={jobs}",
                             "-o", "&:build_tests=False", "-o", "&:update_checks=False",
                             "--lockfile", str(lock)]
            if build_system == "Linux":
                # Conan's build-tree paths must never become runtime search
                # paths in a distributed executable (including empty entries).
                build_options += ["-c", 'tools.cmake.cmaketoolchain:extra_variables={"CMAKE_SKIP_RPATH":True}']
                source_prefix = f"/usr/src/cryfs-{version}"
                epoch = capture(["git", "show", "-s", "--format=%ct", revision], src)
                env["SOURCE_DATE_EPOCH"] = epoch
                boost_include, boost_prefix = conan_boost_include(conan, build_options, src, cache, log, env)
                # Conan joins these flags into a CMake compiler command string.
                maps = [shlex.quote(f"-ffile-prefix-map={path}={prefix}")
                        for path, prefix in [(src, source_prefix), (boost_include, boost_prefix)]]
                build_options += ["-c", "tools.build:cxxflags=" + json.dumps(maps)]
                native_manifest["reproducibility"] = {
                    "source_prefix": source_prefix,
                    "boost_include_prefix": boost_prefix,
                    "source_date_epoch": int(epoch),
                }
            run(conan + ["build", "."] + build_options, src, log, env)
            binary = src / "build/Release/src/cryfs-cli/cryfs"
            if platform.system() == "Darwin":
                native_manifest = stage_macos_cryfs(binary, output)
            else:
                shutil.copy2(binary, output / name)
            run([str(output / name), "--version"], src, log, env)
            toolchain = capture(["cmake", "--version"], src).splitlines()[0]
            license_paths = [src / "LICENSE.txt"]
        license_dir = output / "licenses" / name
        license_dir.mkdir(parents=True, exist_ok=True)
        if not all(path.is_file() for path in license_paths):
            raise RuntimeError(f"Required {name} upstream license missing")
        for path in license_paths:
            copy_notice(path, license_dir / path.name)
        source_notices(src, license_dir / "source")
        if name == "cryfs" and platform.system() == "Darwin":
            macos_private_notices(binary, native_manifest["private_libraries"], license_dir / "native")
        for relative, expected in patches.items():
            path = ROOT / relative
            if sha256(path) != expected:
                raise RuntimeError("Reviewed patch changed during the build: " + relative)
            destination = license_dir / "patches" / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            copy_notice(path, destination)
        # Record Go dependency pins or CryFS Conan recipe/submodule identities.
        dependency_files = [p for p in (src / "go.mod", src / "go.sum", src / "conanfile.py") if p.is_file()]
        for path in dependency_files:
            shutil.copy2(path, license_dir / path.name)
        dependencies = None
        native_dependencies = ""
        target = {"os": platform.system(), "machine": platform.machine()}
        if name == "gocryptfs":
            dependencies = go_module_notices(src, license_dir / "modules", module_flags)
            target = {"GOOS": target_os, "GOARCH": target_arch}
            (license_dir / "modules.json").write_text(json.dumps(dependencies, indent=2) + "\n")
            runtime_notices = go_runtime_notices(src, license_dir / "go-runtime")
            (license_dir / "go-runtime.json").write_text(json.dumps(runtime_notices, indent=2) + "\n")
        elif build_system == "FreeBSD":
            native_dependencies, dependencies = freebsd_runtime_packages(output / name, src, license_dir / "native")
        else:
            shutil.copy2(ROOT / "native/folder/cryfs/conan.lock", license_dir / "conan.lock")
            dependencies = json.loads((license_dir / "conan.lock").read_text())
            # Conan packages preserve dependency licenses separately from their
            # compiled archives. Retain those notices for the static linkage.
            for path in (cache / "conan/p").glob("**/p/licenses"):
                dest = license_dir / "dependencies" / path.parent.parent.name
                shutil.copytree(path, dest, dirs_exist_ok=True)
        if platform.system() == "Linux":
            probe = subprocess.run(["ldd", str(output / name)], capture_output=True, text=True)
            native_dependencies = probe.stdout + probe.stderr
            if name == "cryfs":
                dynamic = capture(["readelf", "-d", str(output / name)], src)
                if "(RPATH)" in dynamic or "(RUNPATH)" in dynamic:
                    raise RuntimeError("CryFS still embeds runtime search paths; distribution is blocked")
        elif platform.system() == "Darwin":
            native_dependencies = capture(["otool", "-L", str(output / name)], src)
        return {"schema": 1, "backend": name, "source": url, "revision": revision,
                "version": version, "patches": patches, "build_platform": platform.platform(), "target": target,
                "toolchain": toolchain, "sha256": sha256(output / name),
                "dependencies": {p.name: sha256(p) for p in dependency_files},
                "dependency_identities": dependencies,
                "license_files": {str(path.relative_to(license_dir)): sha256(path)
                                  for path in sorted(license_dir.rglob("*")) if path.is_file()},
                "native_dependencies": native_dependencies, "log": str(logpath), **native_manifest}


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
