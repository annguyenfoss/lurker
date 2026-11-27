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
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
PINS = {
    "gocryptfs": ("https://github.com/rfjakob/gocryptfs.git", "756cd453d7afe69fd2a00a7592fd2d9869a6f756", "v2.7.0"),
    "cryfs": ("https://github.com/cryfs/cryfs.git", "5d0cc36785d93e12250696a3ffc928042f2e8d61", "1.0.3"),
}
PATCHES = {"cryfs": ["native/folder/cryfs/fsyncdir.patch"]}
PLATFORM_PATCHES = {
    "Linux": {"cryfs": ["native/folder/cryfs/linux-localstate.patch",
                        "native/folder/cryfs/linux-exclusive-publication.patch"]},
    "Darwin": {"cryfs": ["native/macos/cryfs-fuse-t.patch",
                         "native/folder/cryfs/macos-exclusive-publication.patch"]},
    "FreeBSD": {"gocryptfs": ["native/folder/gocryptfs/freebsd-futimes.patch"]},
    "NetBSD": {"cryfs": ["native/folder/cryfs/netbsd-platform.patch"]},
    "OpenBSD": {"cryfs": ["native/folder/cryfs/openbsd-platform.patch"]},
}
DARWIN_GO_FUSE = ("https://github.com/hanwen/go-fuse.git", "2d50fff207d78c197869fa492eff3a79bc1d8408")
DARWIN_GO_FUSE_PATCH = "native/macos/gocryptfs-fuse-t.patch"
FREEBSD_LICENSE_ROOT = Path("/usr/local/share/licenses")
NETBSD_NOTICE_INPUTS = {
    "boost-license": ("https://raw.githubusercontent.com/NetBSD/pkgsrc/trunk/licenses/boost-license",
                      "c9bff75738922193e67fa726fa225535870d2aa1059f91452c411736284ad566"),
    "spdlog-1.17.0": ("https://raw.githubusercontent.com/gabime/spdlog/v1.17.0/LICENSE",
                      "d678c20826e1392578508e1c78649a1e3c31bda05ef4388d967dbd654461b140"),
    "fmtlib-12.2.0": ("https://raw.githubusercontent.com/fmtlib/fmt/12.2.0/LICENSE",
                      "a65fcc5a1095fc33cc37e4fff69366e49d7e92f613558a2b95ed1b70e33758b4"),
}
OPENBSD_NOTICE_INPUTS = {
    "boost-license": ("https://raw.githubusercontent.com/boostorg/boost/boost-1.91.0/LICENSE_1_0.txt",
                      "c9bff75738922193e67fa726fa225535870d2aa1059f91452c411736284ad566"),
    "spdlog-1.17.0": NETBSD_NOTICE_INPUTS["spdlog-1.17.0"],
    "fmt-11.2.0": ("https://raw.githubusercontent.com/fmtlib/fmt/11.2.0/LICENSE",
                   "07580f2a3b35709ce703d523f447b242f6dfec7582a8c0df102c7fa2849375f8"),
}



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



def netbsd_refuse_environment(build: Path, env: dict, include=Path("/usr/include")) -> tuple[dict, dict]:
    """Select the actual base API29 headers before pkgsrc SYSTEM includes."""
    headers = [include / name for name in ("fuse.h", "fuse_opt.h")]
    if any(not header.is_file() for header in headers):
        raise RuntimeError("NetBSD base ReFUSE development headers are missing")
    overlay, pkgconfig = build / "refuse-include", build / "refuse-pkgconfig"
    overlay.mkdir(parents=True, exist_ok=True)
    pkgconfig.mkdir(parents=True, exist_ok=True)
    for header in headers:
        (overlay / header.name).write_text('#include "' + str(header) + '"\n')
    contents = ("prefix=/usr\nlibdir=${prefix}/lib\nincludedir=${prefix}/include\n"
                "Name: NetBSD base ReFUSE\nDescription: Native ReFUSE high-level API29\n"
                "Version: 2.9\nLibs: -L${libdir} -lrefuse -lpuffs -lutil\n"
                "Cflags: " + shlex.quote("-Wp,-I," + str(overlay)) + "\n")
    (pkgconfig / "fuse.pc").write_text(contents)
    native_env = env.copy()
    native_env["PKG_CONFIG_PATH"] = ":".join(filter(None, [str(pkgconfig), "/usr/pkg/lib/pkgconfig", env.get("PKG_CONFIG_PATH")]))
    native_env["LDFLAGS"] = (env.get("LDFLAGS", "") + " -L/usr/pkg/lib -Wl,-rpath,/usr/pkg/lib").strip()
    return native_env, {"headers": {str(path): sha256(path) for path in headers},
                        "pkg_config_sha256": sha256(pkgconfig / "fuse.pc"),
                        "preprocessor_include": "-Wp,-I," + str(overlay)}


