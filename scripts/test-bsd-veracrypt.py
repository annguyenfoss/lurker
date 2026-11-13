#!/usr/bin/env python3
"""Qualify the private BSD engine on disposable files (not the Lurker CLI).

Run only on a designated BSD test guest as root. No existing image, mount or
device is selected. Failed resources stay available for targeted recovery.
"""
import argparse
import json
import os
from pathlib import Path
import platform
import re
import secrets
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--backend", type=Path, required=True)
    parser.add_argument("--root", type=Path, default=Path("/tmp"))
    parser.add_argument("--ciphers", nargs="+", choices=["AES", "Serpent", "Twofish"], default=["AES", "Serpent", "Twofish"])
    options = parser.parse_args()
    target = platform.system()
    if target not in ("FreeBSD", "OpenBSD") or os.geteuid() != 0:
        parser.error("Only root on a designated FreeBSD/OpenBSD test guest may run this suite")
    root = Path(tempfile.mkdtemp(prefix="lurker-bsd-vc-test-", dir=options.root))
    binary = options.backend.resolve(strict=True)
    results = []
    started = time.monotonic()
    def execute(name, argv, data=None, success=True):
        process = subprocess.run([str(x) for x in argv], input=data, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=180)
        (root / f"{name}.log").write_bytes(process.stdout)
        if success != (process.returncode == 0):
            raise RuntimeError(f"{name}: exit={process.returncode}, log={root / (name + '.log')}")
        return process.stdout.decode(errors="replace")
    try:
        for cipher in options.ciphers:
            name = cipher.lower()
            image = root / f"{name}.hc"
            mount = root / f"{name}-mnt"
            mount.mkdir()
            password = secrets.token_urlsafe(32).encode() + b"\n"
            common = [binary, "--text", "--non-interactive", "--stdin", "--pim=0", "--keyfiles="]
            execute(name + "-create", common + ["--create", image, "--force", "--volume-type=normal", f"--encryption={cipher}", "--hash=SHA-512", "--filesystem=none", "--size=96M", "--random-source=/dev/urandom"], password)
            def unlock(suffix, readonly=False):
                argv = common + ["--mount", image, "--protect-hidden=no", "--filesystem=none"]
                if readonly:
                    argv.append("--mount-options=ro")
                execute(name + suffix + "-unlock", argv, password)
                listing = execute(name + suffix + "-list", [binary, "--text", "--list", "--verbose", image])
                found = re.search(r"^Virtual Device: (/dev/(?:md[0-9]+|vnd[0-9]+c))$", listing, re.MULTILINE)
                if not found:
                    raise RuntimeError(f"Cannot identify owned native device; listing={root / (name + suffix + '-list.log')}")
                return found.group(1)
            device = unlock("-initial")
            if target == "FreeBSD":
                fs_device = device
                execute(name + "-newfs", ["/sbin/newfs", "-U", device])
                filesystem = "ufs"
            else:
                unit = device.removeprefix("/dev/").removesuffix("c")
                with open(f"/dev/r{unit}c", "r+b", buffering=0) as disk:
                    disk.write(bytes(1024 * 1024))
                    os.fsync(disk.fileno())
                execute(name + "-label", ["/sbin/disklabel", "-wAT-", unit], b"/ *\n")
                fs_device = f"/dev/{unit}a"
                execute(name + "-newfs", ["/sbin/newfs", "-q", "-O", "2", f"/dev/r{unit}a"])
                filesystem = "ffs"
            execute(name + "-mount", ["/sbin/mount", "-t", filesystem, fs_device, mount])
            marker = secrets.token_bytes(4096)
            with (mount / "marker").open("wb") as stream:
                stream.write(marker)
                stream.flush()
                os.fsync(stream.fileno())
            execute(name + "-umount", ["/sbin/umount", mount])
            execute(name + "-close", [binary, "--text", "--non-interactive", "--dismount", image])
            execute(name + "-wrong-password", common + ["--mount", image, "--protect-hidden=no", "--filesystem=none"], b"definitely-wrong\n", success=False)
            device = unlock("-reopen")
            fs_device = device if target == "FreeBSD" else device[:-1] + "a"
            execute(name + "-remount", ["/sbin/mount", "-t", filesystem, fs_device, mount])
            if (mount / "marker").read_bytes() != marker:
                raise RuntimeError(f"{cipher}: durable marker mismatch")
            execute(name + "-reumount", ["/sbin/umount", mount])
            execute(name + "-reclose", [binary, "--text", "--non-interactive", "--dismount", image])
            readonly = "unsupported: upstream OpenBSD attach has no read-only route"
            if target == "FreeBSD":
                device = unlock("-readonly", readonly=True)
                execute(name + "-ro-mount", ["/sbin/mount", "-t", filesystem, "-o", "ro", device, mount])
                try:
                    (mount / "must-fail").write_bytes(b"forbidden")
                except OSError:
                    readonly = "passed"
                else:
                    raise RuntimeError("Read-only filesystem accepted a write")
                execute(name + "-ro-umount", ["/sbin/umount", mount])
                execute(name + "-ro-close", [binary, "--text", "--non-interactive", "--dismount", image])
            image.unlink()
            mount.rmdir()
            results.append({"cipher": cipher, "lifecycle": "passed", "wrong_password": "passed", "fsync_reopen": "passed", "readonly": readonly})
        report = {"status": "passed", "scope": "private_engine_only", "platform": target, "backend": str(binary), "results": results, "seconds": round(time.monotonic() - started, 2), "evidence": str(root)}
    except Exception as error:
        report = {"status": "failed", "scope": "private_engine_only", "platform": target, "results": results, "error": str(error), "evidence": str(root)}
    (root / "result.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, sort_keys=True))
    raise SystemExit(0 if report["status"] == "passed" else 1)


if __name__ == "__main__":
    main()
