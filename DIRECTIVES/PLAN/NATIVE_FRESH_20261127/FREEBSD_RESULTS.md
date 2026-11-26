# FreeBSD fresh qualification checkpoint

This is an ordinary-route checkpoint, not a final complete host pass. Final container/device matrices and reboot
persistence are still in progress. Product integration, desktop callbacks and
canonical package installation have completed the gates recorded below. Old reports are not counted as fresh passes.
Private receipts are under `target/goal-validation/native-fresh-20261127/hosts/freebsd/`;
remote logs and preserved fixtures are under `/srv/lurker-fresh-20261127/`.

## Host and artifact provenance

FreeBSD's running kernel, installed kernel and userland are 15.1-RELEASE-p7,
`kern.osreldate=1501000`. Fresh `pkg update` and `pkg upgrade -y` found no
pending ports, kernel-module or pkgbase upgrades. PEFS's ABI annotation is
1501000; PEFS, HAMMER2 and FUSE modules loaded. Ordinary CMake, Ninja, Boost,
range-v3 and spdlog build prerequisites were installed for the native CryFS
build. Existing NTFS 2022.10.3, exfat-fuse 1.4.0 and LKL 4.16 packages were
checked against the configured repository; no newer ordinary packages appeared.

The original root pool was nearly full. A new 64 GiB qcow2 was attached to
`freebsd-core-01`, verified as `/dev/vtbd1`, and made into the separate
`lurkerfresh20261127` build/fixture pool. Old Cargo caches were preserved there
with symlinks at their original paths. Old fixtures were not removed. The
build disk remains necessary for those caches; it is not destructive test storage.

The exact baseline `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`, native run
`20261126T204256Z-a32c3772`, passed CLI/helper build, `cargo check --workspace`
and workspace tests: **122 passed, 0 failed, 11 ignored**. One Cargo worker was
used. Earlier failed/blocked attempts and an intentionally cancelled build
order are retained separately. Baseline staged CLI SHA-256:
`bccebf721ad35162c1b89c5e017190a59ea1bb767078f758a4e649da4c4ff28a`;
helper: `b809af5f824ae2eced807e9fd1effe35d1e894967a20153c71086439dae75553`.
Verified cached native private VeraCrypt 1.26.31 and gocryptfs 2.7.0 retain
source, patch, library and license identities. Their fresh runtime receipts
qualify administrator-owned staged binaries, not an installed package.

CryFS was added after a native upstream proof. Exact candidate
`1c62f1feb534cf484eb2343a83300d9100c3a79a`, native run
`20261126T212936Z-6872f321`, passed fresh CLI/helper build, workspace check
and **123 tests, 0 failed, 11 ignored**. CLI SHA-256:
`a47b8bdb3f6171b92918135af0f438de1d3f8966226cc6711040d34add4e90ac`;
helper: `5e65f245b20dbe00a5e0ddd1d2788976104e385d8dc4d82b069cfc2f88bc01e9`.

Private CryFS uses stable 1.0.3, pinned upstream commit
`5d0cc36785d93e12250696a3ffc928042f2e8d61`, with the existing common durability
patch. The native common factory built the administrator-staged engine with
SHA-256 `971e3202738195258f75a44a889036d424056f0a7545a7e0e0d3539dfaeb254d`.
Its version reports `1.0.3+0.g5d0cc36.dirty` because of the reviewed patch; this
is the stable pinned base, not an experimental 2.x build. Manifest retains
compiler/CMake/package inputs, four runtime package identities and 18 license
files. Native builder and stager focused checks passed (12 and 45 tests).

The first seven CryFS backing receipts had a manually supplied source-label
typo. The actual immutable runner/source manifests and binary hashes identify
the correct `1c62f1feb...` revision above. External receipts were corrected
with explicit `source_identity_correction`, preserving original JSON under
`provenance-originals/`; encrypted inner receipts were not rewritten. Future
launchers read the protected artifact identity and verify its CLI hash.

## Fresh container coverage and ordinary-route limitations

All initial container cases preceded device work. Shared payload harness base
is `c5feaec`, with identified private FreeBSD wrappers. Payloads are the actual
downloaded Bitcoin, Ethereum and Transformer PDFs plus text. Each passing case
created/formatted, opened a real native read/write mount, copied bytes, closed
normally, reopened, verified, appended text/created another file, closed,
reopened and checked final hashes before closing again.

There are **41 initial product attempts: 30 passes and 11 non-passes**. The
initial seven direct-format guards include the subsequently corrected GELI/NTFS
gap. Btrfs/XFS have newer positive native controls pending an eligibility
decision; APFS still lacks a qualified write route. Four directory failures are
identified below. This is not 41 eligible passing combinations.

