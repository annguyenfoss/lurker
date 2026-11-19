# Native BSD packages

Build and stage the exact product commit on the target BSD using
`scripts/build-unix.py` or the verified remote runner. Then wrap that archive
on the same guest, as the build user. The wrapper verifies the archive digest,
source commit, native audit, packaged payload bytes, links and file ownership.
It preserves the original staged manifest inside the native package.

```sh
python scripts/build-bsd-package.py \
  --archive /absolute/stage/package.tar.gz \
  --sha256 FULL_STAGED_ARCHIVE_SHA256 --commit FULL_SOURCE_COMMIT \
  --version 0.1.0 --output /absolute/output/lurker-0.1.0.pkg
```

On NetBSD and OpenBSD, use `lurker-0.1.0.tgz` for the output name. Save the
JSON receipt printed by the wrapper. It identifies the product source, archive,
package and wrapper digests, package ABI, selected backends and desktop payload.
An output path already in use is refused.

| Target | Native tool | Install prefix |
| --- | --- | --- |
| FreeBSD | `pkg create` | `/usr/local` |
| DragonFly BSD | `pkg create` | `/usr/local` |
| NetBSD | `pkg_create` | `/usr/pkg` |
| OpenBSD | `pkg_create` | `/usr/local` |

Stage with the backends for the target. A CLI-only stage produces a CLI-only
package. Desktop packages need the native X11 libraries, xkbcommon and sudo.
Use the distribution's X11 sets on NetBSD and OpenBSD. The package installs
the private helper alongside the desktop binary. Administrators supply the
sudo policy and any native FUSE device access. FreeBSD PEFS requires the
`pefs-kmod` package matching the running kernel. The wrapper confirms that
the staged PEFS utility and matching module come from that package and adds
it as a dependency.

Use `pkg add` and `pkg delete` on FreeBSD and DragonFly. Use `pkg_add` and
`pkg_delete` on NetBSD and OpenBSD. OpenBSD's distribution policy requires a
trusted source or signed packages for normal distribution. Use `-Dunsigned`
only for a locally built artifact with verified provenance in a disposable
test. The wrapper creates artifacts and declares dependencies without
installing them or changing administrator policy.

During tests, keep the package database and install prefix both isolated.
OpenBSD `pkg_add -B` relocates payload paths and needs an explicit
`PKG_DBDIR` for database isolation. Seed the alternate root with the
distribution runtime dependencies before checking native install, and
verify the original database and install paths independently.

## KVM validation

The following native packages were built from the recorded portable stage
sources. Install and removal used separate package databases and roots with
native dependency checks enabled. Archive digests, package digests, toolchain
source and logs are retained under `target/bsd-native-wrapper/`.

| Guest and payload | Product source | Native installation proof |
| --- | --- | --- |
| FreeBSD 15.1, full CLI and desktop | `a60e4bc` | Installed with 22 official dependency artifacts. All 208 GELI, PEFS, VeraCrypt, and gocryptfs CLI checks passed, along with four desktop lifecycles and an X11 window. The isolated package was removed and the original package database matched its baseline. Earlier `029cf99` and `2ebae8f` native packages each passed 13 GELI checks. |
| NetBSD 11.1, full CLI and desktop | `24dcc6e` | Installed with sudo and xkbcommon. CGD passed 13 lifecycle checks before removal. |
| OpenBSD 8.0, full CLI and desktop | `264cd6e` | Installed with signed dependencies and native shared-library checks. CRYPTO passed 13 lifecycle checks. The installed VeraCrypt helper supported login-user writes and SQLite WAL persistence after close and reopen, kept its raw FUSE data private, and safely refused read-only attach. The package was then removed. |
| DragonFly 6.4.2, full CLI and desktop | `a60e4bc` | Installed with native dependency checks on an offline archive clone. The installed CLI passed 91 LUKS1 lifecycle checks, including three binary-keyfile suites, and eight UFS exhaustion checks. The installed desktop opened an X11 window and its helper completed the login-user lifecycle. Package removal passed. |

The lifecycle cases create encrypted storage, write and fsync data, close,
reopen and verify the contents. They also cover rejected credentials and busy,
aliased and read-only operations. Backend and desktop qualifications outside
these native installer checks retain their individual package provenance.

DragonFly's full native install used a fresh verified archive copy because
the original guest's package database was damaged. The original KVM guest
built and tested the frozen workspace in `20261119T184037Z-a58833ac`
with 90 passing, zero failing and three ignored tests. Its independent
workspace check passed in `20261119T184445Z-dd83cbd7`. The clone staged the
verified binaries with the native sudo dependency and passed the three
ignored native tests separately. The initial runtime attempt lacked a
sudoers file. The callback test resumed with a validated temporary policy
for the installed helper, then removed that policy.

