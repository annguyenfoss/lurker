#!/usr/bin/env python3
"""Build separately installed NetBSD FUSE host prerequisites, never Lurker payloads."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
REVISION = "4299530ca04c23eff6e740a3d97d29bca16c22fe"
PATCH = ROOT / "native/netbsd/fuse-access.patch"
PATCH_SHA256 = "af5c644c2b5ee197640da0ab2fc69f7d7c2026664f53dbf2da7e627ed2607a82"
SOURCE_SHA256 = {
    "lib/libperfuse/Makefile": "cde968417fd9aee9c7baaf05fe955f7829e213c10743242b59a632fdc39f7cee",
    "lib/libperfuse/debug.c": "f5fc464350adaffab6b07e99ce7fee33f8fd6609c38bb3bbed98b7d70d5697a6",
    "lib/libperfuse/fuse.h": "0b7000766a3301e1905f478fe609e58dd4f32f8f23f3f64f1974e938d1743cb8",
    "lib/libperfuse/libperfuse.3": "6d0e3142db424a521d7ed20e6dccb7c9403b3237a7f214427a333e4a660114f7",
    "lib/libperfuse/ops.c": "3fb49a11abad77a86a3fea1ad5f794ddfd258314abec95dbdddd1cc343a6157b",
    "lib/libperfuse/perfuse.c": "bc44a28a6bf09b1793ffbeac753f0d39043e4d8de59b365af238268521f7b32a",
    "lib/libperfuse/perfuse.expsym": "ac76f552475bc42d9f2a81f43f706e3f545177fd8ee57a1439fb2ba1f4451781",
    "lib/libperfuse/perfuse.h": "b16b2430ff6c853f64bba2d4643447e6bd6cc92258f487bce5d5b043dbc5d641",
    "lib/libperfuse/perfuse_if.h": "bbc6a6a1692b7751da85d870579a11e7486bee88f8bc1d83e04f1463bdea5fee",
    "lib/libperfuse/perfuse_priv.h": "4753224b7ae2c3f594d1e78ecc967914282a7428dad8cb8d83df25ffcb40b8f3",
    "lib/libperfuse/shlib_version": "6ec254d326637e9b1fd05c7b025e6ff07f6953bcfc01341c699ae04a48b4a85c",
    "lib/libperfuse/subr.c": "1c2ed3d510005fc9273e48fb6ffd1403cba2286ee1e43d2d0d57607170efc772",
    "lib/libpuffs/Makefile": "0d542b572038c19fe5ce7a48d13db8a01bd4a7f4d5810df1b91edbe1bb869d25",
    "lib/libpuffs/TODO": "8d94fb0d454afdce802b804f47359407b600a5ae18ef793a54764b22817691a4",
    "lib/libpuffs/callcontext.c": "50723ecf9e8894e3d7c83f3638880980dcfefead9e190af558f4328d66fbe611",
    "lib/libpuffs/creds.c": "f2a75f5e1456d46c98500e8d71f732ee6af0831eea80e12c9711a1fd58e78ec1",
    "lib/libpuffs/dispatcher.c": "fdcca48e91088b22d6aa8a1317f77387dfd40820bb94046fbd7805888feac786",
    "lib/libpuffs/flush.c": "fe197f435c76f48658380177937d4ad85df9677857996e02b86e71815478dceb",
    "lib/libpuffs/framebuf.c": "5cd4af44f1aa258cdaa801ea876bd04c288f907c15efb696ab918038162318ca",
    "lib/libpuffs/null.c": "5517a205a3fbf71398a4d9eae8ce3ae8ad7fa27b48a1e7153c2670735da0efbd",
    "lib/libpuffs/opdump.c": "8b35cd43a3950222c1bb08525a0c25da428d4eca37d225584d665a27acfe92bb",
    "lib/libpuffs/paths.c": "2c3f3bf2a390f034f8f9bf4b33535312486f6b78b1beca90022cdcb4bcff68f6",
    "lib/libpuffs/pnode.c": "fc5cba5478a6ae3156e4560cef9e0005cc007d9a92ee36a4a60c5b823033b267",
    "lib/libpuffs/puffs.3": "b6482a4a56626d9bd7c7f605b1887d12a02bc9158e2b217de1ffea07ca184799",
    "lib/libpuffs/puffs.c": "69e0ad3218f3bb4e6fe3794262996c0befbab6fc81e5b3f5ccdbf2319103fed4",
    "lib/libpuffs/puffs.expsym": "f10e6ce1d09fe8c1f9d94189fc87d3c5a12df591365048d81497a89c6f551d20",
    "lib/libpuffs/puffs.h": "8494880740a49f7620eefacb218f4d933213fe23e641de783ed56e3ba39ae962",
    "lib/libpuffs/puffs_cc.3": "541903f7aa96e9358d3a7e21b8204e710c8f406cc019bf3e6befd42d246f569c",
    "lib/libpuffs/puffs_cred.3": "fd030e58dd1479321f18d0f2cad3fdd8ed98fac2511f8e99978989b6153d7b5b",
    "lib/libpuffs/puffs_flush.3": "b5f8bb1334309d4d9b11eba42d5fc30674688cba2d14fbd992511afdd9a89d9e",
    "lib/libpuffs/puffs_framebuf.3": "e1d3d9ffb1e7fca23d1f3ff8a3caa367eb69b30ee750e838c2c8e2216a993319",
    "lib/libpuffs/puffs_node.3": "4502ac2ffda557cee29124827343307f94ea3600f2724a342311d2845a285fd2",
    "lib/libpuffs/puffs_ops.3": "ce51f28b19a0be32898d9f539f466ee07de8818e36a036a7e0678b7842d78c42",
    "lib/libpuffs/puffs_path.3": "af91c5dfa1990457a71e7e08fee29c4ce1c3bf1e4ed6ba7805b98936a92fab23",
    "lib/libpuffs/puffs_priv.h": "d13b574a921d77c828c74df292261ea69c4ebf79b144108d7ccd82ca38050e9d",
    "lib/libpuffs/puffsdump.h": "0446f64eeb3464cf9c1da4228428396e431e59586bf601442531cdaab3b101c3",
    "lib/libpuffs/requests.c": "93d4aa1327aa79fbde057d558bf71a891789a93988bb09d23a404051df2cc601",
    "lib/libpuffs/shlib_version": "c9b2c003dabd18b43b1f76d6e1529e300c9975798caddb83ce6e0e38d3d4b85e",
    "lib/libpuffs/subr.c": "3bb31b59f8d93d2c8ba52d9e414d851a5c2197a3a918e949622db58630393433",
    "lib/libpuffs/suspend.c": "1d21b1aa25237bcb3a7acdb73d98120f48c7638b3669843283d94765b9e3ba80",
    "lib/librefuse/HACKING.md": "b75ec7cc2aa204d8cb79046a8b34f2fbb603dcd3295e0412c467a6d14de05c8b",
    "lib/librefuse/Makefile": "9fdfc75e11d39ee33c42380b79aa523cea5102a2d37d7db4e453fa23cc8e7644",
    "lib/librefuse/TODO": "3e94d59cff37b9ee44aa9ac7367863902968d5ac2cec725077ff29674bfddcfa",
    "lib/librefuse/fuse.h": "527943962d4b1e83cb2fcb8cff54e2b6cad1cb7940443478517e3455aaeee666",
    "lib/librefuse/fuse_internal.h": "6c23d727d6c90a2a78e596671d41193f467b6214dc0ada5d6084b30ec84087c3",
    "lib/librefuse/fuse_log.h": "eba7e65b0ac3c4c1080450b9a9fc263b204008c36742a46963152866f6ba7945",
    "lib/librefuse/fuse_lowlevel.h": "18893eed16ba74176a493bbcea8dc83626353e4c4308e654ebc436ecec045c6e",
    "lib/librefuse/fuse_opt.h": "aad47107bd1301b497d001eeed79254503c879132520de9613858f96f2cf765f",
    "lib/librefuse/refuse.3": "3b903241be765c49f7b7a7eee462e7ed55f399d16f398c68f311933462eaca7c",
    "lib/librefuse/refuse.c": "d5f0d8afbf01b8169e7a94c138b222cee6a3fd31e02f65d3eb283984b9192a6e",
    "lib/librefuse/refuse.expsym": "c695b9a88c138c84dee6bc7b46ed5718d51e976cb669ac5ef4eb68d01e645f2d",
    "lib/librefuse/refuse/Makefile.inc": "9ab7a87cb3d7fcce98e9a0079873e3e0c47c2baa8106e52f4b4495cc3128bc0f",
    "lib/librefuse/refuse/buf.c": "8dc96c2dc35cce8684d6008490ec24e1f0489d9091988a6759db8c0a9a1dccac",
    "lib/librefuse/refuse/buf.h": "8c8ac5c356fd05bb3e2cc80e46902d7dc1af9fbb13cbcc984e037049444063cf",
    "lib/librefuse/refuse/chan.c": "76a2d5eddfaa664552289df83b45fe030783425ad70a4b721b811333772e150a",
    "lib/librefuse/refuse/chan.h": "17bc400c94098d4916407303cdec81977b50fc980688f71bfe5dd1ec86d80d22",
    "lib/librefuse/refuse/fs.c": "ec8c62fa2c8184f107dee3c6a79da7ea8a362301166fe7e087098ca1bab2836e",
    "lib/librefuse/refuse/fs.h": "98f3a78edb50b5324ff8dab0891d9cf6e53a4593d184060b00d93b077421d3b6",
    "lib/librefuse/refuse/legacy.c": "a9b934a1501c7af0558b6bc49e3ef44b2804ae4ef472c1296817214ded0a9541",
    "lib/librefuse/refuse/legacy.h": "65f2c6ec82328893ffef0b914a1da5be49a63555d60f5888ef98124d5ddd58c0",
    "lib/librefuse/refuse/poll.c": "57e7ed089137b535ef10896e64c315370c0e49b3b35329dda0ef813fe9dbbb2f",
    "lib/librefuse/refuse/poll.h": "a72abef0969981d573eb5b3be20a3b6f7aa1d6120f165e5d6a9469863d80d653",
    "lib/librefuse/refuse/session.c": "9eabc21a0e92aa3cf2b1c4f274f797c634ca335216928604a243c3ca33bfd612",
    "lib/librefuse/refuse/session.h": "42e2afeea0c457a970c717d779ea1fde68303a3ce18e24da99c453baa7733c14",
    "lib/librefuse/refuse/v11.c": "6b36d8a0de41c60ded536afcc21e36da315920eb8077cd2b9897ce30e4abfda9",
    "lib/librefuse/refuse/v11.h": "032f23ddc15116b32d860f6a0766b57807f6c3bf3187ae07fd361e7a66dab36d",
    "lib/librefuse/refuse/v21.c": "cfa2e68688e5e2e8493aa5a1363ce27277cd41f6f10ed45e3ca2bf1c9772b320",
    "lib/librefuse/refuse/v21.h": "dfe05465f6da94fae7895a8248f318d5b9a1bee48b4097cb80d682281b17d45b",
    "lib/librefuse/refuse/v22.c": "c770b9c89aa973edd75c4ae7298c262ba72292aee4825bc23837c4071628614c",
    "lib/librefuse/refuse/v22.h": "ebba72a92966b4f4a4ad9aa32eff49909850c88d59a39253330814bb9ad8c973",
    "lib/librefuse/refuse/v23.h": "735c7e06ce0a08fa0ae8ed9965a0d39cefb88909c585955ff1ce3e70687880c3",
    "lib/librefuse/refuse/v25.c": "7269a764a095338a8ee4ae750c8ca7016c39bd011bbd3ba4f10a0c17ac44744a",
    "lib/librefuse/refuse/v25.h": "0bc2cfb997631bad8c7b9fa4e6b9b0b0f2969e8d8a1ed8d528802c2acedf2724",
    "lib/librefuse/refuse/v26.c": "707a9358d5992efb49fdc9601b3b532c47f712105ff2fbdfd3a6d76b9dda57bb",
    "lib/librefuse/refuse/v26.h": "214aa589dc1572071f43158bf74bd13f16b8a88bdb4033b3e31156958bfb530d",
    "lib/librefuse/refuse/v28.h": "3b9f496ee91fd290a34dd5037a0850d7227bc563231d5d6c3dd557f769c58f4d",
    "lib/librefuse/refuse/v29.h": "85cba375c11c681889353301e96e9940645177e355417d0b2b0403ad795c9ac5",
    "lib/librefuse/refuse/v30.c": "c3071102df5c7a897c03ff7ce66971474d74c87adbbf6feaaacea5165b672374",
    "lib/librefuse/refuse/v30.h": "946eda3b0a908fa76740b6adb06e48018c496afc18e1cb78d7ea61d29a768305",
    "lib/librefuse/refuse/v32.c": "062261c1d2ea9442241c4eb371096b90797af7bcfc515be00dbdd8149fdc6f58",
    "lib/librefuse/refuse/v32.h": "e39ac19919ace75ce35f94b01ea472aa143e886246835b0c9e1ec25e32abddbe",
    "lib/librefuse/refuse/v34.h": "bbda0aea1203b0117c7471846f81f095817febfdb29200fad84d23dcd25bd4ec",
    "lib/librefuse/refuse/v35.h": "071b3606f9a1dba8337930617af0a4cd9bb5d254945eff7aef9dc591c25acc85",
    "lib/librefuse/refuse/v38.h": "5b1fd90216443ba1efc7c6eb09f0a7c9abd6b95b6497dcc30fc8f9712cdfaed4",
    "lib/librefuse/refuse_compat.c": "12b2f0bd25d6147e7906b58bb41a585354edef4cc635c98a5ba622f8bca2866f",
    "lib/librefuse/refuse_log.c": "5228831f4befba6845e8b637a0295b45cbaa81716525c73d2f6caaeb6640e88f",
    "lib/librefuse/refuse_lowlevel.c": "2e45d85cec01c688fb25d82a659179295eec1b8f231ad14023ec806e348b8cc2",
    "lib/librefuse/refuse_opt.c": "8e8d7bfd8779d6ac9c0378216554d1853217a264c5452e9ab5010161ec805524",
    "lib/librefuse/refuse_signals.c": "b297d6e976d4073aceec5a634afae37bb6e9b8601720a89939814722284e1049",
    "lib/librefuse/shlib_version": "ebecf7efe3e331b98638df673c709528b6d8b8798242082ab32f792e37ae5461"
}
PATCHED_SHA256 = {
    "lib/libperfuse/ops.c": "14c9420fdec410c1aa75708a7ed1da289afc71c4557b7d316edd804441dfa57d",
    "lib/libpuffs/dispatcher.c": "48258182f3afe0f02c5f95afc638bc62b4f981acb889088e7a020d35e5f890df",
    "lib/librefuse/fuse_internal.h": "ba68b2e4dc97cb666baf4b90289b2369dd259585d934d3e607d5ed6299ebfe44",
    "lib/librefuse/refuse.c": "a763b5e10f7579996cc1da275191b5b7110f1c855a667ef87cc4b211520d5b5b"
}
FEATURES = {"libpuffs.so.2": "puffs_access_abi_version",
            "libperfuse.so.0": "perfuse_access_cache_version",
            "librefuse.so.2": "refuse_access_policy_version"}
LIBRARIES = [
    ("puffs", "libpuffs.so.2", "puffs.c callcontext.c creds.c dispatcher.c flush.c framebuf.c null.c opdump.c paths.c pnode.c requests.c subr.c suspend.c".split(), ["-lpthread", "-lutil"]),
    ("perfuse", "libperfuse.so.0", "perfuse.c ops.c subr.c debug.c".split(), ["-lpuffs"]),
    ("refuse", "librefuse.so.2", "refuse.c refuse_compat.c refuse_log.c refuse_lowlevel.c refuse_opt.c refuse_signals.c".split()
     + ["refuse/" + name + ".c" for name in "buf chan fs legacy poll session v11 v21 v22 v25 v26 v30 v32".split()], ["-lpuffs", "-lpthread", "-lutil"]),
]


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(argv, log, *, cwd=None, env=None):
    with log.open("ab") as stream:
        result = subprocess.run(argv, cwd=cwd, env=env, stdout=stream,
                                stderr=subprocess.STDOUT, timeout=180)
    if result.returncode:
        raise ValueError(f"native command failed ({result.returncode}); inspect {log}")


def acquire(cache):
    source = cache / REVISION
    source.mkdir(parents=True, exist_ok=True)
    for name, expected in SOURCE_SHA256.items():
        path = source / name
        if path.is_symlink():
            raise ValueError("source cache symlink: " + str(path))
        if not path.exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            request = urllib.request.Request(
                f"https://raw.githubusercontent.com/NetBSD/src/{REVISION}/{name}",
                headers={"User-Agent": "NetBSD-native-prerequisite-builder"})
            with urllib.request.urlopen(request, timeout=60) as response:
                data = response.read(4 * 1024 * 1024 + 1)
            if len(data) > 4 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != expected:
                raise ValueError("source download checksum differs: " + name)
            path.write_bytes(data)
        if not path.is_file() or digest(path) != expected:
            raise ValueError("source cache checksum differs: " + name)
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if platform.system() != "NetBSD" or platform.release().split(".")[0] != "11":
        parser.error("this source/ABI prerequisite targets native NetBSD 11")
    output = args.output.absolute()
    if output.exists() or output.is_symlink():
        parser.error("output must not already exist")
    parent = output.parent
    metadata = parent.stat()
    if parent.resolve() != parent or metadata.st_uid != os.geteuid() or metadata.st_mode & 0o022:
        parser.error("output parent must be real, owned by the builder and not group/world writable")
    if PATCH.is_symlink() or digest(PATCH) != PATCH_SHA256:
        parser.error("reviewed native access patch checksum differs")
    if digest("/usr/include/puffs.h") != SOURCE_SHA256["lib/libpuffs/puffs.h"]:
        parser.error("installed public puffs ABI header differs from the pinned native source")
    cache = args.cache.absolute()
    cache.mkdir(parents=True, exist_ok=True)
    source = acquire(cache)
    with tempfile.TemporaryDirectory(prefix=".netbsd-fuse-", dir=parent) as temporary:
        work = Path(temporary)
        build_source = work / "source"
        shutil.copytree(source, build_source)
        for name, expected in SOURCE_SHA256.items():
            if digest(build_source / name) != expected:
                raise ValueError("copied native source checksum differs: " + name)
        log = work / "build.log"
        run(["/usr/bin/patch", "--batch", "--forward", "--fuzz=0", "-p1", "-i", str(PATCH)],
            log, cwd=build_source)
        for name, expected in PATCHED_SHA256.items():
            if digest(build_source / name) != expected:
                raise ValueError("patched native source checksum differs: " + name)
        objects = work / "objects"
        objects.mkdir()
        for library, soname, files, dependencies in LIBRARIES:
            root = build_source / "lib" / ("lib" + library)
            compiled = []
            for name in files:
                target = objects / (library + "-" + name.replace("/", "-") + ".o")
                run(["/usr/bin/cc", "-O2", "-g", "-fPIC", "-D_KERNTYPES", "-D_REENTRANT",
                     "-I" + str(root), "-I" + str(build_source / "lib/libpuffs"),
                     "-c", str(root / name), "-o", str(target)], log)
                compiled.append(str(target))
            run(["/usr/bin/cc", "-shared", "-Wl,-soname," + soname,
                 "-o", str(objects / soname), *compiled, "-L" + str(objects), *dependencies], log)
            if library == "puffs":
                (objects / "libpuffs.so").symlink_to(soname)
        published = work / "published"
        published.mkdir(mode=0o755)
        for name in FEATURES:
            shutil.copyfile(objects / name, published / name)
            (published / name).chmod(0o755)
        # A separate native process ensures its dependencies load from this exact
        # build, independently of libraries already mapped by the builder.
        probe = ("import ctypes,json; p=" + repr(str(published)) + "; names=" + repr(FEATURES)
                 + "; r={}\nfor name,symbol in names.items():\n lib=ctypes.CDLL(p+'/'+name); fn=getattr(lib,symbol); fn.restype=ctypes.c_uint; fn.argtypes=[]; r[symbol]=fn(); assert r[symbol]==1\nprint(json.dumps(r))")
        environment = os.environ.copy()
        environment["LD_LIBRARY_PATH"] = str(published)
        features = json.loads(subprocess.check_output([sys.executable, "-c", probe],
                                                       env=environment, text=True, timeout=30))
        shutil.copytree(build_source, published / "source")  # Retain upstream notices and exact sources.
        shutil.copyfile(PATCH, published / "applied.patch")
        shutil.copyfile(log, published / "build.log")
        manifest = {"schema": 1, "kind": "netbsd-fuse-host-prerequisite", "os": platform.system(),
                    "release": platform.release(), "architecture": platform.machine(),
                    "upstream": "https://github.com/NetBSD/src", "upstream_commit": REVISION,
                    "patch_sha256": PATCH_SHA256, "original_sources": SOURCE_SHA256,
                    "patched_sources": PATCHED_SHA256, "features": features,
                    "compiler": subprocess.check_output(["/usr/bin/cc", "--version"], text=True).splitlines()[0],
                    "sdk_headers": {name: digest(name) for name in ["/usr/include/puffs.h", "/usr/include/fs/puffs/puffs_msgif.h", "/usr/include/sys/vnode.h", "/usr/include/sys/stat.h"]},
                    "libraries": {name: digest(published / name) for name in FEATURES}}
        (published / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        published.rename(output)
    print(json.dumps({"status": "built-host-prerequisite", "output": str(output),
                      "manifest_sha256": digest(output / "manifest.json"),
                      "libraries": manifest["libraries"]}))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print("NetBSD native FUSE prerequisite build failed: " + str(error), file=sys.stderr)
        sys.exit(1)
