# Previously unachieved filesystem retests

This record supplements, rather than replaces, the original [round results](RESULTS.md). Retests targeted previously unachieved CLI container or directory-backing cases. Existing passing payloads were not repeated except for the DragonFly ext4 case after its base userland changed. The tested payload is PDF and text with normal close/reopen, mutation and hash verification. Windows, physical devices, GUI flows, SQLite and fault tests remain outside this round.

## New results

| Target | Newly passing cases | Scope and conditions |
| --- | --- | --- |
| Linux, six distributions | UFS1: 12 encrypted containers and 18 directory backings | LUKS1 and privileged VeraCrypt; three directory engines. Requires an unmodified UFS module built with the upstream write option. Default LUKS2 remains incompatible with the native 512-byte probe. |
| Linux, Arch and Fedora | HAMMER2: six encrypted containers and six directory backings | Stock 7.3 kernels and unmodified HAMMER2 module. |
| Linux, Debian, Gentoo and openSUSE | HAMMER2: nine encrypted containers and nine directory backings | Tested on official alternate 7.3 kernels; each host was returned to its retained stable default kernel. Writable HAMMER2 therefore requires booting the tested newer kernel and its module. Ubuntu had already passed. |
| Linux, Gentoo | APFS and ZFS: six encrypted containers and six directory backings | Matching kernel SDK enabled the unmodified providers. |
| Linux, openSUSE | ZFS: three encrypted containers and three directory backings | Correct split kernel SDK source/object paths resolved the prior build failure. |
| Linux, six distributions | CryFS on native UFS2: six directory-backing cases | Tested on manually mounted disposable native UFS2 backing with write-enabled UFS. This does not establish the same backing through Lurker's existing encrypted UFS2 container path, which uses fuse-ufs and remains hardlink-limited. |
| macOS | ext4 and XFS: two VeraCrypt encrypted containers | Private unmodified FUSE-T drivers access the existing authenticated decrypted regular image. Apple-image and other encryption routes are not claimed. |
| DragonFly BSD | ext4: one LUKS1 RO import | Passed on official stock 6.5 kernel and matching world with the tested compatible 4 KiB ext4 profile. |

The Linux retest has 84 distinct new passing cases: 63 before distribution handoff, ten additional on openSUSE, seven on Gentoo, and one native-UFS2 CryFS backing case each on Arch, Debian, Ubuntu and Fedora. macOS adds two CLI pairs and DragonFly adds one RO CLI pair. Native controls and repeated affected DragonFly verification are not included in these totals.

## Remaining limitations

- Linux UFS1/default LUKS2 remains refused: the selected mapper exposes 4096-byte logical sectors while the kernel's `44bsd` probe starts at 512 bytes. CryFS/exFAT still lacks the required hardlinks. On Fedora, after the kernel update, OpenZFS availability was restored with unmodified upstream main `76c1fb8` (2.4.99); module loading and empty pool/dataset checks passed, but prior payload results used stable 2.4.4 on the older kernel. No previously passed payload was repeated.
- FreeBSD Btrfs/XFS ordinary append still fails with both packaged LKL and a newly built unmodified upstream LKL. A separate unmodified fileshare/NFS driver passed native raw-image controls, including a formatter combination, but its writable loopback export lacks user authentication. No Lurker CLI pass is claimed for that route.
- OpenBSD ext4/XFS/APFS CLI RO attempts remain blocked before authentication by the existing crypto attachment boundary. HAMMER2's kernel port requires an excluded patch; the assessed userspace reader is RO-only.
- DragonFly XFS/APFS readers fail on encrypted vnode device I/O; NTFS-3G hits vnode locking errors. The exFAT native driver works in a control, but the ordinary FUSE mount helper does not expose sufficient source identity for the existing Lurker lifecycle. A discarded native-mount prototype panicked the disposable guest on ordinary close; the guest was recovered, no upstream change was made, and the final stock kernel/world and cleanup were verified.
- macOS Btrfs, UFS2 RO, NTFS RW and ZFS remain blocked or unqualified. PolyDrive was incorrectly treated as free: its download provides a three-day trial, with a paid lifetime unlock, so it is excluded by the free-of-charge requirement regardless of the earlier unexplained download stall. The tested NTFS-3G driver rejects the existing VeraCrypt auxiliary image's required lock. ZFS needs the excluded Buran kernel-extension/security/restart setup. See the macOS report's subsequent installation and license audit for the later package state.
- NetBSD's seven promised filesystem rows were already achieved. Its owner reconciled prior receipts and ran no new payload tests.

## Evidence and validation

Detailed reports: [Linux checkpoint](LINUX_RETEST.md), [macOS](MACOS_RETEST.md), [FreeBSD](FREEBSD_RETEST.md), [OpenBSD](OPENBSD_RETEST.md), [NetBSD](NETBSD_RETEST.md), and [DragonFly](DRAGONFLY_RETEST.md). The Linux checkpoint predates distribution-owner continuations; their complete receipts and final inventories are retained under `target/goal-validation/filesystem-retest/`, including `gentoo/HANDOFF.md`, `opensuse/HANDOFF.md` and each other distribution's agent JSON.

The exact macOS candidate passed workspace check and tests: 108 passed, 11 ignored; this included existing headless desktop unit tests, not a desktop user flow. Linux scoped core/CLI/helper checks passed 108 tests, 12 ignored; the later diagnostic-only change passed an exact-source core check. All agents report no owned active mounts, mappings, loops, pools, images or workers, with the exception that individual OS resource types vary. Preserved fixtures, installed providers, alternate kernels and logs remain as recorded in the OS reports.
