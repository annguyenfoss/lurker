# DragonFly filesystem round results

Implementation complete, 2026-11-25. All owned jobs are terminal; host ready for the fresh reviewer.

## Frozen artifacts and jobs

- Product: `bb00d25` (includes adapter commit `f288f6e`); exact Git archive built on `adjutant@dragonflybsd-core-01`.
- Native build/test log: `/tmp/lurker-filesystem-round-dragonfly-20261125/exact-build.log`. Initial modified snapshot and exact committed product both passed core/CLI/helper tests; exact product check: 69 passed, 3 ignored, 0 failed. A subsequent mapper-occupancy guard commit `16303e0` passed 7 targeted DragonFly tests (2 native opt-in tests ignored).
- Binary staging: `/opt/lurker-filesystem-round-dragonfly/bin/`, root-owned and not user-writable. Current pinned existing `lurker-tcplay` protocol 2 rebuilt unprivileged with `scripts/build-dragonfly-tcplay.py`; no upstream changes added. Backend receipt: `/tmp/lurker-filesystem-round-dragonfly-20261125/backend/dragonfly-tcplay-source.json`.
- Payload coordinators are terminal. Five RW pairs passed all seven CLI operations each (create plus three normal mount/close cycles): LUKS1/UFS1, LUKS1/HAMMER2, VeraCrypt/UFS1, VeraCrypt/HAMMER2, VeraCrypt/FAT32. LUKS1/NTFS RO import/read/normal-close passed. Fixture root `/var/tmp/ldfs`; setup failures retained at `/var/tmp/lurker-filesystem-round-dragonfly-fixtures`. Import harness commit `a9652ad`; `ca86bf6` makes unsuccessful receipts return a failing process status.
- Local evidence: `target/goal-validation/filesystem-round/dragonfly/`.

## Inventory and installation

Stock DragonFly 6.4.2 x86_64; Rust 1.98.1; two CPUs, 4 GiB RAM. Existing native cryptsetup 1.1.2, newfs, newfs_hammer2, newfs_msdos, mount_ext2fs and mount_ntfs. No pre-existing native Lurker storage sessions. `/tmp` is a 974 MiB tmpfs; persistent fixtures use `/var/tmp` on the separate build filesystem.

`pkg install -y e2fsprogs-core exfat-utils fusefs-ntfs fusefs-exfat` failed because the pre-existing system package database is malformed. `pkg check -d -a` did not repair it; no manual database edits were made. Avoided changing the global package database: `pkg -o PKG_SELF_UPGRADE=false fetch -y -d` fetched signed repository packages and dependencies, extracted into a root-owned isolated tools prefix under `/opt/lurker-filesystem-round-dragonfly/tools/root/`. Tool runtime uses that prefix's `lib/e2fsprogs` and `lib` through `LD_LIBRARY_PATH`. Exact package archive SHA-256s are retained.

Formatter versions: e2fsprogs 1.47.4, ntfs-3g/mkntfs 2022.10.3, exfat-utils and fusefs-exfat 1.4.0. Ordinary exFAT formatting succeeded. The initially absent FUSE module was subsequently built from the exact unmodified stock v6.4.2 source tag `54d7118100cb99b741fe92cfd8d542c623b6f8d0`: sparse checkout `sys include`, then `make SYSDIR=<source>/sys` in `sys/vfs/fuse`. Installed root-owned at `/opt/lurker-filesystem-round-dragonfly/modules/fuse.ko` and loaded normally; SHA-256 `6ee7dd01a673538f744db93f118f1c20c013f0bec994febb49d5d76cba63ed3c`. No startup autoload change. Despite a present `/dev/fuse`, upstream exFAT reports `mount_fusefs: mount: Bad address` both normally and with one ordinary `subtype=exfat` correction; ntfs-3g reports `mount_fusefs: -o nempty: option not supported`. OS mount checks confirm neither established a mount. Thus FUSE-dependent candidates remain blocked by the stock upstream userspace/driver integration, superseding the initial missing-module finding. No kernel replacement, upstream patches, GUI, physical devices, SQLite payload, faults or forced closes were attempted.