The seven closed lifecycle fixtures all had a LUKS1 header with the selected
AES, Serpent or Twofish cipher, XTS/plain64, a 512-bit key and PBKDF2-HMAC-SHA256
with one active slot. The installed desktop rendered a 1280×860 X11 window
as UID 1300. The native package SHA-256 is
`aa883e408098fbb470fa906eb13edca792b7342a112d98735bf20ddcd2956d59`.
The runtime, header checks, original failed attempt and archive identities are
in `target/dragonfly-frozen/`. Native device, mount and session inventories
matched before and after testing. The desktop/package clone shut down normally
and its isolated network was removed.

### DragonFly native cryptsetup provenance

The original 6.4.2 x86-64 KVM guest reports `cryptsetup 1.1.2` for its
statically linked base-system `/sbin/cryptsetup`. The executable SHA-256 is
`93c3c006d15f2ed8ec446f303e1c3908c87c65ca07425f59755e24811b101f45`.
Lurker uses that base-system program and the kernel's native dm-crypt target.

The 6.4.2 source builds the imported 1.1.2 code against DragonFly's own
libdm and private LibreSSL. The release-branch history includes a 2018
change to use the native `mlockall`, a 2020 dm-crypt fix for resource waits
under memory pressure, and a 2022 compatibility change for the LibreSSL
upgrade. The release's installed dm, opencrypto and vnode headers matched
the `v6.4.2` source byte for byte. As of November 19, 2026 UTC, eight
reviewed files covering cryptsetup setup and key management, dm-crypt,
opencrypto and vnode handling were byte-identical between `v6.4.2` and 6.4
maintenance branch head `fa3b19cbc1d785319e3359e67e3f806e59af267e`. The
reviewed opencrypto history includes a 2023 CTR key-length fix, outside
the selected XTS profiles. Comprehensive LUKS1 security-fix coverage
remains a public-release gate.

The released dm-crypt target accepts the three selected XTS/plain64 profiles
with 512-bit combined keys. It selects the AES, Twofish and Serpent XTS
identifiers from the installed opencrypto header, all registered by the
native software provider. On this release, dm-crypt forwards flush requests
to its backing vnode, which issues a synchronous `VOP_FSYNC` when it
receives one. That conditional path leaves abrupt-power durability to be
qualified separately. The development branch switched its default dm-crypt
module in 2025 and still lists all three XTS ciphers in its new kernel crypto
API. The native runtime results cover 6.4.2.

https://raw.githubusercontent.com/DragonFlyBSD/DragonFlyBSD/v6.4.2/contrib/cryptsetup/README.DRAGONFLY

https://raw.githubusercontent.com/DragonFlyBSD/DragonFlyBSD/v6.4.2/gnu/sbin/cryptsetup/Makefile

https://github.com/DragonFlyBSD/DragonFlyBSD/commit/239cd26e6de1b7874f315638a56288ca9aec92c0

https://github.com/DragonFlyBSD/DragonFlyBSD/commit/8345afad56bed8074dc4836acd98dfb6a80bbeb7

https://github.com/DragonFlyBSD/DragonFlyBSD/commit/6f5ec8b51c959914fd0128aa84612579c0400168

https://github.com/DragonFlyBSD/DragonFlyBSD/tree/fa3b19cbc1d785319e3359e67e3f806e59af267e

https://github.com/DragonFlyBSD/DragonFlyBSD/commit/02793a4afc03607c685abfc69cc73f22a999abba

https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/dev/disk/dm/crypt/dm_target_crypt.c

https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/opencrypto/cryptosoft.c

https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/dev/disk/vn/vn.c

https://github.com/DragonFlyBSD/DragonFlyBSD/commit/f9c79d28d9ec0c95b4e78c6374f774077944db5e

https://github.com/DragonFlyBSD/DragonFlyBSD/blob/4ab67026e6d18a4be2a7ea459c919705bee786af/sys/dev/disk/dm/crypt/dm_target_crypt.c

### DragonFly interrupted creation

On the original 6.4.2 guest, the CLI from the verified `a60e4bc` package
stage passed five checks on a private AES LUKS1 binary-keyfile fixture. The
test killed the create process group after verifying its staging journal and
live backing vnode, before it established a crypto mapping. The existing
`--force` destination retained its original bytes and inode. The journal
survived and a normal `lurker unmount --type luks1` against the abandoned
stage released its vnode and journal. The stage digest was unchanged, and
the native mount, mapping, vnode and session inventories returned to their
exact baseline. This tests process interruption and explicit recovery with
the guest still running. The transcript, fixture identity and source
comparisons are in `target/dragonfly-native-review/`.
