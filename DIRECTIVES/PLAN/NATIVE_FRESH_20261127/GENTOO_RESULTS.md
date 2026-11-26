# Gentoo fresh native qualification — 2026-11-27

Status: in progress. No historical pass is carried into this round.

- Owner: Gentoo host agent; SSH inventory key `gentoo`, guest `gentoo-core-01`.
- Product baseline: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`; downloaded-payload harness revision `c5feaec` (Linux forwarding `88e279c`).
- Private receipts/checkpoint: `target/goal-validation/native-fresh-20261127/hosts/gentoo/`.
- Remote job root: `/tmp/lurker-native-fresh-20261127-gentoo`; persistent build/fixture root: `/srv/lurker-native-fresh-20261127-gentoo`.

Initial inventory found Gentoo 2.18, running `6.18.64-p1-gentoo-dist-bin`, boot ID `ff09bae8-da3f-48a9-b001-0486833afec3`, and working `sudo -n`. The XFS system disk had only 4.7 GiB free. A newly created 40 GiB build disk at `/srv/build/lurker-native-fresh-20261127/gentoo/build.qcow2` was attached through `qemu:///system`, identified in the guest as serial `lurker-build-gentoo`, and formatted/mounted only after verifying that identity. Existing disks and retained fixtures were preserved.

A supported Gentoo repository sync completed successfully with the signed manifest dated 2026-11-26 19:38:04 UTC. The world update plan contains zstd 1.6.0, sudo 1.9.17_p2-r1, the prebuilt 6.18.65 LTS distribution kernel, and its virtual dependency. The update and a fresh login-user, one-worker release workspace build are running; completion, running-kernel identity, hashes and validation remain pending.

Installed optional filesystem facilities are discovery aids. They include OpenZFS 2.4.4, an external APFS module, fuse-ufs, hammer2-fuse, and native formatting tools. The stock distribution configuration disables UFS write support. An existing separately built UFS module must be identified separately and does not establish a stock-kernel writable UFS result. No customized kernel is being built.

The required 10 × 13 applicability ledger, fresh downloaded-payload container receipts, credential/cipher conformance, newly identified scratch-device tests, reboot persistence and final frozen-candidate checks remain in progress. A staged CLI/helper result will be identified separately from an installed-package result.
