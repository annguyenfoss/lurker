# Writable filesystem integration matrix

This proposal covers filesystems for data partitions, external drives, and
block devices exposed by VeraCrypt and other disk containers. It prioritizes
the main native filesystem on each OS, followed by established formats with
writable support across multiple OSes. See the
[OS filesystem survey](filesystem-support-matrix.md) for driver sources,
version restrictions, and the broader inventory.

**C** = create or format. **RW** = mount read/write. **Package** = install
separately. **Paid** = commercial driver. A named driver alongside **Paid**
is an open-source alternative. **—** = outside the plan. Third-party
drivers and cells marked **Candidate** or **Target** still need Lurker
testing on the target OS. Windows routes run directly on Windows.

| Filesystem | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD | Windows |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **ext4** | Native C/RW | Paid C/RW | Candidate RW (subset) | — | — | — | Paid RW / Ext4Fsd RW (subset) |
| **Btrfs** | Native C/RW | — | — | — | — | — | WinBtrfs C/RW |
| **XFS** | Native C/RW | — | — | — | — | — | — |
| **APFS** | Paid RW | Native C/RW | — | — | — | — | Paid RW |
| **UFS1 / FFSv1** | — | — | Native C/RW | Native C/RW | Native C/RW | Native C/RW | — |
| **UFS2 / FFSv2** | — | — | Native C/RW | Native C/RW | Native C/RW | Target C/RW | — |
| **HAMMER2** | — | — | — | — | — | Native C/RW | — |
| **NTFS** | Native RW + formatter package | Paid RW / NTFSTool RW (26+) | Package RW | Package RW | Package RW | — | Native C/RW |
| **exFAT** | Native C/RW | Native C/RW | Package C/RW | Package C/RW | Package C/RW | Target C/RW | Native C/RW |
| **OpenZFS** | Package C/RW | Candidate C/RW | Native C/RW | — | Candidate C/RW | — | — |

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
   against its journal and feature limitations. Test Ext4Fsd on Windows
   against current Linux formatter defaults, journal recovery, and writable
   mounts on partitions and unlocked containers. Test NTFSTool on macOS 26
   or later, including writable mounts of unlocked containers. Keep
   third-party APFS, ext4, and NTFS drivers optional. See the OS filesystem
   survey for beta and experimental drivers.
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
