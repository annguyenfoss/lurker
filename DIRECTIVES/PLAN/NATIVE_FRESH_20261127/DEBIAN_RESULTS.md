# Debian fresh native qualification — in progress

This is the new 2026-11-27 round, not a carry-forward of earlier passes.
Private evidence is under `target/goal-validation/native-fresh-20261127/hosts/debian/`.
The atomic checkpoint there identifies the live job and next work.

## Fresh host and build observations

- Debian 13.8, running security kernel `6.12.118+deb13-cloud-amd64`, initial boot
  `76968973-6ed7-4502-a634-a65e0905e301`. APT index refresh and supported full
  upgrade completed successfully: zero packages upgraded and no held packages.
  ACL/attribute tools and available distribution directory-engine packages were
  installed; product tests use separately verified private pinned engines.
- Official backports kernel `7.3.4+deb13-cloud-amd64` and matching headers were
  already installed. GRUB currently selects 6.12.118. Kernel cohorts matter:
  current HAMMER2 requires 7.3, while current stable OpenZFS documents support
  through 7.2. No custom kernel was built for this round.
- Fresh exact source `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d` passed remote
  `cargo check --workspace`, `cargo test --workspace --no-fail-fast` and
  `cargo build --workspace`, with one worker. Runner:
  `20261126T203501Z-01a21f5f`; test summary 147 passed, zero failed, 13 ignored.
- CLI SHA-256:
  `04200941d7744221ace7bb52f79945a3fa72b3ee364b6438373f4be22f377d0c`.
  Helper SHA-256:
  `d5a7617c89ad64905afd5076792efb323cdbea23a5a8dc76c5f085adfb3feb7f`.
  Binaries are copied into the new administrator-controlled private prefix
  `/usr/local/lib/lurker-native-fresh-20261127-debian`; these are staged-binary
  results, not installed-package qualification.
- CryFS 1.0.3 with both current durability patches, gocryptfs 2.7.0 and the
  VeraCrypt creator passed current-source input/provenance checks. Cryptomator
  with current Linux patches was copied read-only from Arch's verified cache
  and independently verified against the same source on Debian. No earlier
  runtime receipt counts as a new pass.

## Initial batch findings

The initial ten-filesystem × three disk-encryption sweep used fresh
independent fixtures. Operationally completed cases mounted a real filesystem, wrote
the three downloaded PDFs and text, closes normally, reopens with Auto, verifies,
appends and creates a second file, then closes/reopens and verifies again.
Receipts retain hashes, mount identities, permissions/ACL/reflink probes and logs.

Native APFS revealed a shared product defect before storage mutation:
canonicalizing the `modinfo` and `modprobe` symlinks to `kmod` loses their
multicall invocation name. Native `modinfo apfs` and `modprobe apfs` succeed.
The shared repair is committed; final-candidate validation remains pending.
An inherited writable directory mode in the newly copied private prefix caused
the first ext4/VeraCrypt attempt to fail its trust check. The owned prefix mode
was corrected; that failed attempt also remains for a separate retest.

A second shared defect invalidates the selected-ext4 baseline qualification:
canonical `mkfs.ext4` becomes `mke2fs`, whose default creates ext2. Both initial
LUKS/ext4 mount receipts report `ext2`, despite completing their byte lifecycle.
The ext4 directory cases in this Debian-specific initial recipe also used those
LUKS-created ext2 backings. These are wrong-filesystem failures, not ext4 passes.
The Linux owner is batching explicit ext4 format selection with the kmod repair.
The Debian driver now rejects an unexpected native mount type before payload
access; its rejection and positive ext4 checks ran on the remote host. The
shared raw-backing harness invokes `mkfs.ext4` directly and is a separate route.

The initial 30 disk-container operations finished: 22 completed their lifecycle
and eight failed operations. After filesystem-identity review, only 20 qualify
for their requested filesystem; two completed on the wrong filesystem. The
initial directory sweep also finished with 22 operational completions and eight
operation failures; two completions used the wrong ext4 backing. Cryptomator has
passed Btrfs; its initial ext4 attempt failed because my raw image copy retained
private Java-library modes after root ownership. The owned staged copy has been
normalized to ordinary staged executable/data permissions. The failure is retained
for a fresh retest and is not attributed to the product or upstream engine.

