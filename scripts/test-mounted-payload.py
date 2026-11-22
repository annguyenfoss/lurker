#!/usr/bin/env python3
"""Write and verify a common payload across the native mount and restart tests."""

import argparse
import hashlib
import json
import os
from pathlib import Path


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
    if {path.name for path in directory.iterdir()} != expected.keys():
        raise ValueError(f"Unexpected file inventory in {directory}")
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
