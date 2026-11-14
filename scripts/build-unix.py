#!/usr/bin/env python3
"""Build and stage a complete native Unix package without installing anything.

Uses existing user-selected toolchains. Native storage tests are separate;
this command neither mounts storage nor installs host prerequisites.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import signal
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def stager():
    spec = importlib.util.spec_from_file_location("lurker_unix_stager", ROOT / "scripts/stage-unix.py")
    module = importlib.util.module_from_spec(spec)
    # Execute the reviewed source directly; loading a generated pyc would also
    # mutate an otherwise immutable deployed snapshot before verification.
    exec(compile((ROOT / "scripts/stage-unix.py").read_bytes(), str(ROOT / "scripts/stage-unix.py"), "exec"), module.__dict__)
    return module


def verify_immutable_source(source, metadata_path=None):
    source.verify_clean_build_source()
    if not source.manifest:
        return
    expected = set(source.manifest["files"])
    allowed_metadata = set()
    if metadata_path:
        metadata = Path(metadata_path).resolve()
        if metadata.is_relative_to(source.root):
            allowed_metadata.add(str(metadata.relative_to(source.root)))
    actual = {str(path.relative_to(source.root)) for path in source.root.rglob("*")
              if path.is_file() or path.is_symlink()}
    if actual - allowed_metadata != expected:
        raise ValueError("deployed source contains unexpected/missing inputs; use a fresh immutable snapshot with build caches outside it")


def verify_userspace_cache(directory, module):
    root = module.safe_root(directory)
    envelope = module.json_file(root / "artifact-manifest.json")
    if envelope.get("schema") != 1 or envelope.get("backend") != "veracrypt-userspace":
        raise ValueError("cached Linux native helpers require a complete userspace artifact envelope")
    module.verify_platform(envelope)
    module.verify_files(root, envelope["files"])
    actual = {str(path.relative_to(root)) for path, _ in module.entries(root)} - {"artifact-manifest.json"}
    if actual != set(envelope["files"]):
        raise ValueError("cached Linux native helpers contain unmanifested/missing files")


def supplied_artifacts(values, selected, system, module):
    result = {}
    for item in values:
        kind, separator, directory = item.partition("=")
        if not separator or not directory or kind not in module.ARTIFACTS:
            raise ValueError("--backend requires a known KIND=DIRECTORY")
        if kind in result:
            raise ValueError("duplicate backend artifact: " + kind)
        backend = module.ARTIFACTS[kind]
        if backend not in selected:
            raise ValueError("artifact supplied for an unselected backend: " + kind)
        if ((system == "Linux" and kind == "veracrypt-bsd")
                or (system != "Linux" and kind.startswith("veracrypt-") and kind != "veracrypt-bsd")):
            raise ValueError("VeraCrypt artifact does not match the native OS")
        path = Path(directory).expanduser().absolute()
        if path.is_symlink() or not path.is_dir():
            raise ValueError("backend artifact root must be a real directory")
        result[kind] = str(path)
    if "veracrypt-create" in result and "veracrypt-userspace" in result:
        raise ValueError("userspace VeraCrypt already includes its creator")
    return result


def native_key(source, system, prefixes, java_home=None):
    """Changes to native recipes/patches get fresh objects, not stale stamps."""
    digest = hashlib.sha256()
    identity = {"os": system, "release": platform.release(), "architecture": platform.machine(),
                "java_home": str(java_home or ""),
                "environment": {key: os.environ.get(key, "") for key in
                                ("CC", "CXX", "CFLAGS", "CXXFLAGS", "LDFLAGS", "JAVA_HOME", "PKG_CONFIG_PATH", "PKG_CONFIG_LIBDIR", "WX_CONFIG",
                                 "GOOS", "GOARCH", "GOAMD64", "GOARM64", "GOFLAGS", "CGO_ENABLED")}}
    for tool in ("cc", "c++", "go"):
        try:
            probe = subprocess.run([tool, "version" if tool == "go" else "--version"],
                                   capture_output=True, text=True, timeout=10)
            identity[tool] = (probe.returncode, probe.stdout, probe.stderr)
        except FileNotFoundError:
            identity[tool] = None
    if java_home:
        release = Path(java_home) / "release"
        identity["java_release"] = release.read_text() if release.is_file() else None
    digest.update(json.dumps(identity, sort_keys=True).encode())
    if source.manifest:
        names = source.manifest["files"]
    else:
        names = subprocess.check_output(["git", "-C", str(ROOT), "ls-tree", "-r", "--name-only",
                                         source.commit], text=True).splitlines()
    for name in sorted(name for name in names if any(name == prefix or name.startswith(prefix + "/") for prefix in prefixes)):
        contents = source.read(name)
        digest.update(name.encode() + b"\0" + contents)
    return digest.hexdigest()[:24]


def recipe(args, system, selected, artifacts, cache, work, module):
    """Return argv stages; never invoke a shell or execute a storage payload."""
    python = sys.executable
    scripts = ROOT / "scripts"
    source_args = ["--source-root", str(ROOT), "--source-commit", args.source_commit]
    if args.source_manifest:
        source_args += ["--source-manifest", str(args.source_manifest)]
    stages = []
    cargo_target = Path(os.environ.get("CARGO_TARGET_DIR", str(cache / "cargo"))).expanduser().absolute()
    binaries = cargo_target / "release"
    build = ["cargo", "build", "--locked", "--release", "--jobs", str(args.jobs), "--target-dir", str(cargo_target),
             "-p", "lurker-cli", "-p", "lurker-helper"]
    if not args.cli_only:
        build += ["-p", "lurker-desktop"]
    stages.append(("rust-build", build))
    binary_record = work / "rust-binaries.json"
    record = [python, str(scripts / "stage-unix.py"), "record", *source_args, "--kind", "binaries",
              "--directory", str(binaries), "--output", str(binary_record)]
    if not args.cli_only:
        record.append("--desktop")
    stages.append(("rust-provenance", record))

    def native(kind, directory, marker, command):
        artifacts[kind] = str(directory)
        if not (directory / marker).is_file():
            stages.append(("build-" + kind, command))

    if "veracrypt" in selected and not any(kind.startswith("veracrypt-") for kind in artifacts):
        directory = cache / ("veracrypt-" + args.native_keys["veracrypt"]) / "output"
        if system == "Linux":
            if (directory / "sources.json").is_file():
                # Validate the previous build's bytes before rebinding unchanged
                # native code to a newer application source commit.
                stages.insert(0, ("userspace-cache-verification", [python, str(Path(__file__).resolve()),
                                  "--verify-userspace-cache", str(directory)]))
            native("veracrypt-userspace", directory, "sources.json",
                   [python, str(scripts / "build-linux-userspace.py"), "--source-dir", str(directory.parent / "src"),
                    "--output-dir", str(directory), "--jobs", str(args.jobs)])
            # This is a file envelope for a cached native build, not a claim
            # that unchanged third-party objects were rebuilt at this commit.
            envelope = directory / "artifact-manifest.json"
            stages.append(("userspace-provenance", [python, str(scripts / "stage-unix.py"), "record", *source_args,
                           "--kind", "veracrypt-userspace", "--directory", str(directory),
                           "--output", str(work / "userspace-artifact-manifest.json")]))
            stages.append(("userspace-envelope", [python, str(Path(__file__).resolve()), "--copy-envelope",
                           str(work / "userspace-artifact-manifest.json"), str(envelope)]))
        else:
            native("veracrypt-bsd", directory, "bsd-veracrypt-source.json",
                   [python, str(scripts / "build-bsd-backends.py"), "--source-dir", str(directory.parent / "src"),
                    "--output-dir", str(directory), "--jobs", str(args.jobs)])
    folders = [name for name in ("gocryptfs", "cryfs") if name in selected and name not in artifacts]
    for name in folders:
        directory = cache / ("folders-" + args.native_keys["folders"]) / "output"
        native(name, directory, name + ".json",
               [python, str(scripts / "build-folder-backends.py"), name, "--cache", str(directory.parent / "src"),
                "--output", str(directory), "--jobs", str(args.jobs)])
    if "cryptomator" in selected and "cryptomator" not in artifacts:
        directory = cache / ("cryptomator-" + args.native_keys["cryptomator"]) / "output"
        if not (directory / "manifest.json").is_file() and not args.java_home:
            raise ValueError("--java-home (or JAVA_HOME) is required to build the private Cryptomator runtime")
        native("cryptomator", directory, "manifest.json",
               [python, str(scripts / "build-cryptomator.py"), "--java-home", str(args.java_home),
                "--maven", args.maven, "--output", str(directory)])
    stage = [python, str(scripts / "stage-unix.py"), "stage", *source_args, "--binaries", str(binaries),
             "--binary-manifest", str(binary_record), "--output", str(args.output)]
    if not args.cli_only:
        stage.append("--desktop")
    for name in sorted(selected):
        stage += ["--select", name]
    for kind, directory in sorted(artifacts.items()):
        stage += ["--backend", kind + "=" + directory]
    stages.append(("package", stage))
    return stages


def execute(stages, work, timeout, commit):
    result = {"schema": 1, "source_commit": commit, "status": "running", "stages": []}
    for name, command in stages:
        log = work / (name + ".log")
        started = time.monotonic()
        with log.open("xb") as output:
            try:
                process = subprocess.Popen(command, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
                code = process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                # The direct child is unreaped, so its process-group ID cannot
                # have been reused. Terminate only this build's descendants.
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                    time.sleep(1)
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
                code = 124
        result["stages"].append({"name": name, "exit_code": code,
                                 "seconds": round(time.monotonic() - started, 2), "log": str(log)})
        result["status"] = "failed" if code else "running"
        write_result(work, result)
        print(json.dumps(result["stages"][-1]), flush=True)
        if code:
            return result
    result["status"] = "built"
    write_result(work, result)
    return result


def prerequisites(stages, args):
    tools = {"cargo"}
    for name, _ in stages:
        if name == "build-veracrypt-userspace":
            tools.update(("gcc", "g++", "make", "wx-config", "pkg-config", "patch", "strip", "flex", "bison", "bc", "perl"))
        elif name == "build-veracrypt-bsd":
            tools.update(("gmake", "c++", "pkg-config", "patch"))
        elif name == "build-gocryptfs":
            tools.update(("git", "go"))
        elif name == "build-cryfs":
            tools.update(("git", "uv", "cmake", "pkg-config"))
        elif name == "build-cryptomator":
            tools.add(args.maven)
            for tool in ("java", "javac", "jlink"):
                path = Path(args.java_home) / "bin" / tool
                if not path.is_file() or not os.access(path, os.X_OK):
                    raise ValueError("missing JDK build prerequisite: " + str(path))
    missing = sorted(tool for tool in tools if not shutil.which(tool))
    if not any(shutil.which(tool) for tool in ("readelf", "llvm-readelf")):
        missing.append("readelf or llvm-readelf")
    if missing:
        raise ValueError("missing build prerequisites (nothing installed): " + ", ".join(missing))


def write_result(work, value):
    temporary = work / "result.json.tmp"
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(work / "result.json")


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "--verify-userspace-cache":
        try:
            verify_userspace_cache(Path(sys.argv[2]), stager())
            return 0
        except (ValueError, OSError, KeyError) as error:
            print(json.dumps({"status": "blocked", "error": str(error)}), file=sys.stderr)
            return 2
    # Private argv-only stage used to publish the validated native envelope.
    if len(sys.argv) == 4 and sys.argv[1] == "--copy-envelope":
        source, target = map(Path, sys.argv[2:])
        temporary = target.with_name(target.name + ".tmp-" + str(os.getpid()))
        with temporary.open("xb") as output:
            output.write(source.read_bytes())
        temporary.replace(target)
        return 0
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--source-manifest", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cache-root", type=Path, default=Path.home() / ".cache/lurker/build-unix")
    parser.add_argument("--backend", action="append", default=[], metavar="KIND=DIRECTORY")
    parser.add_argument("--select", action="append", default=[], metavar="BACKEND")
    parser.add_argument("--cli-only", action="store_true", help="Build a CLI-only development package instead of both products")
    parser.add_argument("--java-home", type=Path, default=os.environ.get("JAVA_HOME"))
    parser.add_argument("--maven", default="mvn")
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--stage-timeout", type=int, default=3600)
    args = parser.parse_args()
    module = stager()
    try:
        system = platform.system()
        if system not in module.SUPPORTED or os.geteuid() == 0:
            raise ValueError("Build natively on Linux or a supported BSD as the login user, without sudo")
        if args.jobs < 1 or args.stage_timeout < 1:
            raise ValueError("worker count and stage timeout must be positive")
        selected = set(args.select) or {name for name, systems in module.ALLOW.items() if system in systems}
        if any(system not in module.ALLOW.get(name, ()) for name in selected):
            raise ValueError("selected backend is outside the locked scope for this OS")
        source = module.Source(ROOT, args.source_commit, args.source_manifest)
        verify_immutable_source(source, args.source_manifest)
        args.output = args.output.expanduser().absolute()
        if args.output.exists() or args.output.is_symlink():
            raise ValueError("package output must not exist")
        artifacts = supplied_artifacts(args.backend, selected, system, module)
        args.native_keys = {
            "veracrypt": native_key(source, system, ["scripts/build-linux-userspace.py" if system == "Linux" else "scripts/build-bsd-backends.py",
                                                     "native/linux-userspace" if system == "Linux" else "native/bsd-veracrypt", "native/veracrypt"]),
            "folders": native_key(source, system, ["scripts/build-folder-backends.py", "native/folder/cryfs"]),
            "cryptomator": native_key(source, system, ["scripts/build-cryptomator.py", "native/folder/cryptomator"], args.java_home),
        }
        cache = args.cache_root.expanduser().absolute() / (system + "-" + platform.machine())
        cache.mkdir(mode=0o700, parents=True, exist_ok=True)
        info = cache.lstat()
        if cache.is_symlink() or info.st_uid != os.geteuid() or info.st_mode & 0o022:
            raise ValueError("build cache must be a user-owned directory without group/world write access")
        cache = cache.resolve()
        work = cache / ("run-" + time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()) + "-" + str(os.getpid()))
        work.mkdir(mode=0o700)
        args.output.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        lock = cache / "build.lock"
        try:
            lock.mkdir(mode=0o700)
        except FileExistsError:
            raise ValueError("another build owns the cache lock; reconcile its live process before removing " + str(lock))
        try:
            (lock / "owner.json").write_text(json.dumps({"pid": os.getpid(), "source_commit": args.source_commit,
                                                        "work": str(work)}) + "\n")
            stages = recipe(args, system, selected, artifacts, cache, work, module)
            prerequisites(stages, args)
            result = execute(stages, work, args.stage_timeout, args.source_commit)
        finally:
            (lock / "owner.json").unlink(missing_ok=True)
            lock.rmdir()
        print(json.dumps({"status": result["status"], "source_commit": args.source_commit,
                          "output": str(args.output), "result": str(work / "result.json")}))
        return 0 if result["status"] == "built" else 1
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(json.dumps({"status": "blocked", "error": str(error)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