UFS1 succeeds through the existing matching upstream module compiled with
`CONFIG_UFS_FS_WRITE`. This is an explicit configuration prerequisite, not a
stock Debian capability. LUKS2/UFS1 is blocked by the fixed default logical-sector
profile; no profile has been weakened. UFS2 uses separately installed native
FUSE `fuse-ufs` 0.6.0. Its upstream describes experimental writes and no ACLs;
successful small lifecycle results do not remove that upstream limitation.

## Native prerequisites and portable CryFS donor

Fresh source builds installed APFS v0.3.22 for official 6.12 and HAMMER2 for
official 7.3, preserving original driver bytes. No kernel source or configuration
was changed. The host now runs `7.3.4+deb13-cloud-amd64`. Its native `ntfs`
read/write driver and explicit native HAMMER2 mounting passed fresh raw
three-PDF/text write/close/reopen/append/verify controls. Native ntfs3 supports
exclusive rename, while NTFS-3G rejects it. exFAT supports exclusive rename but
not hardlinks. These are native controls, separately identified from product passes.

The optional fuse-ufs prerequisite was rebuilt from upstream
`ee5c4a447da98220a11edb69a225d51379afc4fb` with the recorded namespace and barrier
patches. Debian passed 28 rufs tests, seven native rename/collision controls,
full raw payload reopens, forwarded file/directory fsync traces, and injected
file/directory EIO with successful retries. Driver SHA-256:
`b8518fe1e4f4b664929232bfca6aa7c39cfdd8ff458e39e335f592960fef1a49`.
Cryptomator and new CryFS product payloads subsequently passed on UFS2; CryFS
passed again through the mapped backing after the barrier repair. This is a
patched experimental host prerequisite, not a bundled filesystem or stock
Debian capability.

Debian built the shared Linux CryFS donor as login user, one worker, from exact
Lurker inputs `195b56be87b38361aaa45a085954e794338ba09a`, stable upstream 1.0.3
and the three inventoried Linux patches. The copied Conan cache passed integrity
checks. Current stager, all 16 license/notice files, ELF and provider checks
passed, with no RPATH/RUNPATH. Fresh exFAT and patched-UFS2 product payloads passed.

- Binary SHA-256: `2ec7b6f5bd7b999b81750fec75ffb3ca13174f4643d2326ba250143e9f444a14`.
- Shared closure: `target/goal-validation/native-fresh-20261127/hosts/debian/build/cryfs-linux-debian-195b56b.tar.gz`.
- Archive SHA-256: `fa5e8f14e008b0df5bb5faff6247bbac0810af4e04c32dc1d074a5f9e5744db2`.
- Built on glibc 2.41; required symbols GLIBC 2.38 / GLIBCXX 3.4.32.
  Manifest, library-provider audit and runtime receipts accompany the archive;
  all five other Linux owners received the handoff for independent validation.

Three Btrfs-backed directory vaults were verified and intentionally left mounted
before the orderly boot from `76968973-6ed7-4502-a634-a65e0905e301` to
`c70701ba-a2bf-49f1-bb3c-82dd225b5f3c`. After mounting the identified fixture
storage, all three reopened, matched all five hashes and closed normally.
This is baseline-artifact reboot evidence; candidate validation remains pending.
The preexisting default 6.12 boot choice was preserved.

## Owned storage and remaining work

The system disk and old work are preserved. New fixture storage is
`/srv/build/lurker-native-fresh-20261127/debian/fixture-storage.qcow2`, attached
to domain `debian-core-01`. Guest identity was checked as a new 32 GiB disk with
observed virtio serial `lurker-fresh-debian-` before formatting. This owned disk
was subsequently grown online to 64 GiB and then 128 GiB, with serial, source, size and mount
checked before resizing its ext4 filesystem. It is mounted at
`/var/tmp/lurker-native-fresh-20261127-debian-storage`. Closed current-round
fixtures are retained there with their original evidence paths preserved.
The later destructive device matrix will use a separately identified scratch disk.

