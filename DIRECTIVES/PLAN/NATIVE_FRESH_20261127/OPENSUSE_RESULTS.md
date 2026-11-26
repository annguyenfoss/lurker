# openSUSE fresh native qualification

Status: active; no prior pass receipt is carried forward.

- Product baseline: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`.
- Host: `adjutant@opensuse-core-01`, openSUSE Leap 16.1, running supported kernel `6.12.0-160100.9-default`, initial boot `3466a285-c815-45d6-a9c0-86f92b463485`.
- Updates: fresh `zypper refresh` and `zypper update` completed; diffutils, libruby3_4-3_4, ruby3.4 and shim updated. Subsequent `zypper list-updates` reports none. No repository or vendor changes made. The separately installed OBS kernel 7.3.6 remains an alternate; qualification starts on the supported running kernel.
- Native tools: cryptsetup, Btrfs, ext4, XFS, exFAT, NTFS and APFS formatters already current. Matching external UFS and OpenZFS 2.4.4 modules load on the running kernel. Fresh upstream APFS module `6cf6de8ad6fa4cb634a700b5d0436d73ee01942c` built against its installed headers and loaded using targeted `modprobe --allow-unsupported`; no global module policy changed.
- Independent engines: 137 cached private backend files freshly hash-verified against their package manifest, with VeraCrypt source/archive/creator patch, gocryptfs v2.7.0 and stable CryFS 1.0.3 inputs matching current pins. Cryptomator build inputs match current source except its non-build README; exact differences retained privately. These are reused build inputs, never reused runtime passes.
- Workspace check/test/build completed: run `20261126T203533Z-e9713258`, one Cargo worker, 147 tests passed, zero failed, 13 ignored. Native GUI callbacks remain ignored without a configured display fixture. CLI SHA-256 `4e0b9ea15b1c6ba93f2d14cee2996025dfd22ae0472a83fcb080562f5b533e2c`; helper `8b50ddbe4e8d17b8bce20fe910bacc0b93c8580f7c948b30a595a7de91dad52f`.
- Fresh native payload/feature controls passed all ten requested filesystems. The first six passed immediately; explicit trusted PATH corrected UFS/ZFS harness discovery. Native kernel UFS2 passed independently of FUSE, and all three directory engines passed their complete payload lifecycles on native UFS2 and ZFS. Generic UFS identity is accompanied by superblock/version evidence.
- Initial baseline matrix completed with 46 pass / 10 raw failures across 56 receipt rows (two backing setup failures each block three engines). Native UFS2/ZFS followups increase the distinct baseline union to 51/60: six HAMMER2 cells await the installed ordinary 7.3 kernel; Cryptomator/NTFS-3g and CryFS/exFAT remain affected; LUKS2/UFS1 is unavailable under the unchanged 4096-byte sector guard. Initial FUSE UFS2 failures and a ZFS harness PATH failure are preserved alongside successful native followups. This is baseline evidence, not final candidate acceptance.
- Cipher/credential variations passed 18/18: LUKS2/LUKS1/VeraCrypt × AES/Serpent/Twofish × password/binary keyfile, with Auto reopen and full downloaded payload checks. Explicit/Auto conformance passed 12/12 for six engines, including credential rejection, duplicate/aliased source and busy close cases.
- Real CLI/helper lifecycle checks passed LUKS2 and VeraCrypt. Fresh strong Unicode/space fixture credentials preserve host password policy (`4ab17e5`). A separate terminal ECHO/TCSAFLUSH race was corrected (`7d8827e`). The ordinary login user passed both CLI and helper scenarios using the coordinator-approved temporary sudo-fix overlay, while the host retained its original `sudo -n -v` rejection / `sudo -n true` success policy. Probe identity and binaries are recorded privately; the shared committed final candidate remains pending.
- Actual helper keyfile creation exposed cryptsetup status text before its JSON response. The baseline reboot recipe records a final-JSON-line parsing workaround; the final candidate must pass strict full-stdout JSON parsing. The original failing protocol receipt and ciphertext remain preserved.
- Ordinary one-time reboot into the already installed stock `7.3.6-lp160.4.gf857b30-default` completed; boot ID is `d342d68c-2b6a-41ff-ba72-fdfe9e40bd60`. Three real helper JSON binary-keyfile fixtures (LUKS2/Btrfs, LUKS1/ext4, VeraCrypt/exFAT) survived mounted-guest reboot, reopened through Auto, verified all five payload hashes, accepted a new fsynced write, and closed normally. No custom kernel was built.
- Matching ordinary external APFS, latest HAMMER2 `09c130bf8bac021b0bb6a8ed9c9685a5bd9bbbd0`, and OpenZFS master `3b6d727ddd8d6d005774605964f044f364ee6456` modules built against packaged 7.3 headers and loaded. Upstream ZFS META identifies 2.4.99 and supports 7.3; stable 2.4.4 supports only through 7.2. ZFS SPL companion and usrmerge staging were handled explicitly (`51e38b0`). Only root-controlled zpool/zfs symlinks were switched to matching new userland; old targets are recorded. Fresh HAMMER2, ZFS and stock NTFS3 native controls passed 3/3 on 7.3. The six baseline HAMMER2 product cases are running (PID 3835).
- Shared harness checks passed remotely: Transformer and legacy Lamport payload aliases; bounded payload worker failure/timeout tests; real tmpfs payload phases; actual-filesystem/rw mount gate including rejection before writes. These smoke checks do not substitute for encrypted filesystem receipts.
- No scratch disk is attached. All container attempts precede a new uniquely identified qcow2. Final exact-source build, complete matrix, updated CryFS donor verification, device cases and final reconciliation remain outstanding.

Private evidence: `target/goal-validation/native-fresh-20261127/hosts/opensuse/`.
Baseline source/job evidence was copied before reboot into `/var/tmp/lurker-native-fresh-20261127-opensuse/build/exact-baseline-job/`; transient `/tmp` paths are historical receipts only.
Persistent fresh fixtures: `/var/tmp/lurker-native-fresh-20261127-opensuse/`.
Administrator-owned runtime prefix: `/usr/local/lib/lurker-native-fresh-20261127-opensuse/`.

Next: finish baseline HAMMER2 cases; build the final frozen Linux candidate remotely; rerun all applicable container/directory cases, strict helper keyfile JSON and affected driver/backend flows; attach a fresh serial-fenced qcow2 for device cases; reconcile the 130-cell ledger and cleanup. The private ledger now records 51 baseline passes with source-specific initial/followup history and separate pending final-candidate status.

Current upstream eligibility references:

- https://github.com/linux-apfs/linux-apfs-rw — GPL-2.0; experimental native writable module, explicitly qualified as such.
- https://github.com/openzfs/zfs/releases/tag/zfs-2.4.4 — current eligible CDDL implementation; supported Linux kernel range includes 6.12.
- https://github.com/jdmanring/linux_hammer2 — BSD/GPL native Linux HAMMER2 module; upstream requires kernel 7.3 or newer.
- https://github.com/realchonk/fuse-ufs — open-source UFS2 FUSE route; native UFS route assessed separately.
- https://github.com/rfjakob/gocryptfs/releases/tag/v2.7.0
- https://github.com/cryfs/cryfs/releases/tag/1.0.3

Additional eligible ordinary-driver source: https://github.com/openzfs/zfs/blob/master/META (7.3-compatible current master; explicitly a beta driver, separate from a custom kernel).
