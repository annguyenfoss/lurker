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

The BSD VeraCrypt build ships both the wxWidgets exception and its full
Library GPL text from the pinned wxWidgets archive. The backend manifest
records both SHA-256 digests, and Unix staging requires both entries.
Native FreeBSD and OpenBSD KVM checks restaged the verified VeraCrypt
1.26.31 engines with both texts. The engine hashes matched the previous
stages, and the notice-incomplete stages were refused. Each guest passed
45 builder and staging tests. Receipts are under
`target/goal-validation/bsd-wx-notices/`.

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
| OpenBSD 8.0, full CLI and desktop | `e607cc7` | Installed with signed dependencies and native shared-library checks. CRYPTO passed 13 lifecycle checks, and RAID1C passed 19 lifecycle and 11 invoking-user checks. The installed VeraCrypt helper passed login-user writes and SQLite WAL persistence after close and reopen, kept its raw FUSE data private, and safely refused read-only attach. The package was then removed. |
| DragonFly 6.4.2, full CLI and desktop | `a60e4bc` | Installed with native dependency checks on an offline archive clone. The installed CLI passed 91 LUKS1 lifecycle checks, including three binary-keyfile suites, and eight UFS exhaustion checks. The installed desktop opened an X11 window and its helper completed the login-user lifecycle. Package removal passed. |
| DragonFly 6.4.2, full CLI and desktop | `ca3c845` | Rebuilt and installed on a separate offline archive clone. All 91 LUKS1 lifecycle checks, eight UFS exhaustion checks, two core native regressions, the desktop callback lifecycle and the X11 window passed. Package removal restored the native package inventory. |

The lifecycle cases create encrypted storage, write and fsync data, close,
reopen and verify the contents. They also cover rejected credentials and busy,
aliased and read-only operations. Backend and desktop qualifications outside
these native installer checks retain their individual package provenance.

### Native installer refresh at `14ede46`

The OpenBSD 8.0 amd64 package from run `20261120T042218Z-88688721` passed
160 installed checks in an isolated stock KVM guest. CRYPTO passed 26
explicit and Auto checks. VeraCrypt passed 104 across AES, Serpent and
Twofish with passwords and binary keyfiles. RAID1C passed 19 CLI checks
and 11 invoking-user checks. The installed desktop rendered an X11
window as UID 1000. Three Slint callback tests created, mounted, closed
and reopened vaults as the same login user through the installed helper.
Both mounts were synchronous FFS for each backend. The test restored
the authorization, package and native resource inventories, and retired
the isolated guest, overlay and network. The initial package SHA-256 was
`26eac1549d13fbc2622afa0117b7a77023d2338390db0ab0b6a7ed84e6664323`.
The build and runtime receipts are under `target/openbsd-matrix-14ede46/`.

The initial archive omitted the Rust notice catalog. Run
`20261120T052010Z-6eafe5ba` rebuilt the same source with the actual Cargo
build log. The corrected package kept the three Rust binaries byte for
byte and bound 369 Cargo entries to the verified lockfile. Its package
and installed inventories verified the catalog and all 638 notice files.
Fifteen crates had license declarations and no separate notice file in
the registry archive.

The corrected stock package repeated all 160 storage checks, the X11
probe and all three installed-helper callbacks. The tests restored the
inventories and retired the guest. The corrected native SHA-256 is
`858588d9d664d7f10e62bf62f846117564306e2073b0798de63ad5d2324ec13b`.
The two package identities and the corrected runtime evidence are under
`target/openbsd-notices-14ede46/`.

The NetBSD 11.1 amd64 package from run `20261120T042227Z-dc85c5ea` passed
26 installed CGD checks in a stock KVM guest without a network interface.
The explicit and Auto routes each verified files and SQLite data after
close and reopen. The installed desktop rendered a 1280×860 X11 window
as UID 1300. Package, mount, device and session inventories returned to
their baselines, and the guest and its disposable disks were retired.
The package SHA-256 was
`d58bfdd9f2d7bf7141d061acdd9539c389cb26facb7b022a262d1c8d5eb7d3b7`.
The build and runtime receipts are under `target/netbsd-14ede46-native/`.
A second no-network stock guest ran the CGD Slint callback lifecycle as
UID 1300 through the helper from the same package. The test removed its
authorization rule and restored the package, mount, device and session
inventories before powering off. The callback evidence is under
`target/netbsd-14ede46-callbacks/`.

