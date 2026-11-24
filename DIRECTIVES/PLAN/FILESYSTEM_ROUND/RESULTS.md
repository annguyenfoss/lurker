# Filesystem installation and CLI round results

Status: complete. Best-effort dependency installation, Lurker CLI integration, native payload testing and all six independent OS reviews are complete across eleven targets. Unavailable and limited routes remain explicit below.

## Scope and interpretation

Ten filesystems: ext4, Btrfs, XFS, APFS, UFS1/FFSv1, UFS2/FFSv2, HAMMER2, NTFS, exFAT and ZFS/OpenZFS. Eleven native targets: six Linux distributions, four BSDs and Buran/macOS. Existing integrated encryption methods only. Windows, physical storage, GUI, SQLite, forced dismount, power loss, performance tuning, security audit and release packaging are outside this round.

`--filesystem NAME` selects the inner filesystem for CLI file-container creation where supported by the installed native route. Omitting it preserves existing defaults and crypto profiles. Native APFS encryption is necessarily APFS-only. Directory encryption methods have no inner-filesystem selection; their separate backing-filesystem tests are counted separately. Auxiliary FAT32 regressions are not counted as requested filesystem coverage.

A passing writable lifecycle means a fresh Lurker CLI container, PDF and UTF-8 text writes, normal close/reopen, verified bytes/hashes, append/add-file, and a final normal close/reopen/hash verification. Creation and imported-volume access are distinct capabilities. These small payload tests do not establish full ACL, xattr, journal, snapshot or crash-durability parity.

## Target results

| Target | Requested create/RW container coverage | Additional import coverage | Directory/backing coverage | Independent review |
| --- | --- | --- | --- | --- |
| Arch Linux | 29 pairs across eight writable filesystems | 5 RO: UFS1 with LUKS1/VeraCrypt; HAMMER2 with all three privileged routes | 22/24 | Passed; ext4/UFS2 rerun |
| Debian | 29 pairs across eight writable filesystems | 5 RO, same as Arch | 22/24 | Passed; ext4 rerun |
| Ubuntu | 32 pairs across nine writable filesystems, including HAMMER2 | 2 RO: UFS1 with LUKS1/VeraCrypt | 25/27 | Passed; ext4/HAMMER2 rerun |
| Fedora | 29 pairs across eight writable filesystems | 5 RO, same as Arch | 22/24 | Passed; ext4 rerun |
| openSUSE | 26 pairs across seven writable filesystems; ZFS ABI blocked | 5 RO, same as Arch | 19/21 | Passed; ext4/UFS2 rerun |
| Gentoo | 23 pairs across six writable filesystems; APFS/ZFS kernel SDK unavailable | 8 RO: UFS1 with LUKS1/VeraCrypt; APFS/HAMMER2 with all three privileged routes | 16/18 | Passed; ext4 rerun |
| FreeBSD | 13 pairs: GELI and VeraCrypt with UFS1, UFS2, ext4, exFAT, HAMMER2 and ZFS; VeraCrypt also creates NTFS | VeraCrypt APFS/Btrfs/XFS RO; GELI NTFS RO and RW import | 12/14: gocryptfs 7/7, PEFS 5/7 | Passed; final-guard VeraCrypt/ZFS case rerun |
| OpenBSD | 12 pairs: CRYPTO, RAID1C and VeraCrypt with FFSv1, FFSv2, NTFS and exFAT | Three native RO reader controls; CLI crypto RO remains unsupported | No integrated directory engine | Passed |
| NetBSD | 7 CGD pairs: FFSv1, FFSv2, ext4, exFAT, NTFS, ZFS and HAMMER2 | RO options deliberately unexercised for these RW pairs | No integrated directory engine | Passed |
| DragonFly BSD | 4 requested pairs: LUKS1 and VeraCrypt with UFS1 and HAMMER2 | LUKS1/NTFS RO | No integrated directory engine | Passed; both HAMMER2 routes rerun |
| macOS | 5 requested pairs: Apple APFS/exFAT, native encrypted APFS, VeraCrypt APFS/exFAT | Encrypted Apple NTFS RO, two cycles with write refusal | Six backing combinations attempted; prerequisites/engine limits prevent full mount lifecycles | Passed; two exFAT publication regressions verified |

