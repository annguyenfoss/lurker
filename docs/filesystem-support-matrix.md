# OS / Filesystem Research

## Planning summary

This planning summary follows the stated use case: format a decrypted physical partition or file container, then reopen and mount it for normal application file access. Tools run on the host OS and are freely obtainable by individual users; virtual machines, WSL, paid routes and extract–edit–reinsert workflows are excluded. Native and userspace mounts count equally. Separate host-installed formatters and mount drivers may be combined, but an unverified combination is not a confirmed creation route.

| Filesystem | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD | Windows |
| --- | --- | --- | --- | --- | --- | --- | --- |
| ext4 | ✅ | ☑️ | ☑️ | ⚠️ R/O | ☑️ | ⚠️ R/O | ☑️ |
| Btrfs | ✅ | ☑️¹ | ☑️ | — | — | — | ☑️ |
| XFS | ✅ | ☑️ | ☑️ | ⚠️ R/O | — | ⚠️ R/O | ⚠️ R/O |
| APFS | ☑️ | ✅ | ⚠️ R/O | ⚠️ R/O | — | ⚠️ R/O | ☑️² |
| UFS1 / FFSv1 | ☑️ | — | ✅ | ✅ | ✅ | ✅ | ⚠️ R/O |
| UFS2 / FFSv2 | ☑️ | ⚠️ R/O | ✅ | ✅ | ✅ | — | ⚠️ R/O |
| HAMMER2 | ✅³ | — | ☑️³ | ☑️ | ☑️ | ✅ | — |
| NTFS | ☑️ | ☑️ | ☑️ | ☑️ | ☑️ | ☑️ | ✅ |
| exFAT | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| ZFS / OpenZFS | ✅ | ✅ | ✅ | — | ☑️ | — | ✅³ |

**Legend**

- **✅** Full feature support.
- **☑️** Largely usable, with some filesystem features missing or unverified.
- **⚠️ R/O** Read-Only.
- **—** Unsupported.
- **¹** Disk/container creation not available.
- **²** Disk/container creation not available.
- **³** Largely usable; reliability not established.

Maturity is independent of feature completeness. Linux HAMMER2 physical-device reliability remains unqualified by its scratch-image tests. Windows OpenZFS is largely implemented; its remaining stream-renaming limitation does not prevent the core workflows.

This is filesystem capability research based on documentation/source inspection, not a claim that these combinations are integrated or runtime-qualified in Lurker.

## Detailed solution survey

The earlier per-solution survey below is retained in full, including alternatives excluded from the planning summary. Its detailed tags and counts describe that survey snapshot; the planning summary incorporates the later workflow and beta assessments.

