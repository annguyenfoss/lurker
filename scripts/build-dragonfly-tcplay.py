#!/usr/bin/env python3
"""Build the private DragonFly VeraCrypt wrapper without elevation."""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
NATIVE = ROOT / "native/dragonfly-tcplay"
UPSTREAM = "https://raw.githubusercontent.com/DragonFlyBSD/DragonFlyBSD"
BINARY = "lurker-tcplay"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare(pin, cache, work, offline):
    for relative, expected in pin["sha256"].items():
        cached = cache / pin["revision"] / relative
        if not cached.is_file() or digest(cached) != expected:
            if offline:
                raise SystemExit(f"Missing or invalid cached source: {relative}")
            url = f"{UPSTREAM}/{pin['revision']}/{relative}"
            with urllib.request.urlopen(url, timeout=60) as response:
                data = response.read()
            if hashlib.sha256(data).hexdigest() != expected:
                raise SystemExit(f"Upstream checksum differs: {relative}")
            cached.parent.mkdir(parents=True, exist_ok=True)
            cached.write_bytes(data)
        path = work / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(cached, path)


def notices(pin, work):
    # Preserve notices before distributing the binary, including the cipher sources.
    seen = set()
    texts = []
    for relative in pin["sha256"]:
        source = (work / relative).read_text()
        prefix = source.split("#include", 1)[0]
        for comment in re.findall(r"/\*.*?\*/", prefix, re.DOTALL):
            if "copyright" in comment.lower() and comment not in seen:
                seen.add(comment)
                texts.append(f"{relative}\n{comment}")
    return "\n\n".join(texts) + "\n"


def build(cache, output, offline):
    pin = json.loads((NATIVE / "sources.json").read_text())
    with tempfile.TemporaryDirectory(prefix="work-", dir=cache) as temp:
        work = Path(temp)
        prepare(pin, cache, work, offline)
        license_text = notices(pin, work)
        patch = NATIVE / "lurker.patch"
        wrapper = NATIVE / "wrapper.c"
        with patch.open("rb") as stream:
            subprocess.run(["/usr/bin/patch", "-s", "-p1"], cwd=work, stdin=stream, check=True)
        binary = work / BINARY
        # Use the same base LibreSSL ABI as the DragonFly 6.4 tcplay binary.
        subprocess.run([
            "/usr/bin/cc", "-std=gnu11", "-O2", "-fstack-protector-strong",
            "-DMAJ_VER=3", "-DMIN_VER=3", "-D_FILE_OFFSET_BITS=64",
            "-Ilib/libtcplay", "-Isys", "-I/usr/include/priv", "-o", str(binary),
            str(wrapper), *pin["sources"], "-L/usr/lib/priv", "-lprivate_crypto", "-ldm", "-lprop", "-lutil",
        ], cwd=work, check=True)
        version = subprocess.check_output([str(binary), "protocol"], text=True).strip()
        if version != "lurker-tcplay 2":
            raise SystemExit("Unexpected wrapper protocol")
        if subprocess.check_output([str(binary), "physical-protocol"], text=True).strip() != "lurker-tcplay physical 1":
            raise SystemExit("Unexpected physical wrapper protocol")
        output.mkdir(parents=True, exist_ok=True)
        license_path = output / "licenses" / "DragonFly-tcplay.txt"
        license_path.parent.mkdir(exist_ok=True)
        license_path.write_text(license_text)
        shutil.copy2(binary, output / BINARY)
        manifest = {
            "schema": 1, "protocol": version,
            "runtime": {"physical_source_protocol": 1},
            "target": {"os": platform.system(), "machine": platform.machine()},
            "revision": pin["revision"],
            "inputs": {name: digest(NATIVE / name) for name in
                       ("sources.json", "wrapper.c", "lurker.patch")},
            "files": {BINARY: digest(binary), "licenses/DragonFly-tcplay.txt": digest(license_path)},
        }
        (output / "dragonfly-tcplay-source.json").write_text(
            json.dumps(manifest, indent=2, sort_keys=True) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--offline", action="store_true")
    options = parser.parse_args()
    if (platform.system(), platform.machine()) != ("DragonFly", "x86_64"):
        parser.error("The private wrapper requires native DragonFly x86-64")
    cache = options.source_dir.resolve()
    cache.mkdir(parents=True, exist_ok=True)
    build(cache, options.output_dir.resolve(), options.offline)


if __name__ == "__main__":
    main()
