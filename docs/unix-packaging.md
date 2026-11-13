# Unix staging and package inputs

`scripts/stage-unix.py` stages Linux, FreeBSD, DragonFly BSD, NetBSD and OpenBSD artifacts. It does not build, install, elevate, run a payload, qualify encryption operations or change host permissions. macOS uses `scripts/build-macos-dmg.py` instead.

The package layout is:

```text
bin/lurker
bin/lurker-helper
bin/lurker-desktop                         # only with --desktop
libexec/lurker/backends/lurker-veracrypt-create
libexec/lurker/backends/lurker-veracrypt
libexec/lurker/backends/lurker-veracrypt-bsd   # BSD console engine
libexec/lurker/backends/lurker-fs
libexec/lurker/backends/gocryptfs
libexec/lurker/backends/cryfs
libexec/lurker/backends/cryptomator/        # launcher, private runtime, modules, sources/notices
share/lurker/licenses/
share/lurker/provenance/
share/lurker/manifest.json
```

Only explicitly selected applicable backends ship. Selection does not claim runtime qualification. Native LUKS/GELI/CGD/softraid/PEFS facilities remain host dependencies; private third-party runtimes must be supplied for selected VeraCrypt, gocryptfs, CryFS and Cryptomator operations. Missing required tools, runtimes, notices, provenance or shared libraries block packaging. CryFS 1.x specifically needs FUSE 2 (`libfuse.so.2` plus `fusermount`); Cryptomator and the Linux VeraCrypt userspace stack need FUSE 3 / `fusermount3`. gocryptfs accepts either Linux mount helper and uses `mount_fusefs` on FreeBSD. These are recorded per backend, rather than treating the two FUSE ABIs as interchangeable.

Build the Rust products in a clean checkout at one exact commit, then record their identities. Use the user's managed toolchains. `--source-commit` requires the full 40-character SHA. A deployed Git archive uses its verified `--source-manifest source-manifest.json` instead of Git metadata. Build outputs should be outside the immutable deployed source tree.

```bash
cargo build --release -p lurker-cli -p lurker-helper
python scripts/stage-unix.py record --kind binaries \
  --directory target/release --source-commit "$COMMIT" \
  --output target/rust-binaries.json
```

Record mode is a build-caller attestation linking hashes to the verified clean source tree; it cannot retrospectively prove how an arbitrary supplied binary was compiled. The release build runner must record immediately after its successful build and retain build logs. The stager verifies this identity and all native builder metadata, and records the final payload digests.

Backend build entry points (no installation):

- Linux VeraCrypt creator and optional userspace/LKL stack: `scripts/build-linux-userspace.py`; `--only-creator` omits the userspace stack.
- gocryptfs and stable CryFS: `scripts/build-folder-backends.py`.
- Cryptomator and reduced private JRE: `scripts/build-cryptomator.py`.
- BSD VeraCrypt: `scripts/build-bsd-backends.py` produces `lurker-veracrypt-bsd`, private `lib/`, notices and `bsd-veracrypt-source.json`.

Artifact arguments are repeated `--backend KIND=DIRECTORY`. Supported kinds are `veracrypt-create`, `veracrypt-userspace`, `veracrypt-bsd`, `gocryptfs`, `cryfs`, `cryptomator`. The userspace payload includes its creator; do not supply the creator twice. Folder builder aggregate directories are accepted: only the named executable, its manifest and its notices are copied. Full userspace/BSD VeraCrypt and Cryptomator preserve nested assets and internal relative symlinks.

The creator's `creator-source.json` binds its binary, pinned revision and current source patch. Folder builder `gocryptfs.json` / `cryfs.json` bind revision, platform and executable checksum. Cryptomator's `manifest.json` binds the complete runtime/module tree to its dependency lock. The stager also compares its packaged `Main.java`, module descriptor and Maven project against the exact selected Lurker commit; a new helper cannot silently reuse an older private image. README-only changes do not require rebuilding Java. Full Linux userspace payloads additionally require a file envelope because the older `sources.json` lists source pins without output hashes:

```bash
python scripts/stage-unix.py record --kind veracrypt-userspace \
  --directory target/release/userspace --source-commit "$COMMIT" \
  --output target/release/userspace/artifact-manifest.json
```

BSD VeraCrypt uses its builder's `bsd-veracrypt-source.json`, which binds the pinned upstream archives, native platform, executable and private-library hashes. The stager preserves `lib/` beside `lurker-veracrypt-bsd`, copies notices/provenance, and leaves build logs and unrelated aggregate-directory executables out. Native helper protocol and actual operations need separate qualification.

A Linux example stages the privileged VeraCrypt route and native LUKS support:

```bash
mkdir -m 700 target/packages
python scripts/stage-unix.py stage --binaries target/release \
  --binary-manifest target/rust-binaries.json --source-commit "$COMMIT" \
  --select luks --select veracrypt \
  --backend veracrypt-create=target/release/userspace \
  --output target/packages/lurker-linux
```

The output parent must belong to the caller and have no group/world write permission. Existing output files, directories (including empty ones), and symlinks are never replaced. The tool constructs a completed private backing tree adjacent to the requested output, then publishes the **output prefix as one atomic symlink**. Keep its adjacent hidden backing directory. `PREFIX/package.tar.gz` contains ordinary `lurker/bin`, `lurker/libexec` and `lurker/share` directories, without the archive itself; extract this archive for an actual root-owned installation. Archive owners are normalized to uid/gid 0. `PREFIX/package.sha256` and the final JSON stdout record bind the archive bytes. A failed staging operation removes only its own unfinished backing tree.

All input roots must be real directories. External, broken or looping source symlinks, traversal paths, special files, setuid/setgid payloads and colliding destinations are rejected. Internal JRE/library symlinks are retained with relative targets. Elevated operations must still canonicalize and validate the installed executable and ancestor ownership; a user-owned staging prefix does not become trusted for elevation merely because the stager created it.

Dependency audit uses the native `readelf`/`llvm-readelf` metadata reader, never `ldd` or an executable's version flag. It checks native ELF machine type, interpreter, RPATH/RUNPATH and recursively resolves linked libraries. Current-architecture ELF resources embedded in JARs are included. Missing libraries and developer/relative library search paths fail. Supplied private libraries remain private; host dependencies are explicitly recorded with resolved paths and hashes. The generated manifest deliberately says `self_contained: false`: packaging cannot supply kernel crypto, mount authorization, FUSE devices or PEFS kernel modules. The private JRE has one explicit loader qualification: its Java launcher/JLI loads `runtime/lib/server/libjvm.so` before the known java.base libraries (`libjava`, `libzip`, `libnet`, `libverify`, `libjimage`) need that SONAME. This relationship is accepted only with the verified Cryptomator manifest, native launcher/JLI/JVM, `-server KNOWN` configuration and matching JVM SONAME. Such edges are labeled `preloaded-by-private-java-launcher`; no arbitrary whole-prefix library search is allowed. Native packaged Java/Cryptomator tests must qualify the loader behavior. Metadata inspection does not prove every other `dlopen` path, plugin, kernel feature or mount permission; native packaged-artifact tests remain required.

Run staging unit tests on a test host:

```bash
python -m unittest discover -s scripts/tests -p test_stage_unix.py
```

Tests cover external symlinks, traversal, special/setuid files, internal runtime links, source/binary identity, missing runtimes, dependency failures and competing publication. They execute no encryption operations.
