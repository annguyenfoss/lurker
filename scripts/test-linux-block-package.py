#!/usr/bin/env python3
"""All Linux block-create profiles on private /tmp images and owned loop devices.

Run only as root in a disposable VM. Never accepts an existing block device.
Every destructive CLI call re-verifies its loop backing inode and filesystem.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def trusted(path):
    path = Path(path).resolve(strict=True)
    for component in [path, *path.parents]:
        metadata = component.stat()
        if metadata.st_uid != 0 or metadata.st_mode & 0o022:
            raise ValueError("Root tests require administrator-controlled executables: " + str(component))
    return str(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", type=Path, required=True)
    parser.add_argument("--result", type=Path, required=True)
    args = parser.parse_args()
    if os.geteuid() != 0: parser.error("Run as root inside a disposable VM")
    prefix = args.prefix.resolve(strict=True)
    cli = trusted(prefix / "bin/lurker")
    losetup = trusted(shutil.which("losetup"))
    root = Path(tempfile.mkdtemp(prefix="lurker-block-package-", dir="/tmp"))
    key = root / "binary key.bin"; key.write_bytes(os.urandom(128)); key.chmod(0o600)
    env = os.environ.copy(); env["LC_ALL"] = "C.UTF-8"
    env.pop("LURKER_BACKEND_DIR", None)
    report = {"schema": 1, "suite": "linux-packaged-block-profiles", "status": "running", "root": str(root),
              "source_commit": json.loads((prefix / "share/lurker/manifest.json").read_text())["source_commit"],
              "binary_sha256": hashlib.sha256(Path(cli).read_bytes()).hexdigest(), "cases": [], "started": time.time()}
    counter = 0
    def save():
        args.result.parent.mkdir(parents=True, exist_ok=True)
        temporary = args.result.with_suffix(".json.tmp"); temporary.write_text(json.dumps(report, indent=2) + "\n")
        temporary.chmod(0o600); temporary.replace(args.result)
    def run(arguments):
        nonlocal counter
        counter += 1
        log = root / f"command-{counter:03}.log"
        with log.open("wb") as stream:
            result = subprocess.run(list(map(str, arguments)), env=env, stdin=subprocess.DEVNULL, stdout=stream, stderr=subprocess.STDOUT, timeout=300)
        if result.returncode: raise RuntimeError(f"Command exited {result.returncode}; {log}")
    def verify_loop(device, backing):
        rows = json.loads(subprocess.check_output([losetup, "--json", "--list", "--output", "NAME,BACK-INO,BACK-MAJ:MIN"], text=True))["loopdevices"]
        wanted = backing.stat(); identity = (os.major(wanted.st_dev), os.minor(wanted.st_dev), wanted.st_ino)
        matches = [row for row in rows if row["name"] == device]
        if len(matches) != 1: raise RuntimeError("Owned loop device disappeared or is ambiguous")
        observed = tuple(map(int, matches[0]["back-maj:min"].split(":"))) + (matches[0]["back-ino"],)
        if observed != identity: raise RuntimeError("Loop device was replaced; refusing destructive access")
    try:
        for backend in ["luks", "luks1", "veracrypt"]:
            for cipher in ["aes", "serpent", "twofish"]:
                entry = {"backend": backend, "cipher": cipher, "status": "running"}; report["cases"].append(entry); save()
                started = time.monotonic(); device = None
                backing = root / f"{backend}-{cipher}.img"; point = root / f"mount-{backend}-{cipher}"; point.mkdir()
                with backing.open("xb") as file: file.truncate(256 * 1024 * 1024)
                try:
                    device = subprocess.check_output([losetup, "--find", "--show", str(backing)], text=True).strip()
                    verify_loop(device, backing)
                    run([cli, "create", "--type", backend, "--cipher", cipher, "--key-file", key, "--force", device])
                    verify_loop(device, backing)
                    run([cli, "mount", "--type", backend, "--key-file", key, device, point])
                    with (point / "durable.bin").open("wb") as file:
                        file.write(b"block profile persistence\n"); file.flush(); os.fsync(file.fileno())
                    run([cli, "unmount", "--type", backend, point])
                    run([cli, "mount", "--type", backend, "--key-file", key, device, point])
                    if (point / "durable.bin").read_bytes() != b"block profile persistence\n": raise RuntimeError("Block data did not survive reopen")
                    with (point / "durable.bin").open("ab") as file:
                        file.write(b"writable after reopen\n"); file.flush(); os.fsync(file.fileno())
                    run([cli, "unmount", "--type", backend, point])
                    entry["status"] = "passed"
                except Exception as error:
                    entry.update(status="failed", diagnostic=str(error)); raise
                finally:
                    if os.path.ismount(point):
                        try: run([cli, "unmount", "--type", backend, point])
                        except Exception as error: entry["cleanup_error"] = str(error)
                    if device and not os.path.ismount(point):
                        try:
                            verify_loop(device, backing)
                            holders = Path("/sys/class/block") / Path(device).name / "holders"
                            if list(holders.iterdir()): raise RuntimeError("Loop still has a dependent mapping; retaining backing storage")
                            subprocess.run([losetup, "--detach", device], check=True)
                            if entry["status"] == "passed": backing.unlink()
                        except Exception as error: entry["cleanup_error"] = str(error)
                    entry["duration_seconds"] = round(time.monotonic() - started, 3); save()
                    if entry.get("cleanup_error"): raise RuntimeError(entry["cleanup_error"])
        report["status"] = "passed"
    except Exception as error: report.update(status="failed", diagnostic=str(error))
    finally:
        report["finished"] = time.time(); report["duration_seconds"] = round(report["finished"] - report["started"], 3); save()
    print(json.dumps({"status": report["status"], "passed": sum(item["status"] == "passed" for item in report["cases"]), "required": 9, "result": str(args.result)}))
    return report["status"] != "passed"


if __name__ == "__main__":
    raise SystemExit(main())
