# Private Cryptomator backend

This independently licensed helper uses Cryptomator CryptoFS 2.8.0 and FUSE-NIO
5.1.0. It implements protocol 1: `init EXISTING_EMPTY_DIRECTORY`,
`mount VAULT MOUNTPOINT`, `--version`, and `--capabilities`.
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
The product exposes one mount/unmount action, with writable access and no RO/RW
selector. Password storage is erased after unlock where the Java API permits; this does not promise that the JVM has no
temporary copies. Plaintext is accessible to processes running as the user while
mounted.

Build with JDK 25 or later and Maven (build prerequisites only). The JDK needs
`jmods` or JEP 493 runtime-image linking support; an IDE runtime may lack both:

```sh
python scripts/build-cryptomator.py --java-home /path/to/jdk25 --maven /path/to/mvn
```

The result contains its own reduced Java runtime. Ship the entire `cryptomator`
directory without rearranging `mods` or `runtime`; no installed Java or
Cryptomator application is required. Linux libfuse3/fusermount3 and macOS FUSE-T
remain external OS prerequisites. On macOS 26 or later the helper selects the
upstream FUSE-T provider with explicit `backend=fskit`; there is no SMB, NFS or
macFUSE fallback. Install FUSE-T with FSKit support separately, open
`/Applications/fuse-t.app`, and enable its file-system extension in System
Settings > General > Login Items & Extensions > File System Extensions.
Build separately on each target OS/architecture. The manifest binds the native
provider, runtime, transport and minimum OS alongside the helper source hashes.
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
The macOS 27 arm64 prototype passed the strict 13-case CLI lifecycle suite with
FUSE-T 1.2.9/FSKit and a private Java 27 runtime, including SQLite WAL, read-only,
busy close, credentials and reopen. Every production image/package still needs
its own native qualification. The upstream FUSE-T default SMB transport returns
`ENOTSUP` for byte-range locks (`SQLITE_IOERR_LOCK`); NFS refuses WAL mode. Neither
is accepted. `pluginkit -e use` sets a user election without granting FSKit
Settings consent; a disabled extension still refuses mounting. Sign the private
JVM with narrowly scoped JIT/external-library entitlements when packaging.
FUSE-T documents its FSKit backend for macOS 26+, independent of Cryptomator's
separate native FSKit adapter:
https://github.com/macos-fuse-t/fuse-t

Upstream also offers a separate native FSKit adapter for macOS 27/Apple silicon.
Embedding it requires an app extension with an Apple-issued provisioning profile
authorizing `com.apple.developer.fskit.fsmodule`; an ad-hoc build cannot mount.
This is a separate integration and remains unqualified, not an automatic fallback:
https://github.com/cryptomator/fskit-nio-adapter

With FUSE-NIO 5.1.0, the dedicated Arch daemon-crash suite also passes: SQLite
FULL/WAL commits and explicit file/directory fsync while the writer's descriptors
remain open, verified daemon SIGKILL, normal CLI unmount, reopen, exact file data
and WAL replay/integrity verification, including callback registration. This proves the tested daemon-crash scenario,
not arbitrary power-loss recovery or durable directory metadata on every host.