def netbsd_refuse_symbols(text: str) -> None:
    symbols = {line.split()[-1].split("@", 1)[0] for line in text.splitlines() if line.split()}
    if "__fuse_main" not in symbols or any("fuse_main_real" in name for name in symbols):
        raise RuntimeError("CryFS must use the NetBSD base ReFUSE API29 entry point, not pkgsrc compatibility FUSE")


def build_netbsd_cryfs(src: Path, jobs: int, log, env) -> tuple[Path, dict]:
    for tool in ("cmake", "ninja", "c++", "pkg-config", "pkg_info", "ldd", "nm"):
        if not shutil.which(tool):
            raise RuntimeError("Missing NetBSD CryFS build prerequisite: " + tool)
    build = src / "build/NetBSD"
    native_env, refuse = netbsd_refuse_environment(build, env)
    configure = ["-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release", "-DBUILD_TESTING=OFF",
                 "-DCRYFS_UPDATE_CHECKS=OFF", "-DDISABLE_OPENMP=ON", "-DCMAKE_SKIP_RPATH=TRUE",
                 "-DCMAKE_CXX_FLAGS=-DBOOST_STACKTRACE_GNU_SOURCE_NOT_REQUIRED",
                 "-DCMAKE_PREFIX_PATH=/usr/pkg", "-DCMAKE_POLICY_VERSION_MINIMUM=3.5"]
    run(["cmake", "-S", str(src), "-B", str(build), *configure], src, log, native_env)
    run(["cmake", "--build", str(build), "--parallel", str(jobs)], src, log, native_env)
    binary = build / "src/cryfs-cli/cryfs"
    symbols = capture(["nm", "-u", str(binary)], src)
    netbsd_refuse_symbols(symbols)
    return binary, {"native_build": {
        "provider": "NetBSD packages and base ReFUSE", "configure": configure,
        "compiler": capture(["c++", "--version"], src).splitlines()[0],
        "fuse_provider": "NetBSD base ReFUSE", "fuse_api": 29,
        "fuse_entrypoint": "__fuse_main", "refuse": refuse,
        "link_flags": native_env["LDFLAGS"],
    }}


def pinned_native_notice(name: str, inputs: dict, cache: Path, destination: Path) -> dict:
    url, expected = inputs[name]
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / (expected + ".txt")
    if not path.exists():
        with urllib.request.urlopen(url, timeout=60) as response:
            data = response.read(1024 * 1024 + 1)
        if len(data) > 1024 * 1024 or hashlib.sha256(data).hexdigest() != expected:
            raise RuntimeError("Native package notice checksum differs: " + name)
        path.write_bytes(data)
    if path.is_symlink() or sha256(path) != expected:
        raise RuntimeError("Native package notice cache differs: " + name)
    destination.parent.mkdir(parents=True, exist_ok=True)
    copy_notice(path, destination)
    return {"source": url, "sha256": expected, "notice": str(destination.name)}


def pinned_netbsd_notice(name: str, cache: Path, destination: Path) -> dict:
    return pinned_native_notice(name, NETBSD_NOTICE_INPUTS, cache, destination)


