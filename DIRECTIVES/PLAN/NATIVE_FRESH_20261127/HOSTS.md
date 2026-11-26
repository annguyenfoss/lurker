# Per-host handoffs

Read BRIEF.md and required project directives before acting. One host owner runs
updates, build, container matrix, scratch-device tests and final validation in order.
Each owner records concise milestones in HOST_RESULTS.md (HOST replaced by key)
and private evidence under hosts/HOST/. All source changes need atomic commits.

Common build recipe: after host updates/preflight, freeze the current commit and
use scripts/remote-test.py with the inventory host key and suite workspace, or a
reviewed equivalent exact Git archive job with existing managed toolchain/caches.
Inspect the recipe before use. Build login-user binaries; install a reviewed copy
and dependencies into an administrator-controlled private prefix for elevated
runtime tests. Test the real CLI and helper; preserve hashes and source identities.
Reuse inspected scripts/test-filesystem-cli.py, test-mounted-payload.py and native
conformance suites only when they actually cover the planned cases. The small
filesystem harness currently generates its own PDF, so add/use the downloaded
common payload separately; that harness alone does not fulfill this round.
For VM block tests the host owner may attach its own new qcow2 through virsh
-c qemu:///system, selecting a free target after current domblklist inspection.
Record a unique serial lurker-fresh-HOST and source path under a round-specific
/srv/build/lurker-native-fresh-20261127/ directory. No old disks are expendable.
Return live PID/run/session handles with checkpoint paths when work remains.
Do not stop at research/preflight: perform authorized updates and native tests.

## archlinux — Arch Linux

- Inventory key: `archlinux`; SSH: `adjutant@archlinux-core-01`.
- Update approach: pacman full synchronized update; matching running kernel and modules after orderly reboot.
- Source ownership: Linux shared owner: crates/lurker-core/src/linux*.rs and Linux-only scripts. Coordinate workflow.rs/system.rs/model.rs and shared harnesses with coordinator.
- Host-specific assessment: Probe stock ext4/Btrfs/XFS, NTFS/exFAT, available APFS/UFS/HAMMER2/ZFS routes independently; assess installed third-party modules against running kernel. LUKS1/LUKS2/VeraCrypt plus directory engines are primary native controls. Inspect the existing LKL route but do not count it as an OS-native filesystem-driver pass or silently remove existing functionality.
- Remote job root: `/tmp/lurker-native-fresh-20261127-archlinux`.
- Local evidence: `target/goal-validation/native-fresh-20261127/hosts/archlinux/`.
- Tracked report: `ARCHLINUX_RESULTS.md` in this directory.
- Done means updated-host evidence, 10 × 13 applicability ledger, actual applicable
  container and device receipts, final candidate validation, clean owned resources
  and a report distinguishing product gaps, native limitations and pending work.

## debian — Debian

- Inventory key: `debian`; SSH: `adjutant@debian-core-01`.
- Update approach: apt index refresh and full supported upgrade; matching kernel/modules; inspect held packages.
- Source ownership: Own Debian-specific recipe/report only; propose shared Linux fixes to Arch owner.
- Host-specific assessment: Probe distribution and optional backports/package availability for every filesystem. Do not infer support from Arch. Use native LUKS1/LUKS2/VeraCrypt controls and directory engines on eligible backing filesystems.
- Remote job root: `/tmp/lurker-native-fresh-20261127-debian`.
- Local evidence: `target/goal-validation/native-fresh-20261127/hosts/debian/`.
- Tracked report: `DEBIAN_RESULTS.md` in this directory.
- Done means updated-host evidence, 10 × 13 applicability ledger, actual applicable
  container and device receipts, final candidate validation, clean owned resources
  and a report distinguishing product gaps, native limitations and pending work.

## ubuntu — Ubuntu

- Inventory key: `ubuntu`; SSH: `adjutant@ubuntu-core-01`.
- Update approach: apt index refresh and full supported upgrade; matching kernel/modules; inspect held packages.
- Source ownership: Own Ubuntu-specific recipe/report only; propose shared Linux fixes to Arch owner.
- Host-specific assessment: Probe stock and supported extra modules, FUSE routes and ZFS kernel matching; cover all ten rows independently with LUKS1/LUKS2/VeraCrypt and directory engines.
- Remote job root: `/tmp/lurker-native-fresh-20261127-ubuntu`.
- Local evidence: `target/goal-validation/native-fresh-20261127/hosts/ubuntu/`.
- Tracked report: `UBUNTU_RESULTS.md` in this directory.
- Done means updated-host evidence, 10 × 13 applicability ledger, actual applicable
  container and device receipts, final candidate validation, clean owned resources
  and a report distinguishing product gaps, native limitations and pending work.