| Filesystem | GELI inner | VeraCrypt inner | PEFS backing | gocryptfs backing | CryFS backing |
| --- | --- | --- | --- | --- | --- |
| ext4 | Passed | Passed | Passed | Passed | Passed |
| UFS1 | Passed | Passed | Passed | Passed | Passed |
| UFS2 | Passed | Passed | Passed | Passed | Passed |
| HAMMER2 | Passed | Passed | Passed | Passed | Passed |
| NTFS | Passed scoped retest | Passed | Native backing failure | Passed | Product publication gap |
| exFAT | Passed | Passed | Native backing failure | Passed | Product publication gap |
| ZFS | Passed | Passed | Passed | Passed | Passed |
| Btrfs | Product guard; native control now passes | Product guard; native control now passes | Policy decision pending | Policy decision pending | Policy decision pending |
| XFS | Product guard; native control now passes | Product guard; native control now passes | Policy decision pending | Policy decision pending | Policy decision pending |
| APFS | Guard/no native write route | Guard/no native write route | Ineligible backing | Ineligible backing | Ineligible backing |

Fresh native package controls created XFS and Btrfs, mounted through packaged
LKL, wrote payload, closed and reopened. Both then failed append with `EBADF`;
these initial controls are not Create + Read/Write passes. Later `direct_io`
controls described below supersede that driver assessment. Initial controls are in
`native-filesystems-1795725868`; preliminary setup failures are retained.
Initial GELI/NTFS formatting timed out with the native port's default cache;
that receipt and the original product guard are retained. A later 512-byte
sector control also timed out after 120 seconds. Scoped `truss` exposed an
infinite UBLIO retry: a 262144-byte read at the provider's last cache block
repeatedly returns 261632 bytes. The installed port's `README.FreeBSD`
documents `NTFS_USE_UBLIO=0`; using it passes the complete native physical
NTFS payload lifecycle with **unchanged 4096-byte AES-XTS-256 GELI**.
Shared commit `68b768c` sets this only for the exact trusted native `mkntfs`
and `ntfs-3g` paths after clearing the environment. With the owned guard
removed, product container (seven operations) and physical partition (14
operations) lifecycles both pass, including Auto and conformance guards.
This was an ordinary driver cache configuration gap, not an OS or sector-size
impossibility. These are scoped overlay retests; final frozen replay remains
pending. Native fixture is `native-geli4096-ntfs-noublio-1795731158`, product
container `lurker-fs-cli-miq3lut4`, physical `product-geli-preview-1795731313`
on `/dev/vtbd2p7`. Native PEFS over fresh NTFS failed a nested Bitcoin write
with `EBADF`; over exFAT it failed opening the nested payload with `ENOENT`.
These controls did not use Lurker's metadata symlink, so removing that symlink
would not establish a valid lifecycle.

Stable unmodified upstream CryFS, with the product's AES-256-GCM profile,
**passes the full fresh payload lifecycle on both NTFS and exFAT**. The bundled
engine fails initialization because the common durable publication patch uses
hard links: NTFS returns `EIO`, exFAT `ENOSYS`. This is a **product gap**, not an
upstream CryFS limitation. Matched control root is
`fixtures/publication-controls-1795730212`. Native exclusive rename preserved
an existing sentinel with `EEXIST`, but publication to an absent name and all
eight concurrent contenders returned `EOPNOTSUPP`; directory fsync succeeded.

The FreeBSD 15.1 `fuse_vnop_rename` implementation explicitly rejects every
nonzero `ap->a_flags` before dispatching to userspace. Both installed drivers
link FUSE 2; FUSE 3.18.3 is also installed, but a library-only rebuild cannot
supply the missing kernel operation. exfat-fuse 1.4.0's FUSE 3 rename callback
also ignores its flags. A correct fix would need native kernel FUSE RENAME2
plumbing and driver no-clobber support, not a check followed by ordinary
rename, a partially visible target, or weakened durability. Custom kernel
work is deferred by the round instructions until the complete ordinary-route
assessment is reported. No such custom build has started.

The private `cases/applicability.json` enumerates all **130 method/filesystem
cells**, with initial receipts and scoped retests. GELI/NTFS now raises the
current successful pair count to 31; the initial 41 attempts remain 30 passes
and 11 non-passes. Final frozen receipt reconciliation is pending. OS-specific hdiutil,
diskutil, CGD and softraid methods are not FreeBSD providers. No eligible native
LUKS1/LUKS2 read/write implementation has been established: libluksde is an
experimental read-only LUKS1 reader and explicitly excludes LUKS2. libfsapfs is
an experimental read-only APFS reader. Cryptomator's current JNI FUSE provider
has Linux/macOS/Windows ABI layouts without a FreeBSD provider; it needs a
native port, not merely installation of an ordinary missing package. These
are explicit current limitations, not compatibility-layer passes.

