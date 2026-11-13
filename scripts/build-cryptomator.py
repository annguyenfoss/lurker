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
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "native/folder/cryptomator"


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
        subprocess.run([
            str(java_home / "bin/jlink"), *link_options,
            "--add-modules", "java.base,java.compiler,java.naming,java.xml,jdk.unsupported",
            "--no-header-files", "--no-man-pages", "--strip-debug", "--compress", "zip-6",
            "--output", str(stage / "runtime"),
        ], env=env, check=True)
        launcher = stage / "lurker-cryptomator"
        launcher.write_text('''#!/bin/sh
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
''')
        launcher.chmod(0o755)
        licenses = stage / "licenses"
        licenses.mkdir()
        shutil.copy2(SOURCE / "LICENSE.txt", licenses / "LURKER-CRYPTOMATOR-AGPL-3.0.txt")
        shutil.copy2(SOURCE / "README.md", licenses / "CRYPTOMATOR-NOTICES.md")
        # Corresponding helper sources are part of the package; transitive source URLs
        # and binary digests remain in the lock and manifest for release source bundles.
        shutil.copytree(project, stage / "source", ignore=shutil.ignore_patterns("target"))
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
            "cryptofs": "2.8.0", "fuse_nio": "5.1.0", "helper_license": "AGPL-3.0-only",
            "sources": json.loads((SOURCE / "dependencies.lock.json").read_text()),
            "tests": test_summary,
            "files": {str(path.relative_to(stage)): digest(path) for path in sorted(stage.rglob("*")) if path.is_file()},
        }
        (stage / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
        # Version discovery never initializes or mounts encrypted storage.
        subprocess.run([str(launcher), "--version"], env=env, check=True)
        stage.rename(args.output)
    print(json.dumps({"backend": "cryptomator", "status": "built", "artifact": str(args.output)}))


if __name__ == "__main__":
    main()
