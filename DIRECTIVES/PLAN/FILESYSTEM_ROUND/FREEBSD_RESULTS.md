# FreeBSD filesystem round: implementation handoff

Implementation terminal, 2026-11-25. Host `adjutant@freebsd-core-01` is released for the fresh reviewer. No native job or transfer remains live.

## Result and identities

- Product: `1a37591a8fc7eba9cd1aa44db8c1e74d9ed499ee`; exact committed CLI built remotely and GELI/ZFS lifecycle passed after the final label guard.
- Installed test CLI: `/var/run/lurker-filesystem-round-freebsd/bin/lurker`, SHA-256 `a2d17a2c78bde516f59faa607937341af11c92984de9e29a3ede4afcfdade442`. Complete private VeraCrypt and gocryptfs runtimes are administrator-owned beside it.
- Exact-commit core/CLI/helper tests: **86 passed, 0 failed, 10 ignored**. Ignored native/fault suites were not run. This round has no GUI, physical-device, SQLite, reboot or fault qualification.
- **13 fresh CLI create/RW pairs passed**: GELI with UFS1, UFS2, ext4, exFAT, HAMMER2, ZFS; VeraCrypt with those six plus NTFS.
- **Four RO import cases passed**: VeraCrypt/APFS, VeraCrypt/Btrfs, VeraCrypt/XFS, GELI/NTFS4K. A separate GELI/NTFS4K RW import control passed append/add-file and encrypted close/reopen; this does not qualify new NTFS creation through GELI.
- Directory-on-filesystem results: **12/14 passed** (gocryptfs 7/7; PEFS 5/7). The two remaining cases are described below.

Every create/RW pass used the real CLI with a fresh file container, a valid PDF and UTF-8 text, ordinary close/reopen, hash comparison, appended text plus a second file, and another ordinary close/reopen/hash comparison. Each complete case has seven CLI actions. RO imports used independently formatted images, verified PDF/text hashes and the OS readonly flag, and closed normally. Directory rows are separate backing-store cases.

## All ten filesystems

`G` = GELI; `V` = private VeraCrypt. “Not separate” means no extra RO run beyond the requested writable lifecycle; it is not an observed RO pass. Receipt locations and exact source/binary identities for every row are in `target/goal-validation/filesystem-round/freebsd/summary.json`.

| Filesystem | Installed format / mount route | CLI creation and RW | Observed RO | Metadata and limits |
| --- | --- | --- | --- | --- |
| ext4 | e2fsprogs 1.47.4; native ext2fs | G + V pass | Not separate | Stock formatter profile; native ownership initialization. Feature/journal parity unqualified. |
| Btrfs | packaged LKL 4.16 | Creation refused | V pass | Legacy-compatible Linux fixture; append writes fail in this LKL package. G's 4096-byte providers are incompatible with its 512-byte virtual device. |
| XFS | xfsprogs 3.2.4; LKL 4.16 | Creation refused | V pass | Requires native `opts=norecovery` for RO; no log replay. Append writes fail upstream. G's sector geometry is incompatible. |
| APFS | libfsapfs 20240429 | No FreeBSD formatter route | V pass | Reader uses the existing decrypted regular VeraCrypt FUSE image, proven against the md backing inode. Raw md/GELI byte reads fail; G imports refused. |
| UFS1 / FFSv1 | base newfs `-O 1 -U`; native ufs | G + V pass | Not separate | Native owner/mode initialization; soft updates requested. |
| UFS2 / FFSv2 | base newfs `-O 2 -U`; native ufs | G + V pass | Not separate | Existing default retained; native private root initialization. |
| HAMMER2 | matching 1.2.13.1501000 package/module | G + V pass | Not separate | `DATA` label. Package describes experimental writes; only this content lifecycle is qualified. |
| NTFS | NTFS-3G 2022.10.3 | V pass; G new creation refused | G/4K pass | G/4K imports also pass RW. Native mkntfs4K times out on both provider and ordinary file. Driver uid/gid/umask supply ownership. |
| exFAT | exfat-utils + fusefs-exfat 1.4.0 | G + V pass | Not separate | Driver-assigned uid/gid/umask; Unix symlink metadata unavailable. |
| ZFS/OpenZFS | base OpenZFS 2.4.2 | G + V pass | Not separate | Private single-disk pool, legacy root mount, cachefile=none. Labels must describe one disk without children/spares/cache; imported leaf must equal the recorded mapper. Export retains other mounted datasets/snapshots. |

