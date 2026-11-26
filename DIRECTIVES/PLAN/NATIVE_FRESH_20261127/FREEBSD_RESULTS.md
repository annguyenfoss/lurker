# FreeBSD fresh qualification checkpoint

This is an ordinary-route checkpoint, not a final complete host pass. Device
product integration, final frozen validation, package staging and reboot
persistence are still in progress. Old reports are not counted as fresh passes.
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
seven direct-format guards remain unavailable combinations; the four directory
failures are identified below. This is not 41 eligible passing combinations.

| Filesystem | GELI inner | VeraCrypt inner | PEFS backing | gocryptfs backing | CryFS backing |
| --- | --- | --- | --- | --- | --- |
| ext4 | Passed | Passed | Passed | Passed | Passed |
| UFS1 | Passed | Passed | Passed | Passed | Passed |
| UFS2 | Passed | Passed | Passed | Passed | Passed |
| HAMMER2 | Passed | Passed | Passed | Passed | Passed |
| NTFS | Fixed-sector native failure | Passed | Native backing failure | Passed | Product publication gap |
| exFAT | Passed | Passed | Native backing failure | Passed | Product publication gap |
| ZFS | Passed | Passed | Passed | Passed | Passed |
| Btrfs | Guard/native append failure | Guard/native append failure | Ineligible backing | Ineligible backing | Ineligible backing |
| XFS | Guard/native append failure | Guard/native append failure | Ineligible backing | Ineligible backing | Ineligible backing |
| APFS | Guard/no native write route | Guard/no native write route | Ineligible backing | Ineligible backing | Ineligible backing |

Fresh native package controls created XFS and Btrfs, mounted through packaged
LKL, wrote payload, closed and reopened. Both then failed append with `EBADF`;
these are not Create + Read/Write passes. Corrected controls are in
`native-filesystems-1795725868`; preliminary setup failures are retained.
Native GELI/NTFS with the fixed 4096-byte encrypted sector timed out in
`mkntfs` after 90 seconds, consistent with the product guard; metadata/profile
was not weakened. Native PEFS over fresh NTFS failed a nested Bitcoin write
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
cells**; its final receipt reconciliation is pending. OS-specific hdiutil,
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

No owned crypto mappings or plaintext mounts remain at this checkpoint. The
scoped conformance nullfs mount and dedicated build pool remain intentional.
`vfs.usermount` was temporarily enabled for login-user FUSE cases and restored
to its original zero after each batch. No orderly reboot has yet been counted.

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
