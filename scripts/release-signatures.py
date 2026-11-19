#!/usr/bin/env python3
"""Sign and verify the approved SHA-256 inventory of a Unix release."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile


CHECKSUMS = "SHA256SUMS"
SIGNATURE = "SHA256SUMS.sig"
ENTRY = re.compile(rb"([a-f0-9]{64})  ([A-Za-z0-9][A-Za-z0-9._+-]*)")
GPG_STATUS = b"[GNUPG:] "


def fingerprint(value):
    if not re.fullmatch(r"(?:[a-fA-F0-9]{40}|[a-fA-F0-9]{64})", value):
        raise argparse.ArgumentTypeError("use the full signing key fingerprint")
    return value.upper()


def open_regular(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    stream = os.fdopen(descriptor, "rb")
    if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
        stream.close()
        raise ValueError(f"Expected a regular file: {path}")
    return stream


def checksums(data):
    if not data.endswith(b"\n"):
        raise ValueError(f"Expected a nonempty, newline-terminated {CHECKSUMS}")
    result = {}
    for number, line in enumerate(data[:-1].split(b"\n"), 1):
        match = ENTRY.fullmatch(line)
        if match is None:
            raise ValueError(f"Invalid checksum entry on line {number}")
        digest, name = (value.decode("ascii") for value in match.groups())
        if name in result or name in (CHECKSUMS, SIGNATURE):
            raise ValueError(f"Duplicate or reserved artifact name: {name}")
        result[name] = digest
    return result


def verify_artifacts(directory, expected, selected=()):
    for name in selected:
        if name not in expected:
            raise ValueError(f"Artifact missing from the signed inventory: {name}")
    artifacts = {name: expected[name] for name in sorted(set(selected) if selected else expected)}
    for name, digest in artifacts.items():
        with open_regular(directory / name) as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != digest:
                raise ValueError(f"Artifact checksum differs: {name}")
    return artifacts


def sign(directory, data, signer):
    artifacts = verify_artifacts(directory, checksums(data))
    with tempfile.TemporaryDirectory(prefix=".release-sign-", dir=directory) as scratch:
        manifest = Path(scratch) / CHECKSUMS
        signature = Path(scratch) / SIGNATURE
        manifest.write_bytes(data)
        subprocess.run([
            "gpg", "--no-options", "--local-user", signer + "!", "--digest-algo", "SHA256",
            "--output", str(signature), "--detach-sign", "--", str(manifest),
        ], check=True)
        # Publish only a complete signature. A competing signer or an existing
        # signature owns the destination and must remain untouched.
        os.link(signature, directory / SIGNATURE)
    return artifacts


def verify(directory, data, signer, keyring, selected):
    with tempfile.TemporaryDirectory(prefix="lurker-release-verify-") as scratch:
        manifest = Path(scratch) / CHECKSUMS
        signature = Path(scratch) / SIGNATURE
        manifest.write_bytes(data)
        with open_regular(directory / SIGNATURE) as stream:
            signature.write_bytes(stream.read())
        status = subprocess.run([
            "gpgv", "--homedir", scratch, "--keyring", str(keyring.resolve(strict=True)),
            "--weak-digest", "SHA1", "--status-fd", "1", "--", str(signature), str(manifest),
        ], stdout=subprocess.PIPE, check=True).stdout

    # Check the actual signing key fingerprint, even if the keyring contains
    # multiple trusted publisher keys. VALIDSIG alone can accompany a warning.
    records = [line[len(GPG_STATUS):].split() for line in status.split(b"\n")
               if line.startswith(GPG_STATUS)]
    valid = [record[1] for record in records if record[:1] == [b"VALIDSIG"] and len(record) > 1]
    good = [record for record in records if record[:1] == [b"GOODSIG"]]
    if valid != [signer.encode("ascii")] or len(good) != 1:
        raise ValueError("Signature must match the selected signing key")
    return verify_artifacts(directory, checksums(data), selected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_subparsers(dest="operation", required=True)
    for mode in ("sign", "verify"):
        operation = modes.add_parser(mode)
        operation.add_argument("directory", type=Path, help="Directory containing SHA256SUMS")
        operation.add_argument("--signer", required=True, type=fingerprint,
                               help="Full fingerprint of the actual signing key or subkey")
        if mode == "verify":
            operation.add_argument("--keyring", type=Path, required=True,
                                   help="Independently trusted, current publisher keyring")
            operation.add_argument("--artifact", action="append", default=[],
                                   help="Verify only this listed artifact, repeat as needed")

    args = parser.parse_args()
    try:
        directory = args.directory.resolve(strict=True)
        with open_regular(directory / CHECKSUMS) as stream:
            data = stream.read()
        if args.operation == "sign":
            artifacts = sign(directory, data, args.signer)
        else:
            artifacts = verify(directory, data, args.signer, args.keyring, args.artifact)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")
    print(json.dumps({"operation": args.operation, "signer": args.signer,
                      "checksums_sha256": hashlib.sha256(data).hexdigest(), "artifacts": artifacts}))


if __name__ == "__main__":
    main()
