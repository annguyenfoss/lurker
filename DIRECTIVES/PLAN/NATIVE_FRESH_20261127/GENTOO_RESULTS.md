# Gentoo fresh native qualification — 2026-11-27

Status: in progress. No historical pass is carried into this round.

- Owner: Gentoo host agent; SSH inventory key `gentoo`, guest `gentoo-core-01`.
- Product baseline: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`; downloaded-payload harness revision `c5feaec` (Linux forwarding `88e279c`).
- Private receipts/checkpoint: `target/goal-validation/native-fresh-20261127/hosts/gentoo/`.
- Remote job root: `/tmp/lurker-native-fresh-20261127-gentoo`; persistent build/fixture root: `/srv/lurker-native-fresh-20261127-gentoo`.

Initial inventory found Gentoo 2.18, running `6.18.64-p1-gentoo-dist-bin`, boot ID `ff09bae8-da3f-48a9-b001-0486833afec3`, and working `sudo -n`. The XFS system disk had only 4.7 GiB free. A newly created 40 GiB build disk at `/srv/build/lurker-native-fresh-20261127/gentoo/build.qcow2` was attached through `qemu:///system`, identified in the guest as serial `lurker-build-gentoo`, and formatted/mounted only after verifying that identity. Existing disks and retained fixtures were preserved.

A supported Gentoo repository sync completed successfully with the signed manifest dated 2026-11-26 19:38:04 UTC. The world update plan contains zstd 1.6.0, sudo 1.9.17_p2-r1, the prebuilt 6.18.65 LTS distribution kernel, and its virtual dependency. All four updates completed (exit 0). An orderly reboot activated `6.18.65-gentoo-dist-bin`, boot ID `19e53769-eb90-43bb-aa2a-194e05100945`. The fresh login-user, one-worker release workspace build completed in 14m35s. `cargo check --workspace --locked` and `cargo test --workspace --locked` passed (147 tests passed, 0 failed, 13 ignored). The exact staged desktop also passed a fresh real X11/winit/software window probe as uid 1300.

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

The initial LTS sweep completed 61 execution rows: 47 raw harness passes and 14 failures, with no backing-mount cleanup errors. Raw harness counts are not the qualification count. Reconciliation excludes five supplementary LKL cases from native-driver acceptance and rejects the three privileged ext4/ext2 mismatches. The 130-cell initial ledger therefore has 40 native passes, 14 failures, 6 blocked directory-on-filesystem cells and 70 explicitly inapplicable OS-specific backend cells. This is an initial baseline assessment, not final candidate qualification.

Additional initial findings:

- NTFS/LUKS2 reported a successful open with no OS mount. The underlay guard refused writing. The runner then moved that fixture while its mapper remained open, contaminating a subsequent userspace VeraCrypt create. After matching the loop backing inode and device to the retained fixture, normal `lurker unmount` by the exact mapper closed it successfully. The mapping leak/readiness and failed-fixture movement findings were handed to the Linux owner; `cases/ntfs-cleanup.json` retains the identity and cleanup receipt.
- Directory gocryptfs, CryFS and Cryptomator passed on native ext4, Btrfs, XFS, APFS and ZFS backings. UFS2 passed gocryptfs but failed CryFS initialization (link-based durable publication: EPERM) and Cryptomator file I/O (EIO). NTFS-3G failed Cryptomator I/O; exFAT failed CryFS initialization. Those source images and diagnostic sessions are retained; native-driver and engine controls/retests remain pending.
- Both installed stock Gentoo configurations, 6.18.65 and 7.3.6, disable `CONFIG_UFS_FS_WRITE`. No custom kernel or UFS write module has been built this round.

The maintained official OpenZFS development source `3b6d727ddd8d6d005774605964f044f364ee6456` (2.4.99, Linux maximum 7.3) was built successfully for stock 7.3.6, with matching userland installed in the administrator-controlled `/opt/lurker-native-fresh-openzfs-2.4.99` prefix. Its RUNPATH and dependency resolution point to that private native-prerequisite closure. It has not yet been activated or qualified on the running kernel. This authorized beta assessment is justified by released 2.4.4's kernel compatibility limit; it remains separate from the completed 2.4.4/LTS observations. A host-only fuse-ufs prerequisite build uses upstream `ee5c4a447da98220a11edb69a225d51379afc4fb` and the Fedora owner's reviewed FUSE_RENAME2/no-clobber patch (SHA-256 `c9d71d6a6c24f69b0e1d64653983ad9995e9fd8e7b0b3409dc8deabdc352dc13`). The Gentoo build passed 28 rufs tests, seven atomic rename/collision controls on UFS2 and exFAT, and a fresh full three-paper UFS2 persistence control. It was installed only after confirming no active UFS sessions, with the original binary backed up. The new driver SHA-256 is `50d5e3e9275bb646917a5af33062c0d2bef52627f1e484293242850e73d4aab5`; the private upstream clone records the atomic change as `5bdeaffbdfff9740dd1e6614ef117b276f46b09e`. Filesystem drivers remain host prerequisites and are not bundled into Lurker.

The serialized initial conformance completed all 27 runs and 324 checks without failure: password and binary-keyfile-only AES/Serpent/Twofish on LUKS1, LUKS2 and privileged/login-user VeraCrypt, plus the fixed-profile directory engines. Auto discovery, wrong credentials, duplicate/aliased sources, busy close and normal durable reopen were exercised by the existing suites. Live next stages are additional desktop-helper/Slint callback checks, an identified mounted ordinary-reboot fixture, the stock 7.3 HAMMER2 and OpenZFS beta container controls, the frozen shared-fix candidate and affected/full retests, then a newly verified scratch device and reboot persistence. No scratch-device qualification is claimed yet.