## fedora — Fedora

- Inventory key: `fedora`; SSH: `adjutant@fedora-core-01`.
- Update approach: dnf supported distribution/package/kernel upgrade; inspect reboot requirement and native module availability.
- Source ownership: Own Fedora-specific recipe/report only; propose shared Linux fixes to Arch owner.
- Host-specific assessment: Check SELinux and FUSE/module prerequisites using scoped native settings; record needed configuration rather than broadly disabling protections. Cover all ten rows with native controls and applicable encryption.
- Remote job root: `/tmp/lurker-native-fresh-20261127-fedora`.
- Local evidence: `target/goal-validation/native-fresh-20261127/hosts/fedora/`.
- Tracked report: `FEDORA_RESULTS.md` in this directory.
- Done means updated-host evidence, 10 × 13 applicability ledger, actual applicable
  container and device receipts, final candidate validation, clean owned resources
  and a report distinguishing product gaps, native limitations and pending work.

## opensuse — openSUSE

- Inventory key: `opensuse`; SSH: `adjutant@opensuse-core-01`.
- Update approach: identify installed Leap/Tumbleweed release; use its supported zypper update/dist-upgrade procedure.
- Source ownership: Own openSUSE-specific recipe/report only; propose shared Linux fixes to Arch owner.
- Host-specific assessment: Record vendor/repository changes and module compatibility. Probe all ten rows and native encryption controls; no assumed equivalence with Fedora.
- Remote job root: `/tmp/lurker-native-fresh-20261127-opensuse`.
- Local evidence: `target/goal-validation/native-fresh-20261127/hosts/opensuse/`.
- Tracked report: `OPENSUSE_RESULTS.md` in this directory.
- Done means updated-host evidence, 10 × 13 applicability ledger, actual applicable
  container and device receipts, final candidate validation, clean owned resources
  and a report distinguishing product gaps, native limitations and pending work.

## gentoo — Gentoo

- Inventory key: `gentoo`; SSH: `adjutant@gentoo-core-01`.
- Update approach: synchronize supported repository and assess world/kernel update with resource-bounded jobs; record actual completed/pending updates.
- Source ownership: Own Gentoo-specific recipe/report only; propose shared Linux fixes to Arch owner.
- Host-specific assessment: Inspect package USE flags and installed kernel configuration. Ordinary package/kernel updates precede tests; customized kernel experiments remain deferred until all normal routes are assessed. Avoid starting a full custom kernel build now.
- Remote job root: `/tmp/lurker-native-fresh-20261127-gentoo`.
- Local evidence: `target/goal-validation/native-fresh-20261127/hosts/gentoo/`.
- Tracked report: `GENTOO_RESULTS.md` in this directory.
- Done means updated-host evidence, 10 × 13 applicability ledger, actual applicable
  container and device receipts, final candidate validation, clean owned resources
  and a report distinguishing product gaps, native limitations and pending work.

## freebsd — FreeBSD

- Inventory key: `freebsd`; SSH: `adjutant@freebsd-core-01`.
- Update approach: identify supported release; freebsd-update as applicable and pkg upgrades; match PEFS/module ABI.
- Source ownership: Own crates/lurker-core/src/bsd/freebsd.rs, its subtree, and FreeBSD-only scripts. Coordinator owns shared bsd.rs.
- Host-specific assessment: Assess native UFS1/UFS2/ZFS and eligible FUSE/package routes for seven other filesystems. Primary GELI, PEFS, VeraCrypt; research requested LUKS1/LUKS2 and directory engines rather than inheriting old exclusions. Keep directory-on-backing distinct from disk encryption.
- Remote job root: `/tmp/lurker-native-fresh-20261127-freebsd`.
- Local evidence: `target/goal-validation/native-fresh-20261127/hosts/freebsd/`.
- Tracked report: `FREEBSD_RESULTS.md` in this directory.
- Done means updated-host evidence, 10 × 13 applicability ledger, actual applicable
  container and device receipts, final candidate validation, clean owned resources
  and a report distinguishing product gaps, native limitations and pending work.

## openbsd — OpenBSD