def build_openbsd_cryfs(src: Path, jobs: int, log, env) -> tuple[Path, dict]:
    for tool in ("cmake", "ninja", "c++", "pkg-config", "pkg_info", "ldd", "readelf"):
        if not shutil.which(tool):
            raise RuntimeError("Missing OpenBSD CryFS build prerequisite: " + tool)
    run(["pkg-config", "--exists", "fuse"], src, log, env)
    abi = capture(["pkg-config", "--modversion", "fuse"], src)
    if not abi.startswith("2."):
        raise RuntimeError("OpenBSD CryFS requires the native base FUSE2 ABI")
    build = src / "build/OpenBSD"
    configure = ["-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release", "-DBUILD_TESTING=OFF",
                 "-DCRYFS_UPDATE_CHECKS=OFF", "-DDISABLE_OPENMP=ON", "-DCMAKE_SKIP_RPATH=TRUE",
                 "-DCMAKE_CXX_FLAGS=-DBOOST_STACKTRACE_GNU_SOURCE_NOT_REQUIRED",
                 "-DCMAKE_POLICY_VERSION_MINIMUM=3.5"]
    run(["cmake", "-S", str(src), "-B", str(build), *configure], src, log, env)
    run(["cmake", "--build", str(build), "--target", "src/cryfs-cli/cryfs",
         "--parallel", str(jobs)], src, log, env)
    return build / "src/cryfs-cli/cryfs", {"native_build": {
        "provider": "OpenBSD packages and base FUSE", "configure": configure,
        "compiler": capture(["c++", "--version"], src).splitlines()[0],
        "fuse_provider": "OpenBSD base FUSE", "fuse_abi": abi, "privilege": "root",
    }, "runtime": {"privilege": "root", "source_kind": "directory", "native_unveil_protocol": 1}}


def openbsd_runtime_packages(binary: Path, src: Path, licenses: Path,
                             include=Path("/usr/include/fuse")) -> tuple[str, list[dict], dict]:
    linked = capture(["ldd", str(binary)], src)
    libraries = []
    fuse = []
    for line in linked.splitlines():
        fields = line.split()
        if len(fields) >= 7 and fields[2] == "rlib":
            path = fields[-1]
            if path.startswith("/usr/local/"):
                libraries.append(path)
            if path.startswith("/usr/lib/libfuse.so."):
                fuse.append(Path(path))
    if "not found" in linked or len(fuse) != 1 or not libraries:
        raise RuntimeError("OpenBSD CryFS requires resolved native packages and base FUSE")
    provider = fuse[0].resolve(strict=True)
    soname = re.findall(r"\(SONAME\).*?\[([^]]+)\]", capture(["readelf", "-d", str(provider)], src))
    needed = re.findall(r"\(NEEDED\).*?\[([^]]+)\]", capture(["readelf", "-d", str(binary)], src))
    if (provider.parent != Path("/usr/lib") or len(soname) != 1
            or not re.fullmatch(r"libfuse\.so\.[1-9][0-9]*\.[0-9]+", soname[0])
            or [name for name in needed if name.startswith("libfuse")] != soname
            or (Path("/usr/lib") / soname[0]).resolve(strict=True) != provider):
        raise RuntimeError("OpenBSD host FUSE SONAME/engine ABI differs")
    host_fuse = {"api": "fuse2", "library": soname[0], "path": str(Path("/usr/lib") / soname[0]),
                 "sha256": sha256(provider)}
    packages = {}
    headers = ["/usr/local/include/boost/version.hpp", "/usr/local/include/range/v3/all.hpp"]
    for path in [*libraries, *headers]:
        owner = capture(["pkg_info", "-q", "-E", path], src)
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.+-]*", owner):
            raise RuntimeError("Ambiguous OpenBSD native package owner: " + path)
        if owner not in packages:
            info = capture(["pkg_info", "-f", owner], src)
            if re.findall(r"^@name (\S+)$", info, re.M) != [owner]:
                raise RuntimeError("OpenBSD native package identity differs: " + owner)
            family, _, version = owner.rpartition("-")
            if family in {"boost", "range-v3"}:
                notice, license_name = "boost-license", "BSL-1.0"
            else:
                notice = family + "-" + re.sub(r"(?:p[0-9]+)?(?:v[0-9]+)?$", "", version)
                if notice not in OPENBSD_NOTICE_INPUTS:
                    raise RuntimeError("Unreviewed OpenBSD native package license/version: " + owner)
                license_name = "MIT"
            copied = pinned_native_notice(notice, OPENBSD_NOTICE_INPUTS, src.parent / "native-notices",
                                           licenses / owner / "LICENSE")
            origin = re.search(r"^@comment pkgpath=(\S+)", info, re.M)
            if not origin:
                raise RuntimeError("OpenBSD native package origin is missing: " + owner)
            packages[owner] = {"name": family, "version": version, "package": owner,
                               "origin": origin.group(1), "license": license_name,
                               "upstream_notice": copied, "libraries": [], "headers": []}
        packages[owner]["libraries" if path in libraries else "headers"].append(path)
    for name in ("fuse.h", "fuse_common.h", "fuse_opt.h"):
        notice, marker, _ = (include / name).read_bytes().partition(b"#ifndef")
        if not marker or b"Copyright" not in notice or b"Permission to use, copy" not in notice:
            raise RuntimeError("OpenBSD base FUSE header license notice is missing: " + name)
        destination = licenses / "OpenBSD-base-FUSE" / (name + ".notice")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(notice)
    return linked, [packages[key] for key in sorted(packages)], host_fuse


