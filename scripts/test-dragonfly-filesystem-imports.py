#!/usr/bin/env python3
"""Native DragonFly LUKS1 import fixtures and readonly CLI payload checks.

Run from an administrator-owned copy beside test-filesystem-cli.py. Uses only
fresh file containers, native crypto, upstream formatters, and normal close.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import secrets
import subprocess
import tempfile

spec = importlib.util.spec_from_file_location("payload", Path(__file__).with_name("test-filesystem-cli.py"))
payload = importlib.util.module_from_spec(spec)
spec.loader.exec_module(payload)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--binary", required=True, type=Path)
    p.add_argument("--tools", required=True, type=Path)
    p.add_argument("--root", required=True, type=Path)
    p.add_argument("--commit", required=True)
    args = p.parse_args()
    for fs in ["ext4", "ntfs"]:
        directory = Path(tempfile.mkdtemp(prefix="import-" + fs + "-", dir=args.root))
        directory.chmod(0o700)
        image = directory / "vault.img"
        mountpoint = directory / "mount"
        mountpoint.mkdir(mode=0o700)
        password = secrets.token_urlsafe(30)
        expected = {"paper.pdf": payload.sample_pdf(), "page.txt": "Imported UTF-8 payload: Việt Nam — 日本語.\n".encode()}
        for name, data in expected.items():
            (directory / name).write_bytes(data)
        result = {"filesystem": fs, "backend": "luks1", "mode": "readonly", "commit": args.commit,
                  "binary_sha256": payload.file_hash(args.binary), "fixture": str(directory),
                  "status": "running", "commands": []}
        vnode = None
        mapping = "lurker_import_" + fs + "_" + secrets.token_hex(4)
        mapped = False
        cli_mounted = False
        env = dict(os.environ, LD_LIBRARY_PATH=str(args.tools / "lib/e2fsprogs") + ":" + str(args.tools / "lib"))

        def run(command, auth=False):
            completed = subprocess.run(list(map(str, command)), input=(password + "\n").encode() if auth else None,
                                       capture_output=True, timeout=120, env=env)
            log = directory / f"{len(result['commands']):02d}-native.log"
            log.write_bytes(completed.stdout + completed.stderr)
            result["commands"].append({"argv": list(map(str, command)), "exit_code": completed.returncode, "log": str(log)})
            if completed.returncode:
                raise RuntimeError("Native command failed: " + str(command[0]))
            return completed.stdout.decode().strip()

        try:
            with image.open("xb") as stream:
                stream.truncate(512 * 1024 * 1024)
            vnode = run(["/usr/sbin/vnconfig", "vn", image])
            if not vnode.startswith("/dev/"):
                vnode = "/dev/" + vnode
            run(["/sbin/cryptsetup", "--batch-mode", "--cipher", "aes-xts-plain64", "--key-size", "512",
                 "--hash", "sha256", "--iter-time", "5000", "luksFormat", vnode], auth=True)
            run(["/sbin/cryptsetup", "--batch-mode", "luksOpen", vnode, mapping], auth=True)
            mapped = True
            device = "/dev/mapper/" + mapping
            if fs == "ext4":
                # Ordinary upstream feature profile; no driver/source patching.
                run([args.tools / "sbin/mkfs.ext4", "-F", device])
                for name in expected:
                    run([args.tools / "sbin/debugfs", "-w", "-R", f"write {directory / name} /{name}", device])
            else:
                run([args.tools / "sbin/mkntfs", "-F", "-Q", device])
                for name in expected:
                    run([args.tools / "sbin/ntfscp", device, directory / name, "/" + name])
            run(["/sbin/cryptsetup", "luksClose", mapping])
            mapped = False
            run(["/usr/sbin/vnconfig", "-u", vnode])
            vnode = None
            command = [str(args.binary), "mount", "--type", "luks1", "--readonly", "--color=never", str(image), str(mountpoint)]
            status = payload.terminal(command, password, 120, directory / "cli-mount.log")
            result["cli_mount"] = status
            if status["exit_code"] or status["timed_out"]:
                raise RuntimeError("Readonly CLI import mount failed")
            cli_mounted = True
            if not os.path.ismount(mountpoint):
                raise RuntimeError("Readonly mount missing")
            result["sha256"] = {}
            for name, data in expected.items():
                if (mountpoint / name).read_bytes() != data:
                    raise RuntimeError("Imported payload mismatch: " + name)
                result["sha256"][name] = hashlib.sha256(data).hexdigest()
            status = payload.terminal([str(args.binary), "unmount", "--type", "luks1", "--color=never", str(mountpoint)], None, 120, directory / "cli-unmount.log")
            result["cli_unmount"] = status
            if status["exit_code"] or status["timed_out"]:
                raise RuntimeError("Readonly CLI import close failed")
            cli_mounted = False
            result["status"] = "passed"
        except Exception as error:
            result["status"] = "failed"
            result["error"] = str(error)
        finally:
            if cli_mounted or os.path.ismount(mountpoint):
                result["cleanup"] = payload.terminal([str(args.binary), "unmount", "--type", "luks1", "--color=never", str(mountpoint)], None, 120, directory / "cleanup.log")
            if mapped:
                run(["/sbin/cryptsetup", "luksClose", mapping])
            if vnode:
                run(["/usr/sbin/vnconfig", "-u", vnode])
            result["mount_remaining"] = os.path.ismount(mountpoint)
            payload.atomic_json(directory / "result.json", result)
        print(json.dumps({"filesystem": fs, "status": result["status"], "receipt": str(directory / "result.json")}))


if __name__ == "__main__":
    main()