The first Linux candidate was `ca7d2137fca741008648360e98acc06ce559de3d`.
Remote workspace job `20261126T221438Z-c632c5b9` passed check, tests and build:
156 tests passed, zero failed, 15 ignored. Final CLI/helper/desktop SHA-256s are
`1e5e454037a195fc2042ca1315cf25e186d8b2932a2862ce4ae5146e33eea125`,
`36180396cd9671e133096d6f946a3462b8acc71d907052e1f7ac3aac42624a3a`,
and `e2bd595384fa78ba9a9c848bd746a1a52c3ee3a6d719e780774c1bc418c33496`.

The independent `7c48a25` stager validated the native VeraCrypt creator, current
gocryptfs/Cryptomator and the qualified new CryFS donor against frozen inputs.
A Cargo JSON log records actual dependency notices. Stage archive SHA-256 is
`ceeac4577f21df60ac0e51ecf88256ceb96d3b85feba502cdd96965f1e3e07af`.
The native-only Linux packager now accepts either verified VeraCrypt artifact
route and tests only the routes included. Debian passed 17 package/archive/policy
tests and built/installed the new `0.1.0+fresh20261127` package; package SHA-256
is `997c548efecc0f03cc0aede18b1371c625ada78ccb76c0cc59ac87bd9ba3fc9e`.
Installed binary hashes match the workspace artifacts. This is a prepared-host
installation; clean-host dependency absence is not claimed.

That seven-filesystem batch completed 21 disk-container passes and 19 folder
passes. Two HAMMER2 folder creates and the installed Cryptomator conformance
close collided with my concurrently scheduled directory tests; the product's
operation lock rejected them. Failed receipts/backing images remain retained.
The Cryptomator mount closed normally after serialization. These are test
scheduling failures, not filesystem-support failures. The installed native cipher
suite passed all nine backend/cipher suites (108 checks), keyfile-negative suites
passed all three methods, and normal-user helper full payloads passed all 21
strict JSON responses. The initial candidate is superseded below.

The follow-up candidate is `576505d554d798422840a06322dfa93b899365bc`, including
the desktop changes and the read-only-superblock correction. Debian's owned
mount guard independently passed writable ext4 and rejected an actual
VFS-rw/superblock-ro ext4 bind view. Exact workspace run
`20261126T223904Z-f54a075f` passed check/test/build: 159 passed, zero failed,
15 ignored. Current-source staging revalidated all native private engines and
Rust notices; the new package installed with exact workspace hashes.

- CLI: `6211f7e19af9920b2dd4c94c86c92cc3da230247db26bf97d835ad693f0d0a47`.
- Helper: `d90f43bc97bc334a0eeb44288354c909777fc17e3ba408dd91a4d1cc647e20ab`.
- Desktop: `f057ea2dd770be1be20c60d77c9d5067d15796fe4867930158a87a65b41f94ca`.
- Stage archive: `c7272303c7927453f196b81a7d37a89f5051ed2c7d2787510f587144ca7d43d7`.
- Installed Debian package `0.1.0+fresh20261127.576505d`:
  `24da9205fe18dae81caea5f75d5a2d855e3b65ae179689b72088a9ff85d0cc51`.

The follow-up desktop rendered as UID 1300 in an authenticated private X11
session and cleaned up its owned resources. Final storage and disk-only
conformance/helper batches are running independently; directory gates are
serialized. All final container and physical cases will use this follow-up.
Remaining: complete repaired candidate containers in both kernel cohorts; cipher and credential/conformance
cases; candidate reboot persistence; new scratch-device cases; final validation
and cleanup
and reconciliation of all 130 applicability cells. No complete-host pass is claimed.

Current upstream evidence is retained with fetch times and hashes. Primary routes:

https://github.com/linux-apfs/linux-apfs-rw

https://github.com/jdmanring/linux_hammer2

https://github.com/realchonk/fuse-ufs

https://github.com/openzfs/zfs/releases

https://github.com/cryfs/cryfs/releases

https://github.com/rfjakob/gocryptfs/releases