Survey date 2026-11-24. Every previously researched offering is restored: 160
solution entries across 70 OS/filesystem cells, without consolidation.
Each cell contains tags only. Blank filesystem labels continue the preceding
filesystem; each entry matches the [solution key](#solution-key) in cell order.
The [tag guide](filesystem-research-tags.md) defines the research bar and the
feature benchmark. The [implementation assessment](filesystem-implementation-assessment.md)
records each solution’s verdict, limitations and sources.

All status tags are prefixes in this order: `Paid`, `Beta`, `Partial` or
`Partial*`, `Outdated`. Omitted statuses mean free, production, fully/largely
implemented and current, respectively. `Partial` records confirmed important
feature gaps; `Partial*` records undetermined completeness, with the missing
evidence explained in the assessment. This marks all 55 unresolved offerings
explicitly; 68 offerings have confirmed gaps and 37 are largely implemented.
This is documentation/source research, not runtime qualification or a Lurker
integration claim. Existing format/mount target tags are retained.

| Filesystem | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD | Windows |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **ext4** | F M R/W | Partial NF M R/W | Partial F M R/W | Partial F M R | Partial F M R | Partial F M R | Partial* F NM |
|  | Partial NF M R | Partial* NF M R/W | Beta Partial F M R/W |  | Beta Partial NF M R/W |  | Partial* F NM |
|  |  | Partial* NF MP R/W | Partial Outdated NF M R/W |  |  |  | Partial* FP NM |
|  |  | Partial NF M R |  |  |  |  | Partial NF M R/W |
|  |  | Beta Partial F M R/W |  |  |  |  | Partial NF M R |
|  |  | Beta Partial F M R/W |  |  |  |  | Partial* NF MP R |
|  |  | Paid Partial FP MP R/W (1) |  |  |  |  | Paid Partial* FP MP R/W (3) |
|  |  | Paid Partial* FP MP R/W (2) |  |  |  |  | Paid Partial* NF MP R/W (4) |
|  |  |  |  |  |  |  | Paid Partial* FP MP R/W (5) |
|  |  |  |  |  |  |  | Paid Partial* NF MP R (6) |
| **Btrfs** | F M R/W | Partial NF M R/W | Partial Outdated NF M R/W | — | — | — | Partial F M R/W |
|  | Partial NF M R | Partial* NF M R/W |  |  |  |  | Partial NF M R |
|  |  | Partial* NF MP R/W |  |  |  |  | Partial* NF MP R |
|  |  | Beta Partial NF M R |  |  |  |  | Paid Partial* NF MP R (3) |
|  |  | Paid Partial* FP MP R/W |  |  |  |  | Paid Partial* FP MP R/W (5) |
|  |  |  |  |  |  |  | Paid Partial* NF MP R (6) |
| **XFS** | F M R/W | Partial NF M R/W | F NM | Beta Partial NF M R | — | — | Partial NF M R |
|  | Partial NF M R | Partial* NF M R/W | Beta Partial NF M R |  |  |  | Partial* NF MP R |
|  |  | Partial* NF MP R/W | Partial Outdated NF M R/W |  |  |  | Paid Partial* NF MP R/W (3) |
|  |  | Beta Partial F M R/W |  |  |  |  | Paid Partial* FP MP R/W (5) |
|  |  | Paid Partial* FP MP R/W |  |  |  |  | Paid Partial* NF MP R (6) |
| **APFS** | Partial NF M R | F M R/W | Beta Partial NF M R | Beta Partial NF M R | — | — | Partial NF M R |
|  | Beta Partial F M R/W |  |  |  |  |  | Partial* NF MP R |
|  | Beta Partial NF M R |  |  |  |  |  | Beta Partial NF M R/W |
|  | Partial NF M R |  |  |  |  |  | Beta Partial NF MP R/LW |
|  | Paid Partial NF MP R/W |  |  |  |  |  | Paid Partial NF MP LR/W (7) |
|  |  |  |  |  |  |  | Paid Partial* MP R/W (8) |
|  |  |  |  |  |  |  | Paid Partial* NF MP R (6) |
|  |  |  |  |  |  |  | Paid Partial* FP MP R (13) |
| **UFS1 / FFSv1** | Partial FI M R | Partial* Outdated FI NM | F M R/W | F M R/W | F M R/W | F M R/W | Partial Outdated F MI R/LW |
|  | Beta Partial FI M R/W |  |  |  |  |  | Partial NF M R |
|  | Partial* Outdated FI NM |  |  |  |  |  |  |
|  | Partial NF M R |  |  |  |  |  |  |
| **UFS2 / FFSv2** | Partial FI M R | Partial* Outdated FI NM | F M R/W | F M R/W | F M R/W | FI NM | Partial Outdated F MI R/LW |
|  | Beta Partial FI M R/W |  | Beta Partial NF M R/W | Beta Partial NF M R/W |  |  | Partial NF M R |
|  | Partial* Outdated FI NM |  |  |  |  |  |  |
|  | Beta Partial NF M R/W |  |  |  |  |  |  |
|  | Partial NF M R |  |  |  |  |  |  |
| **HAMMER2** | Beta F M R/W | — | Beta Partial* F M R/W | FI NM | FI NM | F M R/W | FI NM |
|  | Beta Partial* NF M R |  | Beta Partial* NF M R |  |  |  |  |
|  | FI NM |  | FI NM |  |  |  |  |
| **NTFS** | Partial F M R/W | Partial* NF M R | Partial F M R/W | Partial* NF M R | Beta Partial NF M R/LW | Partial* Outdated NF M R | F M R/W |
|  | Partial F M R/W | Partial* NF MP R/W | Beta NF M R | Partial F M R/W | Partial F M R/W | F NM | Partial NF M R |
|  | Partial NF M R | Partial* NF MP R/W |  | Beta NF M R |  |  |  |
|  | Partial* NF M R/W | Partial NF MP R/W |  |  |  |  |  |
|  | Paid Partial* F M R/W | Partial F M R/W |  |  |  |  |  |
|  |  | Partial NF M R/W |  |  |  |  |  |
|  |  | Partial NF M R/W |  |  |  |  |  |
|  |  | Beta Partial F M R/W |  |  |  |  |  |
|  |  | Partial* NF MP R |  |  |  |  |  |
|  |  | Paid Partial* FP MP R/W (9) |  |  |  |  |  |
|  |  | Paid Partial* F M R/W (10) |  |  |  |  |  |
|  |  | Paid Partial* FP MP R/W (11) |  |  |  |  |  |
|  |  | Paid Partial* FP MP R/W (2) |  |  |  |  |  |
|  |  | Paid Partial* FP MP R/W (12) |  |  |  |  |  |
|  |  | Paid Partial* NF MP R/W (14) |  |  |  |  |  |
|  |  | Paid Partial* FP MP R/W (15) |  |  |  |  |  |
| **exFAT** | F M R/W | F M R/W | F M R/W | F M R/W | F M R/W | F NM | F M R/W |
|  | F M R/W | Paid Partial* FP MP R/W |  |  |  |  | NF M R |
|  | NF M R |  |  |  |  |  |  |
| **ZFS / OpenZFS** | F M R/W | F M R/W | F M R/W | — | Partial F M R/W | — | Beta Partial* F M R/W |
|  | Partial NF M R | Partial* NF M R/W |  |  |  |  | Partial NF M R |
|  |  |  |  |  |  |  | Paid Partial* NF MP R |

**Legend.** `F` formats both partitions and images, `FP` partitions, `FI` images,
`NF` neither. `M` mounts both targets, `MP` partitions, `MI` images, `NM` neither.
`R` is read-only and `R/W` is read/write. `LR`, `R/LW`, `LR/W` and `LR/LW`
indicate limited reads, limited writes, or both, as defined in the tag guide.
`Paid` flags retail offerings, `Beta` experimental maturity, `Partial` confirmed
important feature gaps, `Partial*` undetermined completeness, and `Outdated`
legacy maintenance status. These are independent prefixes; defaults are
implicit. `—` means the audit found no qualifying solution. A single
target tag reports that target. Where evidence leaves a capability
unverified, the notes identify the limitation and the line omits an
unverified tag. Formatting and mounting are independent, including when
a formatter is combined with a separate mount driver. Every `Partial*` has a
specific uncertainty and checked sources in the implementation assessment;
it does not assert a confirmed missing feature.

## Solution key

Names follow the top-to-bottom order of each cell. Every cell is listed.

### Linux

- **ext4** — ext4 + e2fsprogs, FsRover.
- **Btrfs** — Btrfs + btrfs-progs, FsRover.
- **XFS** — XFS + xfsprogs, FsRover.
- **APFS** — apfs-fuse, linux-apfs-rw + apfsprogs, libfsapfs, FsRover, Paragon APFS for Linux.
- **UFS1 / FFSv1** — makefs + Linux UFS, Linux UFS write option + makefs, UFS2Tool, FsRover.
- **UFS2 / FFSv2** — makefs + Linux UFS, Linux UFS write option + makefs, UFS2Tool, fuse-ufs, FsRover.
- **HAMMER2** — linux_hammer2 + hammer2-utils, hammer2-fuse, portable makefs.
- **NTFS** — kernel NTFS + mkntfs, NTFS-3G, FsRover, Paragon NTFS for Linux Free, Paragon NTFS for Linux Professional.
- **exFAT** — exFAT + exfatprogs, exfat-fuse + exfat-utils, FsRover.
- **ZFS / OpenZFS** — OpenZFS, FsRover.

### macOS

- **ext4** — xlinuxfs, anylinuxfs, PolyDrive Disk, ExtendFS, Ext4Kit, open_ext4_for_mac, Paragon extFS for Mac, vmount Pro.
- **Btrfs** — xlinuxfs, anylinuxfs, PolyDrive Disk, btrfs4mac, vmount Pro.
- **XFS** — xlinuxfs, anylinuxfs, PolyDrive Disk, fuse-xfs, vmount Pro.
- **APFS** — Disk Utility / APFS.
- **UFS1 / FFSv1** — UFS2Tool.
- **UFS2 / FFSv2** — UFS2Tool.
- **HAMMER2** — No qualifying solution found.
- **NTFS** — macOS, PolyDrive Disk, NTFSTool, TT NTFS Native, NTFS-3G + macFUSE, NTFS-3G for FUSE-T, anylinuxfs, NTFS4Mac, CrossDriver Free, Paragon NTFS for Mac, Tuxera NTFS for Mac, iBoysoft NTFS for Mac, vmount Pro, CrossDriver NTFS for Mac Pro, EaseUS NTFS for Mac, NTFSTool Pro.
- **exFAT** — Disk Utility / exFAT, vmount Pro.
- **ZFS / OpenZFS** — OpenZFS on OS X, anylinuxfs.

### FreeBSD

- **ext4** — ext2fs + e2fsprogs, fusefs-ext2, fusefs-lkl.
- **Btrfs** — fusefs-lkl.
- **XFS** — xfsprogs, libfsxfs, fusefs-lkl.
- **APFS** — libfsapfs.
- **UFS1 / FFSv1** — UFS1.
- **UFS2 / FFSv2** — UFS2, fuse-ufs.
- **HAMMER2** — hammer2 port, hammer2-fuse, portable makefs.
- **NTFS** — NTFS-3G, libfsntfs.
- **exFAT** — fusefs-exfat + formatter.
- **ZFS / OpenZFS** — OpenZFS.

### OpenBSD

- **ext4** — ext2fs + e2fsprogs.
- **Btrfs** — No qualifying solution found.
- **XFS** — libfsxfs.
- **APFS** — libfsapfs.
- **UFS1 / FFSv1** — FFSv1.
- **UFS2 / FFSv2** — FFSv2, fuse-ufs.
- **HAMMER2** — portable makefs.
- **NTFS** — mount_ntfs, ntfs-3g, libfsntfs.
- **exFAT** — exfat-fuse.
- **ZFS / OpenZFS** — No qualifying solution found.

### NetBSD

- **ext4** — ext2fs + e2fsprogs, fuse-ext2.
- **Btrfs** — No qualifying solution found.
- **XFS** — No qualifying solution found.
- **APFS** — No qualifying solution found.
- **UFS1 / FFSv1** — FFSv1.
- **UFS2 / FFSv2** — FFSv2.
- **HAMMER2** — portable makefs.
- **NTFS** — mount_ntfs, fuse-ntfs-3g + ntfsprogs.
- **exFAT** — fuse-exfat.
- **ZFS / OpenZFS** — base ZFS.

### DragonFly BSD

- **ext4** — ext2fs + e2fsprogs-core.
- **Btrfs** — No qualifying solution found.
- **XFS** — No qualifying solution found.
- **APFS** — No qualifying solution found.
- **UFS1 / FFSv1** — DragonFly UFS.
- **UFS2 / FFSv2** — portable makefs.
- **HAMMER2** — HAMMER2.
- **NTFS** — mount_ntfs, packaged mkntfs.
- **exFAT** — exfat-utils.
- **ZFS / OpenZFS** — No qualifying solution found.

### Windows

- **ext4** — DiskInternals Ext4 Formatter, DiskGenius Free, MiniTool Partition Wizard Free, Ext4Fsd, FsRover, CrossDriver Free, Paragon Linux File Systems for Windows, DiskInternals Linux Writer Pro, CrossDriver for Linux Disks Pro, DiskInternals Linux Reader Pro.
- **Btrfs** — WinBtrfs, FsRover, CrossDriver Free, Paragon Linux File Systems for Windows, CrossDriver for Linux Disks Pro, DiskInternals Linux Reader Pro.
- **XFS** — FsRover, CrossDriver Free, Paragon Linux File Systems for Windows, CrossDriver for Linux Disks Pro, DiskInternals Linux Reader Pro.
- **APFS** — FsRover, CrossDriver for Mac Disks Free, winapfs, apfs_for_windows, Paragon APFS for Windows, OWC MacDrive 12, DiskInternals Linux Reader Pro, CrossDriver for Mac Disks Pro.
- **UFS1 / FFSv1** — UFS2Tool, FsRover.
- **UFS2 / FFSv2** — UFS2Tool, FsRover.
- **HAMMER2** — portable makefs (Cygwin).
- **NTFS** — Windows NTFS, FsRover.
- **exFAT** — Windows exFAT, FsRover.
- **ZFS / OpenZFS** — OpenZFS on Windows, FsRover, DiskInternals Linux Reader Pro.

## Paid products

(1) Paragon extFS for Mac. (2) vmount Pro. (3) Paragon Linux File
Systems for Windows. (4) DiskInternals Linux Writer Pro. (5) CrossDriver for
Linux Disks Pro. (6) DiskInternals Linux Reader Pro. (7) Paragon APFS for
Windows. (8) OWC MacDrive 12. (9) Paragon NTFS for Mac. (10) Tuxera NTFS for
Mac. (11) iBoysoft NTFS for Mac. (12) CrossDriver NTFS for Mac Pro.
(13) CrossDriver for Mac Disks Pro. (14) EaseUS NTFS for Mac. (15) NTFSTool
Pro.

## Research notes and sources

### Across operating systems

- An attached image counts when it is exposed as a usable disk device and the
  formatter or mount driver can operate on it. Linux loop devices, macOS
  disk-image attachment, FreeBSD `mdconfig`, OpenBSD `vnconfig`, NetBSD
  `vndconfig`, DragonFly `vnconfig` and Windows VHD/VHDX are the standard paths.
  Raw images and partitioned images can require different attachment steps.
  https://man7.org/linux/man-pages/man8/losetup.8.html ·
  https://support.apple.com/guide/disk-utility/dskutl11888/mac ·
  https://man.freebsd.org/cgi/man.cgi?query=mdconfig&sektion=8 ·
  https://man.openbsd.org/vnconfig.8 · https://man.netbsd.org/vnd.4 ·
  https://man.dragonflybsd.org/?command=vnconfig&section=8 ·
  https://learn.microsoft.com/en-us/windows-server/storage/disk-management/manage-virtual-hard-disks
- A writable entry applies to the on-disk profiles the driver accepts.
  Cross-BSD UFS/FFS writes need a variant check, including journal and extended
  attribute formats. NetBSD UFS2ea has a distinct signature. ZFS creation means
  creating a pool with a filesystem. Its active feature flags determine
  whether another implementation can import the pool read/write, read-only,
  or at all. https://wiki.netbsd.org/features/UFS2ea/ ·
  https://openzfs.github.io/openzfs-docs/man/master/8/zpool-create.8.html ·
  https://openzfs.github.io/openzfs-docs/Basic%20Concepts/Pool%20Structure/Feature%20Flags.html
- FsRover exposes a real read-only Linux FUSE mount or Windows drive letter.
  Its published supported filesystem list and disk/image input list are the
  basis for those entries. GNU GRUB has separate UFS1 and UFS2 readers.
  https://a1ive.github.io/FsRover/ · https://github.com/a1ive/FsRover ·
  https://git.savannah.gnu.org/cgit/grub.git/tree/grub-core/fs/ufs.c ·
  https://git.savannah.gnu.org/cgit/grub.git/tree/grub-core/fs/ufs2.c

### Linux

- **ext4, Btrfs, XFS, exFAT.** Kernel drivers and their formatter projects cover
  physical devices and loop-attached images.
  https://docs.kernel.org/filesystems/ · https://github.com/tytso/e2fsprogs ·
  https://btrfs.readthedocs.io/en/latest/mkfs.btrfs.html ·
  https://git.kernel.org/pub/scm/fs/xfs/xfsprogs-dev.git/ ·
  https://github.com/exfatprogs/exfatprogs ·
  https://packages.debian.org/trixie/exfat-fuse · https://github.com/relan/exfat
- **APFS.** The apfs-fuse and libfsapfs routes mount read-only. libfsapfs is a
  pre-stable implementation. The Linux APFS kernel module and its formatter
  are experimental, with no encrypted volume support in that module. Paragon
  documents read/write access to supported APFS disks, no APFS formatting,
  and restrictions on encrypted volumes.
  https://github.com/sgan81/apfs-fuse ·
  https://github.com/libyal/libfsapfs/blob/main/manuals/fsapfsmount.1 ·
  https://github.com/linux-apfs/linux-apfs-rw · https://github.com/linux-apfs/apfsprogs ·
  https://www.paragon-software.com/home/apfs-linux/
- **UFS1 / FFSv1 and UFS2 / FFSv2.** Debian's `makefs` accepts versions 1 and
  2 and produces images. The Linux UFS mount needs the correct `ufstype`.
  The kernel documentation describes writes for `44bsd` and `ufs2`. The
  running kernel must enable the experimental write option. fuse-ufs is
  a separate experimental writer for FreeBSD UFS2, with no ACL support.
  UFS2Tool publishes a Linux image formatter and reserves its mount for Windows.
  https://manpages.debian.org/trixie/makefs/makefs.8.en.html ·
  https://docs.kernel.org/admin-guide/ufs.html ·
  https://github.com/torvalds/linux/blob/master/fs/ufs/Kconfig ·
  https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/fs/ufs/super.c ·
  https://github.com/realchonk/fuse-ufs · https://github.com/SvenGDK/UFS2Tool/releases
- **HAMMER2.** The Linux module project documents writable device and
  loop-backed mounts and uses `newfs_hammer2` from the utilities project.
  The separate hammer2-fuse implementation forces a read-only mount. The
  portable makefs implementation can build images independently.
  https://github.com/jdmanring/linux_hammer2 ·
  https://github.com/kusumi/hammer2-utils ·
  https://github.com/kusumi/hammer2-fuse/blob/master/src/main.rs ·
  https://github.com/kusumi/makefs
- **NTFS and ZFS.** Select the in-kernel NTFS driver available in the target
  kernel and install a formatter. NTFS-3G is a separate userspace driver and
  includes `mkntfs`. Paragon's Professional edition includes a formatter.
  The Free edition mounts read/write and lists Linux kernels through 7.0.x.
  OpenZFS is separately installed and documents file and device vdevs.
  https://docs.kernel.org/filesystems/ntfs3.html ·
  https://docs.kernel.org/filesystems/ntfs.html · https://github.com/tuxera/ntfs-3g ·
  https://www.paragon-software.com/home/ntfs-linux-professional/ ·
  https://github.com/openzfs/zfs

### macOS

- **ext4, Btrfs, XFS.** xlinuxfs covers attached images and partitions via
  FSKit, with the image attachment method depending on macOS version.
  PolyDrive Disk documents physical devices on Apple Silicon and macOS 26+.
  anylinuxfs provides ordinary app access through local NFS from its
  dedicated microVM. vmount Pro explicitly lists format and Finder access
  for its supported drive formats. A partition tag on these lines reflects
  the documented device workflow. The image workflow remains unverified.
  https://github.com/HuanchuanTech/xlinuxfs · https://polydrive.net/disk/ ·
  https://github.com/nohajc/anylinuxfs · https://www.vmount.app/
- **ext4 alternatives.** ExtendFS is read-only. Ext4Kit has a formatter and
  experimental writes. open_ext4_for_mac has an FSKit driver and formatter,
  with signing and provisioning requirements when building from source.
  Paragon extFS is a retail formatter and read/write mount for supported
  ext4 profiles. https://github.com/kthchew/ExtendFS ·
  https://github.com/rayhanadev/Ext4Kit ·
  https://github.com/h3ct0r/open_ext4_for_mac ·
  https://www.paragon-software.com/home/extfs-mac/
- **Btrfs and XFS alternatives.** btrfs4mac mounts devices and images through
  its read-only FUSE frontend. It does not replay the log tree or support
  all multi-device layouts. The separate fuse-xfs fork documents a
  formatter and opt-in writes on images and devices.
  https://github.com/gereoffy/btrfs4mac · https://github.com/karan-vk/fuse-xfs
- **APFS and exFAT.** Disk Utility supports the formats on physical devices
  and in disk images. vmount Pro also advertises drive formatting and
  Finder access for exFAT.
  https://support.apple.com/guide/disk-utility/dsku19ed921c/mac ·
  https://support.apple.com/guide/disk-utility/dskutl11888/mac ·
  https://www.vmount.app/
- **UFS1 / FFSv1, UFS2 / FFSv2, HAMMER2.** The archived UFS2Tool release can
  create UFS images on macOS. Its mount command is Windows-only. The audit
  found no macOS HAMMER2 mount or formatter that met the bar. The portable
  HAMMER2 formatter names Linux, Cygwin and the BSDs.
  https://github.com/SvenGDK/UFS2Tool/blob/v4.1/macOS.md ·
  https://github.com/SvenGDK/UFS2Tool/releases · https://github.com/kusumi/makefs
- **NTFS.** Apple documents read-only support. PolyDrive and NTFSTool offer
  writes through FSKit. TT NTFS Native publishes a macOS 26+ Apple Silicon
  release. The macFUSE route uses NTFS-3G with Mounty as an optional frontend.
  The FUSE-T port uses a separate transport. NTFS4Mac adds FSKit formatting and documents testing on
  Intel, including a mounted RAM disk, with Apple Silicon validation
  pending in the project checklist. Tuxera documents creating NTFS
  volumes and disk images. Paragon, iBoysoft, vmount Pro and CrossDriver
  Pro document physical-drive formatting and writable mounts. EaseUS sells
  a physical-drive mount, with mounting and ejection covered by its guide.
  NTFSTool Pro also documents an erase-as-NTFS operation on external drives.
  https://support.apple.com/en-us/101830 · https://polydrive.net/disk/ ·
  https://github.com/ntfstool/ntfstool · https://mounty.app/ ·
  https://github.com/macos-fuse-t/ntfs-3g · https://github.com/vahidzekic/ntfs4mac ·
  https://www.paragon-software.com/home/ntfs-mac/ · https://ntfsformac.tuxera.com/ ·
  https://iboysoft.com/ntfs-for-mac/ · https://www.crossdriver.com/product/ntfs-new ·
  https://ntfs.techtag.ch/ · https://toolbox.easeus.com/support/ntfsformac/ ·
  https://toolbox.easeus.com/ntfs-for-mac/buy.html ·
  https://ntfstool.com/external-drive-formats-mac
- **ZFS.** OpenZFS on OS X publishes installers with OS and architecture
  requirements. anylinuxfs publishes ZFS support using its microVM and
  mounts partitions and partitioned disk images.
  https://www.openzfsonosx.org/ · https://github.com/openzfsonosx/openzfs-fork/releases ·
  https://github.com/nohajc/anylinuxfs

### FreeBSD

- **ext4, Btrfs, XFS.** Native ext2fs supports read/write on compatible ext4
  profiles without journal replay or encryption. The separate fuse-ext2
  port advertises writes and uses e2fsprogs. The LKL port mounts all three
  formats but pins Linux 4.16-era code and is architecture-restricted.
  xfsprogs provides a formatter. libfsxfs supplies a separate read-only
  FUSE mount. https://man.freebsd.org/cgi/man.cgi?query=ext2fs&sektion=4 ·
  https://github.com/freebsd/freebsd-ports/tree/main/filesystems/ext2 ·
  https://github.com/freebsd/freebsd-ports/tree/main/filesystems/lkl ·
  https://github.com/freebsd/freebsd-ports/tree/main/filesystems/xfsprogs ·
  https://github.com/freebsd/freebsd-ports/tree/main/filesystems/libfsxfs
- **APFS.** libfsapfs ships `fsapfsmount` with FUSE. A public apfs-fuse
  patch exists separately. The audit confirmed the packaged libfsapfs route.
  https://github.com/freebsd/freebsd-ports/tree/main/filesystems/libfsapfs ·
  https://github.com/sgan81/apfs-fuse/pull/139
- **UFS1 / FFSv1, UFS2 / FFSv2, HAMMER2.** Base `newfs -O 1` and
  `newfs -O 2` cover both UFS formats. fuse-ufs is an experimental UFS2
  writer. The HAMMER2 port includes a formatter and a kernel mount with
  experimental writes. Match the module to the running kernel. The
  separate hammer2-fuse implementation is read-only. Portable makefs
  provides a standalone image formatter.
  https://man.freebsd.org/cgi/man.cgi?query=newfs&sektion=8 ·
  https://github.com/realchonk/fuse-ufs ·
  https://github.com/freebsd/freebsd-ports/tree/main/filesystems/hammer2 ·
  https://github.com/kusumi/hammer2-fuse · https://github.com/kusumi/makefs
- **NTFS, exFAT, ZFS.** The handbook documents packaged NTFS-3G and exFAT
  mounts using FreeBSD's FUSE kernel module. Formatters come from NTFS-3G
  and the separate exFAT utilities. libfsntfs provides an experimental
  read-only mount. Base OpenZFS supports block and file-backed pool creation
  and dataset mounts.
  https://docs.freebsd.org/en/books/handbook/filesystems/ ·
  https://github.com/freebsd/freebsd-ports/tree/main/filesystems/exfat-utils ·
  https://github.com/freebsd/freebsd-ports/tree/main/filesystems/libfsntfs ·
  https://docs.freebsd.org/en/books/handbook/zfs/

### OpenBSD

- **ext4, Btrfs, XFS, APFS.** The ext2fs driver accepts a read-only subset of
  ext4, with e2fsprogs providing independent formatting. A formatter's
  default ext4 profile can enable incompatible features. Current ports ship
  `fsxfsmount` and `fsapfsmount` as experimental read-only FUSE paths for
  XFS and APFS. No qualifying Btrfs route was found in the base/ports or
  publicly documented drivers checked.
  https://github.com/openbsd/src/blob/master/sys/ufs/ext2fs/ext2fs_vfsops.c ·
  https://github.com/openbsd/ports/tree/master/sysutils/e2fsprogs ·
  https://github.com/openbsd/ports/tree/master/sysutils/libyal/libfsxfs ·
  https://github.com/openbsd/ports/tree/master/sysutils/libfsapfs
- **UFS1 / FFSv1, UFS2 / FFSv2, HAMMER2.** Base `newfs` handles both FFS
  generations. The fuse-ufs project documents an OpenBSD build targeting
  FreeBSD UFS2. The portable `makefs` project produces HAMMER2 images.
  The separate OpenBSD HAMMER2 kernel port source exists, with a usable
  mount on a current OpenBSD release unverified in this audit.
  https://man.openbsd.org/OpenBSD-8.0/newfs.8 · https://github.com/realchonk/fuse-ufs ·
  https://github.com/kusumi/makefs · https://github.com/kusumi/openbsd_hammer2
- **NTFS, exFAT, ZFS.** Base NTFS mounts read-only. The packaged NTFS-3G and
  exFAT routes provide formatters and writable FUSE mounts. libfsntfs is
  a separate experimental read-only mount. The upstream ZFS platform list
  and OpenBSD base and ports yielded no qualifying ZFS route.
  https://man.openbsd.org/mount_ntfs.8 · https://man.openbsd.org/fuse.4 ·
  https://github.com/openbsd/ports/tree/master/sysutils/ntfs-3g ·
  https://github.com/openbsd/ports/tree/master/sysutils/exfat-fuse ·
  https://github.com/openbsd/ports/tree/master/sysutils/libyal/libfsntfs ·
  https://github.com/openbsd/ports · https://github.com/openzfs/zfs

### NetBSD

- **ext4, Btrfs, XFS, APFS.** The native ext2fs implementation can read
  compatible ext4 profiles. pkgsrc e2fsprogs supplies formatting. The
  fuse-ext2 package calls its write support unsafe for valuable data.
  No qualifying Btrfs, XFS or APFS route appeared in the base/pkgsrc and
  upstream ports checked. https://www.netbsd.org/about/interop.html ·
  https://github.com/NetBSD/src/blob/netbsd-11/sys/ufs/ext2fs/ext2fs_vfsops.c ·
  https://github.com/NetBSD/pkgsrc/tree/trunk/sysutils/e2fsprogs ·
  https://github.com/NetBSD/pkgsrc/tree/trunk/filesystems
- **UFS1 / FFSv1, UFS2 / FFSv2, HAMMER2.** Base `newfs` can create ordinary
  FFSv1 and FFSv2, with FFSv2ea a separate profile. The portable
  `makefs` formats HAMMER2 images. A separate kernel port exists, with a
  usable mount on current NetBSD unverified in this audit.
  https://man.netbsd.org/NetBSD-11.1/newfs.8 ·
  https://wiki.netbsd.org/features/UFS2ea/ · https://github.com/kusumi/makefs ·
  https://github.com/kusumi/netbsd_hammer2
- **NTFS, exFAT, ZFS.** Base NTFS writes are experimental in-place
  overwrites on eligible files only, with no create, delete or resize.
  NetBSD recommends the base driver in read-only mode. For normal writes,
  pkgsrc offers `fuse-ntfs-3g` and a separate `ntfsprogs` formatter.
  pkgsrc exFAT includes its formatter. Base ZFS is available on supported
  architectures. The NetBSD project documents lockups on low-memory systems.
  https://man.netbsd.org/NetBSD-11.1/mount_ntfs.8 ·
  https://github.com/NetBSD/pkgsrc/tree/trunk/filesystems/fuse-ntfs-3g ·
  https://github.com/NetBSD/pkgsrc/tree/trunk/sysutils/ntfsprogs ·
  https://github.com/NetBSD/pkgsrc/tree/trunk/filesystems/fuse-exfat ·
  https://wiki.netbsd.org/zfs/

### DragonFly BSD

- **ext4, Btrfs, XFS, APFS.** Stock 6.4.2 ext2fs reads compatible ext4
  profiles and describes extents as read-only. DPorts supplies a formatter.
  DPorts libfsapfs and libfsxfs mounts require FUSE. The stock kernel excludes
  its FUSE module and no matching module package was found in the 6.4
  repository. No usable Btrfs route was found in base or DPorts.
  https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/ext2fs/ext2fs.h ·
  https://github.com/DragonFlyBSD/DPorts/tree/master/filesystems/e2fsprogs-core ·
  https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/Makefile ·
  https://avalon.dragonflybsd.org/dports/dragonfly:6.4:x86:64/LATEST/
- **UFS1 / FFSv1, UFS2 / FFSv2, HAMMER2.** The installation handbook uses
  native UFS1 and HAMMER2. The portable FreeBSD-derived `makefs` supports
  producing UFS2 images on DragonFly. No UFS2 mount was confirmed for
  stock 6.4.2. https://www.dragonflybsd.org/docs/handbook/Installation/ ·
  https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/ufs/fs.h ·
  https://github.com/kusumi/makefs/blob/master/src/usr.sbin/makefs/ffs.c
- **NTFS, exFAT, ZFS.** The legacy base NTFS mount is read-only. NTFS-3G
  and exFAT package formatters work independently of FUSE. The userspace
  mounts need a kernel module that the 6.4 release build omits. The 6.4
  package manifest for fusefs-ntfs includes `mkntfs`. The
  development branch restored the module and received I/O fixes, with
  DragonFly exFAT/NTFS mounts on that branch unverified here. No eligible ZFS
  driver or formatter was found in base or the official package catalog.
  https://man.dragonflybsd.org/?command=mount_ntfs&section=8 ·
  https://github.com/DragonFlyBSD/DPorts/tree/master/filesystems/exfat-utils ·
  https://github.com/relan/exfat/blob/v1.4.0/libexfat/io.c ·
  https://avalon.dragonflybsd.org/dports/dragonfly:6.4:x86:64/LATEST/All/fusefs-ntfs-2022.10.3_2.pkg ·
  https://github.com/DragonFlyBSD/DragonFlyBSD/commit/4d178a6a8bdff7603d9bcb6c5fff68a4e2efd337 ·
  https://github.com/DragonFlyBSD/DragonFlyBSD/commit/053116cafd76a7ce620ebdfedcf6869e986e327d

### Windows

- **ext4, Btrfs, XFS.** Ext4Fsd mounts the supported ext4 feature subset.
  The separate DiskInternals formatter documents physical and image targets.
  DiskGenius documents ext4 formatting in its partition formatter and
  supports formatting partitions in disk images. MiniTool's Free edition
  documents formatting physical ext4 partitions.
  WinBtrfs includes `mkbtrfs` and a Windows filesystem driver. Paragon's
  current product page says XFS is writable, while older vendor documents
  list it read-only. It can format ext4, not XFS or Btrfs. DiskInternals
  Writer Pro provides ext4 drive-letter writes and Reader Pro offers
  read-only drive letters. CrossDriver Free advertises read-only drive
  letters. Pro adds writing and formatting. Its dedicated VM uses Windows
  Virtual Machine Platform and exposes the drive to Windows apps.
  Physical-only commercial lines report documented physical workflows.
  https://github.com/bobranten/Ext4Fsd · https://www.diskinternals.com/ext4-formatter/ ·
  https://www.diskgenius.com/manual/format-partition.php ·
  https://www.diskgenius.com/manual/operate-virtual-disk.php ·
  https://www.partitionwizard.com/partitionmanager/format-ext4-windows.html ·
  https://github.com/maharmstone/btrfs ·
  https://www.paragon-software.com/home/linuxfs-windows/ ·
  https://download.paragon-software.com/doc/Paragon_Linux_File_Systems_Windows.pdf ·
  https://www.diskinternals.com/linux-writer/ · https://www.diskinternals.com/linux-reader/ ·
  https://www.crossdriver.com/product/linux-disk-for-windows
- **APFS.** winapfs supports physical devices and images with experimental
  writes on compatible unencrypted data volumes. apfs_for_windows
  separately documents restrictions on writable directory size and nesting.
  Paragon and MacDrive provide retail drive-letter mounts. Paragon's
  read/write path requires a compatible single-store volume. Some compressed
  system files are unreadable, and snapshot volumes mount read-only. OWC
  confirms APFS mounts but describes formatting only for Mac disks in
  general, so MacDrive APFS formatting remains unverified. CrossDriver Free
  provides read-only drive-letter access. CrossDriver Pro formats APFS
  on physical devices. Its APFS mount writes still depend on a confirmed
  product release. Linux Reader Pro advertises APFS access and drive-letter
  mapping.
  https://github.com/christofilojohn/winapfs ·
  https://github.com/RandyNorthrup/apfs_for_windows ·
  https://www.paragon-software.com/home/apfs-windows/ ·
  https://paragon-software.zendesk.com/hc/en-us/articles/16290128217745-APFS-for-Windows-Supported-Volumes ·
  https://www.owc.com/solutions/macdrive · https://fs.crossdriver.com/en/mac-disk/ ·
  https://www.diskinternals.com/linux-reader/
- **UFS1 / FFSv1, UFS2 / FFSv2, HAMMER2.** UFS2Tool can format devices and
  images, with a Dokan mount for images. It can modify existing files
  and rejects creating new files through the mount. The archived v4.1
  source implements both routes. Portable `makefs` documents Cygwin
  support for creating HAMMER2 images. No Windows HAMMER2 mount was found.
  https://github.com/SvenGDK/UFS2Tool/tree/v4.1 ·
  https://github.com/SvenGDK/UFS2Tool/blob/v4.1/Program.cs#L2574-L2609 ·
  https://github.com/SvenGDK/UFS2Tool/blob/v4.1/Ufs2DokanOperations.cs#L97-L127 ·
  https://github.com/kusumi/makefs
- **NTFS, exFAT, ZFS.** Windows supports formatting and mounting both
  partition and attached VHD/VHDX targets for NTFS and exFAT. Windows
  OpenZFS publishes prerelease builds, with disk and file vdev code and
  Windows drive-letter integration. Linux Reader Pro advertises a
  separate read-only ZFS access path.
  https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/format ·
  https://learn.microsoft.com/en-us/windows-server/storage/disk-management/manage-virtual-hard-disks ·
  https://github.com/openzfsonwindows/openzfs/releases ·
  https://github.com/openzfsonwindows/openzfs/blob/development/module/zfs/vdev_file.c ·
  https://www.diskinternals.com/linux-reader/
