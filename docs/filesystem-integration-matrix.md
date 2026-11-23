# Writable filesystem integration matrix

This proposal covers filesystems for data partitions, external drives, and
block devices exposed by VeraCrypt and other disk containers. It prioritizes
the main native filesystem on each OS, followed by established formats with
writable support across multiple OSes. See the
[OS filesystem survey](filesystem-support-matrix.md) for driver sources,
version restrictions, and the broader inventory.

**C** means create or format. **RW** means open and mount read/write.
**Native** uses host facilities. **Package** requires a separately installed
driver or tools. **Candidate** requires native qualification on the listed
driver and volume type. **Paid** means a supported commercial driver is
required. A dash leaves the OS/filesystem pair outside the proposed scope.
An **Open source** line names the native implementation or an alternative
with a write path. Under a dash, it records a research option. Windows
targets use drivers and tools that run directly on Windows.

| Filesystem | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD | Windows |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **ext4** | Native C/RW<br>Open source: Linux ext4 + e2fsprogs | Candidate C/RW, paid option<br>Open source: Ext4Kit (beta C/RW), xlinuxfs (RW) | Candidate native RW subset<br>Open source: ext2fs | — | —<br>Open source: fuse-ext2 (experimental RW) | — | Candidate RW, paid option<br>Open source: Ext4Windows, Ext4Fsd |
| **Btrfs** | Native C/RW<br>Open source: Linux Btrfs + btrfs-progs | —<br>Open source: xlinuxfs (RW) | —<br>Open source: fusefs-lkl (older kernel) | — | — | — | Candidate C/RW<br>Open source: WinBtrfs |
| **XFS** | Native C/RW<br>Open source: Linux XFS + xfsprogs | —<br>Open source: xlinuxfs (RW) | —<br>Open source: fusefs-lkl (older kernel) + xfsprogs | — | — | — | — |
| **APFS** | Candidate RW, paid option<br>Open source: linux-apfs-rw (experimental) | Native C/RW | — | — | — | — | Candidate RW, paid option<br>Open source: winapfs (experimental) |
| **UFS1 / FFSv1** | —<br>Open source: Linux UFS (experimental RW, select variants) | — | Native C/RW<br>Open source: FreeBSD UFS1 + newfs | Native C/RW<br>Open source: OpenBSD FFS1 + newfs | Native C/RW<br>Open source: NetBSD FFSv1 + newfs | Native C/RW<br>Open source: DragonFly UFS1 + newfs | —<br>Open source: UFS2Tool + Dokan (limited image RW) |
| **UFS2 / FFSv2** | —<br>Open source: Linux UFS or fuse-ufs (experimental RW) | — | Native C/RW<br>Open source: FreeBSD UFS2 + newfs | Native C/RW<br>Open source: OpenBSD FFS2 + newfs | Native C/RW<br>Open source: NetBSD FFSv2 and FFSv2ea + newfs | C/RW target, driver and formatter need qualification<br>Open-source port candidate: fuse-ufs | —<br>Open source: UFS2Tool + Dokan (limited image RW) |
| **HAMMER2** | —<br>Open source: linux_hammer2 (development RW) | — | —<br>Open source: freebsd_hammer2 (experimental RW) | —<br>Open source: openbsd_hammer2 (development port) | —<br>Open source: netbsd_hammer2 (development port) | Native C/RW<br>Open source: DragonFly HAMMER2 | — |
| **NTFS** | Native RW, formatter package C<br>Open source: ntfs or ntfs3, NTFS-3G + mkntfs | Candidate RW, paid option<br>Open source: NTFSTool, NTFS4Mac, NTFS-3G via FUSE-T | Candidate RW<br>Open source: NTFS-3G | Candidate RW<br>Open source: NTFS-3G | Candidate RW<br>Open source: NTFS-3G | —<br>Open source: NTFS-3G (needs working FUSE) | Native C/RW |
| **exFAT** | Native C/RW<br>Open source: Linux exFAT + exfatprogs | Native C/RW<br>Open source: legacy fuse-exfat (outside macOS runtime scope) | Package C/RW<br>Open source: fuse-exfat + exFAT utilities | Package C/RW<br>Open source: exfat-fuse + exFAT utilities | Package C/RW<br>Open source: fuse-exfat + exFAT utilities | Candidate C/RW once release FUSE works<br>Open source: fusefs-exfat + exfat-utils | Native C/RW |
| **OpenZFS** | Package pool and dataset C/RW<br>Open source: OpenZFS | Candidate pool and dataset C/RW<br>Open source: OpenZFS on macOS | Native pool and dataset C/RW<br>Open source: base OpenZFS | — | Candidate pool and dataset C/RW<br>Open source: NetBSD ZFS port | — | —<br>Open source: OpenZFS on Windows (release candidate) |

