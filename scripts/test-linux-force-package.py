#!/usr/bin/env python3
"""Verify forced replacement through all Linux file-create routes in a test VM.

Owns fresh /tmp images only. Race/identity cases are core unit tests; these real
CLI cases qualify guarded replacement, preservation without force, and reopen.
"""
import argparse
import errno
import hashlib
import json
import os
from pathlib import Path
import pwd
import shutil
import subprocess
import sys
import tempfile
import time


def trusted(path):
    path = Path(path).resolve(strict=True)
    for entry in [path, *path.parents]:
        metadata = entry.stat()
        if metadata.st_uid != 0 or metadata.st_mode & 0o022:
            raise ValueError("Privileged tests require administrator-controlled tools: " + str(entry))
    return str(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", type=Path, required=True)
    parser.add_argument("--result", type=Path, required=True)
    args = parser.parse_args()
    if os.geteuid() != 0 or os.uname().nodename.split(".")[0] == "endeavour":
        parser.error("Run as root in the disposable Linux test VM")
    cli = trusted(args.prefix / "bin/lurker")
    runuser = trusted(shutil.which("runuser"))
    python = trusted(sys.executable)
    user = pwd.getpwnam("adjutant")
    root = Path(tempfile.mkdtemp(prefix="lurker-force-package-", dir="/tmp")); root.chmod(0o755)
    manifest = args.prefix / "share/lurker/manifest.json"
    report = {"schema": 1, "status": "running", "root": str(root), "cases": [], "started": time.time(),
              "source_commit": json.loads(manifest.read_text())["source_commit"],
              "package_manifest_sha256": hashlib.sha256(manifest.read_bytes()).hexdigest(),
              "binary_sha256": hashlib.sha256(Path(cli).read_bytes()).hexdigest()}
    env = os.environ.copy()
    for name in ["XDG_RUNTIME_DIR", "LURKER_BACKEND_DIR", "LURKER_USERSPACE_DIR"]: env.pop(name, None)
    env["LC_ALL"] = "C.UTF-8"
    def save():
        temporary = args.result.with_suffix(".json.tmp")
        temporary.write_text(json.dumps(report, indent=2) + "\n"); temporary.chmod(0o600); temporary.replace(args.result)
    def mounted(path):
        return any(line.split()[4] == str(path) for line in Path("/proc/self/mountinfo").read_text().splitlines())
    io_program = r'''
import errno,os,sys
from pathlib import Path
phase,point=sys.argv[1],Path(sys.argv[2])
if phase=='old':
 with (point/'old marker').open('wb') as stream:stream.write(b'old filesystem');stream.flush();os.fsync(stream.fileno())
elif phase=='new':
 assert not (point/'old marker').exists(), 'Force retained the old filesystem'
 with (point/'new marker').open('wb') as stream:stream.write(b'new filesystem');stream.flush();os.fsync(stream.fileno())
elif phase=='readonly':
 assert (point/'new marker').read_bytes()==b'new filesystem'
 try:(point/'forbidden').write_bytes(b'must fail')
 except OSError as error:assert error.errno==errno.EROFS, error
 else:raise AssertionError('Readonly replacement permits writes')
'''
    try:
        for backend, unprivileged in [("luks", False), ("luks1", False), ("veracrypt", False), ("veracrypt", True)]:
            entry = {"backend": backend, "unprivileged": unprivileged, "cipher": "aes", "status": "running"}
            report["cases"].append(entry); save(); started = time.monotonic()
            directory = root / (backend + ("-user" if unprivileged else "-kernel")); directory.mkdir(mode=0o700)
            key = directory / "binary key.bin"; key.write_bytes(os.urandom(128)); key.chmod(0o600)
            point = directory / "mount"; point.mkdir(mode=0o700)
            source = directory / "volume image.hc"
            if unprivileged:
                for path in [directory, key, point]: os.chown(path, user.pw_uid, user.pw_gid)
            owner = [runuser, "-u", "adjutant", "--"] if unprivileged else []
            base = owner + [cli, "--color=never"]
            count = 0
            def call(command, success=True):
                nonlocal count
                count += 1
                with (directory / f"command-{count:03}.log").open("wb") as log:
                    outcome = subprocess.run(command, env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, timeout=300)
                if (outcome.returncode == 0) != success: raise RuntimeError(f"Unexpected exit {outcome.returncode}; {directory}/command-{count:03}.log")
            def command(action, *operands):
                return base + [action, "--type", backend] + (["--unprivileged"] if unprivileged else []) + list(map(str, operands))
            def io_check(phase): call(owner + [python, "-c", io_program, phase, str(point)])
            try:
                create = command("create", "--cipher", "aes", "--key-file", key, source, "0.15")
                call(create + ["--force"])
                call(create, success=False)
                call(command("mount", "--key-file", key, source, point)); io_check("old")
                call(command("unmount", point))
                original = (source.stat().st_dev, source.stat().st_ino)
                call(create + ["--force"])
                assert (source.stat().st_dev, source.stat().st_ino) != original, "Force did not replace the original inode"
                call(command("mount", "--key-file", key, source, point)); io_check("new")
                call(command("unmount", point))
                call(command("mount", "--readonly", "--key-file", key, source, point)); io_check("readonly")
                call(command("unmount", point))
                assert not mounted(point)
                source.unlink()
                entry.update(status="passed", checks=6, closed_fixture_data_reclaimed=True)
            finally:
                if mounted(point):
                    try: call(command("unmount", point))
                    except Exception as error: entry["cleanup_error"] = str(error)
                entry["duration_seconds"] = round(time.monotonic() - started, 3); save()
        report["status"] = "passed"
    except Exception as error:
        report.update(status="failed", diagnostic=str(error)); report["cases"][-1]["status"] = "failed"
    finally:
        report["finished"] = time.time(); report["duration_seconds"] = round(report["finished"] - report["started"], 3); save()
    print(json.dumps({"status": report["status"], "passed": sum(case["status"] == "passed" for case in report["cases"]), "required": 4, "result": str(args.result)}))
    return report["status"] != "passed"


if __name__ == "__main__":
    raise SystemExit(main())
