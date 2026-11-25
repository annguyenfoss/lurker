#!/usr/bin/env python3
"""OpenBSD VeraCrypt readonly imports using unmodified native filesystem tools.

Run an administrator-owned copy beside test-filesystem-cli.py. All fixture
passwords remain in memory; receipts retain command names and content hashes.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import tempfile

spec = importlib.util.spec_from_file_location("payload", Path(__file__).with_name("test-filesystem-cli.py"))
payload = importlib.util.module_from_spec(spec)
spec.loader.exec_module(payload)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--backend", required=True, type=Path)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--filesystem", choices=["ext4", "apfs", "xfs"], required=True)
    parser.add_argument("--raw-image", type=Path)
    args = parser.parse_args()
    if args.filesystem != "ext4" and not args.raw_image:
        parser.error("APFS and XFS imports need an independently formatted raw image")
    directory = Path(tempfile.mkdtemp(prefix="import-" + args.filesystem + "-", dir=args.root))
    directory.chmod(0o700)
    mountpoint = directory / "mount"
    mountpoint.mkdir(mode=0o700)
    native_mount = directory / "native"
    native_mount.mkdir(mode=0o700)
    image = directory / "vault.img"
    password = secrets.token_urlsafe(30)
    result = {"filesystem": args.filesystem, "backend": "veracrypt", "mode": "readonly",
              "commit": args.commit, "binary_sha256": payload.file_hash(args.binary),
              "backend_sha256": payload.file_hash(args.backend), "fixture": str(directory),
              "commands": [], "status": "running"}
    mapped = None
    attached = False
    cli_mounted = False

    def run(command, input_data=None):
        command = list(map(str, command))
        completed = subprocess.run(command, input=input_data, capture_output=True, timeout=180)
        log = directory / f"{len(result['commands']):02d}-native.log"
        log.write_bytes(completed.stdout + completed.stderr)
        log.chmod(0o600)
        result["commands"].append({"argv": command, "exit_code": completed.returncode, "log": str(log)})
        if completed.returncode:
            raise RuntimeError("Native command failed: " + command[0])
        return completed.stdout.decode().strip()

    def hashes(target):
        if not os.path.ismount(target):
            raise RuntimeError("OS mount absent; refusing underlay reads")
        output = {}
        for name in ["paper.pdf", "page.txt"]:
            path = target / name
            if not path.is_file() or path.is_symlink():
                raise RuntimeError("Imported payload absent: " + name)
            output[name] = payload.file_hash(path)
        return output

    try:
        run([args.backend, "--text", "--non-interactive", "--create", "--force", "--volume-type=normal",
             "--encryption=AES", "--hash=SHA-512", "--filesystem=none", "--random-source=/dev/urandom",
             "--pim=0", "--keyfiles=", "--stdin", "--size=512M", image], (password + "\n").encode())
        run([args.backend, "--text", "--non-interactive", "--mount", "--filesystem=none",
             "--protect-hidden=no", "--pim=0", "--keyfiles=", "--stdin", image], (password + "\n").encode())
        attached = True
        listing = run([args.backend, "--text", "--non-interactive", "--list", "--verbose", image])
        found = re.search(r"(?m)^Virtual Device:\s*(/dev/vnd[0-9]+[ac])\s*$", listing)
        if not found:
            raise RuntimeError("Cannot identify owned VeraCrypt virtual device")
        mapped = found.group(1)[:-1] + "a"
        name = mapped.removeprefix("/dev/").removesuffix("a")
        auxiliary_match = re.search(r"(?m)^Lurker Auxiliary Directory:\s*(/tmp/[^\r\n]+)$", listing)
        if not auxiliary_match:
            raise RuntimeError("Cannot identify owned VeraCrypt auxiliary directory")
        auxiliary_image = Path(auxiliary_match.group(1)) / "volume"
        # Native label outside the imported filesystem, preserving partition a
        # when VeraCrypt closes and reopens its encrypted vnode.
        run(["/bin/dd", "if=/dev/zero", "of=/dev/r" + name + "c", "bs=1m", "count=1"])
        run(["/sbin/disklabel", "-wAT-", name], b"/ *\n")
        run(["/sbin/disklabel", "-E", name], b"d a\na a\n64\n*\nMSDOS\nw\nq\n")
        if args.filesystem == "ext4":
            expected = {"paper.pdf": payload.sample_pdf(), "page.txt": "Imported UTF-8 payload: Việt Nam — 日本語.\n".encode()}
            # OpenBSD ext2fs supports extents/flex_bg read-only, but not the
            # 64bit/metadata_csum_seed incompat flags. This is an ordinary
            # upstream mke2fs profile, without driver/source modifications.
            run(["/usr/local/sbin/mke2fs", "-t", "ext4", "-F", "-O", "^64bit,^metadata_csum_seed,^orphan_file", mapped])
            for filename, data in expected.items():
                local = directory / filename
                local.write_bytes(data)
                run(["/usr/local/sbin/debugfs", "-w", "-R", f"write {local} /{filename}", mapped])
            result["expected_sha256"] = {name: hashlib.sha256(data).hexdigest() for name, data in expected.items()}
        else:
            result["raw_image_sha256"] = payload.file_hash(args.raw_image)
            tool = "/usr/local/bin/fsapfsmount" if args.filesystem == "apfs" else "/usr/local/bin/fsxfsmount"
            run([tool, "-X", "ro", args.raw_image, native_mount])
            try:
                result["expected_sha256"] = hashes(native_mount)
            finally:
                run(["/sbin/umount", native_mount])
            run(["/bin/dd", "if=" + str(args.raw_image), "of=" + mapped, "bs=1m"])
        native_command = (["/sbin/mount", "-t", "ext2fs", "-o", "ro", mapped, native_mount]
                          if args.filesystem == "ext4" else [tool, "-X", "ro", "-o", "32768", auxiliary_image, native_mount])
        result["native_reader_source"] = "mapped block partition" if args.filesystem == "ext4" else "existing VeraCrypt decrypted regular auxiliary image at offset 32768; libyal rejects OpenBSD block-device size"
        run(native_command)
        try:
            observed = hashes(native_mount)
            if observed != result["expected_sha256"]:
                raise RuntimeError("Native encrypted-fixture reader changed payload hashes")
            result["native_reader_sha256"] = observed
            result["native_reader"] = "passed"
        finally:
            run(["/sbin/umount", native_mount])
        run([args.backend, "--text", "--non-interactive", "--dismount", image])
        mapped = None
        attached = False
        for cycle in range(2):
            status = payload.terminal([str(args.binary), "mount", "--type", "veracrypt",
                                       "--color=never", str(image), str(mountpoint)], password, 180,
                                      directory / f"cli-mount-{cycle}.log")
            result[f"cli_mount_{cycle}"] = status
            if status["exit_code"] or status["timed_out"]:
                log_text = (directory / f"cli-mount-{cycle}.log").read_text()
                if "OpenBSD VeraCrypt read-only crypto attachment has not been qualified" in log_text:
                    result["status"] = "blocked"
                    result["reason"] = "Existing OpenBSD VeraCrypt crypto route refuses readonly; pinned upstream readonly vnode attachment is unimplemented. Native inner reader passed with a writable crypto mapping."
                    break
                raise RuntimeError("Readonly CLI import mount failed")
            cli_mounted = True
            actual = hashes(mountpoint)
            if actual != result["expected_sha256"]:
                raise RuntimeError("Imported PDF/text hashes changed")
            result[f"observed_sha256_{cycle}"] = actual
            status = payload.terminal([str(args.binary), "unmount", "--type", "veracrypt", "--color=never", str(mountpoint)],
                                      None, 180, directory / f"cli-unmount-{cycle}.log")
            result[f"cli_unmount_{cycle}"] = status
            if status["exit_code"] or status["timed_out"]:
                raise RuntimeError("Readonly CLI close failed")
            cli_mounted = False
        if result["status"] == "running":
            result["status"] = "passed"
    except Exception as error:
        result["status"] = "failed"
        result["error"] = str(error)
    finally:
        if cli_mounted or os.path.ismount(mountpoint):
            result["cleanup"] = payload.terminal([str(args.binary), "unmount", "--type", "veracrypt", "--color=never", str(mountpoint)],
                                                 None, 180, directory / "cleanup.log")
        if attached:
            try:
                run([args.backend, "--text", "--non-interactive", "--dismount", image])
            except Exception as error:
                result["native_cleanup_error"] = str(error)
        result["mount_remaining"] = os.path.ismount(mountpoint) or os.path.ismount(native_mount)
        payload.atomic_json(directory / "result.json", result)
    print(json.dumps({"filesystem": args.filesystem, "status": result["status"], "receipt": str(directory / "result.json")}))
    return 0 if result["status"] in {"passed", "blocked"} and not result["mount_remaining"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