Representative baseline conformance completed **11 suites, 132 passed,
0 failed**: GELI password/keyfile with Auto, PEFS password/keyfile with Auto,
VeraCrypt AES/Serpent/Twofish each with password/keyfile and Auto, and login-user
gocryptfs. Cases include overwrite refusal, duplicate/alias acquisition, wrong
credentials, busy-close preservation and normal persistence. The existing
SQLite WAL workload ran only after reading the database instructions.
Ordinary-user staged helper GELI/UFS2 binary-key create/three-open lifecycle
also passed seven operations with native helper listings; an earlier private
JSON field typo is retained separately.

## Physical controls and product integration

A separate fresh 4 GiB qcow2 at
`/srv/build/lurker-native-fresh-20261127/freebsd/device.qcow2` is attached to
`freebsd-core-01` as vdc, guest `/dev/vtbd2`, serial `lurker-fresh-fb-data`.
Size 4294967296, serial, topology and idle state were verified before any
partition or format command. GPT partitions and failed fixtures are preserved.

Native controls passed full create/three-open payload/append/final-hash/normal
close cycles for GELI/UFS2 on `/dev/vtbd2p1` and private VeraCrypt/UFS2 on
`/dev/vtbd2p2`. They are native controls, not product passes.

Owned adapter commits `ff9f312` and `3130c21` add immutable GEOM inspection,
separate idle/system/boot/swap protection, physical GELI operations and
recovery while retaining vnode-backed md identity. Active identity uses stat,
rdev, GEOM geometry, partition UUID, disk serial, offset and sector checks
without opening an exclusively attached raw provider. Native scoped checks
passed 13 tests with two explicit privileged ignores; the root-owned ignored
test then passed real active-GELI identity, backing and mapping verification,
busy rejection, normal detach and system-disk guards.

Shared product integration preview (base `3130c21` plus hashed root-owned
source overlays) built CLI/helper, passed core 65/0/11 and CLI 19/0 tests.
Fresh `/dev/vtbd2p4` UFS2 product lifecycle passed **14 CLI operations**, including
payload persistence, explicit/Auto opens, wrong binary key, duplicate open,
busy-close preservation, no-force refusal, system/build-pool disk refusal and
partitioned-whole-disk refusal. Preview fixture is
`product-geli-preview-1795730659`. Earlier p3 creation succeeded but the private
wrapper called a function absent from its older harness; that failed receipt
and its closed partition are retained. Full frozen product/device validation
and desktop physical-source exposure remain pending.

After that preview, owned crypto mappings and plaintext mounts had closed. The
scoped conformance nullfs mount and dedicated build pool remain intentional.
Each completed user-FUSE batch restored `vfs.usermount` to its original zero.
The newer final matrix below is actively using its own fixtures; final cleanup
and orderly reboot persistence have not yet been counted.

## Final ordinary candidate and remaining native gaps

Exact source `d601bc5c4d2af95ecf24145829d7c282a8284791`, native run
`20261126T232638Z-7ffd0e78`, built all three products and passed workspace check
and **139 tests, zero failures, 13 explicit privileged ignores**. The consistent
profile is dev with debug information and incremental compilation disabled.
Canonical staging under umask 077, native package generation, two install/check/
remove cycles, the final reinstall, file hashes and root-owned public modes all
passed. Archive SHA-256 is
`81d7816a0117720b37542d7655eef4de0105148f634158a29dcf8f87256c7f05`;
native package is
`1c953abd084185482e21793770476b9fd414ec653fe89789ff584308516d4a45`.
Installed products are:

- CLI: `7844aaf653305ad696aa0ba2272d9c705f6abd484de8787c268da0835267e751`
- helper: `38c4682c6a3c4fcf75ae59b2b50d3a8a403bfef20a75dfe49d8d19d9c90e7588`
- desktop: `400bb6e30cc218221883b34501ea8b75b227a38a6eded9455f21785f33763f16`