- Inventory key: `openbsd`; SSH: `adjutant@openbsd-core-01`.
- Update approach: supported syspatch/sysupgrade/package update after release/architecture assessment; use explicit /bin/sh for privileged scripts.
- Source ownership: Own crates/lurker-core/src/bsd/openbsd.rs, its subtree and OpenBSD-only scripts; coordinator owns shared bsd.rs.
- Host-specific assessment: Primary softraid CRYPTO/RAID1C and VeraCrypt with native FFS; inspect every other filesystem and requested portable engine afresh. RAID1C device tests need two independent scratch members; coordinate an extra qcow2 with coordinator if needed.
- Remote job root: `/tmp/lurker-native-fresh-20261127-openbsd`.
- Local evidence: `target/goal-validation/native-fresh-20261127/hosts/openbsd/`.
- Tracked report: `OPENBSD_RESULTS.md` in this directory.
- Done means updated-host evidence, 10 × 13 applicability ledger, actual applicable
  container and device receipts, final candidate validation, clean owned resources
  and a report distinguishing product gaps, native limitations and pending work.

## netbsd — NetBSD

- Inventory key: `netbsd`; SSH: `adjutant@netbsd-core-01`.
- Update approach: supported base/security and pkgin/pkgsrc updates; record whether a newer running kernel/base is actually installed.
- Source ownership: Own crates/lurker-core/src/bsd/netbsd.rs, its subtree and NetBSD-only scripts; coordinator owns shared bsd.rs.
- Host-specific assessment: Primary CGD with required parameter sidecar; FFS WAPBL applies to FFS only. Probe native/FUSE UFS, exFAT, NTFS, ext4, HAMMER2, ZFS and remaining rows; research all requested portable crypto engines. Never treat an unsafe/no-flush route as a clean persistence pass.
- Remote job root: `/tmp/lurker-native-fresh-20261127-netbsd`.
- Local evidence: `target/goal-validation/native-fresh-20261127/hosts/netbsd/`.
- Tracked report: `NETBSD_RESULTS.md` in this directory.
- Done means updated-host evidence, 10 × 13 applicability ledger, actual applicable
  container and device receipts, final candidate validation, clean owned resources
  and a report distinguishing product gaps, native limitations and pending work.

## dragonflybsd — DragonFly BSD

- Inventory key: `dragonflybsd`; SSH: `adjutant@dragonflybsd-core-01`.
- Update approach: supported pkg/base updates; inspect current release and kernel/module ABI; use approved system Rust/Python as necessary.
- Source ownership: Own crates/lurker-core/src/bsd/dragonfly.rs, its subtree and DragonFly-only scripts; coordinator owns shared bsd.rs.
- Host-specific assessment: Primary LUKS1 and private VeraCrypt/tcplay with HAMMER2/UFS; probe every filesystem and requested engine afresh, including LUKS2 availability. Match payload alignment and device identities; never transplant another OS filesystem driver or count a network mount.
- Remote job root: `/tmp/lurker-native-fresh-20261127-dragonflybsd`.
- Local evidence: `target/goal-validation/native-fresh-20261127/hosts/dragonflybsd/`.
- Tracked report: `DRAGONFLYBSD_RESULTS.md` in this directory.
- Done means updated-host evidence, 10 × 13 applicability ledger, actual applicable
  container and device receipts, final candidate validation, clean owned resources
  and a report distinguishing product gaps, native limitations and pending work.

## macos — macOS on Buran

- Inventory key: `macos`; SSH: `adjutant@buran`.
- Update approach: softwareupdate inventory and eligible non-reboot updates; Homebrew/host-driver updates with scoped changes; never reboot Buran.
- Source ownership: Own crates/lurker-core/src/macos.rs, its subtree and macOS-only scripts; coordinate shared folder.rs/system.rs and build inputs.
- Host-specific assessment: Primary hdiutil Apple encrypted disk image, diskutil APFS encryption, VeraCrypt, gocryptfs/CryFS/Cryptomator. Reassess all ten filesystem rows and requested LUKS options with eligible free native software. Prefer FUSE-T where applicable; distinguish host-local existing encryption transport from an ineligible network-filesystem workaround. Identify Toshiba USB model/serial/external USB/size and current partitions before writes. Container matrix first, then only a few representative USB cases; do not touch internal storage or reboot to activate a driver.
- Remote job root: `/tmp/lurker-native-fresh-20261127-macos`.
- Local evidence: `target/goal-validation/native-fresh-20261127/hosts/macos/`.
- Tracked report: `MACOS_RESULTS.md` in this directory.
- Done means updated-host evidence, 10 × 13 applicability ledger, actual applicable
  container and device receipts, final candidate validation, clean owned resources
  and a report distinguishing product gaps, native limitations and pending work.