## Platform policy

LUKS1 still defaults to UFS1. VeraCrypt still defaults to HAMMER2; FAT32 remains its exchange regression. Both encryption routes can select UFS1 or HAMMER2. Native LUKS1 foreign imports dispatch read-only ext2fs or NTFS and reject writable mounts. NTFS is observed working; ext4 remains unqualified because the native reader fails on payload reads. VeraCrypt retains its existing platform-wide read-only refusal because its native mapper cannot enforce read-only writes. Directory backends are not implemented on DragonFly and are structurally inapplicable to this round.

Native third-party formatters do not support DragonFly mapper size/locking ioctls. Independent foreign import fixtures therefore format ordinary files with unmodified upstream utilities, populate the files with debugfs or ntfscp, and copy sectors through a native encrypted mapping before closing it. This is fixture creation only, not a filesystem bridge.

## Ten filesystem cells

| Filesystem | Installed route | Format/create | CLI pairing and RO/RW | Observed lifecycle / limit |
| --- | --- | --- | --- | --- |
| ext4 | e2fsprogs + stock ext2fs | Independent import file | LUKS1 RO; RW unavailable | Both default and one compatibility profile mount but reading PDF returns EINVAL; unencrypted native control reproduces; no RO pass |
| Btrfs | No compatible kernel or functional FUSE route | Unavailable | None | No mount runtime |
| XFS | Reader candidates require nonfunctional stock FUSE | Unavailable | None | Prerequisite blocked |
| APFS | Reader candidates require nonfunctional stock FUSE; no native creator | Unavailable | None | Prerequisite blocked |
| UFS1 / FFSv1 | Native newfs + mount_ufs | Selected create | LUKS1 and VeraCrypt RW | PDF/UTF-8 write, close/reopen/hash, append/add-file, close/reopen/hash passed on both routes |
| UFS2 / FFSv2 | No native DragonFly UFS2 mount route | Unavailable | None | Native UFS1 does not establish UFS2 support |
| HAMMER2 | Native newfs_hammer2 + local mount syscall | Selected create | LUKS1 and VeraCrypt RW | Same three-mount PDF/text payload passed on both routes; broader durability findings remain outside this content-persistence round |
| NTFS | mkntfs/ntfscp + stock mount_ntfs | Independent import file | LUKS1 RO; RW unavailable | Independent NTFS PDF/text fixture read/hash and normal close passed; native limited in-place writing is insufficient for RW workload; FUSE helper fails upstream |
| exFAT | exfat-utils + fusefs-exfat | Standalone format passed | Encrypted mount blocked | Stock source-built FUSE loads, but direct upstream mount fails with Bad address; no real mount |
| ZFS / OpenZFS | No compatible stock mount runtime | Unavailable | None | No native route; no encryption backend added |

Installed upstream manuals expose HAMMER2 PFS/snapshot, compression and checksum controls; this round uses the native `DATA` PFS and leaves those features at their defaults. UFS1 uses native ownership/mode handling. The native NTFS manual describes data attributes and limited in-place writes, which do not support this full RW workload. ext4 source masks include extents, 64-bit addressing and metadata checksums, but the observed reader failures prevent a functional claim. exFAT/NTFS format capability does not establish filesystem metadata parity, and unavailable cells have no qualified metadata/features.

FAT32 is outside the ten-cell matrix and remains a VeraCrypt regression. A payload pass proves PDF/text content persistence across ordinary closes; it does not qualify ACLs, snapshots, compression, journaling, metadata parity or crash durability.

Primary native API/feature references: installed `mount_ntfs(8)` and `mount_ext2fs(8)`; DragonFly release-6.4 source feature masks at https://raw.githubusercontent.com/DragonFlyBSD/DragonFlyBSD/DragonFly_RELEASE_6_4/sys/vfs/ext2fs/ext2fs.h and https://raw.githubusercontent.com/DragonFlyBSD/DragonFlyBSD/DragonFly_RELEASE_6_4/sys/vfs/ext2fs/ext2_vfsops.c .

