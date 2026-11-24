# Filesystem installation and CLI round results

Status: in progress. Five OS implementation/review passes are complete; the six-distribution Linux owner is finishing its final bounded batch before independent review. Do not interpret pending Linux rows as unavailable filesystems.

## Scope and interpretation

Ten filesystems: ext4, Btrfs, XFS, APFS, UFS1/FFSv1, UFS2/FFSv2, HAMMER2, NTFS, exFAT and ZFS/OpenZFS. Eleven native targets: six Linux distributions, four BSDs and Buran/macOS. Existing integrated encryption methods only. Windows, physical storage, GUI, SQLite, forced dismount, power loss, performance tuning, security audit and release packaging are outside this round.

`--filesystem NAME` selects the inner filesystem for CLI file-container creation where supported by the installed native route. Omitting it preserves existing defaults and crypto profiles. Native APFS encryption is necessarily APFS-only. Directory encryption methods have no inner-filesystem selection; their separate backing-filesystem tests are counted separately. Auxiliary FAT32 regressions are not counted as requested filesystem coverage.

A passing writable lifecycle means a fresh Lurker CLI container, PDF and UTF-8 text writes, normal close/reopen, verified bytes/hashes, append/add-file, and a final normal close/reopen/hash verification. Creation and imported-volume access are distinct capabilities. These small payload tests do not establish full ACL, xattr, journal, snapshot or crash-durability parity.

## Target results

| Target | Requested create/RW container coverage | Additional import coverage | Directory/backing coverage | Independent review |
| --- | --- | --- | --- | --- |
| Arch Linux | Final batch pending | Pending | Pending | Pending |
| Debian | Final batch pending | Pending | Pending | Pending |
| Ubuntu | Final batch pending | Pending | Pending | Pending |
| Fedora | Final batch pending | Pending | Pending | Pending |
| openSUSE | Final batch pending | Pending | Pending | Pending |
| Gentoo | Final batch pending | Pending | Pending | Pending |
| FreeBSD | 13 pairs: GELI and VeraCrypt with UFS1, UFS2, ext4, exFAT, HAMMER2 and ZFS; VeraCrypt also creates NTFS | VeraCrypt APFS/Btrfs/XFS RO; GELI NTFS RO and RW import | 12/14: gocryptfs 7/7, PEFS 5/7 | Passed; final-guard VeraCrypt/ZFS case rerun |
| OpenBSD | 12 pairs: CRYPTO, RAID1C and VeraCrypt with FFSv1, FFSv2, NTFS and exFAT | Three native RO reader controls; CLI crypto RO remains unsupported | No integrated directory engine | Passed |
| NetBSD | 7 CGD pairs: FFSv1, FFSv2, ext4, exFAT, NTFS, ZFS and HAMMER2 | RO options deliberately unexercised for these RW pairs | No integrated directory engine | Passed |
| DragonFly BSD | 4 requested pairs: LUKS1 and VeraCrypt with UFS1 and HAMMER2 | LUKS1/NTFS RO | No integrated directory engine | Passed; both HAMMER2 routes rerun |
| macOS | 5 requested pairs: Apple APFS/exFAT, native encrypted APFS, VeraCrypt APFS/exFAT | Encrypted Apple NTFS RO, two cycles with write refusal | Six backing combinations attempted; prerequisites/engine limits prevent full mount lifecycles | Passed; two exFAT publication regressions verified |

Each OS report contains all ten filesystem rows, including unavailable routes and the selected minimal dependency combination:

- [Linux](LINUX_RESULTS.md) — pending.
- [FreeBSD](FREEBSD_RESULTS.md).
- [OpenBSD](OPENBSD_RESULTS.md).
- [NetBSD](NETBSD_RESULTS.md).
- [DragonFly BSD](DRAGONFLY_RESULTS.md).
- [macOS](MACOS_RESULTS.md).

## Material limits retained

- FreeBSD's packaged LKL route fails ordinary XFS/Btrfs append and close; those are read-only imports. Native GELI/NTFS creation is unavailable because the packaged 4 KiB formatter times out, including on an ordinary file; compatible imports work. PEFS/exFAT lacks a required symlink operation; PEFS/NTFS fails nested-file opens. Native controls reproduce the reported upstream limits.
- OpenBSD's existing crypto routes cannot enforce read-only attachment. Native ext4/XFS/APFS reader controls are not mislabeled CLI read-only passes. HAMMER2 would require excluded kernel patching; Btrfs/ZFS have no eligible route.
- NetBSD ext4 remains experimental; Btrfs/XFS/APFS are unavailable. HAMMER2 was built from unmodified upstream source against matching 11.1 sources with standard make-variable configuration.
- DragonFly's stock FUSE module builds/loads unmodified, but the tested exFAT/NTFS mount helpers fail upstream. Native ext4 payload reads fail in the unencrypted control too. Other FUSE-dependent readers remain blocked. Earlier durability findings are not requalified by ordinary-close payload passes.
- Buran's FSKit routes require operator enablement. The xlinuxfs App Store distribution is paid; no purchase was made. Source signing and excluded kernel-extension prerequisites remain explicit. CryFS/exFAT has the existing hard-link requirement. No upstream bypass or patch was attempted.

## Evidence and carried results

Detailed receipts, hashes, native controls, installation commands/versions, source pins and cleanup inventories are retained under ignored `target/goal-validation/filesystem-round/<os>/`. `review-index.json` indexes completed review receipts and their hashes. Reports retain original product and harness identities; a later documentation or platform-only commit does not relabel an earlier binary.

Completed independent review commits: DragonFly `2f793f9`, OpenBSD `1fc920a`, macOS `db26c02`, NetBSD `280a9e8`, FreeBSD `8e587b7`. Source comparison in `carried-source-audit.json` verifies unchanged platform dispatch, API/CLI/workflow, native lifecycle and supervisor inputs for carried results. The later CryFS initialization-mount relocation is Linux-only; the Darwin AppleDouble publication fix is separately exercised on Buran.

Completed hosts have no owned mounts, mappings, sessions or workers left. Existing system mounts/pools remain intact; FreeBSD's temporary `vfs.usermount` change was restored. Installed dependencies and closed failure fixtures are intentionally retained. Linux cleanup and final review are pending.
