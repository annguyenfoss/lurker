# DragonFly BSD fresh qualification — in progress

Host: `adjutant@dragonflybsd-core-01`. Baseline:
`8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`. These are fresh observations;
prior artifacts and reports are not fresh passes. Final candidate qualification,
additional credentials/ciphers, scratch devices and ordinary reboot persistence
remain pending.

## Host and artifacts

- Running base: DragonFly 6.5-DEVELOPMENT,
  `v6.5.0.2269.g9f0e62-DEVELOPMENT`, stock kernel built November 24.
- `pkg update -f` and `pkg upgrade -y` completed with exit 0: nine upgrades and
  129 ABI-matched reinstalls. Rust 1.98.1, Python 3.12.16 and Python 3.14.10.
  The previously incomplete Python 3.14 installation now imports its standard
  library. Package vulnerability notices remain recorded in the upgrade log.
- A November 26 base snapshot is available. DragonFly documents source-based
  base upgrades; a newer running base is pending.
- Fresh exact-baseline CLI/helper build completed in run
  `20261126T205225Z-98c4aa71`, with one Cargo worker. Its workspace checks/tests
  are still running. Flags: incremental disabled, development/test debug info 0.
- CLI SHA-256:
  `fdf41eee9f994aa6dfaf6aa73edac6e284752400c4739ef60ac3da0e826f30c7`.
  Helper SHA-256:
  `a375ac9d03c7e345b06e35c0a4aad4528d77fe34a3a872f65458f149ab799e37`.
- The private tcplay wrapper was freshly rebuilt from checksum-verified cached
  pinned sources. SHA-256:
  `a1e8b4d152c05ad36f948d785e849e5b1ec487f83c76e9345a7cb4d061697be7`.
- Runtime binaries are copied into the administrator-controlled private prefix
  `/opt/lurker-native-fresh-20261127/baseline`. Results are staged-product results;
  they do not establish installed-package or desktop qualification.

## Completed initial container assessment

The initial 20 cases selected all ten filesystems separately with LUKS1 and
VeraCrypt. AES/password UFS1 and HAMMER2 passed with both backends: four passes.
Each copied the downloaded Bitcoin, Ethereum and Transformer PDFs plus text as
uid 1300, closed normally, reopened and compared hashes, appended text and a new
file, and reopened/compared again. Ownership was uid/gid 1300 and mode 0700.
All four plaintext mounts were closed.

The other 16 selections were refused by the baseline adapter. ext4, NTFS and
exFAT are product gaps: fresh independent native controls formatted, mounted,
wrote, normally closed/reopened and verified the downloaded payload, including
an append and a second reopen. Their host-local FUSE mounts were closed.
Btrfs, XFS, APFS, UFS2 and ZFS require separate availability/applicability
reconciliation; no pass is claimed for them.

Installed native filesystem candidates and provenance:

| Candidate | Version/license | Source and observation |
| --- | --- | --- |
| e2fsprogs + fuse-ext2 | 1.47.4 GPLv2+; 0.0.11 GPLv2 | https://github.com/tytso/e2fsprogs and https://github.com/alperakcan/fuse-ext2 — packaged native formatter/FUSE driver; payload lifecycle passed |
| NTFS-3G | 2022.10.3 GPLv2+ | https://github.com/tuxera/ntfs-3g — packaged native formatter/FUSE driver; payload lifecycle passed |
| exfat-utils + fuse-exfat | 1.4.0 GPLv2+ | https://github.com/relan/exfat — packaged native formatter/FUSE driver; payload lifecycle passed |
| libfsapfs | 20240429 LGPL3+ | https://github.com/libyal/libfsapfs — installed reader, no writable creation claim |
| libfsxfs | 20240501 LGPL3+ | https://github.com/libyal/libfsxfs — installed reader, no writable creation claim |

The stock `fuse.ko` loads on this running base. Mount names must fit DragonFly's
80-byte native fields. A discarded experimental identity control incorrectly
used a regular-image vnode as the native FUSE device source and caused a kernel
panic during later close. This is not evidence of a stock-helper failure: a
fresh ordinary failed-mount/close control remained healthy. The corrected native
identity control used a root-private alias of `/dev/fuse`, closed normally and
left zero source handles. Any product integration must preserve that device-vnode
contract and root-private mount identity.

## Evidence and next action

Private local evidence:
`target/goal-validation/native-fresh-20261127/hosts/dragonflybsd/`.
Remote evidence is persistent under
`/home/adjutant/.cache/lurker/native-fresh-20261127-dragonflybsd/evidence`, exposed
through `/tmp/lurker-native-fresh-20261127-dragonflybsd`.
`cases/baseline-cases.jsonl` names all 20 receipts; `cases/native-*/receipt.json`
contains the three native filesystem controls. Failed fixtures are retained.

Next: verify the owned ext4/NTFS/exFAT adapter changes remotely, commit coherent
fixes, rebuild once for the input batch, and run affected lifecycle/cipher/keyfile
and conformance cases before verified new qcow2 device tests. Complete all 130
applicability cells and final frozen-candidate validation before closing the round.