This proves content persistence, not complete ACL, snapshot, journal or other filesystem feature parity. ZFS pools are inside the existing encrypted providers.

## Directory backing stores

| Backing filesystem | PEFS | Login-user gocryptfs |
| --- | --- | --- |
| UFS1 | Pass | Pass |
| UFS2 | Pass | Pass |
| ext4 | Pass | Pass |
| exFAT | Required `.pefs.conf` symlink fails ENOSYS | Pass |
| HAMMER2 | Pass | Pass |
| ZFS | Pass | Pass |
| NTFS | Nested file opens fail EBADF | Pass |

The NTFS PEFS failure is reproduced with unmodified packaged PEFS on a plain native NTFS-3G/512-byte md filesystem: root-level file creation works, but nested exclusive and ordinary file opens both fail EBADF. This is independent of Lurker and encryption. exFAT cannot supply PEFS's required symlink metadata. APFS/Btrfs/XFS backing stores are RO-only in this round, so creating writable directory vaults on them is inapplicable.

Login-user gocryptfs requires `vfs.usermount=1`. It was temporarily enabled for these tests and restored to its original **0** afterward. PEFS used the already installed matching official kernel module.

## Bounded upstream limits and fixes

- LKL/XFS and LKL/Btrfs native 512-byte controls create/write/flush/fsync successfully, but append fails at **stream flush (the actual buffered write), before fsync**, with EBADF. Writable creation/mounts are refused. Native 4096-byte control reproduces sector-zero I/O failure; no LKL modernization or patch was attempted.
- XFS RO's ordinary `norecovery` configuration works. LKL startup now has a bounded native stat readiness check before the CLI reports a usable mount.
- GELI/NTFS mkntfs4K failed after the core's 300-second timeout with explicit native sector size/count. An ordinary empty-file formatter candidate also timed out after 120 seconds. The independent Linux NTFS4K fixture mounts and persists writes through GELI, so the limit is the available formatter; no more formatter retries are required.
- Native btrfs-progs 7.3 source `afdc3a65e33f4d42f768ad50dca20c32e5ee1496` was attempted after installing missing autotools. Configure stops at missing `getmntent`; no source changes were made. `ntfs2btrfs` is installed as an available conversion tool but is not a qualified CLI formatter. Writable LKL is independently blocked.
- Initial RO fixture copying used oversized raw I/O; the OS harness now copies 64 KiB chunks. Earlier raw APFS reader attempts failed on device byte reads; the verified existing VeraCrypt image-file route passes. These resolved attempts remain in evidence and are not counted as passes.

## Receipts, installation and cleanup

