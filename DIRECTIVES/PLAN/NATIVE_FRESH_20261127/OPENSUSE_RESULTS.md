# openSUSE fresh native qualification

Status: active; no prior pass receipt is carried forward.

- Product baseline: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`.
- Host: `adjutant@opensuse-core-01`, openSUSE Leap 16.1, running supported kernel `6.12.0-160100.9-default`, initial boot `3466a285-c815-45d6-a9c0-86f92b463485`.
- Updates: fresh `zypper refresh` and `zypper update` completed; diffutils, libruby3_4-3_4, ruby3.4 and shim updated. Subsequent `zypper list-updates` reports none. No repository or vendor changes made. The separately installed OBS kernel 7.3.6 remains an alternate; qualification starts on the supported running kernel.
- Native tools: cryptsetup, Btrfs, ext4, XFS, exFAT, NTFS and APFS formatters already current. Matching external UFS and OpenZFS 2.4.4 modules load on the running kernel. Fresh upstream APFS module `6cf6de8ad6fa4cb634a700b5d0436d73ee01942c` built against its installed headers and loaded using targeted `modprobe --allow-unsupported`; no global module policy changed.
- Independent engines: 137 cached private backend files freshly hash-verified against their package manifest, with VeraCrypt source/archive/creator patch, gocryptfs v2.7.0 and stable CryFS 1.0.3 inputs matching current pins. Cryptomator build inputs match current source except its non-build README; exact differences retained privately. These are reused build inputs, never reused runtime passes.
- Workspace check/test/build completed: run `20261126T203533Z-e9713258`, one Cargo worker, 147 tests passed, zero failed, 13 ignored. Native GUI callbacks remain ignored without a configured display fixture. CLI SHA-256 `4e0b9ea15b1c6ba93f2d14cee2996025dfd22ae0472a83fcb080562f5b533e2c`; helper `8b50ddbe4e8d17b8bce20fe910bacc0b93c8580f7c948b30a595a7de91dad52f`.
- Fresh native payload/feature controls passed ext4, Btrfs, XFS, APFS, NTFS and exFAT. Initial UFS1/UFS2/HAMMER2/ZFS control attempts hit a harness PATH issue: sudo omits `/usr/local` tools. Owned recipes now use explicit administrator-controlled paths; corrected controls remain queued. Product tool discovery already handles those paths.
- The baseline product matrix is running with harness `c5feaec` plus native-only driver `46c5100`. ext4, Btrfs, XFS, APFS and UFS2 each passed all three disk encryption methods; UFS1 passed LUKS1 and VeraCrypt. LUKS2/UFS1 is rejected before mutation by its existing sector-size guard. HAMMER2 has no matching module on the running 6.12 kernel; the ordinary installed OBS 7.3.6 candidate and fresh external module source `09c130bf8bac021b0bb6a8ed9c9685a5bd9bbbd0` are staged for later testing. Remaining rows are active, not complete.
- Shared harness checks executed remotely: current Transformer and legacy Lamport-name payloads passed write/append/verify on separate real tmpfs mounts (`cfcfd5f`); the bounded payload worker passed five unit tests plus actual mounted common-payload write/append/verify (tested input SHA-256 `97863693dbf1ab8cfced4e36809c9c9108ead265c20a3d9e10674ca668c3503d`). These smoke tests do not substitute for encrypted filesystem receipts.
- Live runtime pipeline: PID 27565, matrix child 41926, checkpoint `/var/tmp/lurker-native-fresh-20261127-opensuse/pipeline.json`. Follow-on PID 37663 waits for that matrix, then runs 18 AES/Serpent/Twofish × password/keyfile profiles with Auto reopen, explicit/Auto failure conformance for six backends, and real CLI/helper JSON checks. No scratch disk has been attached.

Private evidence: `target/goal-validation/native-fresh-20261127/hosts/opensuse/`.
Remote logs/source: `/tmp/lurker-native-fresh-20261127-opensuse/` and `/tmp/lurker-matrix-1300/runs/20261126T203533Z-e9713258-opensuse/`.
Persistent fresh fixtures: `/var/tmp/lurker-native-fresh-20261127-opensuse/`.
Administrator-owned runtime prefix: `/usr/local/lib/lurker-native-fresh-20261127-opensuse/`.

Next: finish the complete baseline container and directory-backing matrix, variation and failure cases; retest harness prerequisites and native UFS2 alternatives; verify ordinary reboot persistence and the installed 7.3 kernel/HAMMER2 candidate; then attach a uniquely identified new qcow2 scratch disk. Batch findings and run final frozen-candidate checks, device cases and cleanup. A 130-cell applicability ledger exists privately; final outcome reconciliation remains outstanding.

Current upstream eligibility references:

- https://github.com/linux-apfs/linux-apfs-rw — GPL-2.0; experimental native writable module, explicitly qualified as such.
- https://github.com/openzfs/zfs/releases/tag/zfs-2.4.4 — current eligible CDDL implementation; supported Linux kernel range includes 6.12.
- https://github.com/jdmanring/linux_hammer2 — BSD/GPL native Linux HAMMER2 module; upstream requires kernel 7.3 or newer.
- https://github.com/realchonk/fuse-ufs — open-source UFS2 FUSE route; native UFS route assessed separately.
- https://github.com/rfjakob/gocryptfs/releases/tag/v2.7.0
- https://github.com/cryfs/cryfs/releases/tag/1.0.3
