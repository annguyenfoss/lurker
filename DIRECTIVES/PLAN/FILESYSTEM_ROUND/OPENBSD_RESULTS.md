# OpenBSD filesystem round — implementation handoff

Implementation terminal, 2026-11-25. Host `adjutant@openbsd-core-01`, OpenBSD 8.0 GENERIC.MP#84 amd64, Rust 1.98.1, Python 3.14.9. Source commits `8892f3e` and `d469704`; readonly reader harness `1d15e4e`. No nested agents, GUI, SQLite workload, fault injection, reboot, physical-device work or upstream modifications were used.

The exact `d469704` archive passed native `cargo check`, `cargo test` and CLI/helper build: **84 passed, 0 failed, 18 ignored**. Earlier runner checkpoint `20261124T185611Z-c1d54d9f` tested `bb00d25` before the small geometry correction. CLI SHA-256 is `d4a11b14aa63af68547b7d3e7f37723114a81b5ae65fbb17b9cbf1348caff978`. The existing complete private VeraCrypt backend was reused, with its original pinned provenance and licenses: binary SHA-256 `0c3db467e0abc33f67997a22f6deba438cfcab228a4095c063715b0a564be99e`. Its existing project patches were not changed. Binary, formatter, driver and harness hashes are in `target/goal-validation/filesystem-round/openbsd/identity.json`.

## Observed native CLI lifecycle

| Encryption route | FFSv1 | FFSv2 | NTFS | exFAT |
| --- | --- | --- | --- | --- |
| softraid CRYPTO | pass | pass | pass | pass |
| softraid RAID1C, two independent file members | pass | pass | pass | pass |
| VeraCrypt | pass | pass | pass | pass |

These **12 fresh pairs / 84 successful CLI operations** each created a 0.128 GB encrypted fixture, mounted it, wrote a valid one-page PDF plus UTF-8 text, normally closed/reopened, verified both hashes, appended text and added a second file, normally closed/reopened, verified all three hashes, and normally closed. Each final receipt records the actual source, executable hash, commands, mountpoint and digests. This demonstrates content persistence through ordinary close/reopen, not comprehensive metadata, ACL, snapshot, journaling or power-loss equivalence.

Evidence index: `target/goal-validation/filesystem-round/openbsd/summary.json`; complete small receipts/logs/build output: `target/goal-validation/filesystem-round/openbsd/evidence.tgz`. Remote evidence/fixtures remain at `/tmp/lurker-filesystem-round-openbsd-20261125`. Administrator-owned candidate executables and scripts remain at `/root/lurker-filesystem-round-openbsd-20261125` for review.

## All ten filesystem cells

“Native RO” below describes the installed filesystem reader. **Every current OpenBSD Lurker encryption route refuses read-only crypto attachment**, so none of these rows advertises a successful CLI RO mount. The four RW filesystems above are the qualified CLI create selections. Existing FFSv2 defaults and synchronous writable FFS mounting remain unchanged.

| Filesystem | Installed format/access route | Create / format on OpenBSD | Native RO / RW | CLI encryption pairing and observed limits |
| --- | --- | --- | --- | --- |
| ext4 | base `ext2fs`; e2fsprogs 1.47.3 | Offline `mke2fs -t ext4` available; CLI create refused | RO subset / no RW | Native encrypted-fixture PDF/text read passed; CLI RO blocked by crypto boundary. Fixture retained extents but disabled `64bit`, `metadata_csum_seed`, `orphan_file`; this is not arbitrary modern ext4 support. |
| Btrfs | No eligible native package/driver found | unavailable | unavailable | No supported pairing; no bridge or kernel modification introduced. |
| XFS | libfsxfs 20260703, `fsxfsmount` | No local formatter; independent Linux fixture | RO / no RW | Native reader passed on imported XFS v4 (`crc=0`, `sparse=0`) PDF/text. CLI RO blocked. Upstream reader lists v4/v5 and extended attributes; only v4 content was exercised. |
| APFS | libfsapfs 20240429, `fsapfsmount` | No local formatter; independent macOS fixture | RO / no RW | Native reader passed on an unencrypted APFS raw filesystem carried inside VeraCrypt. CLI RO blocked. This does not add Apple/APFS encryption as an OpenBSD backend. |
| UFS1 / FFSv1 | base `newfs -O 1`, FFS mount | yes | native RO / RW | CRYPTO, RAID1C and VeraCrypt RW lifecycle passed; synchronous FFS mounts. Native ownership/modes remain in the existing initialization path. |
| UFS2 / FFSv2 | base `newfs -O 2`, FFS mount | yes, existing default | native RO / RW | All three routes passed the same lifecycle. |
| HAMMER2 | Upstream OpenBSD port requires a kernel patch | unavailable within scope | unavailable | No kernel/driver patching attempted. |
| NTFS | ntfs_3g 2026.2.25, `mkntfs`, `ntfs-3g` | yes | native RO / RW | All three routes passed RW lifecycle. Root ownership is presented with explicit invoking uid/gid and `umask=077`; Windows ACL/ADS parity was not tested. |
| exFAT | exfat-fuse 1.4.0, `mkexfatfs`, `mount.exfat-fuse` | yes | native RO / RW | All three routes passed RW lifecycle. Explicit uid/gid/umask replace POSIX ownership initialization; chmod/ACL semantics are not implied. |
| ZFS / OpenZFS | No eligible OpenBSD package/driver found | unavailable | unavailable | No inner-ZFS or native-ZFS encryption route added. |