DragonFly 6.4.2 used optimized binaries from run
`20261120T042450Z-421aa4be`. The build guest lacked the stager's desktop
authorization prerequisite. A stock offline KVM guest staged the same
verified binaries and Rust notices with its installed sudo. The native
package SHA-256 was
`c34cb4a80a54492fa659055acf7e075fc6fa8424c7f895fea623b747758ea3be`.
The workspace check and 94 tests passed with four ignored in run
`20261120T044927Z-1d39e1e1`.

The installed CLI passed 91 LUKS1 checks across AES, Serpent and Twofish,
including binary keyfiles and AES Auto. The installed desktop rendered
a 1280×860 X11 window. Retained closed fixtures left only 74 MiB free
on the guest's `/tmp` tmpfs and prevented two further image creates.
After reclaiming those closed fixtures, the same package passed the
desktop callback lifecycle as UID 1300 and all nine UFS full-storage
checks. SQLite reported `SQLITE_FULL` and recovered its committed data
after space was freed and the vault reopened. Package, mount, mapper,
vnode, session and authorization inventories were restored. The clone
was retired with its stock source disk unchanged. Evidence is under
`target/goal-validation/frozen-14ede46/dragonfly/`.

### Earlier OpenBSD and DragonFly package runs

The OpenBSD `e607cc7` descriptor-staging replay used build
`20261120T014923Z-4517db5c` and native package SHA-256
`7f066d6f1cf7f62eeb61afd5a1d885e445ed9a44f1859827b19d29895fa2c0ad`.
The isolated install, writable lifecycles and removal left the original
package inventory, mounts, native devices and receipt hashes unchanged.
The build receipt and runtime evidence are under
`target/openbsd-descriptor-stage/`.

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

### DragonFly rebuild after the Rust dependency updates

The original guest built the CLI, helper and desktop from
`ca3c8456e507102a1c6e2b9194981afbd48df8db` in
`20261119T204124Z-a986983d`. The same revision passed workspace checks
and 92 tests in `20261119T201722Z-311c61a7`, with three ignored native
tests. The original guest lacked sudo and had a damaged package database.
A verified archive clone staged and wrapped the digest-checked binaries.
The first replay stopped before installation because the clone's dm-crypt
module was unloaded. The repeat ran with the native module loaded.

The seven lifecycle suites covered password and binary-keyfile access for
AES, Serpent and Twofish, plus AES auto-detection. The closed headers
confirmed LUKS1, XTS/plain64, 512-bit keys and PBKDF2-HMAC-SHA256. The
installed desktop rendered a 1280×860 X11 window as UID 1300 and its helper
completed the login-user lifecycle under a temporary sudo policy. Package
removal restored the clone's package, mount, vnode, mapper and session
inventories. The original guest's package database digest and native
inventory matched their baselines. The clone shut down normally after
restoring its transport configuration, and its isolated network was removed.

The native package SHA-256 is
`67941ec7534a0449da3c84ef43ba8e213e2ab97a8fa72a50ca0a377cf2423d6a`.
The installer, archive, result JSON, native inventories and X11 frame are in
`target/dragonfly-ca3c845/`.

### OpenBSD native package with the VeraCrypt write barrier

OpenBSD 8.0 amd64 wrapped the exact `4189679` stage as the build user. The
native package SHA-256 is
`4929fc8a53c78053e40721c69425306828c07920c19a0a487e3b47dc5000a1b9`.
The alternate install root and package database used preinstalled sudo and
xkbcommon dependencies. All ten payloads matched their hashes and root
ownership. The six base tool paths and eight system library hashes matched
the staging receipt. The installed CLI rejected an ordinary-user-owned
backend, a group-writable backend and a group-writable backend ancestor
before the test backend could execute.

The installed package passed 69 lifecycle checks across softraid CRYPTO,
VeraCrypt AES password and binary keyfile, and RAID1C. The RAID1C checks
included application I/O and WAL recovery as the invoking login user.
Inherited OpenBSD syscall traces observed no IPv4 or IPv6 sockets during
wrapping, install, lifecycle and removal. The guest kept its existing
network configuration. Removal cleared the package registration and
payloads. The host package, device and mount inventories matched the
baseline. The archive and summaries are under `target/openbsd-vc-barrier/`.

