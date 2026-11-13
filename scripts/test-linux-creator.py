#!/usr/bin/env python3
"""Check the packaged private creator's argument boundary without creating a volume."""
import argparse
import json
import os
import shlex
import shutil
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--creator", type=Path, required=True)
    parser.add_argument("--cli", type=Path, help="Also check root refuses a private backend beneath writable /tmp")
    args = parser.parse_args()
    if args.cli and os.geteuid() != 0:
        parser.error("--cli root trust check requires root in a disposable test VM")
    program = args.creator.resolve(strict=True)
    version = subprocess.run([program, "--lurker-creator-version"], stdin=subprocess.DEVNULL,
                             capture_output=True, timeout=10)
    if version.returncode or version.stdout != b"lurker-creator-1\n":
        raise SystemExit("Incompatible creator")
    failures = []
    with tempfile.TemporaryDirectory(prefix="lurker-creator-guard-", dir="/tmp") as directory:
        target = str(Path(directory) / "never-created.img")
        prefix = ["-t", "--create", "--filesystem=none"]
        cases = {
            "mount": ["-t", "--mount", target],
            "elevated-service": ["--core-service"],
            "formatter": ["-t", "--create", "--filesystem=ext4", target],
            "keyfile-option-injection": [*prefix, "-k", "--dismount", target],
            "relative-keyfile": [*prefix, "-k", "relative.key", target],
            "cipher-cascade": [*prefix, "--encryption=AES-Twofish-Serpent", target],
            "custom-hash": [*prefix, "--hash=whirlpool", target],
            "custom-pim": [*prefix, "--pim=1", target],
            "random-source": [*prefix, "--random-source=/tmp/random", target],
        }
        for name, arguments in cases.items():
            result = subprocess.run([program, *arguments], stdin=subprocess.DEVNULL,
                                    capture_output=True, timeout=10)
            if result.returncode != 1 or b"Unsupported private creator argument" not in result.stderr:
                failures.append(name)
        if list(Path(directory).iterdir()):
            failures.append("created-data-during-rejected-request")
        if args.cli:
            root = Path(directory)
            cli = root / "lurker"
            shutil.copy2(args.cli.resolve(strict=True), cli)
            backends = root / "backends"
            backends.mkdir()
            creator = backends / "lurker-veracrypt-create"
            marker = root / "untrusted-backend-executed"
            creator.write_text("#!/bin/sh\ntouch -- " + shlex.quote(str(marker)) + "\n")
            creator.chmod(0o755)
            key = root / "test.key"
            key.write_bytes(os.urandom(32))
            key.chmod(0o600)
            outcome = subprocess.run([cli, "create", "--type", "veracrypt", "--key-file", key, target, "0.15"],
                                     stdin=subprocess.DEVNULL, capture_output=True, timeout=10)
            cases["elevated-backend-trust"] = []
            if (outcome.returncode == 0 or b"untrusted path" not in outcome.stderr + outcome.stdout
                    or marker.exists() or Path(target).exists()):
                failures.append("elevated-backend-trust")
    print(json.dumps({"suite": "creator-argument-boundary", "passed": len(cases) - len(failures),
                      "total": len(cases), "failed": failures}))
    return bool(failures)


if __name__ == "__main__":
    raise SystemExit(main())
