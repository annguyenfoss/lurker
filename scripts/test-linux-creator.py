#!/usr/bin/env python3
"""Check the packaged private creator's argument boundary without creating a volume."""
import argparse
import json
import hashlib
import pwd
import os
import shlex
import shutil
from pathlib import Path
import subprocess
import tempfile


def check_companion_trust(original_cli, cases, failures):
    # /run and every executable ancestor are administrator-owned. The rejected
    # boundary is therefore the library/receipt itself, not an unsafe /tmp CLI.
    with tempfile.TemporaryDirectory(prefix="lurker-creator-library-", dir="/run") as directory:
        root = Path(directory)
        cli = root / "lurker"
        shutil.copy2(original_cli, cli); cli.chmod(0o755)
        backends = root / "backends"; backends.mkdir()
        libraries = backends / "lib"; libraries.mkdir()
        marker = root / "creator-executed"
        creator = backends / "lurker-veracrypt-create"
        creator.write_text("#!/bin/sh\ntouch -- " + shlex.quote(str(marker)) + "\nprintf 'lurker-creator-1\\n'\n")
        creator.chmod(0o755)
        name = "libfixture.so.1"
        library = libraries / name
        original = b"private-test-library-not-executable"
        digest = hashlib.sha256(original).hexdigest()
        receipt = backends / "private-libraries.json"
        manifest = {"schema": 1, "libraries": [{"name": name, "soname": name, "sha256": digest}],
                    "files": {"lib/" + name: {"sha256": digest}}}
        key = root / "test.key"; key.write_bytes(os.urandom(32)); key.chmod(0o600)
        target = root / "must-not-exist.hc"
        user = pwd.getpwnam("adjutant")
        for case in ["user-owned-library", "writable-library", "escaped-library-symlink", "changed-library-hash", "user-owned-manifest", "uninventoried-library", "dangling-library-directory"]:
            if library.is_symlink() or library.exists(): library.unlink()
            library.write_bytes(original); library.chmod(0o644)
            receipt.write_text(json.dumps(manifest)); receipt.chmod(0o644); os.chown(receipt, 0, 0)
            if case == "user-owned-library": os.chown(library, user.pw_uid, user.pw_gid)
            elif case == "writable-library": library.chmod(0o666)
            elif case == "escaped-library-symlink":
                library.unlink(); library.symlink_to(key)
            elif case == "changed-library-hash": library.write_bytes(b"changed")
            elif case == "user-owned-manifest": os.chown(receipt, user.pw_uid, user.pw_gid)
            elif case == "uninventoried-library": (libraries / "libunexpected.so").write_bytes(b"unexpected")
            elif case == "dangling-library-directory":
                shutil.rmtree(libraries); libraries.symlink_to(root / "missing-library-directory"); receipt.unlink()
            case_name = "elevated-" + case
            cases[case_name] = []
            outcome = subprocess.run([cli, "create", "--type", "veracrypt", "--key-file", key, target, "0.15"],
                                     stdin=subprocess.DEVNULL, capture_output=True, timeout=10)
            diagnostic = (b"hash mismatch" if case == "changed-library-hash" else
                          b"Uninventoried" if case == "uninventoried-library" else
                          b"bounded regular file" if case == "escaped-library-symlink" else
                          b"Cannot inspect private backend file" if case == "dangling-library-directory" else b"untrusted path")
            if outcome.returncode == 0 or diagnostic not in outcome.stdout + outcome.stderr or marker.exists() or target.exists():
                failures.append(case_name)
            marker.unlink(missing_ok=True)

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
    if args.cli:
        check_companion_trust(args.cli.resolve(strict=True), cases, failures)
    print(json.dumps({"suite": "creator-argument-boundary", "passed": len(cases) - len(failures),
                      "total": len(cases), "failed": failures}))
    return bool(failures)


if __name__ == "__main__":
    raise SystemExit(main())