### FreeBSD clean-machine offline install

A standalone copy of the vendor FreeBSD 15.1-RELEASE amd64 BASIC CLOUDINIT
UFS image booted on a transient KVM guest. The guest had no IPv4 or IPv6
default route, and its control network had no forwarding or DNS. The
vendor package database contained no Lurker, VeraCrypt or gocryptfs.
Native `pkg` 2.6.2_1 installed the checksum-verified `ca3c845` package at
`/usr/local` after installing 72 verified local dependencies for the product
and test harness. These included the matching `pefs-kmod` and an archived
official Xvfb package. The CLI, helper and desktop resolved their shared
libraries on the clean guest.

The installed package passed all 208 CLI cases (39 GELI, 39 PEFS, 104
VeraCrypt and 26 gocryptfs), and rendered a 1280×860 X11 window as UID 1300.
The runner resumed the two pending binary-keyfile suites and the X11 probe
under `/tmp` after their fixture-path checks rejected `/var/tmp`. All storage
and kernel resource inventories matched their baselines after cleanup.
Package removal also restored the vendor package inventory. The guest shut
down normally, its isolated network was removed, and the vendor image
retained its original SHA-256.

The native package SHA-256 is
`9fed88319d7339a1a36ce1aba36e5dca45fdd2d8b04e6974df9f9fa3b17c6022`.
The input receipts, workload logs, and teardown audit are in
`target/freebsd-clean-ca3c8456/`.

### OpenBSD clean-machine offline install

A new amd64 KVM guest installed stock OpenBSD 8.0 and its X11 sets from
`install80.iso`, with the ISO verified against the signed release checksum.
Its disk started blank with no backing image. Its sole network was a
host-only control bridge without forwarding or an IPv4 or IPv6 default route.
The package database was initially empty. The guest verified 16 local official
package archives with the OpenBSD 8.0 package key, then used native `pkg_add`
to install signed dependencies. The offline repository needed both `quirks`
and `updatedb` for package-tool bootstrap, in addition to the declared sudo
and xkbcommon dependencies and their closure. Python was installed separately
for the test harness.

Ordinary `pkg_add` rejected the unsigned `ca3c845` developer archive. The
qualification installed its digest-verified bytes with `-D unsigned` scoped
to that archive. Its SHA-256 is
`e3587d4723b52732df4cd5302652e5a8307cafa740c0410b2dd8fcd2c4c34e55`.
The installed CLI passed 69 checks, including 13 CRYPTO, 26 VeraCrypt AES
password and binary-keyfile, and 30 RAID1C checks covering degraded and login-user
operation. All 16 observed FFS mounts reported `synchronous`. The installed
desktop rendered a 1280×860 X11 window as UID 1000. Native package removal
and storage inventory checks passed. The guest shut down after syncing its
disks, and its transient domain and isolated bridge were removed.

The stock installer could not resolve the automatic firmware update site;
the stock KVM drivers were sufficient for the tested routes. The clean-machine
receipts and rendered frame are under `target/openbsd-clean-20261120/evidence/`.

https://www.openbsd.org/80.html

### OpenBSD native package signing

OpenBSD 8.0 signs native binary packages with
`pkg_sign -s signify2 -s /path/to/maintainer-pkg.sec -o /path/to/output`.
The matching public key is enrolled as `/etc/signify/maintainer-pkg.pub`
on the system that installs the package. Default `pkg_add` validates both
the signed compressed data and the trusted signer.

An isolated clone signed a disposable copy of the exact `ca3c845` native
archive with a throwaway maintainer key. All ten payload files and their
ownership and mode metadata matched the original archive. Stock `pkg_add`
rejected the signer before its public key was enrolled in the clone. After
enrollment it refused a copy with one altered compressed byte, then installed
the valid copy with default signature enforcement. Standalone `signify`
reported a signature mismatch for the altered copy. The native package
database recorded the expected signer and matched the baseline again after
removal. The temporary keys, clone overlay and bridge were removed, and the
backing disk and original native archive retained their hashes. Evidence is
under `target/openbsd-signing-20261120/evidence/`.

A public native release requires an authorized publisher package-signing key
and an authenticated channel for distributing its public key.

https://man.openbsd.org/OpenBSD-8.0/pkg_sign.1

