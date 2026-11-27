#!/usr/bin/env python3
"""Build Lurker's private Cryptomator module and reduced, target-native Java runtime.

JDK 25+ and Maven are build prerequisites only. Nothing is installed on the host.
Runtime artifacts must match the reviewed dependency lock before publication.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "native/folder/cryptomator"
MOUNT_PROFILES = {
    "Linux": {"provider": "org.cryptomator.frontend.fuse.mount.LinuxFuseMountProvider",
              "runtime": "libfuse3", "transport": "fuse3"},
    "Darwin": {"provider": "org.cryptomator.frontend.fuse.mount.FuseTMountProvider",
               "runtime": "fuse-t", "transport": "fskit", "minimum_os": "26.0"},
}
LINUX_PATCH_MODULES = {
    "org.cryptomator.cryptofs": ("org.cryptomator:cryptofs:2.8.0:jar:sources", "cryptofs-2.8.0.jar"),
    "org.cryptomator.frontend.fuse": ("org.cryptomator:fuse-nio-adapter:5.1.0:jar:sources", "fuse-nio-adapter-5.1.0.jar"),
}
PATCH_DATE = "2026-11-14T00:00:00Z"


def linux_patch_configuration(project: Path) -> list[dict]:
    path = project / "linux-patches/manifest.json"
    value = json.loads(path.read_text())
    if value.get("schema") != 1 or not isinstance(value.get("modules"), list) or not value["modules"]:
        raise SystemExit("Invalid Linux Cryptomator patch configuration")
    seen = set()
    outputs = set()
    for entry in value["modules"]:
        if not isinstance(entry, dict) or set(entry) != {"module", "artifact", "base_jar", "source_sha256", "patch", "jar"}:
            raise SystemExit("Invalid Linux Cryptomator patch module fields")
        module = entry.get("module")
        if not isinstance(module, str) or module in seen or module not in LINUX_PATCH_MODULES:
            raise SystemExit("Unreviewed or duplicate Linux Cryptomator patch module")
        seen.add(module)
        if (entry.get("artifact"), entry.get("base_jar")) != LINUX_PATCH_MODULES[module]:
            raise SystemExit("Linux Cryptomator patch does not match its pinned module")
        if not isinstance(entry["source_sha256"], str) or not re.fullmatch(r"[0-9a-f]{64}", entry["source_sha256"]):
            raise SystemExit("Linux Cryptomator source checksum is missing")
        for field, suffix in (("patch", ".patch"), ("jar", ".jar")):
            name = entry.get(field, "")
            if not isinstance(name, str) or Path(name).name != name or not re.fullmatch(r"[a-zA-Z0-9_.-]+", name) or not name.endswith(suffix):
                raise SystemExit("Unsafe Linux Cryptomator patch filename")
        if entry["jar"] in outputs:
            raise SystemExit("Duplicate Linux Cryptomator patch output")
        outputs.add(entry["jar"])
    if value["modules"][0]["module"] != "org.cryptomator.cryptofs":
        raise SystemExit("Compile the CryptoFS patch before the FUSE patch")
    return value["modules"]


def patch_java_sources(archive: Path, patch: Path, destination: Path) -> list[str]:
    originals = {}
    with zipfile.ZipFile(archive) as source:
        for name in source.namelist():
            path = Path(name)
            if path.is_absolute() or ".." in path.parts:
                raise SystemExit("Unsafe Cryptomator source archive path")
            if name.endswith(".java"):
                originals[name] = source.read(name).decode("utf-8").replace("\r\n", "\n")
                output = destination / name
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_text(originals[name])
    for line in patch.read_text().splitlines():
        if line.startswith(("--- ", "+++ ")):
            name = line[4:]
            if name == "/dev/null":
                continue
            path = Path(name)
            if (path.is_absolute() or ".." in path.parts or len(path.parts) < 2
                    or path.parts[0] not in {"a", "b"} or not name.endswith(".java")
                    or path.name == "module-info.java"):
                raise SystemExit("Unsafe Cryptomator source patch path")
    subprocess.run(["patch", "--batch", "--forward", "--fuzz=0", "-p1", "-i", str(patch)], cwd=destination, check=True)
    changed = sorted(str(path.relative_to(destination)) for path in destination.rglob("*.java")
                     if path.read_text() != originals.get(str(path.relative_to(destination))))
    if not changed:
        raise SystemExit("Linux Cryptomator patch changes no Java sources")
    return changed


def build_linux_patches(project: Path, modules: Path, stage: Path, java_home: Path, maven: str, env: dict) -> list[dict]:
    records = []
    patch_root = project / "target/linux-patches"
    patch_root.mkdir()
    (stage / "patches").mkdir()
    previous = []
    for entry in linux_patch_configuration(project):
        work = patch_root / entry["module"]
        work.mkdir()
        subprocess.run([
            maven, "-B", "-q", "-f", str(project / "pom.xml"),
            "org.apache.maven.plugins:maven-dependency-plugin:3.8.1:copy",
            "-Dartifact=" + entry["artifact"], "-DoutputDirectory=" + str(work),
        ], env=env, check=True)
        artifact = entry["artifact"].split(":")
        archive = work / (artifact[1] + "-" + artifact[2] + "-sources.jar")
        if digest(archive) != entry["source_sha256"]:
            raise SystemExit("Linux Cryptomator source archive checksum mismatch")
        patch = (project / "linux-patches" / entry["patch"]).resolve(strict=True)
        full = work / "patched"
        full.mkdir()
        changed = patch_java_sources(archive, patch, full)
        units = work / "units"
        for name in changed:
            output = units / name
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(full / name, output)
        classes = work / "classes"
        classes.mkdir()
        module = entry["module"]
        classpath = os.pathsep.join(str(path) for path in [*previous, *sorted(modules.glob("*.jar"))])
        subprocess.run([
            str(java_home / "bin/javac"), "--release", "25", "-proc:none",
            "--class-path", classpath,
            "-d", str(classes), *[str(units / name) for name in changed],
        ], env=env, check=True)
        output = stage / "patches" / entry["jar"]
        subprocess.run([str(java_home / "bin/jar"), "--create", "--date=" + PATCH_DATE,
                        "--file", str(output), "-C", str(classes), "."], env=env, check=True)
        previous.append(output)
        records.append({**entry, "base_sha256": digest(modules / entry["base_jar"]),
                        "patch_sha256": digest(patch), "jar_sha256": digest(output),
                        "source_files": {name: digest(units / name) for name in changed}})
    return records


def linux_patch_options(root: Path, patches: list[dict]) -> list[str]:
    options = ["--enable-native-access=org.cryptomator.cryptofs,org.cryptomator.jfuse.linux.amd64,org.cryptomator.jfuse.linux.aarch64,org.cryptomator.jfuse.mac",
               "--add-opens", "java.base/sun.nio.ch=org.cryptomator.cryptofs"]
    for entry in patches:
        options += ["--patch-module", entry["module"] + "=" + str(root / "patches" / entry["jar"])]
    if any(entry["module"] == "org.cryptomator.frontend.fuse" for entry in patches):
        options += ["--add-reads", "org.cryptomator.frontend.fuse=org.cryptomator.cryptofs",
                    "--add-exports", "org.cryptomator.cryptofs/org.cryptomator.cryptofs.fh=org.cryptomator.frontend.fuse"]
    return options


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_dependencies(modules: Path) -> None:
    expected = json.loads((SOURCE / "dependencies.lock.json").read_text())["sha256"]
    actual = {path.name: digest(path) for path in modules.glob("*.jar")}
    if actual != expected:
        changed = sorted(key for key in actual.keys() | expected.keys() if actual.get(key) != expected.get(key))
        raise SystemExit("Cryptomator dependency lock mismatch: " + ", ".join(changed))


def runtime_link_options(java_home: Path) -> list[str]:
    if (java_home / "jmods/java.base.jmod").is_file():
        return ["--module-path", str(java_home / "jmods")]
    # JEP 493 permits full JDKs without jmods. Some IDE runtimes lack both
    # jmods and this capability, so check the JDK's advertised support.
    help_text = subprocess.check_output(
        [str(java_home / "bin/jlink"), "--help"], text=True,
        env=dict(os.environ, LC_ALL="C"),
    )
    if "Linking from run-time image enabled" not in help_text:
        raise SystemExit("JDK cannot link a runtime: use a full JDK with jmods or runtime-image linking support")
    return []


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java-home", required=True, type=Path)
    parser.add_argument("--maven", default="mvn")
    parser.add_argument("--output", type=Path, default=ROOT / "target/release/backends/cryptomator")
    parser.add_argument("--test", action="store_true", help="Run CryptoFS integration tests; use only a dedicated test host")
    args = parser.parse_args()
    java_home = args.java_home.resolve(strict=True)
    if platform.system() not in {"Linux", "Darwin"}:
        raise SystemExit("Cryptomator is approved on Linux and macOS only")
    for tool in ("java", "javac", "jlink"):
        if not (java_home / "bin" / tool).is_file():
            raise SystemExit(f"JDK tool missing: {tool}")
    link_options = runtime_link_options(java_home)
    maven = shutil.which(args.maven)
    if not maven:
        raise SystemExit("Maven build prerequisite missing")
    env = dict(os.environ, JAVA_HOME=str(java_home), PATH=str(java_home / "bin") + os.pathsep + os.environ.get("PATH", ""))
    java_version = subprocess.check_output([str(java_home / "bin/java"), "-version"], stderr=subprocess.STDOUT, text=True).strip()
    # Keep Java sources and tests outside the live checkout's generated state.
    args.output = args.output.absolute()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if args.output.exists():
        raise SystemExit(f"Output already exists; choose a new build output: {args.output}")
    with tempfile.TemporaryDirectory(prefix="lurker-cryptomator-build-", dir=args.output.parent) as temporary:
        work = Path(temporary)
        project = work / "source"
        shutil.copytree(SOURCE, project, ignore=shutil.ignore_patterns("target"))
        command = [maven, "-B", "-q", "-f", str(project / "pom.xml"), "clean", "package"]
        if not args.test:
            command.insert(-2, "-DskipTests")
        subprocess.run(command, env=env, check=True)
        modules = project / "target/mods"
        verify_dependencies(modules)
        stage = work / "image"
        stage.mkdir(mode=0o755)
        shutil.copytree(modules, stage / "mods")
        shutil.copy2(project / "target/lurker-cryptomator-1.0.0.jar", stage / "mods")
        linux_patches = build_linux_patches(project, modules, stage, java_home, maven, env) if platform.system() == "Linux" else []
        subprocess.run([
            str(java_home / "bin/jlink"), *link_options,
            "--add-modules", "java.base,java.compiler,java.naming,java.xml,jdk.net,jdk.unsupported",
            "--no-header-files", "--no-man-pages", "--strip-debug", "--compress", "zip-6",
            "--output", str(stage / "runtime"),
        ], env=env, check=True)
        launcher = stage / "lurker-cryptomator"
        launcher_text = '''#!/bin/sh
set -eu
LURKER_CRYPTOMATOR_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
# Ignore inherited JVM injection variables; never accept a user classpath.
unset JAVA_TOOL_OPTIONS JDK_JAVA_OPTIONS _JAVA_OPTIONS CLASSPATH
exec "$LURKER_CRYPTOMATOR_ROOT/runtime/bin/java" \\
  --enable-native-access=org.cryptomator.jfuse.linux.amd64,org.cryptomator.jfuse.linux.aarch64,org.cryptomator.jfuse.mac \\
  -Xmx256m -Xss5m -Dfile.encoding=UTF-8 \\
  --module-path "$LURKER_CRYPTOMATOR_ROOT/mods" \\
  --add-modules ALL-MODULE-PATH \\
  -m org.lurker.cryptomator/org.lurker.cryptomator.Main "$@"
'''
        if linux_patches:
            launcher_text = launcher_text.replace("  --enable-native-access=org.cryptomator.jfuse.linux.amd64,org.cryptomator.jfuse.linux.aarch64,org.cryptomator.jfuse.mac \\\n", "")
            flags = ["  \"" + option + "\" \\\n" for option in linux_patch_options(Path("$LURKER_CRYPTOMATOR_ROOT"), linux_patches)]
            launcher_text = launcher_text.replace('  --module-path ', "".join(flags) + '  --module-path ', 1)
        launcher.write_text(launcher_text)
        launcher.chmod(0o755)
        licenses = stage / "licenses"
        licenses.mkdir()
        shutil.copy2(SOURCE / "LICENSE.txt", licenses / "LURKER-CRYPTOMATOR-AGPL-3.0.txt")
        shutil.copy2(SOURCE / "README.md", licenses / "CRYPTOMATOR-NOTICES.md")
        # Corresponding helper sources are part of the package; transitive source URLs
        # and binary digests remain in the lock and manifest for release source bundles.
        shutil.copytree(project, stage / "source", ignore=shutil.ignore_patterns("target"))
        for entry in linux_patches:
            work = project / "target/linux-patches" / entry["module"]
            shutil.copytree(work / "units", stage / "source/patched-modules" / entry["module"])
            upstream = stage / "source/upstream"
            upstream.mkdir(exist_ok=True)
            archive = entry["artifact"].split(":")
            filename = archive[1] + "-" + archive[2] + "-sources.jar"
            shutil.copy2(work / filename, upstream / filename)
        test_summary = None
        if args.test:
            reports = project / "target/surefire-reports"
            shutil.copytree(reports, stage / "build-evidence/test-reports")
            suites = [ET.parse(path).getroot() for path in reports.glob("TEST-*.xml")]
            test_summary = {name: sum(int(suite.get(name, "0")) for suite in suites)
                            for name in ("tests", "failures", "errors", "skipped")}
            if not test_summary["tests"] or any(test_summary[name] for name in ("failures", "errors", "skipped")):
                raise SystemExit("Native CryptoFS tests did not execute completely")
        manifest = {
            "schema": 1, "backend": "cryptomator", "protocol": 1,
            "os": platform.system(), "architecture": platform.machine(), "java": java_version,
            "mount": MOUNT_PROFILES[platform.system()],
            "fenced_mountpoint_protocol": 1 if platform.system() == "Darwin" else 0,
            "cryptofs": "2.8.0", "fuse_nio": "5.1.0", "helper_license": "AGPL-3.0-only",
            "sources": json.loads((SOURCE / "dependencies.lock.json").read_text()),
            "tests": test_summary,
            "files": {str(path.relative_to(stage)): digest(path) for path in sorted(stage.rglob("*")) if path.is_file()},
        }
        if linux_patches:
            runtime_env = dict(env)
            for name in ("JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS", "CLASSPATH"):
                runtime_env.pop(name, None)
            subprocess.run([
                str(stage / "runtime/bin/java"), *linux_patch_options(stage, linux_patches),
                "--module-path", str(stage / "mods"), "--add-modules", "ALL-MODULE-PATH",
                "-m", "org.cryptomator.cryptofs/org.cryptomator.cryptofs.fh.LinuxCiphertextChannel", "--verify-runtime",
            ], env=runtime_env, check=True)
            manifest["linux_patches"] = {"configuration_sha256": digest(project / "linux-patches/manifest.json"),
                                         "modules": linux_patches, "runtime_verified": True}
        (stage / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
        # Version discovery never initializes or mounts encrypted storage.
        subprocess.run([str(launcher), "--version"], env=env, check=True)
        stage.rename(args.output)
    print(json.dumps({"backend": "cryptomator", "status": "built", "artifact": str(args.output)}))


if __name__ == "__main__":
    main()
