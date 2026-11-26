# Gentoo fresh native qualification — 2026-11-27

Status: in progress. No historical pass is carried into this round.

- Owner: Gentoo host agent; SSH inventory key `gentoo`, guest `gentoo-core-01`.
- Product baseline: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`; downloaded-payload harness revision `c5feaec` (Linux forwarding `88e279c`).
- Private receipts/checkpoint: `target/goal-validation/native-fresh-20261127/hosts/gentoo/`.
- Remote job root: `/tmp/lurker-native-fresh-20261127-gentoo`; persistent build/fixture root: `/srv/lurker-native-fresh-20261127-gentoo`.

Initial inventory found Gentoo 2.18, running `6.18.64-p1-gentoo-dist-bin`, boot ID `ff09bae8-da3f-48a9-b001-0486833afec3`, and working `sudo -n`. The XFS system disk had only 4.7 GiB free. A newly created 40 GiB build disk at `/srv/build/lurker-native-fresh-20261127/gentoo/build.qcow2` was attached through `qemu:///system`, identified in the guest as serial `lurker-build-gentoo`, and formatted/mounted only after verifying that identity. Existing disks and retained fixtures were preserved.

A supported Gentoo repository sync completed successfully with the signed manifest dated 2026-11-26 19:38:04 UTC. The world update plan contains zstd 1.6.0, sudo 1.9.17_p2-r1, the prebuilt 6.18.65 LTS distribution kernel, and its virtual dependency. All four updates completed (exit 0). An orderly reboot activated `6.18.65-gentoo-dist-bin`, boot ID `19e53769-eb90-43bb-aa2a-194e05100945`. The fresh login-user, one-worker release workspace build completed in 14m35s. `cargo check --workspace --locked` passed; workspace tests are running.

Installed optional filesystem facilities are discovery aids. They include OpenZFS 2.4.4, an external APFS module, fuse-ufs, hammer2-fuse, and native formatting tools. The stock distribution configuration disables UFS write support. An existing separately built UFS module must be identified separately and does not establish a stock-kernel writable UFS result. No customized kernel is being built.

The 10 × 13 applicability plan is recorded in private `cases/applicability.json`. Fresh product container receipts, credential/cipher conformance, newly identified scratch-device tests, reboot persistence and final frozen-candidate checks remain in progress. All current CLI/helper observations use an administrator-controlled private stage under `/opt/lurker-native-fresh-20261127-gentoo`, not an installed distribution package.

Fresh stage identities:

- Stage archive SHA-256: `7787dd3196a3dce42fe55f0707a88d018cec1f9cfbd78d6418b89d1a78eca27e`.
- CLI: `bab57e008003b47c6ab073f54d78f529ca4f02bd1ac1f13deb33422af5452150`.
- Helper: `9987c9d447ec99a003861d3212b9db3a424a7e824e9820777324708358262d8d`.
- Desktop: `b1285c967575a1bdcb7efff159057800f691b851cf64f5f40e85b8d023924d65`.

The stage reuses verified independent native encryption artifacts with unchanged Linux inputs; it does not claim they were rebuilt. The first stage attempt correctly rejected an old native envelope identity. A private copy was verified and recorded against the fresh application snapshot, preserving the original cache; the subsequent standard stage passed.

Fresh native controls (`controls-6.18/controls.json`) exercised the downloaded Bitcoin, Ethereum and Transformer PDFs plus text through write, normal close, reopen, byte comparison, append/new-file, close/reopen and comparison. ext4, Btrfs, XFS, APFS, UFS2, NTFS, exFAT and ZFS passed. UFS1 mounted read-only with the stock configuration and failed a write with EROFS. HAMMER2 had no matching 6.18 module. These are filesystem controls, not encrypted-product passes.

The fresh ordinary APFS module uses upstream `6cf6de8ad6fa4cb634a700b5d0436d73ee01942c` (GPL-2.0, commit 2026-11-19). OpenZFS 2.4.4 uses release tag `71a9f9578616a90c3c14bb59629fb4d31bfd68d1` (CDDL). Both were built for the updated stock LTS kernel. The ordinary HAMMER2 module `09c130bf8bac021b0bb6a8ed9c9685a5bd9bbbd0` (1.0.112, Dual BSD/GPL, commit 2026-11-26) was built separately for the already installed stock 7.3.6 kernel; its runtime matrix follows the first container sweep. Sources: https://github.com/linux-apfs/linux-apfs-rw https://github.com/openzfs/zfs/releases/tag/zfs-2.4.4 https://github.com/jdmanring/linux_hammer2

The initial encrypted ext4-selected lifecycles preserved the payload but mounted **ext2**. They fail the requested filesystem identity and do not qualify ext4. Direct native mkfs.ext4 controls mounted real ext4 and observed a journal. This finding was handed to the shared Linux owner as a canonical formatter/argv0 issue. The independent filesystem cases continue against the unchanged initial artifact before batching fixes.
