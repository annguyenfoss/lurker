#!/usr/bin/env python3
"""Separate FreeBSD directory-engine payload cases on disposable inner filesystems.

Each backing store is a fresh CLI GELI container, and is closed normally after
both directory-engine cases. Login-user gocryptfs needs vfs.usermount=1; the
caller restores its prior value. Run an administrator-owned copy beside the common
payload harness. Failed backing stores are retained.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import tempfile

spec = importlib.util.spec_from_file_location("payload", Path(__file__).with_name("test-filesystem-cli.py"))
payload = importlib.util.module_from_spec(spec)
spec.loader.exec_module(payload)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--binary", required=True, type=Path)
    p.add_argument("--root", required=True, type=Path)
    p.add_argument("--commit", required=True)
    p.add_argument("--login-user", default="adjutant")
    p.add_argument("--backing-backend", choices=("geli", "veracrypt"))
    p.add_argument("--filesystems", default="ufs1,ufs2,ext4,exfat,ntfs,hammer2,zfs")
    args = p.parse_args()
    statuses = []
    for fs in args.filesystems.split(','):
        backing_backend = args.backing_backend or ("veracrypt" if fs == "ntfs" else "geli")
        directory = Path(tempfile.mkdtemp(prefix=f"directory-backing-{fs}-", dir=args.root))
        directory.chmod(0o711)
        image, target = directory / "backing.img", directory / "backing"
        target.mkdir(mode=0o700)
        password = secrets.token_urlsafe(30)
        result = {"filesystem": fs, "kind": "directory-on-filesystem", "backing_backend": backing_backend, "commit": args.commit,
                  "binary_sha256": payload.file_hash(args.binary), "fixture": str(directory),
                  "status": "running", "engines": []}
        mounted = False

        def cli(action, operands):
            status = payload.terminal([str(args.binary), action, "--type", backing_backend, "--color=never", *map(str, operands)],
                                      password, 180, directory / f"backing-{action}.log")
            result["backing_" + action] = status
            if status["exit_code"] or status["timed_out"]:
                raise RuntimeError("Backing CLI " + action + " failed")

        try:
            if shutil.disk_usage(args.root).free < 1024 * 1024 * 1024:
                raise RuntimeError("Less than 1 GiB free; retain failure evidence and stop allocating backing fixtures")
            cli("create", ["--filesystem", fs, image, "0.375" if fs == "hammer2" else "0.25"])
            cli("mount", [image, target]); mounted = True
            if not os.path.ismount(target): raise RuntimeError("Backing mount missing")
            for engine in ("pefs", "gocryptfs"):
                command = ["/usr/local/bin/python3.14", str(Path(__file__).with_name("test-filesystem-cli.py")),
                           "--binary", str(args.binary), "--backend", engine, "--filesystem", fs,
                           "--root", str(target), "--commit", args.commit]
                if engine == "gocryptfs": command = ["/usr/local/bin/sudo", "-n", "-u", args.login_user, *command]
                completed = subprocess.run(command, capture_output=True, text=True, timeout=900)
                log = directory / (engine + ".log"); log.write_text(completed.stdout + completed.stderr)
                row = {"backend": engine, "exit_code": completed.returncode, "log": str(log)}
                try:
                    observed = json.loads(completed.stdout.strip().splitlines()[-1])
                    source_receipt = Path(observed["receipt"])
                    shutil.copytree(source_receipt.parent, directory / engine, symlinks=True)
                    row.update(status=observed["status"], receipt=str(directory / engine / "result.json"))
                except Exception as error:
                    row.update(status="failed", error=str(error))
                result["engines"].append(row)
                if os.path.ismount(target) and any(os.path.ismount(path) for path in target.glob('lurker-fs-cli-*/mount')):
                    raise RuntimeError("Directory engine remains mounted; retain backing store")
            cli("unmount", [target]); mounted = False
            result["status"] = "passed" if all(r['status'] == 'passed' for r in result['engines']) else "partial"
            if result["status"] == "passed":
                result["closed_container_sha256"] = payload.file_hash(image)
                image.unlink(); result["successful_container_removed"] = True
        except Exception as error:
            result.update(status="failed", error=str(error))
        finally:
            if mounted:
                try: cli("unmount", [target])
                except Exception as error: result["cleanup_error"] = str(error)
            result["mount_remaining"] = os.path.ismount(target)
            payload.atomic_json(directory / "result.json", result)
        statuses.append(result["status"])
        print(json.dumps({"filesystem": fs, "status": result['status'], "receipt": str(directory / 'result.json')}), flush=True)
    return 0 if statuses and all(status == "passed" for status in statuses) else 1


if __name__ == "__main__":
    raise SystemExit(main())