## Integration priorities

1. **Native data volumes.** Use ext4 and Btrfs on Linux, APFS on macOS,
   UFS2 on FreeBSD, FFSv2 on OpenBSD and NetBSD, HAMMER2 on DragonFly BSD,
   and NTFS on Windows. Keep XFS as an established Linux choice. Include
   both UFS generations in the scope for all four BSDs, with separate
   formatter and mount qualification for each OS and format.
2. **Portable volumes.** Start with exFAT across Linux, macOS and Windows
   and the packaged drivers on FreeBSD, OpenBSD and NetBSD. DragonFly BSD
   6.4.2 omits the FUSE kernel module required for its exFAT driver. NTFS
   gives Windows and Linux a journaled alternative. WinBtrfs is the
   Windows route to qualify for existing Linux Btrfs containers.
3. **Additional writable imports.** Qualify compatible ext4 on FreeBSD
   against its journal and feature limitations. On Windows, evaluate
   Ext4Windows through WinFsp and Ext4Fsd as free ext4 candidates alongside
   the paid option. Verify current Linux formatter defaults, journal
   recovery, and writable mounts on the actual partition and unlocked
   container device. Keep third-party APFS, ext4, and NTFS drivers optional.
   On macOS, evaluate Ext4Kit for ext4 writes and NTFSTool or NTFS4Mac for
   NTFS. APFS write candidates on Linux and Windows
   are experimental and require validation against macOS-created volumes.
4. **OpenZFS pools.** Add FreeBSD and Linux first as a distinct single-device
   pool workflow. Qualify pool creation or import, dataset mount, and clean
   export before offering it. Use common feature flags when testing exchange.
   macOS and NetBSD are follow-up candidates with port-specific constraints.

For each selected route, verify formatting where offered, writable mount,
file creation and modification, clean unmount, and persistence after reopen.
For cross-OS support, create and write on each supported host in turn, then
reopen and verify the data on the other host. Where one host supports writable
import only, create on the formatting host and test edits in both directions.
Qualify physical-device and container-backed routes separately.

## BSD UFS compatibility

Qualify cross-BSD writes separately from each OS's native create and mount
workflow. Test the disk format and journal together. NetBSD FFSv2ea has a
separate disk signature. DragonFly UFS1 uses spare inode fields for 48-bit
timestamps, so its compatibility depends on how the other OS uses those
fields. See https://wiki.netbsd.org/features/UFS2ea/ and
https://lists.dragonflybsd.org/pipermail/commits/2019-March/718475.html.

DragonFly's native UFS driver handles UFS1. The UFS2 target requires a
writable driver and formatter qualified on DragonFly. One candidate is
`fuse-ufs`, which documents experimental writes for FreeBSD UFSv2 and lists
Linux, FreeBSD, and OpenBSD as supported hosts. Evaluating a DragonFly port
requires a working FUSE kernel module, which the stock 6.4.2 build omits.
See https://github.com/realchonk/fuse-ufs and
https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/Makefile.

## Excluded from new integration

- **F2FS** meets the technical requirements for a writable filesystem. Its
  emphasis on NAND flash, including SD cards, eMMC, and SSDs, makes it a
  lower priority for this general-purpose scope. Also exclude **bcachefs**,
  **ReFS**, and **FAT12/16/32** as requested.
- **HFS+, ext2/ext3, HAMMER1, JFS, and ReiserFS** are lower-priority legacy
  choices. The selected filesystems cover their typical new-volume use cases.
- **BSD LFS, NILFS2, UDF, and the specialty flash and zoned filesystems** have
  narrower workloads. Dedicated cluster and SAN filesystems such as
  **GFS2, OCFS2, VxFS, StorNext, and IBM Storage Scale** carry separate
  deployment and management requirements.
- **ISO 9660, SquashFS, EROFS**, and other read-only formats or reader-only
  driver paths are outside the writable-volume scope.
