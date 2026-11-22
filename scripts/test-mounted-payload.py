#!/usr/bin/env python3
"""Write and verify a common payload across the native mount and restart tests."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys


FILES = ("bitcoin.pdf", "ethereum.pdf", "lamport.pdf", "page.txt")
APPEND = b"\nThe second mount appended this entry before restarting.\n"
SECOND = b"This file was created and synced on the second mount.\n"


def durable_write(path, data, mode="xb"):
    with path.open(mode) as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())


def sync_directory(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def verify(directory, expected):
    names = {path.name for path in directory.iterdir()}
    extra = names - expected.keys()
    if sys.platform == "darwin":
        # exFAT and some FUSE mounts store macOS metadata alongside each file.
        for name in extra.copy():
            sidecar = directory / name
            if (name.startswith("._") and name[2:] in expected
                    and not sidecar.is_symlink() and sidecar.is_file()):
                with sidecar.open("rb") as stream:
                    if stream.read(4) == b"\x00\x05\x16\x07":
                        extra.remove(name)
    missing = expected.keys() - names
    if extra or missing:
        raise ValueError(f"Unexpected file inventory in {directory}: missing {sorted(missing)}, extra {sorted(extra)}")
    digests = {}
    for name, wanted in expected.items():
        path = directory / name
        if path.is_symlink() or path.read_bytes() != wanted:
            raise ValueError(f"Mounted payload differs for {name}")
        digests[name] = hashlib.sha256(wanted).hexdigest()
    return digests


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("write", "append", "verify"))
    parser.add_argument("mountpoint", type=Path)
    parser.add_argument("payload", type=Path, help="Directory containing the three PDFs and page.txt")
    args = parser.parse_args()

    mountpoint = args.mountpoint.resolve(strict=True)
    if mountpoint == mountpoint.parent or not os.path.ismount(mountpoint):
        parser.error(f"Expected a mounted disposable volume at {mountpoint}")
    directory = mountpoint / "Lurker test payload"
    expected = {name: (args.payload / name).read_bytes() for name in FILES}

    if args.phase == "write":
        directory.mkdir(mode=0o700)
        for name, contents in expected.items():
            durable_write(directory / name, contents)
        sync_directory(directory)
        sync_directory(mountpoint)
    else:
        if directory.is_symlink():
            parser.error(f"Expected a fixture directory at {directory}")
        if args.phase == "append":
            verify(directory, expected)
            durable_write(directory / "page.txt", APPEND, "ab")
            durable_write(directory / "second.txt", SECOND)
            sync_directory(directory)
        expected["page.txt"] += APPEND
        expected["second.txt"] = SECOND

    print(json.dumps({"phase": args.phase, "mountpoint": str(mountpoint),
                      "sha256": verify(directory, expected)}, sort_keys=True))


if __name__ == "__main__":
    main()
