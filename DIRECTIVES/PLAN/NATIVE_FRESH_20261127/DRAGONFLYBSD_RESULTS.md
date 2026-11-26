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
  `20261126T205225Z-98c4aa71`, with one Cargo worker. The remaining baseline workspace build was cancelled when the
  adapter batch was ready; no workspace pass is claimed. Final complete workspace
  validation remains pending. Flags: incremental disabled, development/test debug info 0.
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

Initial native filesystem candidates and provenance (later host updates follow):

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

## Native filesystem adapter and host prerequisite fixes

- `cf6841a` adds NTFS/exFAT native formatting and FUSE mounts. Each mount has a
  root-private alias of `/dev/fuse`, an exact native identity and normal close.
  NTFS geometry comes from key-free mapper status. Synthetic ownership is private
  to the invoking user.
- The packaged NTFS 2022.10.3 library rejected unsupported advisory locks on raw
  DragonFly character devices. A native host package, `fusefs-ntfs-2026.2.25_1`,
  was built from the current upstream stable archive and pinned BSD port patches.
  A narrow DragonFly compatibility patch tolerates only unsupported raw-device
  locks; regular-file locking remains effective. A held regular-image lock
  rejected mount, left no mount and preserved the ciphertext hash.
  Source: https://download.tuxera.com/opensource/ntfs-3g_ntfsprogs-2026.2.25.tgz
  Archive SHA-256:
  `7754f3b32e8baf9c472459b4e9c981e3ae0f5039107cdd8d8201aed0a949008a`.
- ext4 formatting requires explicit mapper block geometry. The ordinary native
  FUSE driver also failed flush on raw devices: tracing found a two-byte
  superblock write at offset 1036, rejected with EINVAL; device fsync returned 0.
  A host `fusefs-ext2-0.0.11_3` package enables the existing libext2fs aligned I/O
  path for DragonFly character devices. This preserves real flush/fsync and
  leaves regular-image behavior unchanged. Upstream source:
  https://github.com/alperakcan/fuse-ext2/commit/ae35afb9ab08d87c66c1e021df792b3a7c4308b0
- `1f627f9` adds ext4 formatting/mounting and initializes VeraCrypt ext4 root
  ownership through the shared journaled initialization mount. Targeted native
  core checks and 10 DragonFly unit tests passed (2 existing ignored tests).
- Full downloaded-payload Auto-open, append and second-reopen checks passed for
  NTFS/exFAT with both backends using passwords, and ext4 with both backends using
  binary keyfiles. Payload writes ran as uid 1300. These are development candidate
  receipts with recorded source input manifests and binary hashes; complete
  frozen-commit qualification remains pending.
- Filesystem tools are installed as host prerequisites outside Lurker's private
  prefix. Both original filesystem packages are preserved as rollback archives.
  Patch, source, package, binary and dependency identities are in `build/` and
  `inventory/`; no cipher, KDF or encryption default was weakened.
- Official sudo was installed for the desktop helper path. A temporary narrow
  sudoers policy permits only this round's administrator-controlled helper paths;
  its removal remains part of cleanup. Helper capabilities run as the login user
  through that policy; full frontend lifecycle validation remains pending.

## Current continuation

The unmodified stock base at `e17b9176356e3afb3427d71c21108a8a849cd4d5` is building
with two jobs and a 2 GiB free-space guard. Persistent state:
`updates/base-build-state.json`; supervisor PID 23445. No new kernel/base has yet
been installed or booted.

A new 1 GiB qcow2 is attached persistently as libvirt `vda`, with requested serial
`lurker-fresh-dragonflybsd`, at
`/srv/build/lurker-native-fresh-20261127/dragonflybsd/scratch.qcow2`.
The current guest has not enumerated that hotplugged disk. Guest identity and all
destructive operations await the planned updated-base reboot. No existing disk
is a test target, and no scratch-device result is claimed.

Next: finish the stock base update and reboot, verify the new disk identity,
perform native device controls and the physical adapter, and run the final exact
candidate workspace, cipher/credential/conformance and persistence matrix.
Complete all 130 applicability cells, frontend/package checks and cleanup before
closing the round.
