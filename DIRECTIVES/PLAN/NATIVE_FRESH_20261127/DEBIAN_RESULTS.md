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

The initial ten-filesystem × three disk-encryption sweep is running with fresh
independent fixtures. Every successful case mounts a real filesystem, writes
the three downloaded PDFs and text, closes normally, reopens with Auto, verifies,
appends and creates a second file, then closes/reopens and verifies again.
Receipts retain hashes, mount identities, permissions/ACL/reflink probes and logs.

Native APFS revealed a shared product defect before storage mutation:
canonicalizing the `modinfo` and `modprobe` symlinks to `kmod` loses their
multicall invocation name. Native `modinfo apfs` and `modprobe apfs` succeed.
The Linux owner is preparing the shared repair; initial failed receipts remain.
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
initial directory sweep is still running. Current Linux-patched Cryptomator has
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

## Owned storage and remaining work

The system disk and old work are preserved. New fixture storage is
`/srv/build/lurker-native-fresh-20261127/debian/fixture-storage.qcow2`, attached
to domain `debian-core-01`. Guest identity was checked as a new 32 GiB disk with
observed virtio serial `lurker-fresh-debian-` before formatting. This owned disk
was subsequently grown online to 64 GiB, with serial, source, size and mount
checked before resizing its ext4 filesystem. It is mounted at
`/var/tmp/lurker-native-fresh-20261127-debian-storage`. Closed current-round
fixtures are retained there with their original evidence paths preserved.
The later destructive device matrix will use a separately identified scratch disk.

Pending: finish the initial container sweep; all eligible directory/backing
cases; native failure controls; supported-kernel HAMMER2 assessment; cipher and
credential/conformance cases; ordinary reboot persistence; new scratch-device
cases; affected retests and complete frozen-candidate validation; final cleanup
and reconciliation of all 130 applicability cells. No complete-host pass is claimed.

Current upstream evidence is retained with fetch times and hashes. Primary routes:

https://github.com/linux-apfs/linux-apfs-rw

https://github.com/jdmanring/linux_hammer2

https://github.com/realchonk/fuse-ufs

https://github.com/openzfs/zfs/releases

https://github.com/cryfs/cryfs/releases

https://github.com/rfjakob/gocryptfs/releases