## Evidence and cleanup

The failed initial long fixture paths exceeded BSD's native mount-name identity bound; shorter `/var/tmp/ldfs` paths solved it. An old cached protocol-1 wrapper was replaced by the current protocol-2 build before final tests. These are retained setup failures, not product lifecycle failures.

Default e2fsprogs ext4 imports mounted but reads failed with EINVAL. One ordinary profile correction disabled `metadata_csum`, `metadata_csum_seed`, and `orphan_file`, retaining extents, 64bit, a journal and other ext4 features; it failed identically. Direct unencrypted native ext2fs mounting reproduced EINVAL on both observed profiles. No upstream fix was attempted.

Local `result.json` contains the ten-cell matrix, exact artifact hashes, all receipt paths and hashes for PDF/UTF-8 payloads. `evidence.tar.gz` retains build, package, FUSE, native control and per-fixture logs/receipts. No fixture mounts, Lurker mapper devices, attached vnode devices, session records or owned background workers remain. Existing stock base mounts were preserved. Root-owned test artifacts and failed/passed closed fixture files remain for review. Native ext2fs/NTFS and the ordinary source-built FUSE module remain loaded; no reboot or permanent boot configuration was changed.

Reviewer next action: inspect `target/goal-validation/filesystem-round/dragonfly/result.json` and evidence archive first. Review adapter commits `f288f6e`, `16303e0` and harness commits `c5bcf44`, `a9652ad`, `ca86bf6`; rerun only meaningful missing coverage with current shared code. Product payload artifact `bb00d25` predates the final conservative mapper-occupancy guard; its targeted test receipt is separate. No useful FUSE or ext4 route remains to fix within the authorized unmodified-upstream scope.

## Independent review — 2026-11-25

Reviewed all ten filesystem cells and the archived native receipts independently.
All five original RW pairs contain fresh CLI creation, three ordinary mount/close
cycles, PDF/UTF-8 writes, verified original bytes, append/add-file and matching
final SHA-256 hashes. NTFS is a separate LUKS1 RO import/read/close pass; it is
not writable support. Failed setup and upstream ext4 receipts remain failures.
Every passed aggregate receipt matches its copy in `evidence.tar.gz`.

Current committed source `ceb1c50f849ae999ff09825182442eca04de9093` was archived
and built as the login user on DragonFly. Core/CLI/helper tests passed: 69 passed,
0 failed, 3 native opt-in tests ignored. Binaries were installed into the separate
root-owned `/opt/lurker-filesystem-round-dragonfly-review/` prefix. Source review
found the original UFS1/FAT32/NTFS lifecycle inputs unchanged; the final HAMMER2
mapper-occupancy guard was the only relevant product delta. Both LUKS1/HAMMER2
and VeraCrypt/HAMMER2 therefore received fresh complete seven-operation payload
reruns on the current artifact, with invoking owner UID 1300; both passed. The
payload coordinator itself ran as root. No serious product or harness gap was
found and no product patch was needed.

Review artifact hashes, source archive hash, counts, new per-pair receipts and
cleanup are in `target/goal-validation/filesystem-round/dragonfly/review/review.json`;
`review-evidence.tar.gz` retains the new test and fixture logs. Remote evidence:
`/tmp/lurker-filesystem-round-dragonfly-review/`; closed review fixtures:
`/var/tmp/ldrev/`. Current-source CLI SHA-256:
`d7efd303b053ccfcd725c642c4fa8a515efca036a9a1366fcbc51cbbc17d236a`.

The final native inventory again has no fixture mounts, mapper devices, attached
vnodes, session records or owned workers. Base mounts and ordinary loaded
prerequisite modules remain. ext4 native reads still fail upstream; stock FUSE
exFAT/NTFS controls fail before mounting; Btrfs/UFS2/ZFS lack a compatible mount
runtime and XFS/APFS remain FUSE-blocked. These are explicit upstream or missing
runtime limits, not payload passes. All excluded workloads remain unexercised.
