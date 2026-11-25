# FreeBSD: missing writable filesystem routes retest

2026-11-25. **No additional CLI filesystem pair qualifies.** Btrfs and XFS
creation/writable mounts through GELI or VeraCrypt remain blocked by native
driver prerequisites. No product, encryption engine, upstream source or kernel
patch was made. Prior successful filesystems and RO imports were not repeated.

## Updated inputs and bounded alternatives

- Guest: `adjutant@freebsd-core-01`, FreeBSD 15.1-RELEASE-p7, amd64.
  The refreshed official catalogs offer no update to its kernel or relevant
  packages: `fusefs-lkl-4.16.g20180628_5`, `xfsprogs-3.2.4_4`,
  `ntfs2btrfs-20260810`. The packaged LKL 0.1 executable was left untouched.
- Product baseline: `b474181c1e410cd44c3351a374ef47c5117bb0c6`.
  The FreeBSD filesystem adapter was not changed; no new product binary or
  CLI lifecycle pass is claimed. The existing refusals remain appropriate.
- Built unmodified current LKL source
  `d0f76a77e8c2ca68bd7d96190b8da09040eab072` (2026-09-28), containing
  Linux 6.12 and lklfuse 0.3, against packaged FUSE3 3.18.3.
  Distribution archive SHA-256:
  `ef6e5b7cd66c023bc16193013fdacbc1285b9eb86df864b8c6cefe0ad4e0e168`.
  Executed native driver SHA-256:
  `bb06769dfb85a9d39246f3872d8d5d370d35dc7d83fcd92bb806521cdad84e45`.
  Normal GNU build prerequisites, GNU sed/stat/ld selection, regeneration of
  generated host configuration and the official Python launcher were needed.
  Build scripts and failed prerequisite attempts are retained with the final
  successful build log. No source edits were applied.
- Attempted writable fuse-xfs fork
  `f940a1d3e8cbb7a54897eefb34e522042c61e393`. Its normal native configure
  succeeds with the supported `--enable-gettext=no` option, but compilation
  stops at its macOS-specific `libc.h` include; shared-library linking also
  fails. This is a build blocker, not a mount result. The older FreeBSD/NetBSD
  fork `a32c7413f337fc99d00c37640f15962588d91eed` returns ENOSYS from create
  and other write operations, so it is not another writable candidate.
- Combining an NTFS formatter with installed `ntfs2btrfs` cannot resolve the
  independently reproduced mount-write failure. No additional conversion or
  formatter retry was run. The earlier btrfs-progs `getmntent` configure limit
  and GELI/NTFS formatter limit remain under their original receipts.

Primary upstream references:
https://github.com/lkl/linux/tree/d0f76a77e8c2ca68bd7d96190b8da09040eab072
https://github.com/freebsd/freebsd-ports/blob/main/filesystems/lkl/Makefile
https://github.com/karan-vk/fuse-xfs/tree/f940a1d3e8cbb7a54897eefb34e522042c61e393
https://github.com/wchaoyi/fuse-xfs/blob/a32c7413f337fc99d00c37640f15962588d91eed/src/fuse/fuse_xfs.c

## Authoritative native writable controls

The administrator-owned current upstream executable ran against disposable
copies of the existing independently formatted plaintext seeds, on native md
providers. These controls establish prerequisites; they are not encrypted CLI
passes. Foreground execution, the live driver process and the expected existing
PDF hash jointly gate readiness. Each viable mount writes a valid PDF and
UTF-8 text, flushes/fsyncs, closes normally, reopens and verifies those bytes
before the ordinary append/add-file phase.

| Filesystem | Provider sector | Observed result |
| --- | --- | --- |
| XFS | 512 bytes, comparable to VeraCrypt | Initial write/fsync and normal close/reopen/hash succeed; append opens and buffers, then the actual stream flush fails EBADF before fsync. |
| XFS | 4096 bytes, comparable to GELI | Driver exits during initialization: sector-zero I/O error, XFS superblock validation error -5. No payload phase runs. |
| Btrfs | 512 bytes, comparable to VeraCrypt | Initial write/fsync and normal close/reopen/hash succeed; append flush fails EBADF before fsync. |
| Btrfs | 4096 bytes, comparable to GELI | Initial write/fsync and normal close/reopen/hash succeed; append flush fails EBADF before fsync. |

Both 512-byte cases also reject the ordinary `direct_io` FUSE option during
initialization. No source workaround was attempted. These results leave all
four requested encrypted create/RW pairs blocked; none completes the required
append/add-file/final-reopen lifecycle.

Earlier attempts are preserved separately: the initial batch lacked the FUSE
module; a background readiness check could race failed XFS/4096 initialization
and is superseded by the foreground results. Its apparent local-directory
writes are not filesystem evidence. Initial Btrfs detach-busy observations
resolved after native shutdown; the foreground harness waits for driver exit.

## Evidence and cleanup

- Local index: `target/goal-validation/filesystem-retest/freebsd/`.
  `evidence/foreground-results.json` and per-case `driver.log` files are the
  authoritative four native outcomes; `directio-results.json` records the two
  rejected configuration attempts. Build logs, exact scripts, source/archive
  pins and package/module inventories are retained. `evidence.tar.gz` matches
  `evidence.sha256`; `cleanup-final.json` records the final live audit.
- Build and fixtures used a new disposable 16 GiB UFS disk, serial
  `lurker-fs-retest-freebsd`, mounted at
  `/tmp/lurker-filesystem-retest-freebsd`. It was unmounted normally and its
  live-only `vdb` attachment removed. Its source, binaries and failed fixtures
  remain in `/srv/build/lurker-filesystem-retest-freebsd/build.qcow2`, with
  size and full hash in `scratch-disk.json` and `scratch-disk.sha256`.
  The guest system disk was not resized or repartitioned.
- Final audit: no test mounts, md devices, session records or owned workers;
  no registry lock exists; only system `zroot` is imported;
  `vfs.usermount=0`. The temporarily loaded FUSE module was unloaded to match
  preflight. Official build dependencies and the administrator-owned test
  runtime `/var/run/lurker-filesystem-retest-freebsd` remain documented.
- No GUI, physical-device, SQLite, reboot or fault test was run. Host released.
