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
Windows targets use drivers and tools that run directly on Windows.

| Filesystem | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD | Windows |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **ext4** | Native C/RW | Candidate Ext4Kit or paid C/RW | Candidate native RW for compatible profiles | — | — | — | Candidate Ext4Windows or Ext4Fsd RW, paid RW alternative |
| **Btrfs** | Native C/RW | — | — | — | — | — | Candidate WinBtrfs C/RW |
| **XFS** | Native C/RW | — | — | — | — | — | — |
| **APFS** | Candidate experimental `linux-apfs-rw` or paid RW | Native C/RW | — | — | — | — | Candidate experimental `winapfs` or paid RW for supported volumes |
| **UFS1 / FFSv1** | — | — | Native UFS1 C/RW | Native FFS1 C/RW | Native FFSv1 C/RW | Native UFS1 C/RW | — |
| **UFS2 / FFSv2** | — | — | Native UFS2 C/RW | Native FFS2 C/RW | Native FFSv2 and FFSv2ea C/RW | Target C/RW, needs a qualified driver and formatter | — |
| **HAMMER2** | — | — | — | — | — | Native C/RW | — |
| **NTFS** | Native RW, formatter package C | Candidate FSKit or paid RW | Candidate NTFS-3G RW | Candidate NTFS-3G RW | Candidate NTFS-3G RW | — | Native C/RW |
| **exFAT** | Native C/RW | Native C/RW | Package C/RW | Package C/RW | Package C/RW | Candidate C/RW once release FUSE works | Native C/RW |
| **OpenZFS** | Package pool and dataset workflow | Candidate package pool and dataset workflow | Native pool and dataset workflow | — | Candidate pool and dataset workflow | — | — |

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