- Concise local index: `target/goal-validation/filesystem-round/freebsd/summary.json`; inventory, artifacts, refusals and checkpoint JSON are beside it.
- Named logs/receipts: `target/goal-validation/filesystem-round/freebsd/evidence/`. `exact-final-tests.log`, `cleanup-final.json`, `final-hashes.json`, the `native-*` controls and each case `result.json` are retained. The exact executed directory harness was reconstructed and hash-checked against its recorded SHA; its captured source is included. The final committed OS harnesses are at `c2e1313`.
- Remote work/results: `/tmp/lurker-filesystem-round-freebsd-20261125/`; cached target: `/home/adjutant/.cache/lurker/remote-test/cargo/df12bf0446b59ac051a7/target`.
- Installation logs record `pkg install -y e2fsprogs fusefs-exfat fusefs-ext2 fusefs-lkl fusefs-ntfs hammer2 libfsapfs xfsprogs`, then `exfat-utils ntfs2btrfs`, and autotools for the bounded source attempt. pkg upgraded itself from 2.8.4 to 2.9.2. Native modules enabled: fusefs, ext2fs, hammer2, pefs. No upstream/kernel patch was applied.
- Six closed failed RO ciphertext fixtures were moved off the guest only after full SHA-256 comparison. Their bytes and original paths are retained in `target/goal-validation/filesystem-round/freebsd/relocated-fixtures/manifest.json`; other failed fixtures/logs remain at the documented remote locations. Cleanly failed creates discard unpublished staging through the existing lifecycle.
- Final verified cleanup: no owned mounts, processes, md devices, native sessions or initialization directories; root registry lock is free; only system `zroot` is imported; `vfs.usermount=0`. Guest free space is about 2 GiB. Installed packages, modules, trusted candidate runtime and retained evidence remain.

Reviewer next action: read this report and `summary.json`, inspect the small BSD dispatch/FreeBSD adapter commits, verify carried receipt identities, and rerun only a genuinely missing or failed meaningful case. Native-reproduced limits above need no further upstream investigation.

## Independent review, 2026-11-25

Review passed with the existing upstream limits. No serious product or harness
defect requiring a patch was found. The APFS reader selects only the existing
VeraCrypt regular image whose inode matches the recorded md provider; GELI stays
explicitly unavailable for that route. Probes use aligned 4096-byte reads, NTFS
formatting reads the native sector geometry/count, and LKL refuses incompatible
providers. ZFS checks single-device labels before import, verifies the mapped
leaf after import, and refuses export while additional datasets/snapshots remain
mounted. Native UFS2 defaults and ownership initialization are preserved.

Programmatic receipt reconciliation verified all **13 create/RW pairs**, **four
RO imports**, **one GELI/NTFS RW import**, and **12/14 directory cases**
(gocryptfs 7/7, PEFS 5/7), including seven successful CLI actions and the expected
write/append/final payload hashes for every complete writable lifecycle. The
retained exact-commit test log totals **86 passed, 0 failed, 10 ignored**. All six
relocated failed ciphertext fixtures still match their recorded full hashes.
The executed common/directory harness hashes match their provenance. Live hashes
of the candidate CLI, private engines and staged harnesses match the final
artifact record. The final source archive matches both its receipt and a fresh
`git archive` of `1a37591`.

One affected case was missing after the final ZFS label guard: **VeraCrypt/ZFS**.
It now passes a fresh seven-action PDF/text write, reopen, append/add-file and
final reopen lifecycle on the final candidate
`a2d17a2c78bde516f59faa607937341af11c92984de9e29a3ede4afcfdade442`.
The receipt is `evidence/review-veracrypt-zfs.json`; the remote fixture/result is
`/tmp/lurker-filesystem-round-freebsd-20261125/lurker-fs-cli-omw0it4o/`.
Other receipts retain their original revisions: subsequent relevant changes
were checked by source inspection, and FreeBSD modules/shared BSD lifecycle
remain unchanged at review HEAD. Later `system.rs` changes concern Linux
command resolution.

Fresh native cleanup again finds no owned mounts, driver processes, md devices
or session records; the registry lock is free, only system `zroot` is imported,
and `vfs.usermount=0`. No jobs remain live; host is released. Read-only
APFS/Btrfs/XFS, GELI NTFS creation, and PEFS exFAT/NTFS limitations remain as
recorded above, without additional upstream attempts. Review receipts are
`target/goal-validation/filesystem-round/freebsd/review-reconciliation.json`,
`review-live-artifacts.json`, and `review-live-check.json`. This review is not
release, feature-parity, reboot, GUI, physical-device or fault qualification.