def netbsd_runtime_packages(binary: Path, src: Path, licenses: Path, include=Path("/usr/include")) -> tuple[str, list[dict]]:
    linked = capture(["ldd", str(binary)], src)
    if "not found" in linked or "/usr/lib/librefuse.so." not in linked or re.search(r"/libfuse(?:3)?\.so", linked):
        raise RuntimeError("NetBSD CryFS requires resolved base ReFUSE without pkgsrc FUSE/perfuse")
    libraries = [match.group(1) for match in re.finditer(r"=>\s+(/usr/pkg/\S+)", linked)]
    headers = ["/usr/pkg/include/boost/version.hpp", "/usr/pkg/include/range/v3/all.hpp"]
    packages = {}
    for path in [*libraries, *headers]:
        owner = capture(["pkg_info", "-Fe", path], src)
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.+-]*", owner):
            raise RuntimeError("Ambiguous NetBSD native package owner: " + path)
        if owner not in packages:
            info = capture(["pkg_info", "-B", owner], src)
            metadata = dict(line.split("=", 1) for line in info.splitlines() if "=" in line)
            if not info.startswith("Information for " + owner + ":\n"):
                raise RuntimeError("NetBSD native package identity differs: " + owner)
            family, _, version = owner.rpartition("-")
            license_name = metadata.get("LICENSE", "")
            if license_name == "boost-license" and family in {"boost-libs", "boost-headers", "range-v3"}:
                notice = "boost-license"
            else:
                notice = family + "-" + re.sub(r"nb[0-9]+$", "", version)
                if license_name != "mit" or notice not in NETBSD_NOTICE_INPUTS:
                    raise RuntimeError("Unreviewed NetBSD native package license/version: " + owner)
            copied = pinned_netbsd_notice(notice, src.parent / "native-notices", licenses / owner / "LICENSE")
            packages[owner] = {"name": family, "version": version, "package": owner,
                               "origin": metadata.get("PKGPATH"), "license": license_name,
                               "upstream_notice": copied, "libraries": [], "headers": []}
        packages[owner]["libraries" if path in libraries else "headers"].append(path)
    for name in ("fuse.h", "fuse_opt.h"):
        data = (include / name).read_bytes()
        notice, marker, _ = data.partition(b"#ifndef")
        if not marker or b"Copyright" not in notice or b"Redistribution" not in notice:
            raise RuntimeError("NetBSD base ReFUSE header license notice is missing: " + name)
        destination = licenses / "NetBSD-base-ReFUSE" / (name + ".notice")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(notice)
    if not libraries or not packages:
        raise RuntimeError("NetBSD CryFS native package closure is empty")
    return linked, [packages[key] for key in sorted(packages)]


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
        elif build_system == "NetBSD":
            binary, native_manifest = build_netbsd_cryfs(src, jobs, log, env)
            shutil.copy2(binary, output / name)
            run([str(output / name), "--version"], src, log, env)
            toolchain = capture(["cmake", "--version"], src).splitlines()[0]
            license_paths = [src / "LICENSE.txt"]
        elif build_system == "OpenBSD":
            binary, native_manifest = build_openbsd_cryfs(src, jobs, log, env)
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
        elif build_system == "NetBSD":
            native_dependencies, dependencies = netbsd_runtime_packages(output / name, src, license_dir / "native")
        elif build_system == "OpenBSD":
            native_dependencies, dependencies, native_manifest["host_fuse"] = openbsd_runtime_packages(
                output / name, src, license_dir / "native")
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
