#!/usr/bin/env python3
"""Add brief native feature observations to the shared Gentoo payload lifecycle.

Run on the assigned Gentoo host with the same arguments as test-filesystem-cli.py.
LURKER_FEATURE_RECEIPTS optionally names a private evidence directory; otherwise
observations stay beside the owned mountpoint. This wrapper
never substitutes an unmounted underlay, and leaves ordinary payload acceptance
and cleanup to the shared harness.
"""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def probe(argv):
    try:
        result = subprocess.run(list(map(str, argv)), capture_output=True, text=True, timeout=20)
        return {"command": list(map(str, argv)), "exit_code": result.returncode,
                "stdout": result.stdout[-4000:], "stderr": result.stderr[-2000:]}
    except (OSError, subprocess.TimeoutExpired) as error:
        return {"command": list(map(str, argv)), "error": str(error)}


def features(mountpoint):
    if not os.path.ismount(mountpoint):
        raise RuntimeError("Missing native mount; refusing feature observations")
    mount = probe(["findmnt", "--json", "--target", mountpoint,
                   "--output", "SOURCE,FSTYPE,OPTIONS,TARGET"])
    metadata = mountpoint.stat()
    row = {"mountpoint": str(mountpoint), "kernel": os.uname().release, "mount": mount,
           "ownership": {"uid": metadata.st_uid, "gid": metadata.st_gid,
                         "mode": oct(metadata.st_mode & 0o7777)},
           "journaling": {"status": "unknown"}, "cow": {"status": "unknown"},
           "acl": {"status": "unknown"}, "permissions": {"status": "unknown"}}
    info = json.loads(mount.get("stdout") or "{}").get("filesystems", [{}])[0]
    filesystem = info.get("fstype", "")
    source = info.get("source", "")
    if filesystem == "ext4":
        row["journaling"] = probe(["tune2fs", "-l", source])
        row["journaling"]["status"] = ("yes" if "has_journal" in row["journaling"].get("stdout", "") else "unknown")
    elif filesystem == "xfs":
        row["journaling"] = {"status": "yes", "observation": probe(["xfs_info", mountpoint])}
    elif filesystem in {"btrfs", "zfs", "apfs", "hammer2"}:
        row["journaling"] = {"status": "not-applicable", "reason": "Copy-on-write transaction filesystem"}
    elif filesystem in {"exfat", "vfat"}:
        row["journaling"] = {"status": "no", "reason": "Filesystem has no journal"}
    directory = Path(tempfile.mkdtemp(prefix="Lurker feature probe ", dir=mountpoint))
    try:
        original = directory / "original.txt"
        original.write_bytes(b"Original native feature sample.\n")
        descriptor = os.open(original, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        permission = probe(["chmod", "640", original])
        permission["observed_mode"] = oct(original.stat().st_mode & 0o777)
        permission["status"] = "yes" if permission["observed_mode"] == "0o640" else "no"
        row["permissions"] = permission
        acl = probe(["setfacl", "-m", "u:65534:r--", original])
        acl["readback"] = probe(["getfacl", "-ncp", original])
        acl["status"] = "yes" if acl.get("exit_code") == 0 and "user:65534:r--" in acl["readback"].get("stdout", "") else "no"
        row["acl"] = acl
        clone = directory / "clone.txt"
        cow = probe(["cp", "--reflink=always", original, clone])
        if cow.get("exit_code") == 0:
            clone.write_bytes(b"Independent clone update.\n")
            cow["source_unchanged"] = original.read_bytes() == b"Original native feature sample.\n"
        cow["reflink_status"] = "yes" if cow.get("exit_code") == 0 and cow.get("source_unchanged") else "no"
        cow["status"] = "yes" if filesystem in {"btrfs", "zfs", "apfs", "hammer2"} else cow["reflink_status"]
        if filesystem in {"btrfs", "zfs", "apfs", "hammer2"}:
            cow["reason"] = "Native filesystem uses copy-on-write; reflink result recorded separately"
        row["cow"] = cow
        if filesystem == "zfs":
            row["native_properties"] = probe(["zfs", "get", "acltype,compression,recordsize", source])
    finally:
        shutil.rmtree(directory)
    return row


def main():
    spec = importlib.util.spec_from_file_location("filesystem_cli", Path(__file__).with_name("test-filesystem-cli.py"))
    harness = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(harness)
    native_verify = harness.verify_payload
    captured = set()
    override = os.environ.get("LURKER_FEATURE_RECEIPTS")
    evidence = Path(override).resolve(strict=True) if override else None

    def verify(mountpoint, expected):
        digests = native_verify(mountpoint, expected)
        key = str(mountpoint)
        if key not in captured:
            captured.add(key)
            row = features(mountpoint)
            path = (evidence / (mountpoint.parent.name + "-" + mountpoint.name + ".json")
                    if evidence else mountpoint.parent / (mountpoint.name + ".features.json"))
            temporary = path.with_suffix(".tmp")
            temporary.write_text(json.dumps(row, indent=2) + "\n")
            temporary.replace(path)
        return digests

    harness.verify_payload = verify
    return harness.main()


if __name__ == "__main__":
    raise SystemExit(main())