FUSE, including its userspace library, remains a host prerequisite. The final
VeraCrypt SDK contains only private libiconv. Its engine bytes remain
`92edaf1363133aab8844ddd2c01656174967e260cd046569bd2dcbac64eec8b4`.
The host ABI is `libfuse.so.2`; `/usr/local/lib/libfuse.so.2` resolves to the
administrator-owned versioned provider. Producer hashes record build inputs;
compatible administrator updates are accepted after current ownership,
symlink-chain, SONAME, engine-NEEDED and actual cleared-environment loader
resolution checks. The first native FreeBSD runtime test caught an alias-versus-
canonical-path comparison bug in `c7f7092`; shared fix `d601bc5` passes all three
root runtime tests. The independent active-GELI/read-only system-device guard
test also passes. Failed intermediate package/runtime receipts remain retained.

The installed desktop opened a real winit/software-rendered native X11 window
as UID 1300 with no loader overrides. Its fresh frame was visually inspected.
Earlier native helper-backed GUI callbacks passed GELI and physical VeraCrypt;
the physical create/open fields and actual 120% scrolling to both action
buttons passed. Root commits `ca14341` and `2e93f78`, and owned `507f9ea`, retain
those UI changes. The final package window check supplements those callbacks.

The fresh final device is the independent 8 GiB qcow2
`/srv/build/lurker-native-fresh-20261127/freebsd/device-final.qcow2`, domain
`freebsd-core-01`, target vdd, guest `/dev/vtbd3`, serial `lurker-fb-final-data`.
Native size/serial/idle state were verified before creating 17 retained GPT
partitions. Final container and device batches use the same installed binaries;
all seven GELI physical filesystem rows have passed. VeraCrypt, directory
backings, conformance, helper and reboot batches remain in progress. This is
not a final matrix-pass count.

Ordinary NTFS source assessment also built Tuxera 2026.9.28 natively in a
separate root-owned host-tool prefix with the official FreeBSD aligned-I/O
patches. The configured package remains 2022.10.3; the ordinary source control
preserves its own logs and binary hashes. Unmodified upstream CryFS passes the
full payload on the newer driver, while the bundled durability patch still
fails hard-link publication with EIO and kernel rename-no-replace still returns
EOPNOTSUPP. This newer ordinary driver does not solve the product gap. No custom
kernel module or weakened publication/durability workaround has been used.

Additional native XFS and Btrfs controls now pass all three payload opens with
host `lklfuse -o direct_io`. Both also pass through GELI with AES-XTS-256,
PBKDF2 iterations 600000 and 512-byte sectors; all closes and native detaches are
normal. Controls are `native-lkl-directio-v2-1795733146` and
`native-geli512-lkl-directio-1795735582`. The installed official port is
`fusefs-lkl-4.16.g20180628_5`, GPLv2 source
`86dd3afb590eccc1903611bdaa8bac87757eb80d`. This is a native FreeBSD ELF process
statically embedding Linux Kernel Library code with host threads, file I/O and
host FUSE; it has no VM or network layer. Upstream is active, with inspected
commit `d0f76a77e8c2ca68bd7d96190b8da09040eab072` dated 2026-09-28. Product
mount/geometry changes remain held pending the user's explicit eligibility
preference for this separately installed host route. No positive native control
has been relabeled as a product pass, and no name-based impossibility is claimed.

## Primary ordinary-route sources inspected this round

- https://docs.freebsd.org/en/books/handbook/filesystems/
- https://cgit.freebsd.org/ports/plain/security/veracrypt/Makefile
- https://github.com/kusumi/freebsd_hammer2
- https://raw.githubusercontent.com/libyal/libfsapfs/main/README
- https://raw.githubusercontent.com/libyal/libluksde/main/README
- https://raw.githubusercontent.com/cryptomator/jfuse/develop/README.md
- https://raw.githubusercontent.com/cryptomator/fuse-nio-adapter/5.1.0/src/main/java/org/cryptomator/frontend/fuse/mount/LinuxFuseMountProvider.java
- https://raw.githubusercontent.com/cryfs/cryfs/1.0.3/CMakeLists.txt
- https://raw.githubusercontent.com/freebsd/freebsd-src/releng/15.1/sys/fs/fuse/fuse_vnops.c
- https://raw.githubusercontent.com/freebsd/freebsd-src/releng/15.1/sys/kern/vfs_syscalls.c
- https://raw.githubusercontent.com/tuxera/ntfs-3g/2022.10.3/src/ntfs-3g.c
- https://raw.githubusercontent.com/relan/exfat/v1.4.0/fuse/main.c
- https://raw.githubusercontent.com/freebsd/freebsd-ports/main/filesystems/ntfs/files/README.FreeBSD

Additional primary source inputs:

- https://download.tuxera.com/opensource/ntfs-3g_ntfsprogs-2026.9.28.tgz
- https://github.com/lkl/linux
- https://api.github.com/repos/lkl/linux/commits/master
