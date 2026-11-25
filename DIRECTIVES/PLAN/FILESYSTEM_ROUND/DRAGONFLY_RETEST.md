# DragonFly previously missing filesystem retest

2026-11-25. Scope: ext4/XFS/APFS read-only imports, NTFS full read/write,
and exFAT read/write. Previously achieved UFS1, HAMMER2, FAT32 and NTFS
read-only cases were not repeated. No GUI, physical storage, SQLite or
intentional fault workload was run. No encryption backend was added.

## Artifacts and prerequisite changes

Product revision `b474181` was archived and built unprivileged on
`adjutant@dragonflybsd-core-01`. Native core/CLI/helper tests passed:
69 passed, 3 ignored, 0 failed. Root-owned staging is
`/opt/lurker-fs-retest-dragonfly/`; CLI SHA-256 is
`5be707aa3b62de986f09ceadc3c5bf69c6ac32cf765862484ea3563a015c9912`.
The existing protocol-2 private tcplay backend was copied unchanged; none
of the retests used VeraCrypt or new encryption profiles.

The repaired official package manager installed e2fsprogs-core 1.47.4,
fusefs-ext2 0.0.11_2, fusefs-ntfs 2022.10.3_2, exfat-utils/fusefs-exfat
1.4.0_1, libfsxfs 20240501, libfsapfs 20240429 and their dependencies.
No manual package-database edits were made.

Stock 6.4.2 was upgraded using the official, unmodified
`v6.5.0.2269.g9f0e62-DEVELOPMENT` GENERIC snapshot from 2026-11-24.
The BOOT archive SHA-256 is
`dccc46697156bb652cd441bf8e7193a773d07d453d991362fd09ae046d066634`.
The matching official world archive SHA-256 is
`d0043d6463b1feaaca62590f1eef62eb7bced66f01e3ed064ec1df9ad179ca7d`,
matching its published checksum. The original kernel is preserved at
`/boot/kernel.lurker-original`; the original base world is backed up under
`/var/tmp/lurker-fs-retest-dragonfly/world-before.tar.gz`.

Official inputs:

- https://avalon.dragonflybsd.org/snapshots/x86_64/kernels/DragonFly-x86_64-20261124-DEV-v6.5.0.2269.g9f0e62.boot.tar.bz2
- https://avalon.dragonflybsd.org/snapshots/x86_64/assets/releases/DragonFly-x86_64-20261124-DEV-v6.5.0-2269-g9f0e625c6f.world.tar.gz
- https://avalon.dragonflybsd.org/snapshots/x86_64/assets/releases/DragonFly-x86_64-20261124-DEV-v6.5.0-2269-g9f0e625c6f.world.tar.gz.sha256

## Results

| Missing case | Retest outcome |
| --- | --- |
| ext4 RO | **New CLI pass on stock 6.5.** A fresh native LUKS1 import contained independently formatted/populated PDF and UTF-8 text. Native encryption close preceded CLI reopen; both payload hashes matched and CLI closed normally. The same ordinary 4 KiB/no-64bit profile still failed payload reads with EINVAL on 6.4.2 immediately before upgrading. |
| XFS RO | Packaged libfsxfs reads the populated ordinary image, but vnode-backed reads/stat fail because the reader makes unaligned device reads. A mount table entry alone was not counted as success; the failed control was closed normally. No encrypted CLI pass. |
| APFS RO | Packaged libfsapfs reads the populated ordinary image, but opening the vnode fails with EINVAL on a byte-sized metadata read. No encrypted CLI pass. |
| NTFS RW | Current stock mount_fusefs removes the old unsupported `nempty` option blocker, but NTFS-3G rejects the vnode with `Failed to write lock ... Invalid argument`. No RW pass. The existing LUKS1/NTFS RO result was not repeated. |
| exFAT RW | Ordinary-image formatting and real vnode-backed FUSE write/read/normal-close controls worked on 6.5. Lurker integration remains unqualified because the ordinary upstream mount helper exposes only generic `/dev/fuse` identity, described below. No CLI pass. |

