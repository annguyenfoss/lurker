#!/usr/bin/env python3
"""Exercise an extracted Linux package on a disposable VM with concise results.

Run --route kernel as root and --route user as the login user. Every conformance
suite owns a newly allocated /tmp fixture. Production workstation use is forbidden.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, value):
    temporary = path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")
    temporary.chmod(0o600)
    temporary.replace(path)


def verified_package(prefix, commit):
    manifest = json.loads((prefix / "share/lurker/manifest.json").read_text())
    if manifest["source_commit"] != commit or manifest["os"] != "Linux":
        raise ValueError("Package source/OS differs from requested qualification")
    for name, expected in manifest["files"].items():
        relative = Path(name)
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError("Unsafe package inventory path")
        path = prefix / relative
        if "symlink" in expected:
            if not path.is_symlink() or os.readlink(path) != expected["symlink"]:
                raise ValueError("Package symlink differs: " + name)
        elif path.is_symlink() or digest(path) != expected["sha256"]:
            raise ValueError("Package file differs: " + name)
    return manifest


def reclaim_passed_image(result, job):
    """Reclaim large closed images while retaining all JSON and command logs."""
    root = Path(result["root"]).resolve(strict=True)
    if root.parent != job or not root.name.startswith("lurker-conformance-"):
        raise ValueError("Unexpected conformance fixture; refusing cleanup")
    if result["status"] != "passed" or result["mount_present"]:
        return []
    # Directory suites deliberately create a symlink alias. They have no large
    # container image to reclaim; retain their ciphertext and alias as evidence.
    if result.get("backend") in {"gocryptfs", "cryfs", "cryptomator"}:
        return []
    mounts = Path("/proc/self/mountinfo").read_text()
    if str(root) + "/" in mounts:
        raise ValueError("Passed fixture still owns a native mount; retained")
    for backing in Path("/sys/class/block").glob("loop*/loop/backing_file"):
        if backing.read_text().strip().startswith(str(root) + "/"):
            raise ValueError("Passed fixture still owns a loop device; retained")
    files = []
    for name in ("vault 密碼 with spaces.img", "source alias"):
        path = root / name
        if path.is_symlink():
            raise ValueError("Unexpected symlink in owned fixture; retained")
        if path.is_file():
            files.append({"path": str(path), "bytes": path.stat().st_size, "sha256": digest(path)})
    for item in files:
        Path(item["path"]).unlink()
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--suite", type=Path, default=Path(__file__).with_name("test-backends.py"))
    parser.add_argument("--route", choices=["kernel", "user"], required=True)
    parser.add_argument("--result", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=240)
    parser.add_argument("--size-gb", default="0.125", help="Bound each disposable container size")
    parser.add_argument("--all-ciphers", action="store_true", help="Run all AES/Serpent/Twofish variants in addition to directory backends")
    parser.add_argument("--allow-external-tools", action="store_true", help="Record prepared-host results without claiming absent external runtimes")
    args = parser.parse_args()
    if (args.route == "kernel") != (os.geteuid() == 0):
        parser.error("Kernel route needs root; user route must use the login account")
    prefix = args.prefix.resolve(strict=True)
    manifest = verified_package(prefix, args.commit)
    external = {name: shutil.which(name) for name in ["veracrypt", "java", "go"]}
    if any(external.values()) and not args.allow_external_tools:
        parser.error("External VeraCrypt/Java/Go present; use a clean runtime VM or explicitly label a prepared-host run")
    ciphers = ["aes", "serpent", "twofish"] if args.all_ciphers else ["aes"]
    cases = [(backend, cipher, False) for backend in ["luks", "luks1", "veracrypt"] for cipher in ciphers] if args.route == "kernel" else [
        *([("veracrypt", cipher, True) for cipher in ciphers]
          if "veracrypt-userspace" in manifest["bundled_artifacts"] else []),
        ("gocryptfs", None, False), ("cryfs", None, False), ("cryptomator", None, False)]
    job = Path(tempfile.mkdtemp(prefix="lurker-package-qualification-", dir="/tmp"))
    report = {"schema": 1, "source_commit": args.commit, "route": args.route, "prefix": str(prefix),
              "status": "running", "started": time.time(), "external_applications": external,
              "package_manifest_sha256": digest(prefix / "share/lurker/manifest.json"),
              "binary_sha256": digest(prefix / "bin/lurker"), "suite_sha256": digest(args.suite),
              "fixture_size_gb": args.size_gb, "suites": [], "logs": str(job)}
    if args.route == "user" and "veracrypt-userspace" not in manifest["bundled_artifacts"]:
        report["excluded_routes"] = [{"backend": "veracrypt", "route": "userspace",
                                      "status": "not-applicable",
                                      "reason": "Package includes the native creator, not the optional userspace engine"}]
    args.result.parent.mkdir(parents=True, exist_ok=True)
    save(args.result, report)
    env = os.environ.copy()
    for key in ["LURKER_BACKEND_DIR", "LURKER_USERSPACE_DIR", "JAVA_HOME", "CLASSPATH", "LD_LIBRARY_PATH"]:
        env.pop(key, None)
    env["LC_ALL"] = "C.UTF-8"
    try:
        for backend, cipher, unprivileged in cases:
            if backend not in manifest["selected_backends"]:
                raise RuntimeError("Required selected Linux backend missing: " + backend)
            command = [sys.executable, str(args.suite.resolve(strict=True)), "--binary", str(prefix / "bin/lurker"),
                       "--backend", backend, "--commit", args.commit, "--root", str(job), "--size-gb", args.size_gb, "--timeout", str(args.timeout)]
            if backend in {"luks", "luks1", "veracrypt"}:
                command.extend(["--cipher", cipher])
            if unprivileged:
                command.append("--unprivileged")
            label = backend + ("-" + cipher if cipher else "") + ("-userspace" if unprivileged else "")
            log = job / (label + ".log")
            with log.open("wb") as stream:
                process = subprocess.run(command, env=env, stdin=subprocess.DEVNULL, stdout=stream, stderr=subprocess.STDOUT,
                                         timeout=args.timeout * 16)
            lines = log.read_text().strip().splitlines()
            try:
                output = json.loads(lines[-1])
            except (IndexError, json.JSONDecodeError) as error:
                excerpt = "\n".join(lines[-40:])[-8192:]
                raise RuntimeError(f"{label} produced no result; {log}: {excerpt}") from error
            if "result" not in output:
                raise RuntimeError(f"{label} blocked: {output.get('diagnostic', output)}; {log}")
            result = json.loads(Path(output["result"]).read_text())
            entry = {"backend": backend, "cipher": cipher, "unprivileged": unprivileged, "status": result["status"],
                     "counts": result["counts"], "result": output["result"], "log": str(log), "exit_code": process.returncode}
            report["suites"].append(entry)
            if process.returncode or result["status"] != "passed":
                raise RuntimeError(f"{label} failed; retained evidence: {output['result']}")
            entry["reclaimed_payloads"] = reclaim_passed_image(result, job)
            entry["closed_fixture_data_reclaimed"] = bool(entry["reclaimed_payloads"])
            save(args.result, report)
        report["status"] = "passed"
    except Exception as error:
        report.update(status="failed", diagnostic=str(error))
    finally:
        report["counts"] = {status: sum(item["counts"].get(status, 0) for item in report["suites"])
                            for status in ("passed", "failed", "skipped")}
        report["finished"] = time.time()
        report["duration_seconds"] = round(report["finished"] - report["started"], 3)
        save(args.result, report)
    print(json.dumps({"status": report["status"], "route": args.route, "passed_suites": sum(item["status"] == "passed" for item in report["suites"]),
                      "required_suites": len(cases), "counts": report["counts"], "result": str(args.result)}))
    return report["status"] != "passed"


if __name__ == "__main__":
    raise SystemExit(main())
