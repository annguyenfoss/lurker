#!/usr/bin/env python3
"""Check the packaged private creator's argument boundary without creating a volume."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--creator", type=Path, required=True)
    args = parser.parse_args()
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
    print(json.dumps({"suite": "creator-argument-boundary", "passed": len(cases) - len(failures),
                      "total": len(cases), "failed": failures}))
    return bool(failures)


if __name__ == "__main__":
    raise SystemExit(main())