https://man.openbsd.org/OpenBSD-8.0/pkg_add.1

### DragonFly clean-machine offline install

The stock DragonFly 6.4.2 installer populated a blank, standalone KVM disk
with HAMMER2. The untouched source ISO matched the vendor's published MD5,
and the read-only copy matched its SHA-256. The installed guest had a
host-only control network without forwarding or a default route. All 57
local package archives matched DragonFly's HTTPS catalogue and their SHA-256
digests matched on the guest before installation. The configured DragonFly
repository uses an unsigned catalogue.

Stock `pkg add` kept older installed versions of nine same-name prerequisites,
leaving `libcrypto.so.17` unresolved for Xvfb and sudo. An explicit offline
upgrade to the verified DragonFly packages resolved those libraries. The
native package manager then installed the same `ca3c845` package, SHA-256
`67941ec7534a0449da3c84ef43ba8e213e2ab97a8fa72a50ca0a377cf2423d6a`.

The installed package passed 91 LUKS1 lifecycle checks across the three
ciphers and both credential modes, eight UFS exhaustion checks, and two
native core regressions. The CLI used short persistent fixtures on HAMMER2
after the initial paths triggered the native 80-byte mountpoint limit. The
installed desktop rendered a 1280×860 X11 window as UID 1300 and its helper
passed the login-user callback lifecycle. Package removal and the native
resource inventory checks passed. The disposable guest shut down normally
and its isolated network was removed. The original KVM guest's package
database digest and native inventories matched the baseline. Media receipts,
failed attempts, inventories and the rendered frame are in
`target/dragonfly-stock-ca3c845/`.

https://www.dragonflybsd.org/release64/

https://mirror-master.dragonflybsd.org/dports/dragonfly:6.4:x86:64/LATEST/packagesite.pkg

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

### DragonFly guarded unlock source review

A separate static review compared the `da62173` adapter with the exact
DragonFly 6.4.2 sources. Native cryptsetup reads a padded header and keyslot
sectors through its mapping. Lurker validates that geometry and supplies
those reads from the root-private prefix. The adapter checks the composite
layout, native device identities and prefix bytes before unlock. The
original file still backs payload I/O. The review covers the approved
AES, Serpent and Twofish XTS/plain64 profiles with PBKDF2/SHA-256.

The installed tests verify read-only behavior through UFS. The native
mapper receives a read-only flag, and enforcement at the raw device
remains unqualified. A DragonFly 6.4.2 KVM control confirmed that a stock
read-only null mount preserved the backing file's identity and that its
vnode disk refused a writable open. The kernel's device mapper then
refused to put a read-only linear mapping over that disk. Its underlying
device lookup requests writable access. The probe attempted no writes
and restored the native device and mount inventories. See
`target/goal-validation/dragonfly-source-detach/readonly-attachment-control.json`.

The review also identified outstanding work on
bounding native password derivation, handling an interrupted native
keyslot mapping, and preserving the journal on ambiguous vnode queries.

https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/contrib/cryptsetup/luks/keymanage.c

https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/contrib/cryptsetup/luks/keyencryption.c

https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/dev/disk/dm/device-mapper.c

https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/dev/disk/dm/dm_pdev.c

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

### NetBSD clean poweroff and restart

The native NetBSD 11.1 package from
`ca3c8456e507102a1c6e2b9194981afbd48df8db` passed the CGD shutdown lifecycle
on a networkless KVM clone. A consumer fsynced a 65,536-byte file and its
directory entry, committed a SQLite FULL/WAL row, and held a second row
uncommitted. The clone's fixture shutdown hook ran under `/etc/rc.shutdown`,
verified the consumer's identity, waited for it to roll back and exit, and
ran `lurker session-close` without a credential prompt. The mounted
filesystem was gone and all eight CGD and four vnode devices were idle
before the guest powered off.

After booting the same clone, the packaged CLI reopened the container.
The file matched its digest, the database passed its integrity check, and
only the committed row was visible. Normal close left the native inventory
idle. The hook and native package were removed, and the transient domain
was shut down. Both source-disk hashes matched their pretest values.
The first hook attempt stopped at the consumer identity check. The retry
trimmed NetBSD `ps` padding and used a new container. Both consoles and the
final receipt are in `target/netbsd-native-shutdown/`.
