# Private Cryptomator backend

This independently licensed helper uses Cryptomator CryptoFS 2.8.0 and FUSE-NIO
5.1.0. It implements protocol 1: `init EXISTING_EMPTY_DIRECTORY`,
`mount VAULT MOUNTPOINT [--readonly]`, `--version`, and `--capabilities`.
Initialization and mount consume one nonempty UTF-8 password line on stdin.
There is no password argument/environment option, WebDAV fallback, or runtime
download. The process stays in the foreground after mounting. Lurker must verify
and gracefully unmount its recorded mount before terminating this process.
Shutdown never calls the providers' forced-unmount interface.

FUSE-NIO 5.1.0 is required for registered `fsync` support. Version 5.0.5 implemented
the method but disabled its registration; daemon SIGKILL testing exposed lost
SQLite WAL commits even after FULL commit and explicit file fsync. The native
regression test checks the adapter's actual registered operations. Normal close
tests alone cannot qualify durability. See the upstream fix:
https://github.com/cryptomator/fuse-nio-adapter/blob/5.1.0/CHANGELOG.md

The format profile is Cryptomator vault format 8, SIV-GCM (AES-SIV filenames,
AES-GCM contents), upstream scrypt masterkey protection, standard
`vault.cryptomator`/`masterkey.cryptomator` filenames, no implicit migration.
Read-only applies to CryptoFS and the FUSE mount. Password storage is erased after
unlock where the Java API permits; this does not promise that the JVM has no
temporary copies. Plaintext is accessible to processes running as the user while
mounted.

Build with JDK 25 or later and Maven (build prerequisites only). The JDK needs
`jmods` or JEP 493 runtime-image linking support; an IDE runtime may lack both:

```sh
python scripts/build-cryptomator.py --java-home /path/to/jdk25 --maven /path/to/mvn
```

The result contains its own reduced Java runtime. Ship the entire `cryptomator`
directory without rearranging `mods` or `runtime`; no installed Java or
Cryptomator application is required. Linux libfuse3/fusermount3 and a supported
macOS macFUSE installation remain external OS prerequisites. This helper currently
selects macFUSE, not FUSE-T. Build separately on each target OS/architecture.
For native CryptoFS tests, use `--test` only on dedicated test hosts with private
temporary test directories; the default build skips runtime tests.

`dependencies.lock.json` pins every shipped Maven runtime artifact by SHA-256.
Changing dependencies requires reviewing and updating the lock; the builder
rejects missing, added or modified jars. The builder records helper/runtime
digests, toolchain and source references in `manifest.json`. Maven itself and
the build JDK must be obtained and verified separately. Do not treat Maven's
transport checksums or a successful build as a security audit.

This Java helper and CryptoFS/FUSE-NIO are AGPL-3.0 works, distributed separately
from the GPL-3.0 Rust executables. The helper's complete source is included in the
image, along with AGPL text, runtime legal notices, dependency identities and
source URLs. Release distribution must also supply corresponding source and
required notices for every dependency and the JDK. Retain embedded jar license
resources and `runtime/legal`. The Java runtime's GPL/ClassPath exception and
third-party licenses do not replace the helper's AGPL obligations.

Upstream API/build references:

- https://github.com/cryptomator/cryptofs/tree/2.8.0
- https://github.com/cryptomator/fuse-nio-adapter/tree/5.1.0
- https://github.com/cryptomator/cli
- https://github.com/cryptomator/cryptolib

Native filesystem qualification remains separate from CryptoFS-level tests:
CLI create/mount/write/fsync/unmount/reopen, SQLite WAL, read-only, bad password,
busy mount, mount identity replacement and crash/full-storage behavior must pass
on each advertised OS.

The packaged backend passed the dedicated Arch Linux smoke suite (create,
existing-vault protection, Unicode/binary fsync and rename, writable SQLite WAL
and reopen, bad password, durable reopen, graceful close and read-only).
`scripts/test-cryptomator.py --backend /path/to/lurker-cryptomator --runtime-tests`
reproduces those checks in a private `/tmp/lurker-cryptomator-test-*` directory and
retains a concise result with evidence. As with ordinary SQLite WAL databases,
read-only access may need existing shared-memory files or SQLite immutable mode;
the smoke suite uses immutable mode only after the sole writable connection has
closed/checkpointed. This is not a claim of concurrent-writer or crash safety.
macOS arm64 builds and the initial two CryptoFS-level native integration tests pass using
a private reduced Java 27 runtime. macFUSE is absent on the available Mac, so its
mounted lifecycle remains unqualified. An experiment using the upstream FUSE-T
provider with FUSE-T 1.2.9 mounted and wrote files but failed the SQLite WAL check
with a disk I/O error. FUSE-T is consequently not enabled; do not bypass that
decision with library symlinks. Signing/notarization of a Mac package must account
for the bundled JVM's JIT and external native FUSE loading, with appropriately
scoped Java entitlements; an unsigned build is not a release package.

With FUSE-NIO 5.1.0, the dedicated Arch daemon-crash suite also passes: SQLite
FULL/WAL commits and explicit file/directory fsync while the writer's descriptors
remain open, verified daemon SIGKILL, normal CLI unmount, reopen, exact file data
and WAL replay/integrity verification. The three Java tests pass on Arch and macOS,
including callback registration. This proves the tested daemon-crash scenario,
not arbitrary power-loss recovery or durable directory metadata on every host.