The ext4 formatter command is ordinary upstream configuration:
`mkfs.ext4 -F -b 4096 -O ^64bit,^metadata_csum,^metadata_csum_seed,^orphan_file`.
It retains an ext4 filesystem with extents and a journal; this does not
qualify arbitrary ext4 feature profiles or writes. Harness commit
`cd059e2` adds `--ext4-profile 4k-compat` for reproducing that exact profile
without changing the original default. Its Python compilation passed on
the native guest. No product adapter or encryption policy was changed.

The initial 6.5-kernel ext4 receipt is
`/var/tmp/dfret/import-ext4-24jdnr7d/result.json`; the corresponding 6.4
failure is `/var/tmp/dfret/import-ext4-8ehyt1kd/result.json`.
Replacing the native mount/cryptsetup userland with the matching official
world required one affected final retest. It passed again:
`/var/tmp/dfret/import-ext4-dx8xz214/result.json`.
All retain the exact native/CLI commands and normal cleanup results.

## FUSE identity investigation and rejected prototype

Stock 6.5 includes a usable FUSE kernel module. The unmodified matching
upstream `mount_fusefs` helper was also built and installed while testing
the otherwise unchanged 6.4 userland; it accepts NTFS-3G's `nempty`
option. The matching world upgrade supplies the stock helper afterward.

The ordinary helper records every tested mount as source `/dev/fuse` and
filesystem `fuse`. Explicit ordinary `fsname=/dev/mapper/lurker_probe` and
`subtype=lurkerprobe` options did not produce a distinct kernel mount
identity. `subtype.log` and the bounded ktrace/kdump evidence retain this
control. Lurker's shared BSD lifecycle requires verification of the
specific mapper before accepting a mount or releasing its crypto device;
accepting an arbitrary FUSE mount would weaken that invariant.

An isolated native FUSE API prototype tried supplying the backing vnode
as the mount source while using the stock libfuse inherited-descriptor
interface. It mounted, but its ordinary close hit the stock kernel's
`diskclose` open-count assertion. This was an invalid identity-adapter
candidate, not a payload pass or an intentional fault test. The prototype
was abandoned without repository integration, kernel changes or upstream
patches, and was not repeated. The dedicated guest was recovered from
the debugger through its libvirt console. The panic screenshot is retained.
No unrelated mounts or pre-existing Lurker sessions were active.

XFS/APFS ordinary fixtures were reused from the earlier preserved
independent OpenBSD fixtures, without rerunning previously passed cases.
Their raw SHA-256 values are respectively
`a9cc0eb76258393a3e6c4b67e45df11eaea43acfef8f920c17c12809e19b54c7`
and `50037c6612d6a5e5d1b46f48c2d44ff19e249cbed2958db3470f6c632ef8c9e2`.
Original provenance and expected payload hashes are retained locally.

## Evidence and final state

Local evidence is under
`target/goal-validation/filesystem-retest/dragonfly/`; remote logs and
upstream inputs are under `/var/tmp/lurker-fs-retest-dragonfly/`, with
closed disposable fixtures under `/var/tmp/dfret/`.

The guest remains on the official matching stock 6.5 kernel and world.
The world installation preserved `/etc`, `/var`, `/home`, `/usr/local`,
and boot configuration. It fixed the observed 6.4-userland `ps`, `fstat`
and `kldstat` ABI mismatch; all three now run normally. Installed DPorts
packages retain their original 6.4 provenance and compatible existing
runtime libraries. The final ordinary reboot identity is
`kern.boottime` seconds `1795597316` (2026-11-25 09:01:56 UTC).

Final inventory: no fixture mounts, Lurker mappings, session records,
attached vnode files or owned filesystem/test workers remain. Base mounts
were preserved. Closed fixture files, root-owned staging, official inputs
and original-system backups remain for inspection; 2.5 GiB remains free
on the separate build/fixture filesystem. No startup FUSE autoload change
was made. The guest is released for other work.

`result.json` records the final kernel, artifacts, five result cells and
cleanup. `evidence.tar.gz` retains the build log, native controls, source
helper, upstream checksums, world installation/utility checks and all
ext4 receipts. Its SHA-256 is
`68afb9d83bfa4122d5227031e1ef9aefc3896b15a9755b0394e4aa123a5125cc`.
The result establishes only the new ext4 read-only CLI content-persistence
case on the stated stock 6.5 profile. Other requested filesystem/CLI cells
remain explicitly blocked.
