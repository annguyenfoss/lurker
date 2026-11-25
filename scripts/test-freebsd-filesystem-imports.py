#!/usr/bin/env python3
"""FreeBSD RO import checks using independently formatted APFS/Btrfs images.

Run an administrator-owned copy beside test-filesystem-cli.py. Normal native
crypto close precedes the tested CLI reopen; credentials remain on stdin.
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--backends", default="geli,veracrypt")
    parser.add_argument("--filesystems", default="apfs,btrfs,xfs")
    args = parser.parse_args()
    engine = args.binary.parent.parent / "libexec/lurker/backends/lurker-veracrypt-bsd"
    statuses = []
    for backend in args.backends.split(','):
        for fs, raw_name in (("apfs", "apfs-import.cdr"), ("btrfs", "btrfs.img"), ("xfs", "probe-xfs.img"), ("ntfs", "ntfs4k.img")):
            if fs not in args.filesystems.split(','):
                continue
            if backend == "geli" and fs in ("apfs", "btrfs", "xfs"):
                continue
            raw = args.root / raw_name
            directory = Path(tempfile.mkdtemp(prefix=f"import-{backend}-{fs}-", dir=args.root))
            directory.chmod(0o700)
            image, target = directory / "vault.img", directory / "mount"
            target.mkdir(mode=0o700)
            password = secrets.token_urlsafe(30)
            result = {"backend": backend, "filesystem": fs, "mode": "readonly", "commit": args.commit,
                      "binary_sha256": payload.file_hash(args.binary), "raw_sha256": payload.file_hash(raw),
                      "fixture": str(directory), "status": "running", "commands": []}
            backing = mapped = None
            native_vc = cli_mounted = False

            def native(command, auth=False):
                completed = subprocess.run(list(map(str, command)), input=(password + "\n").encode() if auth else None,
                                           capture_output=True, timeout=180)
                log = directory / f"{len(result['commands']):02d}-native.log"
                log.write_bytes(completed.stdout + completed.stderr)
                result["commands"].append({"argv": list(map(str, command)), "exit_code": completed.returncode, "log": str(log)})
                if completed.returncode:
                    raise RuntimeError("Native command failed: " + str(command[0]))
                return completed.stdout.decode().strip()

            def cli(action, operands, auth=True):
                status = payload.terminal([str(args.binary), action, "--type", backend, "--color=never", *map(str, operands)],
                                          password if auth else None, 180, directory / f"cli-{action}.log")
                result["cli_" + action] = status
                if status["exit_code"] or status["timed_out"]:
                    raise RuntimeError("CLI " + action + " failed")

            try:
                cli("create", [image, "0.5"])
                if backend == "geli":
                    backing = "/dev/" + native(["/sbin/mdconfig", "-a", "-t", "vnode", "-f", image])
                    native(["/sbin/geli", "attach", "-j", "-", backing], auth=True)
                    mapped = backing + ".eli"
                else:
                    native([engine, "--text", "--mount", "--non-interactive", "--stdin", "--pim=0", "--filesystem=none",
                            "--protect-hidden=no", "--keyfiles", "", image], auth=True)
                    native_vc = True
                    listing = native([engine, "--text", "--list", "--verbose", image])
                    mapped = next(line.split(":", 1)[1].strip() for line in listing.splitlines() if line.startswith("Virtual Device:"))
                native(["/bin/dd", "if=" + str(raw), "of=" + mapped, "bs=64k"])
                if backend == "geli":
                    native(["/sbin/geli", "detach", backing]); mapped = None
                    native(["/sbin/mdconfig", "-d", "-u", backing.removeprefix("/dev/md")]); backing = None
                else:
                    native([engine, "--text", "--dismount", image]); native_vc = False; mapped = None
                cli("mount", [image, target])
                cli_mounted = True
                if not os.path.ismount(target):
                    raise RuntimeError("CLI read-only mount missing")
                result["payload_sha256"] = {name: payload.file_hash(target / name) for name in ("paper.pdf", "page.txt")}
                if result["payload_sha256"]["paper.pdf"] != hashlib.sha256(payload.sample_pdf()).hexdigest():
                    raise RuntimeError("Independent PDF hash mismatch")
                expected_text = {"apfs": "9cd617210a58d52a62ceee8402a2f15d3f59ec53408a15f965e60ed8e852fef0",
                                 "btrfs": "9460fe7f6dd53641f831e29a6e66e800553a42d328e4831ec5466cc3cb01211c",
                                 "xfs": hashlib.sha256(b"Independent native XFS read-only import. UTF-8: caf\xc3\xa9.\n").hexdigest(),
                                 "ntfs": "4a54aaf657d7208872bc68c10213176425a8921ad5736ca40458f2f0e0a3879c"}
                if result["payload_sha256"]["page.txt"] != expected_text[fs]:
                    raise RuntimeError("Independent UTF-8 text hash mismatch")
                result["readonly_observed"] = bool(os.statvfs(target).f_flag & os.ST_RDONLY)
                if not result["readonly_observed"]:
                    raise RuntimeError("Mount did not enforce read-only mode")
                cli("unmount", [target], auth=False); cli_mounted = False
                result["status"] = "passed"
                result["closed_container_sha256"] = payload.file_hash(image)
                image.unlink()
                result["successful_container_removed"] = True
            except Exception as error:
                result.update(status="failed", error=str(error))
            finally:
                if cli_mounted or os.path.ismount(target):
                    try: cli("unmount", [target], auth=False)
                    except Exception as error: result["cleanup_error"] = str(error)
                if native_vc:
                    native([engine, "--text", "--dismount", image])
                elif mapped:
                    native(["/sbin/geli", "detach", backing])
                if backing:
                    native(["/sbin/mdconfig", "-d", "-u", backing.removeprefix("/dev/md")])
                result["mount_remaining"] = os.path.ismount(target)
                payload.atomic_json(directory / "result.json", result)
            statuses.append(result["status"])
            print(json.dumps({"backend": backend, "filesystem": fs, "status": result["status"], "receipt": str(directory / "result.json")}), flush=True)
    return 0 if statuses and all(status == "passed" for status in statuses) else 1


if __name__ == "__main__":
    raise SystemExit(main())
