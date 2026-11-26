# openSUSE fresh native qualification

Status: active; no prior pass receipt is carried forward.

- Product baseline: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`.
- Host: `adjutant@opensuse-core-01`, openSUSE Leap 16.1, running supported kernel `6.12.0-160100.9-default`, initial boot `3466a285-c815-45d6-a9c0-86f92b463485`.
- Updates: fresh `zypper refresh` and `zypper update` completed; diffutils, libruby3_4-3_4, ruby3.4 and shim updated. Subsequent `zypper list-updates` reports none. No repository or vendor changes made. The separately installed OBS kernel 7.3.6 remains an alternate; qualification starts on the supported running kernel.
- Native tools: cryptsetup, Btrfs, ext4, XFS, exFAT, NTFS and APFS formatters already current. Matching external UFS and OpenZFS 2.4.4 modules load on the running kernel. Fresh upstream APFS module `6cf6de8ad6fa4cb634a700b5d0436d73ee01942c` built against its installed headers and loaded using targeted `modprobe --allow-unsupported`; no global module policy changed.
- Independent engines: 137 cached private backend files freshly hash-verified against their package manifest, with VeraCrypt source/archive/creator patch, gocryptfs v2.7.0 and stable CryFS 1.0.3 inputs matching current pins. Cryptomator build inputs match current source except its non-build README; exact differences retained privately. These are reused build inputs, never reused runtime passes.
- Live workspace check/test/build: run `20261126T203533Z-e9713258`, runner PID 9675, one Cargo worker. Build and tests execute on the guest. Harness revision is `c5feaec`, including downloaded Bitcoin/Ethereum/Transformer/text payloads.

Private evidence: `target/goal-validation/native-fresh-20261127/hosts/opensuse/`.
Remote logs/source: `/tmp/lurker-native-fresh-20261127-opensuse/` and `/tmp/lurker-matrix-1300/runs/20261126T203533Z-e9713258-opensuse/`.
Persistent fresh fixtures: `/var/tmp/lurker-native-fresh-20261127-opensuse/`.
Administrator-owned runtime prefix: `/usr/local/lib/lurker-native-fresh-20261127-opensuse/`.

Next: complete fresh workspace artifact, run all applicable filesystem/container and directory-backing cases before attaching and qualifying a uniquely identified new qcow2 scratch disk, batch findings, then final checks and cleanup. A final 10 × 13 ledger and receipts remain outstanding.

Current upstream eligibility references:

- https://github.com/linux-apfs/linux-apfs-rw — GPL-2.0; experimental native writable module, explicitly qualified as such.
- https://github.com/openzfs/zfs/releases/tag/zfs-2.4.4 — current eligible CDDL implementation; supported Linux kernel range includes 6.12.
- https://github.com/jdmanring/linux_hammer2 — BSD/GPL native Linux HAMMER2 module; upstream requires kernel 7.3 or newer.
- https://github.com/realchonk/fuse-ufs — open-source UFS2 FUSE route; native UFS route assessed separately.
- https://github.com/rfjakob/gocryptfs/releases/tag/v2.7.0
- https://github.com/cryfs/cryfs/releases/tag/1.0.3