The readonly import harness produced three **native reader passes / CLI RO blocked** receipts: `import-ext4-ddqslmlz`, `import-apfs-8q7mk_w6`, `import-xfs-r0ft67d4`. ext4 reads the mapped block partition. Packaged libyal readers reject OpenBSD block nodes because their media size is reported as zero; their ordinary `-o 32768` option successfully reads the existing VeraCrypt decrypted regular auxiliary image instead. This is a standalone native reader control, not a Lurker CLI mount or a new filesystem bridge. The imported APFS raw image hash is `50037c6612d6a5e5d1b46f48c2d44ff19e249cbed2958db3470f6c632ef8c9e2`; XFS raw hash is `a9cc0eb76258393a3e6c4b67e45df11eaea43acfef8f920c17c12809e19b54c7`.

CRYPTO and RAID1C retain their existing readonly refusal. Lurker's VeraCrypt adapter also refuses readonly before acquisition. The pinned upstream `CoreOpenBSD::AttachFileToLoopDevice` has an unimplemented `readOnly` branch, so this round did not remove that boundary or substitute writable crypto. Directory encryption is structurally inapplicable: no directory engine currently passes `supports_platform("openbsd")`; selected future scope does not authorize a new engine integration.

## Installation, implementation and cleanup

Installed signed OS packages through authorized root SSH: `/usr/sbin/pkg_add exfat-fuse ntfs_3g libfsapfs libfsxfs e2fsprogs`. They remain installed as requested prerequisites. The login user has neither sudo authorization nor configured doas; no authorization files were changed. All compilation ran as adjutant, and test executables/backends were copied into administrator-owned paths before elevated execution.

Non-FFS creation now places partition `a` 64 sectors after the disklabel, deletes the fresh default `a` before adding that partition, and verifies its exact offset/type before formatting. Keeping offset zero loses the native label when exchange filesystems write their boot area. Exchange formatters need the block node, not the raw character node. FUSE mounts report source `fusefs`; acquisition checks its exact filesystem type and that the expected root daemon holds the mapped block device, while the existing journal retains exact mount fsid identity. RAID1C uses the same dispatch and ownership initialization.

A first NTFS product smoke exposed the label-editor mistake and passed after `d469704`. Initial reader-harness attempts expected the backend's device to end in `a`; the actual unpartitioned native listing ends in `c`. Those attempts were corrected, and all three owned native attachments were normally closed. Failed fixtures and logs are retained and distinguished from the final receipts. The shared Cargo cache had corrupt `cfg_if` metadata during an explicit CLI build; it was preserved and an isolated target directory was used successfully instead.

Final `cleanup.json` records root filesystem as the sole mount, all four vnodes unused, no softraid arrays, no VeraCrypt volumes, no active registry JSON, and 12.1 GiB free. Native workers and the matrix coordinator are terminal. Only review artifacts, ciphertext fixtures, build output and intentionally installed packages remain. No forced or lazy unmount was used.

Reviewer next action: read `summary.json`, `identity.json` and `cleanup.json`, then inspect the implementation/harness commits and this capability table. Rerun only a missing/failed meaningful case; the three readonly CLI blockers are explicit scope limits, not pass claims.

Primary references:

- https://man.openbsd.org/disklabel
- https://raw.githubusercontent.com/openbsd/src/master/sys/ufs/ext2fs/ext2fs.h
- https://github.com/libyal/libfsxfs/
- https://github.com/libyal/libfsapfs/
- https://github.com/relan/exfat
- https://github.com/tuxera/ntfs-3g/wiki/Manual
- https://github.com/kusumi/openbsd_hammer2
- https://github.com/veracrypt/VeraCrypt/blob/7afc449f2bc69366e8b67b82ad66201325d318c3/src/Core/Unix/OpenBSD/CoreOpenBSD.cpp

## Independent review — 2026-11-25

Reviewed implementation commits `8892f3e` / `d469704` and reader harness
`1d15e4e` against the complete final receipts. All **12 RW pairs / 84 CLI
operations** have the recorded `d4a11b14…978` executable identity, seven
successful operations in the required order, unchanged PDF hashes, appended
text and second-file hashes matching the final reopen, and no remaining mount.
The three imported-reader receipts independently match their expected hashes
and explicitly record CLI RO refusal. They remain native reader controls only.
All ten filesystem rows above agree with the available routes and receipts.

Native `cargo check` and `cargo test` for core, CLI and helper passed on committed
source `7270de9dd02906498ad3f28b5fc0b4b988e85e1c`: **84 passed, 0 failed,
18 ignored**. Its source archive SHA-256 is
`72d99719f9c426166061b4bf561ec78b56a7362b95b9b5057d7bd871a13c3a51`.
OpenBSD adapters, shared BSD lifecycle and VeraCrypt integration are unchanged
from the payload-tested source; defaults, explicit selection refusal, native
label geometry, ownership initialization and ordinary close ordering were
reviewed. No serious product or harness gap was found, so no patch or payload
rerun was needed. This compatibility check does not relabel the original
payload artifacts as a newer build or qualify an installed release package.

Fresh root inventory confirms only the baseline root mount, four unused vnodes,
no softraid array, no VeraCrypt attachment, no session JSON or live Lurker/build
worker. All recorded executable/tool hashes still match; elevated candidate
executables and their ancestors are root-owned without group/other write.
Packages and preserved fixtures remain intentionally. Fresh package queries
returned no Btrfs, ZFS or HAMMER2 match. The existing unavailable-route limits,
native-only RO controls, absent OpenBSD directory engines and untested metadata
features remain as documented above.

Review evidence: `target/goal-validation/filesystem-round/openbsd/` contains
`review-summary.json`, `review-receipts.json`, `review-cleanup.json`,
`review-checkpoint.json`, `review-status.json`, `review-check.log` and
`review-test.log`. Remote compatibility source/logs remain at
`/tmp/lurker-filesystem-round-openbsd-review-20261125`; the host is released.
