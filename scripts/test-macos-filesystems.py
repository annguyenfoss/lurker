#!/usr/bin/env python3
"""Bounded macOS filesystem supplements: native NTFS RO import and directory backing stores.

Runs only disposable file images and normal closes. Uses the common CLI payload
harness; no GUI, physical device, SQLite, fault, or reboot workloads.
"""
import argparse
import errno
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import secrets
import subprocess
import sys
import tempfile
import time


def run(argv, *, input=None, timeout=120):
    result = subprocess.run(list(map(str, argv)), input=input, capture_output=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(f"{argv[0]} exited {result.returncode}: " + result.stderr.decode(errors="replace").strip())
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--phase", choices=["directory", "ntfs-ro"], required=True)
    parser.add_argument("--ntfs-fixture", type=Path, help="Independently formatted raw NTFS containing paper.pdf and page.txt")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("Run on macOS only")
    common = Path(__file__).with_name("test-filesystem-cli.py")
    spec = importlib.util.spec_from_file_location("common_filesystem_cli", common)
    harness = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(harness)
    root = args.root.resolve(strict=True)
    binary = args.binary.resolve(strict=True)
    result = {"phase": args.phase, "commit": args.commit, "binary": str(binary),
              "binary_sha256": harness.file_hash(binary), "started": time.time(), "cases": []}
    receipt = root / (args.phase + "-supplement.json")
    try:
        if args.phase == "directory":
            for filesystem, native_name in [("apfs", "APFS"), ("exfat", "exFAT")]:
                backing = Path(tempfile.mkdtemp(prefix="directory-backing-" + filesystem + "-", dir=root))
                image = backing / "backing.dmg"
                mount = backing / "mount"
                run(["/usr/bin/hdiutil", "create", "-size", "512m", "-type", "UDIF", "-fs", native_name, image])
                run(["/usr/bin/hdiutil", "attach", "-nobrowse", "-mountpoint", mount, image])
                try:
                    if not os.path.ismount(mount):
                        raise RuntimeError("Backing filesystem did not mount")
                    for backend in ["gocryptfs", "cryfs", "cryptomator"]:
                        command = [sys.executable, str(common), "--binary", str(binary), "--backend", backend,
                                   "--filesystem", filesystem, "--root", str(mount), "--commit", args.commit,
                                   "--timeout", "120"]
                        completed = subprocess.run(command, capture_output=True, text=True, timeout=1000)
                        case = {"backend": backend, "backing_filesystem": filesystem,
                                "code": completed.returncode, "stdout": completed.stdout, "stderr": completed.stderr}
                        # Store the small complete receipt outside the backing image too.
                        try:
                            nested = json.loads(completed.stdout.strip().splitlines()[-1])
                            case["payload_receipt"] = json.loads(Path(nested["receipt"]).read_text())
                            payload = case["payload_receipt"]
                            if payload.get("status") != "passed":
                                # A failed FSKit startup can retain Cryptomator's source
                                # fence even though no plaintext mount was established.
                                # Reconcile only this fixture through the normal CLI.
                                closed = subprocess.run([str(binary), "unmount", "--type", backend,
                                                         "--color=never", payload["source"]],
                                                        capture_output=True, text=True, timeout=60)
                                case["normal_failure_cleanup"] = {"code": closed.returncode,
                                                                  "stdout": closed.stdout,
                                                                  "stderr": closed.stderr}
                        except (ValueError, KeyError, OSError, IndexError):
                            pass
                        result["cases"].append(case)
                        harness.atomic_json(receipt, result)
                finally:
                    # A busy backing store is retained; never force-detach it.
                    run(["/usr/bin/hdiutil", "detach", mount])
        else:
            if not args.ntfs_fixture:
                parser.error("--ntfs-fixture is required for ntfs-ro")
            password = secrets.token_urlsafe(30)
            image = root / "ntfs-ro-encrypted.dmg"
            mount = root / "ntfs-ro-mount"
            # Read the independently populated fixture via the native RO path.
            attached = plistlib.loads(run(["/usr/bin/hdiutil", "attach", "-readonly", "-nobrowse", "-plist",
                                           "-imagekey", "diskimage-class=CRawDiskImage", "-mountpoint", mount,
                                           args.ntfs_fixture]))
            device = attached["system-entities"][0]["dev-entry"]
            try:
                if not os.path.ismount(mount):
                    raise RuntimeError("Native NTFS fixture did not mount read-only")
                expected = {name: hashlib.sha256((mount / name).read_bytes()).hexdigest()
                            for name in ["paper.pdf", "page.txt"]}
                run(["/usr/bin/hdiutil", "create", "-srcdevice", device, "-format", "UDRW",
                     "-encryption", "AES-256", "-stdinpass", image], input=password.encode() + b"\0")
            finally:
                run(["/usr/bin/hdiutil", "detach", device])
            for attempt in range(2):
                log = root / f"ntfs-ro-{attempt}-mount.log"
                status = harness.terminal([str(binary), "mount", "--type", "apple", "--color=never",
                                           str(image), str(mount)], password, 60, log)
                case = {"attempt": attempt, "mount": status, "mount_log": str(log)}
                result["cases"].append(case)
                harness.atomic_json(receipt, result)
                if status["exit_code"] or status["timed_out"] or not os.path.ismount(mount):
                    raise RuntimeError("CLI NTFS RO import failed")
                try:
                    observed = {name: hashlib.sha256((mount / name).read_bytes()).hexdigest() for name in expected}
                    if observed != expected:
                        raise RuntimeError("CLI NTFS import payload digest mismatch")
                    case["sha256"] = observed
                    try:
                        (mount / "must-remain-readonly.txt").write_text("read-only probe\n")
                    except OSError as error:
                        case["write_refused_errno"] = error.errno
                        if error.errno != errno.EROFS:
                            raise RuntimeError("Write failed for a reason other than read-only media") from error
                    else:
                        raise RuntimeError("CLI readonly NTFS unexpectedly accepted a write")
                finally:
                    status = harness.terminal([str(binary), "unmount", "--type", "apple", "--color=never", str(mount)],
                                               None, 60, root / f"ntfs-ro-{attempt}-close.log")
                    case["close"] = status
                    if status["exit_code"] or status["timed_out"] or os.path.ismount(mount):
                        raise RuntimeError("CLI NTFS RO close failed")
            result["fixture_sha256"] = harness.file_hash(args.ntfs_fixture)
        result["status"] = "passed" if all(case.get("code", 0) == 0 for case in result["cases"]) else "failed"
    except Exception as error:
        result["status"] = "failed"
        result["error"] = str(error)
    result["finished"] = time.time()
    harness.atomic_json(receipt, result)
    print(json.dumps({"status": result["status"], "receipt": str(receipt), "cases": len(result["cases"])}))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
