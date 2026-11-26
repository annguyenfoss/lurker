# FreeBSD fresh qualification checkpoint

Qualification is in progress. This checkpoint is not a complete host pass.
Private receipts are under `target/goal-validation/native-fresh-20261127/hosts/freebsd/`;
remote logs and new fixtures are under `/srv/lurker-fresh-20261127/`.

- Running kernel, installed kernel and userland: FreeBSD 15.1-RELEASE-p7,
  `kern.osreldate=1501000`. Fresh `pkg update` and `pkg upgrade -y` found no
  pending ports, kernel-module or pkgbase upgrades. The installed PEFS ABI
  annotation is 1501000; PEFS, HAMMER2 and FUSE modules loaded successfully.
- The original root pool was nearly full. A new 64 GiB build/fixture qcow2 was
  attached to `freebsd-core-01`, verified as `/dev/vtbd1`, and made into the
  separate `lurkerfresh20261127` pool. Old Cargo caches were preserved on this
  disk with symlinks at their previous paths; old fixtures were not removed.
  This disk is not the later destructive device-test disk.
- Exact source: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`.
  Native runner `20261126T204256Z-a32c3772` built the CLI/helper and passed
  `cargo check --workspace`; workspace tests are still running. One Cargo
  worker is used. Earlier failed/blocked attempts and an intentionally
  cancelled build order are retained separately.
- The administrator-owned staged CLI SHA-256 is
  `bccebf721ad35162c1b89c5e017190a59ea1bb767078f758a4e649da4c4ff28a`;
  helper SHA-256 is
  `b809af5f824ae2eced807e9fd1effe35d1e894967a20153c71086439dae75553`.
  Verified native private VeraCrypt 1.26.31 and gocryptfs 2.7.0 caches retain
  their original source, patch, library and license identities. Runtime results
  are staged-artifact results, not installed-package qualification.
- Fresh ext4 GELI and VeraCrypt password cases passed create, three mounts,
  write/append/hash verification and three normal closes using the downloaded
  Bitcoin, Ethereum and Transformer PDFs plus text. The complete initial
  ten-filesystem container attempts and eligible PEFS/gocryptfs backing cases
  are continuing. Harness revision is `c5feaec`, separately from product source.
- Fresh native package controls created XFS with `mkfs.xfs` and Btrfs with
  `mkntfs` followed by `ntfs2btrfs`, mounted through FreeBSD's packaged LKL
  driver, wrote the payload, closed and reopened. Both failed append with
  `EBADF`; this is a current native control limitation, not a Create + Read/Write
  pass. Corrected controls are in `native-filesystems-1795725868`; preliminary
  control setup failures are retained separately.
- Stable pinned CryFS 1.0.3 configured successfully natively after installing
  ordinary build prerequisites. Its native build is continuing. The current
  Lurker FreeBSD guard excludes CryFS; native lifecycle results must precede
  any proposed integration change.

The private `cases/applicability.json` contains all 130 method/filesystem cells
with explicit current applicability reasons. Pending cells, device controls,
reboot persistence, conformance variations and final reconciliation remain work.

Current primary eligibility sources, inspected this round:

- https://docs.freebsd.org/en/books/handbook/filesystems/
- https://cgit.freebsd.org/ports/plain/security/veracrypt/Makefile
- https://github.com/kusumi/freebsd_hammer2
- https://raw.githubusercontent.com/libyal/libfsapfs/main/README
- https://raw.githubusercontent.com/libyal/libluksde/main/README
- https://raw.githubusercontent.com/cryptomator/jfuse/develop/README.md
- https://raw.githubusercontent.com/cryptomator/fuse-nio-adapter/5.1.0/src/main/java/org/cryptomator/frontend/fuse/mount/LinuxFuseMountProvider.java
- https://raw.githubusercontent.com/cryfs/cryfs/1.0.3/CMakeLists.txt