Linux totals are 168 create/RW container pairs, 126 directory-backing passes and 30 RO imports: 324 passes. The 18 observed compatibility failures are six default-LUKS2/UFS1 imports and twelve CryFS backing cases on exFAT/UFS2. Unavailable routes are separate from those attempted failures. The three privileged encryption routes are LUKS2, LUKS1 and VeraCrypt. All six distributions also passed login-user VeraCrypt with ext4, Btrfs, XFS, NTFS and exFAT using the existing LKL route. Native APFS/UFS2/HAMMER2/ZFS support varies by installed provider, as detailed in the Linux matrix.

Each OS report contains all ten filesystem rows, including unavailable routes and the selected minimal dependency combination:

- [Linux](LINUX_RESULTS.md).
- [FreeBSD](FREEBSD_RESULTS.md).
- [OpenBSD](OPENBSD_RESULTS.md).
- [NetBSD](NETBSD_RESULTS.md).
- [DragonFly BSD](DRAGONFLY_RESULTS.md).
- [macOS](MACOS_RESULTS.md).

## Material limits retained

- Linux UFS1 is read-only; its native 1024-byte initial probe is incompatible with the default LUKS2 mapper's 4096-byte logical sectors. HAMMER2 is writable only on Ubuntu's sufficiently new unchanged kernel; the other five distributions use an upstream RO reader. Gentoo APFS is RO, and its ZFS kernel SDK is missing. openSUSE OpenZFS fails the upstream vendor-kernel ABI check. CryFS requires a hardlink operation missing from the tested exFAT/UFS2 backing routes. Experimental APFS/UFS2/HAMMER2 drivers retain their upstream feature limits.
- FreeBSD's packaged LKL route fails ordinary XFS/Btrfs append and close; those are read-only imports. Native GELI/NTFS creation is unavailable because the packaged 4 KiB formatter times out, including on an ordinary file; compatible imports work. PEFS/exFAT lacks a required symlink operation; PEFS/NTFS fails nested-file opens. Native controls reproduce the reported upstream limits.
- OpenBSD's existing crypto routes cannot enforce read-only attachment. Native ext4/XFS/APFS reader controls are not mislabeled CLI read-only passes. HAMMER2 would require excluded kernel patching; Btrfs/ZFS have no eligible route.
- NetBSD ext4 remains experimental; Btrfs/XFS/APFS are unavailable. HAMMER2 was built from unmodified upstream source against matching 11.1 sources with standard make-variable configuration.
- DragonFly's stock FUSE module builds/loads unmodified, but the tested exFAT/NTFS mount helpers fail upstream. Native ext4 payload reads fail in the unencrypted control too. Other FUSE-dependent readers remain blocked. Earlier durability findings are not requalified by ordinary-close payload passes.
- Buran's FSKit routes require operator enablement. The xlinuxfs App Store distribution is paid; no purchase was made. Source signing and excluded kernel-extension prerequisites remain explicit. CryFS/exFAT has the existing hard-link requirement. No upstream bypass or patch was attempted.

## Evidence and carried results

Detailed receipts, hashes, native controls, installation commands/versions, source pins and cleanup inventories are retained under ignored `target/goal-validation/filesystem-round/<os>/`. `review-index.json` indexes completed review receipts and their hashes. Reports retain original product and harness identities; a later documentation or platform-only commit does not relabel an earlier binary.

Completed independent review commits: DragonFly `2f793f9`, OpenBSD `1fc920a`, macOS `db26c02`, NetBSD `280a9e8`, FreeBSD `8e587b7`, Linux `8431491`. Source comparison in `carried-source-audit.json` verifies unchanged applicable platform dispatch, API/CLI/workflow, native lifecycle and supervisor inputs for carried results. The later CryFS initialization-mount relocation is Linux-only; the Darwin AppleDouble publication fix is separately exercised on Buran. The Linux review reconciled all 342 executed pair receipts, matched 23 retained installed CLI paths and 92 source/harness files to their recorded identities, and passed nine affected initialization replays without increasing distinct pair counts.

Final implementation checks on Arch passed 108 core/CLI/helper tests, with 12 ignored and zero failures, plus scoped Cargo checks and the Python PTY regression. All eleven targets built their native CLI/helper. GUI/workspace bootstrap was deliberately excluded from this CLI-only round; the reports list exact native test counts and artifact identities.

All eleven hosts have no owned active mounts, mappings, sessions or workers left. Existing system mounts/pools remain intact; FreeBSD's temporary `vfs.usermount` change was restored. Installed dependencies, closed failure fixtures and evidence are intentionally retained. All implementation and review agents have finished and released their hosts.
