# FreeBSD: missing writable filesystem routes retest

2026-11-25. **No additional CLI filesystem pair qualifies.** Btrfs and XFS
creation/writable mounts through GELI or VeraCrypt remain unqualified in
Lurker. A new native writer works, but its unauthenticated loopback bridge
cannot protect a private Lurker mount. No product, encryption engine, upstream
source or kernel patch was made. Prior successful routes were not repeated.

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
- Changing formatters cannot resolve LKL's independently reproduced append
  failure. The later easy-candidate audit below tests `ntfs2btrfs` with a
  different writer. The earlier btrfs-progs `getmntent` configure limit and
  GELI/NTFS formatter limit remain under their original receipts.

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
initialization. No source workaround was attempted. None of these LKL controls
completes the required append/add-file/final-reopen lifecycle.

Earlier attempts are preserved separately: the initial batch lacked the FUSE
module; a background readiness check could race failed XFS/4096 initialization
and is superseded by the foreground results. Its apparent local-directory
writes are not filesystem evidence. Initial Btrfs detach-busy observations
resolved after native shutdown; the foreground harness waits for driver exit.

## Easy standard-tool follow-up

The follow-up audit found one distinct ready-made writer, so it was tested
before considering new runtime integration. Upstream `fileshare` v0.50.1
publishes a static FreeBSD executable that serves its Btrfs/XFS image drivers
over NFSv3. This is the most recent release with published native assets at
audit time; v0.51.0 had none. No wrapper or upstream patch was needed.

- Source tag commit: `a0a9b384e00b6da1cc5fcb6f306682ff858f9f59`.
  Its module manifest pins Btrfs/XFS v0.5.0 and NFS v0.14.0.
- Official archive SHA-256:
  `f426a99073a81b014de83fb3f272972c0f160657b5d1f665b31b0a1a6246dcdd`,
  verified against the release's published `SHA256SUMS`.
  Executed administrator-owned binary SHA-256:
  `6f377b773a7f467fb55d5c1bff717c8c9068b831e1990bb033da389f2b67bd33`.

| Native candidate | Result |
| --- | --- |
| Existing independent Btrfs seed + fileshare/NFS | PDF/text write/fsync, reopen, append/add-file and final reopen/hash succeed. |
| Native `mkfs.xfs -f -m crc=1` + fileshare/NFS | Fresh XFS v5 creation and the same complete content lifecycle succeed. |
| Native `mkntfs -Q -F -s 512` + `ntfs2btrfs` + fileshare/NFS | Fresh 4 GiB sparse image converts and passes the complete content lifecycle. A separate 384 MiB conversion fails chunk-space allocation; minimum size is not qualified. |
| xfuse, libfsxfs | Current upstream implementations are read-only. No redundant mount run. |
| btrfs-fuse, btrfs4mac | Current upstream frontends are read-only. No redundant mount run. |
| DSBMD/automount wrappers | Delegate these formats to LKL; they are not another filesystem writer. |

These are **native content successes, not CLI pair passes**. FreeBSD's normal
`umount` removes each NFS mount and flushes its data, then exits 1 because its
separate mountd notification cannot find the standalone server through
rpcbind. Receipts record this explicitly. The resumed two-case harness verifies
the existing first write before continuing; it does not repeat that phase.
No forced unmount, reboot or fault scenario was used.

The existing `apfs_input(mapped)` check could mechanically supply VeraCrypt's
regular decrypted FUSE image: it proves that image is the recorded md
provider's backing vnode. This compatibility is an inference, not a tested
encrypted route. Fileshare forces device inputs read-only, so GELI lacks that
simple input route.

The product blocker is local access isolation. The unauthenticated NFS export
does not establish a caller's identity; any local process can send RPCs to the
loopback listener independently of the private mountpoint. Fileshare refuses
per-user-restricted NFS shares unless authenticated transport is configured.
Enabling this bridge would weaken Lurker's private-container boundary.
Authenticated transport or an independently qualified local mount frontend
requires further work; no unauthenticated bridge was integrated.

Primary references:
https://github.com/go-fileshare/fileshare/releases/tag/v0.50.1
https://github.com/go-fileshare/fileshare/blob/v0.50.1/serve_nfs.go
https://github.com/go-filesystems/nfs
https://github.com/KhaledEmaraDev/xfuse
https://github.com/libyal/libfsxfs
https://github.com/adam900710/btrfs-fuse
https://github.com/gereoffy/btrfs4mac
https://github.com/mrclksr/DSBMD

## Evidence and cleanup

- Local index: `target/goal-validation/filesystem-retest/freebsd/`.
  `evidence/foreground-results.json` and per-case `driver.log` files are the
  authoritative four native outcomes; `directio-results.json` records the two
  rejected configuration attempts. Build logs, exact scripts, source/archive
  pins and package/module inventories are retained. `evidence.tar.gz` matches
  `evidence.sha256`; `cleanup-final.json` records the final live audit.
- Follow-up receipts: `easy-candidates/evidence/fileshare-resume-results.json`
  and `fileshare-conversion4g-results.json`; the initial two-case receipts
  preserve the first write and unmount-notification failure. Release metadata,
  exact scripts, formatter logs and SHA-verified `fileshare-evidence.tar.gz`
  are in the same follow-up directory.
- Build and fixtures used a new disposable 16 GiB UFS disk, serial
  `lurker-fs-retest-freebsd`, mounted at
  `/tmp/lurker-filesystem-retest-freebsd`. It was unmounted normally and its
  live-only `vdb` attachment removed, including after the follow-up audit.
  Its source, binaries and fixtures
  remain in `/srv/build/lurker-filesystem-retest-freebsd/build.qcow2`, with
  size and full hash in `scratch-disk.json` and `scratch-disk.sha256`.
  The guest system disk was not resized or repartitioned.
- Final audit: no test mounts, md devices, session records or owned workers;
  no registry lock exists; only system `zroot` is imported;
  `vfs.usermount=0`. The temporarily loaded FUSE module was unloaded to match
  preflight. Official build dependencies and the administrator-owned test
  runtime `/var/run/lurker-filesystem-retest-freebsd` remain documented.
- No system NFS service/configuration was installed. All owned listeners stopped.
  Nine pending mountd-notification cache rows were identified by this round's
  exact loopback export and timestamps, backed up and removed; no unrelated
  rows were removed. The native retry also could not contact rpcbind. Before,
  after and cleanup receipts are retained; final `/var/db/mounttab` is empty.
- No GUI, physical-device, SQLite, reboot or fault test was run. Host released.
